use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use log::{debug, error};
use serde_json::json;

use crate::api::does_user_exist;
use crate::api::models::UserDocument;
use crate::ApiState;
use crate::events::{UserCreatedEvent};

pub async fn create_user(
    State(state): State<ApiState>,
    Json(params): Json<UserCreatedEvent>,
) -> impl IntoResponse {
    debug!("Request received: {:#?}", params);
    let user_id = params.user.unwrap().user_id;
    // Make sure the user exists
    if does_user_exist(&state.firestore_client, &user_id).await? {
        return Err((
            StatusCode::FOUND,
            Json(json!({"status": "User already exists"})),
        ));
    };

    // Place the user in the DB. This should just be the user ID
    if let Err(e) = state
        .firestore_client
        .fluent()
        .insert()
        .into("users")
        .document_id(&user_id)
        .object::<UserDocument>(&UserDocument { tasks: vec![] })
        .execute::<UserDocument>()
        .await
    {
        error!("Failed to create user: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": format!("Failed to create user: {}", e)})),
        ));
    };

    return Ok((
        StatusCode::CREATED,
        Json(json!({"status": "User created successfully"})),
    ));
}