use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use firestore::paths;
use log::{debug, error, info};
use serde_json::json;

use crate::api::{does_task_exist, does_user_exist, emit_event};
use crate::ApiState;
use crate::commands::CreateTaskCommand;
use crate::events::{TaskCompletedEvent, TaskCreatedEvent};
use crate::models::{Task, TaskData, TaskStatus};

pub async fn create_task(
    State(state): State<ApiState>,
    Json(params): Json<CreateTaskCommand>,
) -> impl IntoResponse {
    debug!("Request received: {:#?}", params);
    let user_id = params.user_id.clone();

    // Make sure the user exists
    if !does_user_exist(&state.firestore_client, &user_id).await? {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        ));
    };

    // Get the parent path for the user
    let parent_path = match state.firestore_client.parent_path("users", user_id.clone()) {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to get parent path: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get parent path: {}", e)})),
            ));
        }
    };

    // Make sure the task doesn't already exist
    if does_task_exist(&state.firestore_client, &params.task_id, &parent_path).await? {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "Task already exists"})),
        ));
    };

    // Create a new task in the DB
    let task_id = params.task_id.clone();
    let task = Task {
        task_id: params.task_id.clone(),
        data: Some(params.task_data.clone().unwrap_or(TaskData::default())),
        object_path: params.object_path.clone(),
        created_by: user_id.clone(),
        status: TaskStatus::Queued.into(),
        created_at: Some(prost_wkt_types::Timestamp {
            seconds: chrono::Utc::now().timestamp(),
            nanos: 0,
        }),
        updated_at: Some(prost_wkt_types::Timestamp {
            seconds: chrono::Utc::now().timestamp(),
            nanos: 0,
        }),
        ..Task::default()
    };

    // Save the task to the DB
    // Store the execution record in the DB and return the result
    let returned = match state
        .firestore_client
        .fluent()
        .insert()
        .into("tasks")
        .document_id(&task_id)
        .parent(&parent_path)
        .object(&task.clone())
        .execute::<Task>()
        .await
    {
        Ok(_) => {
            // info!("Task created: {:#?}", task.clone());
            task.clone()
        }
        Err(e) => {
            error!("Failed to create task: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create task: {}", e)})),
            ));
        }
    };

    info!("Task created: {:#?}", serde_json::to_string(&task).unwrap());

    emit_event(
        &state.pubsub_client,
        "TaskCreatedEvent",
        &serde_json::to_string(&TaskCreatedEvent {
            task: Some(task.clone()),
        })
        .unwrap(),
    )
    .await?;

    return Ok((
        StatusCode::CREATED,
        Json(json!(returned)),
    ));
}

pub async fn task_complete(
    State(state): State<ApiState>,
    Json(task_completion_event): Json<TaskCompletedEvent>,
) -> impl IntoResponse {
    info!("Request received: {:#?}", &task_completion_event);

    // Make sure the status is valid
    if let Err(e) = TaskStatus::try_from(task_completion_event.status.clone()) {
        error!("Invalid task status: {}", e);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"status": format!("Invalid task status: {}", e)})),
        ));
    }

    // Get the task
    let user_id = task_completion_event.user_id.clone();

    // Make sure the user exists
    if !does_user_exist(&state.firestore_client, &user_id).await? {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"status": "User not found"})),
        ));
    };

    // Get the parent path for the user
    let parent_path = match state
        .firestore_client
        .parent_path("users", user_id)
    {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to get parent path: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": format!("Failed to get parent path: {}", e)})),
            ));
        }
    };

    // Make sure the task exists and is deserializable
    let task = match state
        .firestore_client
        .fluent()
        .select()
        .by_id_in("tasks")
        .parent(parent_path.clone())
        .obj::<Task>()
        .one(&task_completion_event.task_id)
        .await
    {
        Ok(task) => match task {
            Some(task) => {
                info!("Task found: {:#?}", task);
                task
            }
            None => {
                error!("Failed to get task: {}", task_completion_event.task_id);
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(
                        json!({"status": format!("Failed to find task: {}", task_completion_event.task_id)}),
                    ),
                ));
            }
        },
        Err(e) => {
            error!("Database error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": format!("Failed to get task: {}", e)})),
            ));
        }
    };

    // Update the task
    let updated = match state
        .firestore_client
        .fluent()
        .update()
        .fields(paths!(Task::{status, result, duration_seconds, updated_at, last_publish_time}))
        .in_col("tasks")
        .document_id(&task_completion_event.task_id)
        .parent(parent_path)
        .object(&Task {
            status: task_completion_event.status.clone(),
            result: task_completion_event.result.clone(),
            duration_seconds: 0,
            updated_at: Some(prost_wkt_types::Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
            last_publish_time: None,
            ..task.clone()
        })
        .execute::<Task>()
        .await
    {
        Ok(updated) => updated,
        Err(e) => {
            error!("Failed to update task: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": format!("Failed to update task: {}", e)})),
            ));
        }
    };

    return Ok((
        StatusCode::OK,
        Json(json!({"status": "Task updated", "task": updated})),
    ));
}
