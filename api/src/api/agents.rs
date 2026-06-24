//! Agent CRUD endpoints.
//!
//! | Method | Path                        | Action                        |
//! |--------|-----------------------------|-------------------------------|
//! | GET    | `/`                         | List all agents               |
//! | POST   | `/`                         | Create an agent               |
//! | GET    | `/with-roles`               | List agents with full metadata|
//! | POST   | `/solana-pay`               | Create a Solana Pay agent     |
//! | POST   | `/helius-monitor`           | Create a Helius monitor agent |
//! | GET    | `/:id`                      | Get agent state               |
//! | DELETE | `/:id`                      | Delete an agent               |
//! | POST   | `/:id/start`                | Start the agent's strategy    |
//! | POST   | `/:id/stop`                 | Stop the agent's strategy     |
//! | GET    | `/:id/actions`              | Get the agent's action log    |
//! | POST   | `/:id/rpc`                  | Update the RPC endpoint       |
//! | POST   | `/:id/triton`               | Configure a Triton PAYG key   |
//! | POST   | `/:id/role`                 | Set the agent's role          |
//! | POST   | `/:id/helius`               | Configure Helius RPC          |

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use agent_core::{AgentAction, AgentMeta, AgentRole, AgentState, PayMode};
use agent_core::triton::TritonConfig;

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

#[derive(Serialize, Deserialize)]
pub struct SetRoleRequest {
    pub role: String,
}

#[derive(Serialize, Deserialize)]
pub struct SetHeliusRequest {
    pub api_key: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateSolanaPayRequest {
    pub id: String,
    pub mode: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateHeliusMonitorRequest {
    pub id: String,
    pub recipient: String,
    pub amount_sol: f64,
    pub api_key: String,
    pub label: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_agents).post(create_agent))
        // Static paths must come before /:id to avoid Axum matching them as IDs
        .route("/with-roles", get(list_agents_with_roles))
        .route("/solana-pay", post(create_solana_pay))
        .route("/helius-monitor", post(create_helius_monitor))
        .route("/:id", get(get_agent).delete(delete_agent))
        .route("/:id/start", post(start_agent))
        .route("/:id/stop", post(stop_agent))
        .route("/:id/actions", get(get_actions))
        .route("/:id/rpc", post(set_rpc))
        .route("/:id/triton", post(set_triton))
        .route("/:id/role", post(set_role))
        .route("/:id/helius", post(set_helius))
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

async fn list_agents_with_roles(
    State(state): State<AppState>,
) -> Json<Vec<(String, AgentState, AgentMeta)>> {
    Json(state.manager.list_agents_with_roles())
}

async fn create_solana_pay(
    State(state): State<AppState>,
    Json(req): Json<CreateSolanaPayRequest>,
) -> Result<Json<AgentState>, StatusCode> {
    if req.id.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mode = match req.mode.as_str() {
        "Transfer" => PayMode::Transfer,
        "Payment" => PayMode::Payment,
        _ => PayMode::Payment,
    };
    state
        .manager
        .create_solana_pay_agent(req.id, mode)
        .map(Json)
        .ok_or(StatusCode::CONFLICT)
}

async fn create_helius_monitor(
    State(state): State<AppState>,
    Json(req): Json<CreateHeliusMonitorRequest>,
) -> Result<Json<AgentState>, StatusCode> {
    if req.id.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let config = TritonConfig::devnet(req.api_key.clone());
    // Use Helius devnet URLs if api_key is provided
    let config = if req.api_key.trim().is_empty() {
        config
    } else {
        let rpc_url = format!("https://devnet.helius-rpc.com/?api-key={}", req.api_key);
        let ws_url = format!("wss://devnet.helius-rpc.com/?api-key={}", req.api_key);
        TritonConfig::custom(
            rpc_url.clone(),
            rpc_url,
            ws_url,
            req.api_key.clone(),
            "devnet",
        )
    };
    state
        .manager
        .create_triton_monitor_agent(req.id, req.recipient, req.amount_sol, config, req.label)
        .map(Json)
        .ok_or(StatusCode::CONFLICT)
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

async fn set_role(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetRoleRequest>,
) -> Result<Json<bool>, StatusCode> {
    let role = match req.role.to_lowercase().as_str() {
        "leader" => AgentRole::Leader,
        "coordinator" => AgentRole::Coordinator,
        "monitor" => AgentRole::Monitor,
        "analyst" => AgentRole::Analyst,
        "trader" => AgentRole::Trader,
        _ => AgentRole::Worker,
    };
    if state.manager.set_agent_role(&id, role) {
        Ok(Json(true))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn set_helius(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SetHeliusRequest>,
) -> Result<Json<bool>, StatusCode> {
    let url = if req.api_key.trim().is_empty() {
        "https://api.devnet.solana.com".to_string()
    } else {
        format!("https://devnet.helius-rpc.com/?api-key={}", req.api_key)
    };
    if state.manager.set_rpc(&id, url) {
        Ok(Json(true))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
