use std::net::{Ipv4Addr, SocketAddr};
use std::str::FromStr;

use axum::{middleware, Router, routing};
use firestore::{FirestoreDb, FirestoreDbOptions};
use google_cloud_pubsub::client::{Client, ClientConfig};
use log::{error, info};

use crate::api::{print_request_response, tasks, users};

pub mod commands {
    tonic::include_proto!("commands");
}

pub mod models {
    tonic::include_proto!("models");
}

pub mod events {
    tonic::include_proto!("events");
}

mod api;

type Port = u32;

#[derive(Clone)]
pub struct ApiState {
    pub pubsub_client: Client,
    pub firestore_client: FirestoreDb,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    // Get the port to serve the API on
    let port: Port = std::env::var("PORT")
        .unwrap_or("8080".to_string())
        .parse()
        .unwrap();

    let project_id = std::env::var("GOOGLE_PROJECT_ID").unwrap();

    // Construct the pubsub client configuration
    let conf = ClientConfig {
        project_id: Some(project_id.to_string()),
        ..Default::default()
    };
    let config = match conf.with_auth().await {
        Ok(config) => config,
        Err(e) => {
            error!("Error creating PubSub client config: {}", e);
            std::process::exit(1);
        }
    };

    // Create the PubSub client from the configuration
    let pubsub_client = match Client::new(config).await {
        Ok(client) => client,
        Err(e) => {
            error!("Error creating PubSub client: {}", e);
            std::process::exit(1);
        }
    };

    // Open a connection to the Firestore DB
    let firestore_client = match FirestoreDb::with_options(
        FirestoreDbOptions::new(project_id)
            .with_database_id(std::env::var("DATABASE_ID").expect("DATABASE_ID env var not set")),
    )
    .await
    {
        Ok(db) => db,
        Err(e) => {
            error!("Error connecting to Firestore DB: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = firestore_client.ping().await {
        error!("Error pinging Firestore DB: {}", e);
        std::process::exit(1);
    }

    let api_state = ApiState {
        pubsub_client,
        firestore_client,
    };

    // Create a raw URL to serve the API on
    let addr = SocketAddr::from_str(&format!("{}:{}", Ipv4Addr::UNSPECIFIED, port)).unwrap();

    // Construct the router for the API with the necessary routes
    let app = Router::new()
        .route("/commands/task", routing::post(tasks::create_task))
        .route("/events/task-completed", routing::post(tasks::task_complete))
        .route("/events/user-created", routing::post(users::create_user))
        .layer(middleware::from_fn(print_request_response))
        .with_state(api_state);

    // Serve the API on the specified address
    info!("Serving API on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
