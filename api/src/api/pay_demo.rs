//! Pay demo endpoints — payment flow store and sale completion.

use axum::{extract::State, routing::{get, post}, Json, Router};
use serde::Deserialize;
use chrono::Utc;
use agent_core::AgentAction;
use crate::AppState;

#[derive(Deserialize)]
pub struct CompleteSaleRequest {
    pub seller_id: String,
    pub buyer_id: String,
    pub tx_signature: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/flows", get(get_flows))
        .route("/complete-sale", post(complete_sale))
}

async fn get_flows(State(state): State<AppState>) -> Json<Vec<crate::PaymentFlowRecord>> {
    Json(state.flows.lock().unwrap().clone())
}

async fn complete_sale(
    State(state): State<AppState>,
    Json(req): Json<CompleteSaleRequest>,
) -> Json<String> {
    use agent_core::solana_pay::demo_payment_flow;

    state.manager.record_action(&req.seller_id, AgentAction {
        timestamp: Utc::now(),
        action_type: "fetching-data".to_string(),
        details: format!("calling pay.sh with tx={}", req.tx_signature.as_deref().unwrap_or("demo")),
        tx_signature: req.tx_signature.clone(),
        slot: None,
        latency_ms: 0,
    });

    let flow_request_at = Utc::now().to_rfc3339();
    let result = demo_payment_flow("https://debugger.pay.sh/mpp/quote/AAPL", 1_000_000).await;

    let flow_challenge_at = result.challenge.as_ref().map(|_| Utc::now().to_rfc3339());
    let flow_payment_at = result.payment_header.as_ref().map(|_| Utc::now().to_rfc3339());
    let flow_protocol = result.challenge.as_ref().map(|c| c.protocol.to_string());
    let flow_amount = result.challenge.as_ref().map(|c| c.amount);
    let flow_recipient = result.challenge.as_ref().map(|c| c.recipient.clone());
    let flow_token = result.challenge.as_ref().map(|c| c.token.clone());
    let flow_payment_header = result.payment_header.clone();
    let flow_response_body = result.response_body.clone();
    let flow_error = result.error.clone();
    let flow_success = result.success;
    let protocol = result.challenge.as_ref().map(|c| c.protocol.to_string()).unwrap_or_default();

    let data_payload = if result.success {
        result.response_body.unwrap_or_else(|| r#"{"error":"empty response"}"#.to_string())
    } else {
        serde_json::json!({
            "AAPL": 189.42,
            "source": "fallback",
            "error": result.error
        }).to_string()
    };

    state.manager.record_action(&req.seller_id, AgentAction {
        timestamp: Utc::now(),
        action_type: "data-delivered".to_string(),
        details: format!("pay.sh {} → delivered to {}", protocol, req.buyer_id),
        tx_signature: req.tx_signature.clone(),
        slot: None,
        latency_ms: 0,
    });

    state.manager.send_direct(&req.seller_id, &req.buyer_id, "data-delivered", &data_payload);

    state.manager.record_action(&req.buyer_id, AgentAction {
        timestamp: Utc::now(),
        action_type: "data-received".to_string(),
        details: format!("received from {} via pay.sh", req.seller_id),
        tx_signature: req.tx_signature.clone(),
        slot: None,
        latency_ms: 0,
    });

    let delivery_at = if flow_success { Some(Utc::now().to_rfc3339()) } else { None };
    {
        let mut flows = state.flows.lock().unwrap();
        flows.push(crate::PaymentFlowRecord {
            id: format!("sale-{}", Utc::now().timestamp_millis()),
            agent_id: req.seller_id.clone(),
            endpoint: "https://debugger.pay.sh/mpp/quote/AAPL".to_string(),
            status: if flow_success { "success".to_string() } else { "failed".to_string() },
            protocol: flow_protocol,
            amount: flow_amount,
            recipient: flow_recipient,
            token: flow_token,
            payment_header: flow_payment_header,
            response_body: flow_response_body,
            error: flow_error,
            request_at: flow_request_at,
            challenge_at: flow_challenge_at,
            payment_at: flow_payment_at,
            delivery_at,
        });
        if flows.len() > 100 {
            let excess = flows.len() - 100;
            flows.drain(0..excess);
        }
    }

    let json_val = serde_json::from_str(&data_payload)
        .unwrap_or(serde_json::Value::String(data_payload.clone()));
    state.manager.set_shared_state(
        &format!("sale/{}/result", req.seller_id),
        json_val,
        &req.seller_id,
    );

    Json(data_payload)
}
