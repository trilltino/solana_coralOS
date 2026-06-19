//! Agent CRUD endpoints.
//!
//! | Method | Path                        | Action                        |
//! |--------|-----------------------------|-------------------------------|
//! | GET    | `/`                         | List all agents               |
//! | POST   | `/`                         | Create an agent               |
//! | GET    | `/:id`                      | Get agent state               |
//! | DELETE | `/:id`                      | Delete an agent               |
//! | POST   | `/:id/start`                | Start the agent's strategy    |
//! | POST   | `/:id/stop`                 | Stop the agent's strategy     |
//! | GET    | `/:id/actions`              | Get the agent's action log    |
//! | POST   | `/:id/rpc`                  | Update the RPC endpoint       |
//! | POST   | `/:id/triton`               | Configure a Triton PAYG key   |

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use agent_core::{AgentAction, AgentState};

#[derive(Serialize, Deserialize)]
pub struct CreateAgentRequest {
    /// Unique agent identifier. Must be non-empty.
    pub id: String,
}

#[derive(Serialize, Deserialize)]
pub struct SetRpcRequest {
    pub url: String,
}

#[derive(Serialize, Deserialize)]
pub struct SetTritonRequest {
    pub x_token: String,
    pub grpc_endpoint: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_agents).post(create_agent))
        .route("/:id", get(get_agent).delete(delete_agent))
        .route("/:id/start", post(start_agent))
        .route("/:id/stop", post(stop_agent))
        .route("/:id/actions", get(get_actions))
        .route("/:id/rpc", post(set_rpc))
        .route("/:id/triton", post(set_triton))
}

async fn create_agent(
    State(state): State<AppState>,
    Json(req): Json<CreateAgentRequest>,
) -> Result<Json<AgentState>, StatusCode> {
    if req.id.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    state
        .manager
        .create_agent(req.id)
        .map(Json)
        .ok_or(StatusCode::CONFLICT)
}

async fn list_agents(State(state): State<AppState>) -> Json<Vec<(String, AgentState)>> {
    Json(state.manager.list_agents())
}

async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AgentState>, StatusCode> {
    state
        .manager
        .get_agent_state(&id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn delete_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if state.manager.remove_agent(&id) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn start_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<bool>, StatusCode> {
    state
        .manager
        .start_agent(&id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn stop_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<bool>, StatusCode> {
    if state.manager.stop_agent(&id) {
        Ok(Json(true))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn get_actions(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<AgentAction>>, StatusCode> {
    state
        .manager
        .get_actions(&id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn set_rpc(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetRpcRequest>,
) -> Result<Json<bool>, StatusCode> {
    if req.url.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    if state.manager.set_rpc(&id, req.url) {
        Ok(Json(true))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn set_triton(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetTritonRequest>,
) -> Result<Json<bool>, StatusCode> {
    use agent_core::triton::TritonConfig;
    if req.x_token.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let config = match req.grpc_endpoint {
        Some(ep) => TritonConfig::custom(ep.clone(), "https://api.mainnet-beta.solana.com", ep.replace("https://", "wss://").replace("http://", "ws://"), req.x_token, "mainnet-beta"),
        None => TritonConfig::mainnet(req.x_token),
    };
    if state.manager.set_triton(&id, &config) {
        Ok(Json(true))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
