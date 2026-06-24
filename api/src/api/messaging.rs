//! Message bus endpoints.
//!
//! | Method | Path                          | Action                              |
//! |--------|-------------------------------|-------------------------------------|
//! | POST   | `/`                           | Send a message (direct or broadcast)|
//! | GET    | `/:agent_id`                  | Get messages for an agent           |
//! | GET    | `/conversation/:a/:b`         | Get the thread between two agents   |

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use agent_core::AgentMessage;

#[derive(Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub from: String,
    /// `None` sends a broadcast; `Some(id)` sends a direct message.
    pub to: Option<String>,
    pub msg_type: String,
    pub payload: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_all_messages).post(send_message))
        .route("/:agent_id", get(get_messages))
        .route("/conversation/:a/:b", get(get_conversation))
}

async fn get_all_messages(State(state): State<AppState>) -> Json<Vec<AgentMessage>> {
    Json(state.manager.get_all_messages())
}

async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<bool>, StatusCode> {
    if req.from.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let msg = match req.to {
        Some(to_id) => AgentMessage::direct(req.from, to_id, req.msg_type, req.payload),
        None => AgentMessage::broadcast(req.from, req.msg_type, req.payload),
    };
    state.manager.send_message(msg);
    Ok(Json(true))
}

async fn get_messages(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Json<Vec<AgentMessage>> {
    Json(state.manager.get_messages(&agent_id))
}

async fn get_conversation(
    State(state): State<AppState>,
    Path((a, b)): Path<(String, String)>,
) -> Json<Vec<AgentMessage>> {
    Json(state.manager.get_conversation(&a, &b))
}
