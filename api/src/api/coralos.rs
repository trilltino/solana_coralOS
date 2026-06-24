//! CoralOS proxy endpoints.
//! Stores CoralOS URL + token, proxies session listing, and manages MCP agent loops.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Deserialize)]
pub struct CoralOsConfigRequest {
    pub url: Option<String>,
    pub token: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CoralAgent {
    pub name: String,
    pub status: String,
    pub description: String,
    pub links: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CoralSessionExtended {
    pub id: String,
    pub namespace: String,
    pub status: String,
    #[serde(rename = "agentCount")]
    pub agent_count: Option<u32>,
    #[serde(rename = "paymentSessionId")]
    pub payment_session_id: Option<String>,
    pub agents: Vec<CoralAgent>,
}

/// Request body for joining a CoralOS MCP session.
#[derive(Deserialize)]
pub struct McpJoinRequest {
    pub connection_url: String,
    pub agent_name: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/config", put(set_config))
        .route("/sessions/:ns", get(list_sessions))
        .route("/mcp/join", post(mcp_join))
        .route("/mcp/status/:name", get(mcp_status))
}

async fn set_config(
    State(state): State<AppState>,
    Json(req): Json<CoralOsConfigRequest>,
) -> Json<bool> {
    if let Some(url) = req.url {
        *state.coralos_url.lock().unwrap() = url;
    }
    if let Some(token) = req.token {
        *state.coralos_token.lock().unwrap() = token;
    }
    Json(true)
}

async fn list_sessions(
    State(state): State<AppState>,
    Path(ns): Path<String>,
) -> Json<Vec<CoralSessionExtended>> {
    let url = state.coralos_url.lock().unwrap().clone();
    let token = state.coralos_token.lock().unwrap().clone();

    if url.is_empty() {
        return Json(vec![]);
    }

    let client = reqwest::Client::new();
    let sessions_url = format!("{}/api/v1/sessions?namespace={}", url.trim_end_matches('/'), ns);

    let mut req_builder = client.get(&sessions_url);
    if !token.is_empty() {
        req_builder = req_builder.bearer_auth(&token);
    }

    match req_builder.send().await {
        Ok(resp) => {
            match resp.json::<serde_json::Value>().await {
                Ok(data) => {
                    let sessions_arr = data.as_array()
                        .cloned()
                        .unwrap_or_else(|| {
                            data.get("sessions")
                                .and_then(|s| s.as_array())
                                .cloned()
                                .unwrap_or_default()
                        });

                    let sessions: Vec<CoralSessionExtended> = sessions_arr.iter().filter_map(|s| {
                        Some(CoralSessionExtended {
                            id: s.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            namespace: s.get("namespace").and_then(|v| v.as_str()).unwrap_or(&ns).to_string(),
                            status: s.get("status").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                            agent_count: s.get("agentCount").and_then(|v| v.as_u64()).map(|n| n as u32),
                            payment_session_id: s.get("paymentSessionId").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            agents: vec![],
                        })
                    }).collect();
                    Json(sessions)
                }
                Err(_) => Json(vec![]),
            }
        }
        Err(_) => Json(vec![]),
    }
}

/// POST /api/v1/coralos/mcp/join
///
/// Connects to a CoralOS MCP endpoint and spawns a background agent loop.
/// The loop calls `wait_for_mention`, records the mention as an agent action,
/// and replies with an acknowledgement message.
async fn mcp_join(
    State(state): State<AppState>,
    Json(req): Json<McpJoinRequest>,
) -> Result<Json<bool>, StatusCode> {
    use agent_core::CoralMcpSession;

    let session = CoralMcpSession::connect(&req.connection_url, &req.agent_name)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let sessions = state.mcp_sessions.clone();
    let manager = state.manager.clone();
    let agent_name = req.agent_name.clone();

    // Mark as active before spawning so the status endpoint reflects truth immediately.
    sessions.lock().unwrap().insert(agent_name.clone(), true);

    tokio::spawn(async move {
        let mgr = manager.clone();
        let name = agent_name.clone();
        session
            .run_loop(move |mention| {
                let mgr = mgr.clone();
                let name = name.clone();
                async move {
                    let reply = if let Some(agent) = mgr.get_agent(&name) {
                        let strategy = agent.get_strategy();
                        let state = agent.state_arc();
                        strategy.handle_message(&mention.text, state).await
                    } else {
                        format!("agent {} not found in manager", name)
                    };
                    mgr.record_action(
                        &name,
                        agent_core::AgentAction {
                            timestamp: chrono::Utc::now(),
                            action_type: "coral-mention".to_string(),
                            details: mention.text.chars().take(200).collect(),
                            tx_signature: None,
                            slot: None,
                            latency_ms: 0,
                        },
                    );
                    reply
                }
            })
            .await;

        // Mark as inactive when the loop exits (connection dropped / cancelled).
        sessions.lock().unwrap().insert(agent_name, false);
    });

    Ok(Json(true))
}

/// GET /api/v1/coralos/mcp/status/:name
///
/// Returns `true` when the named agent has an active MCP loop, `false` otherwise.
async fn mcp_status(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Json<bool> {
    Json(*state.mcp_sessions.lock().unwrap().get(&name).unwrap_or(&false))
}
