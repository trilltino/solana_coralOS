//! coral-server — Axum REST API wrapping `agent_core::AgentManager`.
//!
//! Listens on 0.0.0.0:8080.

use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use agent_core::AgentManager;

mod api;

/// Recorded payment flow (request → 402 → payment → delivery).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentFlowRecord {
    pub id: String,
    pub agent_id: String,
    pub endpoint: String,
    pub status: String,
    pub protocol: Option<String>,
    pub amount: Option<u64>,
    pub recipient: Option<String>,
    pub token: Option<String>,
    pub payment_header: Option<String>,
    pub response_body: Option<String>,
    pub error: Option<String>,
    pub request_at: String,
    pub challenge_at: Option<String>,
    pub payment_at: Option<String>,
    pub delivery_at: Option<String>,
}

/// Shared application state injected into every Axum handler.
#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<AgentManager>,
    pub flows: Arc<Mutex<Vec<PaymentFlowRecord>>>,
    pub coralos_url: Arc<Mutex<String>>,
    pub coralos_token: Arc<Mutex<String>>,
    /// Tracks which agent names have an active CoralOS MCP loop.
    pub mcp_sessions: Arc<Mutex<std::collections::HashMap<String, bool>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let state = AppState {
        manager: Arc::new(AgentManager::new()),
        flows: Arc::new(Mutex::new(Vec::new())),
        coralos_url: Arc::new(Mutex::new(String::new())),
        coralos_token: Arc::new(Mutex::new(String::new())),
        mcp_sessions: Arc::new(Mutex::new(std::collections::HashMap::new())),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .nest("/api/v1/agents", api::agents::routes())
        .nest("/api/v1/workflows", api::workflows::routes())
        .nest("/api/v1/messages", api::messaging::routes())
        .nest("/api/v1/shared-state", api::shared_state::routes())
        .nest("/api/v1/solana-pay", api::solana_pay::routes())
        .nest("/api/v1/payments", api::pay_demo::routes())
        .nest("/api/v1/swarm", api::coralos::routes())
        .nest("/api/v1/weather", api::weather::routes())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("coral-server listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
