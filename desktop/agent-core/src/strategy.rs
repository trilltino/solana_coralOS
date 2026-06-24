use async_trait::async_trait;
use chrono::Utc;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::interval;

use crate::{AgentAction, AgentState};

/// Pluggable behaviour for an [`Agent`](crate::Agent).
///
/// The runtime calls [`Strategy::run`] in a `tokio::spawn`-ed task.
/// Implementations must poll `state.is_running` and return when it is `false`.
#[async_trait]
pub trait Strategy: Send + Sync {
    /// Main loop. Runs until `state.is_running` becomes `false`.
    async fn run(&self, state: Arc<Mutex<AgentState>>);
    /// Short, stable identifier used to label the agent's strategy field.
    fn name(&self) -> &'static str;

    /// Handle an inbound Coral mention and return a reply string.
    /// Default: echo back. Override in payment strategies for real dispatch.
    async fn handle_message(
        &self,
        text: &str,
        _state: Arc<Mutex<AgentState>>,
    ) -> String {
        format!("agent received: {}", &text[..text.len().min(120)])
    }
}

/// Default strategy: polls the Solana RPC for the current slot every 5 seconds.
pub struct RpcPollStrategy;

impl Default for RpcPollStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcPollStrategy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Strategy for RpcPollStrategy {
    async fn run(&self, state: Arc<Mutex<AgentState>>) {
        let mut ticker = interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;

            let (is_running, rpc_url) = {
                let s = state.lock().expect("agent state lock poisoned");
                (s.is_running, s.rpc_endpoint.clone())
            };
            if !is_running {
                break;
            }
            
            let start = std::time::Instant::now();
            let client = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());

            let action = match client.get_slot() {
                Ok(slot) => AgentAction {
                    timestamp: Utc::now(),
                    action_type: "rpc-poll".to_string(),
                    details: format!("Polled slot {} via {}", slot, rpc_url),
                    tx_signature: None,
                    slot: Some(slot),
                    latency_ms: start.elapsed().as_millis() as u64,
                },
                Err(e) => AgentAction {
                    timestamp: Utc::now(),
                    action_type: "rpc-error".to_string(),
                    details: format!("Error: {}", e),
                    tx_signature: None,
                    slot: None,
                    latency_ms: start.elapsed().as_millis() as u64,
                },
            };

            state
                .lock()
                .expect("agent state lock poisoned")
                .actions
                .push(action);
        }
    }

    fn name(&self) -> &'static str {
        "rpc-poll"
    }
}

/// Strategy for agents that react to commands rather than polling on a timer.
///
/// The loop spins at 1 Hz checking `is_running` so the agent can be stopped
/// promptly without blocking resources.
pub struct IdleStrategy;

impl Default for IdleStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl IdleStrategy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Strategy for IdleStrategy {
    async fn run(&self, state: Arc<Mutex<AgentState>>) {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if !state.lock().expect("agent state lock poisoned").is_running {
                break;
            }
        }
    }

    fn name(&self) -> &'static str {
        "idle"
    }
}
