//! `coral-server` — Axum REST API wrapping [`agent_core::AgentManager`].
//!
//! Listens on `0.0.0.0:8080` and exposes four resource groups:
//! - `GET/POST /api/v1/agents` — agent CRUD
//! - `GET/POST /api/v1/workflows` — workflow management
//! - `POST/GET  /api/v1/messages` — message bus
//! - `GET/POST  /api/v1/state`    — shared key-value state

use axum::{routing::get, Json, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use agent_core::AgentManager;

mod api;

/// Shared application state injected into every Axum handler via [`axum::extract::State`].
#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<AgentManager>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let state = AppState {
        manager: Arc::new(AgentManager::new()),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .nest("/api/v1/agents", api::agents::routes())
        .nest("/api/v1/workflows", api::workflows::routes())
        .nest("/api/v1/messages", api::messaging::routes())
        .nest("/api/v1/state", api::shared_state::routes())
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
