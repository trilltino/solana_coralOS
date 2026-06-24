//! Real weather endpoint — runs WeatherStrategy.handle_message() and returns live data.
//!
//! POST /api/v1/weather   Body: {"city":"London"} | {"lat":51.5,"lon":-0.1}
//!
//! Creates a transient AgentState, calls WeatherStrategy, and returns JSON directly.
//! Also writes the result to SharedState under "result:weather-agent" if a
//! weather-agent exists in the manager (so the live-log can pick it up).

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use agent_core::{
    solana_pay::WeatherStrategy,
    strategy::Strategy,
    AgentState, AgentAction,
};
use chrono::Utc;
use std::sync::{Arc, Mutex};
use crate::AppState;

#[derive(Deserialize)]
pub struct WeatherRequest {
    pub city: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

#[derive(Serialize)]
pub struct WeatherResponse {
    pub ok: bool,
    pub data: serde_json::Value,
    pub latency_ms: u64,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/", post(get_weather))
}

async fn get_weather(
    State(state): State<AppState>,
    Json(req): Json<WeatherRequest>,
) -> Result<Json<WeatherResponse>, StatusCode> {
    let text = if let Some(city) = &req.city {
        serde_json::json!({ "city": city }).to_string()
    } else if let (Some(lat), Some(lon)) = (req.lat, req.lon) {
        serde_json::json!({ "lat": lat, "lon": lon }).to_string()
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    // Transient state container — WeatherStrategy only appends to actions vec
    let agent_state = Arc::new(Mutex::new(AgentState {
        is_running: true,
        actions: vec![],
        rpc_endpoint: String::new(),
        network: "devnet".to_string(),
        strategy: "weather".to_string(),
    }));

    let strategy = WeatherStrategy::new();
    let start = std::time::Instant::now();
    let result_str = strategy.handle_message(&text, Arc::clone(&agent_state)).await;
    let latency_ms = start.elapsed().as_millis() as u64;

    let data: serde_json::Value = serde_json::from_str(&result_str)
        .unwrap_or_else(|_| serde_json::json!({ "raw": result_str }));

    // Write to SharedState if weather-agent exists in the manager (best-effort)
    let city_label = data.get("city").and_then(|c| c.as_str()).unwrap_or("unknown");
    state.manager.record_action(
        "weather-agent",
        AgentAction {
            timestamp: Utc::now(),
            action_type: "data-delivered".to_string(),
            details: format!("weather for {} ({} ms)", city_label, latency_ms),
            tx_signature: None,
            slot: None,
            latency_ms,
        },
    );

    Ok(Json(WeatherResponse { ok: true, data, latency_ms }))
}
