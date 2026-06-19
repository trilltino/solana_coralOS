//! Shared key-value state endpoints.
//!
//! | Method | Path        | Action                    |
//! |--------|-------------|---------------------------|
//! | GET    | `/`         | Snapshot the entire store |
//! | GET    | `/history`  | Get the change-history log|
//! | GET    | `/:key`     | Read a single entry       |
//! | POST   | `/:key`     | Write a value             |
//! | DELETE | `/:key`     | Delete a key              |

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::AppState;
use agent_core::shared_state::{SharedStateEntry, StateChange};

/// Request body for writing a value.
#[derive(Serialize, Deserialize)]
pub struct SetStateRequest {
    pub value: Value,
    pub changed_by: String,
}

/// Request body for deleting a key (only `changed_by` is required).
#[derive(Serialize, Deserialize)]
pub struct DeleteStateRequest {
    pub changed_by: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_all_state))
        .route("/history", get(get_state_history))
        .route("/:key", get(get_state).post(set_state).delete(delete_state))
}

async fn set_state(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<SetStateRequest>,
) -> Result<Json<bool>, StatusCode> {
    if req.changed_by.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Ok(Json(
        state.manager.set_shared_state(&key, req.value, &req.changed_by),
    ))
}

async fn get_state(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<Option<SharedStateEntry>>, StatusCode> {
    Ok(Json(state.manager.get_shared_state(&key)))
}

async fn delete_state(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<DeleteStateRequest>,
) -> Result<Json<bool>, StatusCode> {
    if req.changed_by.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Ok(Json(
        state.manager.delete_shared_state(&key, &req.changed_by),
    ))
}

async fn get_all_state(
    State(state): State<AppState>,
) -> Json<HashMap<String, SharedStateEntry>> {
    Json(state.manager.get_all_shared_state())
}

async fn get_state_history(State(state): State<AppState>) -> Json<Vec<StateChange>> {
    Json(state.manager.get_state_history())
}
