use async_trait::async_trait;
use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::agent::AgentAction;
use crate::agent::AgentState;
use crate::strategy::Strategy;
use super::payment::demo_payment_flow;

/// Strategy for a Solana Pay transfer agent (idle until commands are issued).
pub struct TransferStrategy;

impl Default for TransferStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl TransferStrategy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Strategy for TransferStrategy {
    async fn run(&self, state: Arc<Mutex<AgentState>>) {
        {
            let mut s = state.lock().unwrap();
            let rpc = s.rpc_endpoint.clone();
            s.actions.push(AgentAction {
                timestamp: Utc::now(),
                action_type: "strategy-start".to_string(),
                details: format!(
                    "TransferStrategy started — waiting for commands (RPC: {})",
                    rpc
                ),
                tx_signature: None,
                slot: None,
                latency_ms: 0,
            });
        }

        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let s = state.lock().unwrap();
            if !s.is_running {
                break;
            }
        }
    }

    fn name(&self) -> &'static str {
        "solana-pay-transfer"
    }
}

/// Strategy for a Solana Pay payment agent (optionally polls sandbox endpoints).
pub struct PaymentStrategy {
    pub sandbox_endpoint: Option<String>,
}

impl Default for PaymentStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl PaymentStrategy {
    pub fn new() -> Self {
        Self {
            sandbox_endpoint: Some("https://debugger.pay.sh/mpp/quote/AAPL".to_string()),
        }
    }

    pub fn with_endpoint(endpoint: String) -> Self {
        Self {
            sandbox_endpoint: Some(endpoint),
        }
    }
}

#[async_trait]
impl Strategy for PaymentStrategy {
    async fn run(&self, state: Arc<Mutex<AgentState>>) {
        {
            let mut s = state.lock().unwrap();
            let rpc = s.rpc_endpoint.clone();
            s.actions.push(AgentAction {
                timestamp: Utc::now(),
                action_type: "strategy-start".to_string(),
                details: format!(
                    "PaymentStrategy started — monitoring for 402 endpoints (RPC: {})",
                    rpc
                ),
                tx_signature: None,
                slot: None,
                latency_ms: 0,
            });
        }

        // Optional: periodically check a demo endpoint
        if let Some(ref endpoint) = self.sandbox_endpoint {
            let mut ticker = tokio::time::interval(Duration::from_secs(30));
            // Skip first tick
            ticker.tick().await;

            loop {
                ticker.tick().await;
                let running = {
                    let s = state.lock().unwrap();
                    s.is_running
                };
                if !running {
                    break;
                }

                let result = demo_payment_flow(endpoint, 1_000_000).await;
                let mut s = state.lock().unwrap();
                s.actions.push(AgentAction {
                    timestamp: Utc::now(),
                    action_type: "x402-demo-poll".to_string(),
                    details: format!(
                        "Polled {} — success={} challenge={:?}",
                        endpoint,
                        result.success,
                        result.challenge.as_ref().map(|c| format!("{} {}", c.amount, c.token))
                    ),
                    tx_signature: None,
                    slot: None,
                    latency_ms: 0,
                });
            }
        } else {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let s = state.lock().unwrap();
                if !s.is_running {
                    break;
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "solana-pay-payment"
    }
}
