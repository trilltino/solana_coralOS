//! Solana Pay and x402 endpoints.

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::Deserialize;
use agent_core::solana_pay::{
    DemoPaymentResult, ParsedUrl, PaymentChallenge, ValidationResult,
    TransferUrlFields, encode_transfer_url, parse_url, validate_transfer,
    parse_402_response, demo_payment_flow,
};
use crate::AppState;

#[derive(Deserialize)]
pub struct CreateUrlRequest {
    pub recipient: String,
    pub amount: f64,
    pub label: Option<String>,
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct ParseUrlRequest {
    pub url: String,
}

#[derive(Deserialize)]
pub struct ValidateRequest {
    pub id: String,
    pub signature: String,
    pub expected_recipient: Option<String>,
    pub rpc_url: Option<String>,
}

#[derive(Deserialize)]
pub struct Parse402Request {
    pub headers: Vec<(String, String)>,
}

#[derive(Deserialize)]
pub struct DemoPaymentRequest {
    pub endpoint: String,
    pub budget: u64,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/url", post(create_url))
        .route("/parse", post(parse_url_handler))
        .route("/validate", post(validate_tx))
        .route("/x402/parse", post(parse_402))
        .route("/x402/demo", post(demo_payment))
}

async fn create_url(Json(req): Json<CreateUrlRequest>) -> Json<String> {
    let url = encode_transfer_url(&TransferUrlFields {
        recipient: req.recipient,
        amount: Some(req.amount),
        spl_token: None,
        reference: None,
        label: req.label,
        message: req.message,
        memo: None,
    });
    Json(url)
}

async fn parse_url_handler(Json(req): Json<ParseUrlRequest>) -> Result<Json<ParsedUrl>, StatusCode> {
    parse_url(&req.url)
        .map(Json)
        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)
}

async fn validate_tx(
    State(state): State<AppState>,
    Json(req): Json<ValidateRequest>,
) -> Json<ValidationResult> {
    let rpc_url = req.rpc_url
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            state.manager.get_agent_state(&req.id)
                .map(|s| s.rpc_endpoint)
        })
        .unwrap_or_else(|| "https://api.devnet.solana.com".to_string());

    Json(validate_transfer(&rpc_url, &req.signature, req.expected_recipient.as_deref()).await)
}

async fn parse_402(Json(req): Json<Parse402Request>) -> Json<Option<PaymentChallenge>> {
    Json(parse_402_response(&req.headers))
}

async fn demo_payment(
    State(state): State<AppState>,
    Json(req): Json<DemoPaymentRequest>,
) -> Json<DemoPaymentResult> {
    use chrono::Utc;
    let now = Utc::now().to_rfc3339();
    let result = demo_payment_flow(&req.endpoint, req.budget).await;

    let challenge_at = result.challenge.as_ref().map(|_| Utc::now().to_rfc3339());
    let payment_at = result.payment_header.as_ref().map(|_| Utc::now().to_rfc3339());
    let delivery_at = if result.success { Some(Utc::now().to_rfc3339()) } else { None };

    {
        let mut flows = state.flows.lock().unwrap();
        flows.push(crate::PaymentFlowRecord {
            id: format!("demo-{}", Utc::now().timestamp_millis()),
            agent_id: "solana-pay-tab".to_string(),
            endpoint: req.endpoint.clone(),
            status: if result.success { "success".to_string() } else { "failed".to_string() },
            protocol: result.challenge.as_ref().map(|c| c.protocol.to_string()),
            amount: result.challenge.as_ref().map(|c| c.amount),
            recipient: result.challenge.as_ref().map(|c| c.recipient.clone()),
            token: result.challenge.as_ref().map(|c| c.token.clone()),
            payment_header: result.payment_header.clone(),
            response_body: result.response_body.clone(),
            error: result.error.clone(),
            request_at: now,
            challenge_at,
            payment_at,
            delivery_at,
        });
        if flows.len() > 100 {
            let excess = flows.len() - 100;
            flows.drain(0..excess);
        }
    }

    Json(result)
}
