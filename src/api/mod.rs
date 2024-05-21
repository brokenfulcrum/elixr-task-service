use std::collections::HashMap;

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{Response, StatusCode};
use axum::Json;
use axum::middleware::Next;
use axum::response::IntoResponse;
use firestore::{FirestoreDb, ParentPathBuilder};
use google_cloud_googleapis::pubsub::v1::PubsubMessage;
use google_cloud_pubsub::client::Client;
use google_cloud_pubsub::topic::Topic;
use http_body_util::BodyExt;
use log::{debug, error};
use serde_json::{json, Value};

pub(crate) mod models;
pub(crate) mod tasks;
pub mod users;

pub async fn does_user_exist(
    firestore_client: &FirestoreDb,
    user_id: &str,
) -> Result<bool, (StatusCode, Json<Value>)> {
    // Check if the user exists
    match firestore_client
        .fluent()
        .select()
        .by_id_in("users")
        .one(&user_id)
        .await
    {
        Ok(user) => match user {
            Some(_) => Ok(true),
            None => return Ok(false),
        },
        Err(e) => {
            error!("Database error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": format!("Failed to check user existence: {}", e)})),
            ));
        }
    }
}

pub async fn does_task_exist(
    firestore_client: &FirestoreDb,
    task_id: &str,
    parent_path: &ParentPathBuilder,
) -> Result<bool, (StatusCode, Json<Value>)> {
    // Check if the task exists
    match firestore_client
        .fluent()
        .select()
        .by_id_in("tasks")
        .parent(parent_path)
        .one(&task_id)
        .await
    {
        Ok(task) => match task {
            Some(_) => Ok(true),
            None => return Ok(false),
        },
        Err(e) => {
            error!("Database error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": format!("Failed to check task existence: {}", e)})),
            ));
        }
    }
}

pub async fn get_event_bus_topic(
    pubsub_client: Client,
) -> Result<Topic, (StatusCode, Json<Value>)> {
    let topic_name = std::env::var("EVENT_BUS").unwrap();
    let topic = pubsub_client.topic(topic_name.as_str());
    if !topic.exists(None).await.unwrap() {
        error!("Failed to get topic: {}", topic_name);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "Failed to get event bus!"})),
        ));
    }
    Ok(topic)
}

/// Middleware that logs the request and response bodies. This is useful for debugging.
pub async fn print_request_response(
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, String)> {

    // Buffer and log the request body
    let (parts, body) = req.into_parts();
    let bytes = buffer_and_print("request", body).await?;
    let req = Request::from_parts(parts, Body::from(bytes));

    // Forward the request and store the response
    let res = next.run(req).await;

    // Buffer and log the response body
    let (parts, body) = res.into_parts();
    let bytes = buffer_and_print("response", body).await?;
    let res = Response::from_parts(parts, Body::from(bytes));

    Ok(res)
}

/// Buffer the body and log it. This is useful for debugging.
async fn buffer_and_print<B>(direction: &str, body: B) -> Result<Bytes, (StatusCode, String)>
    where
        B: axum::body::HttpBody<Data = Bytes>,
        B::Error: std::fmt::Display,
{
    let bytes = match body.collect().await {
        Ok(collected) => {
            collected.to_bytes()},
        Err(err) => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("failed to read {direction} body: {err}"),
            ));
        }
    };

    if let Ok(body) = std::str::from_utf8(&bytes) {
        debug!("{direction} body = {body:?}");
    }

    Ok(bytes)
}

pub async fn emit_event(
    pubsub_client: &google_cloud_pubsub::client::Client,
    event_type: &str,
    event_data: &str,
) -> Result<(), (StatusCode, Json<Value>)> {

    // Get the datastore topic and establish a publisher
    let topic = get_event_bus_topic(pubsub_client.clone()).await?;
    let publisher = topic.new_publisher(None);

    // Set some attributes so PubSub directs the message to the correct service
    let mut attributes = HashMap::new();
    attributes.insert("messageType".to_string(), event_type.to_string());
    attributes.insert("Content-Type".to_string(), "application/json".to_string());

    println!(
        "Emitting event with attributes {:#?} and data {:#?}",
        attributes, event_data
    );
    // Construct the event with just the task ID
    let user_created_event = PubsubMessage {
        data: event_data.into(),
        attributes,
        ..Default::default()
    };

    // Publish the message to PubSub immediately
    if let Err(e) = publisher
        .publish_immediately(vec![user_created_event], None)
        .await
    {
        error!("Error publishing message: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": format!("Failed to publish message: {}", e)})),
        ));
    };

    return Ok(());
}