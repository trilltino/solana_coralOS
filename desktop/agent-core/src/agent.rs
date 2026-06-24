use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::strategy::{RpcPollStrategy, Strategy};

/// A single recorded action taken by an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentAction {
    pub timestamp: DateTime<Utc>,
    /// Short machine-readable label (e.g. `"rpc-poll"`, `"url-generated"`).
    pub action_type: String,
    /// Human-readable description of what happened.
    pub details: String,
    pub tx_signature: Option<String>,
    pub slot: Option<u64>,
    /// Wall-clock latency of the operation in milliseconds.
    pub latency_ms: u64,
}

/// Serialisable snapshot of an agent's current state, suitable for IPC.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentState {
    pub is_running: bool,
    pub actions: Vec<AgentAction>,
    pub rpc_endpoint: String,
    pub network: String,
    /// Name of the active [`Strategy`] implementation (e.g. `"rpc-poll"`).
    pub strategy: String,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            is_running: false,
            actions: Vec::new(),
            rpc_endpoint: "https://api.devnet.solana.com".to_string(),
            network: "devnet".to_string(),
            strategy: "rpc-poll".to_string(),
        }
    }
}

/// A single autonomous agent with a pluggable [`Strategy`] and an action log.
///
/// All mutable state is protected by `Mutex` so the agent can be shared
/// across threads (required by the Tauri `State` extractor and `tokio::spawn`).
pub struct Agent {
    state: Arc<Mutex<AgentState>>,
    strategy: Mutex<Arc<dyn Strategy>>,
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

impl Agent {
    /// Create an agent with the default [`RpcPollStrategy`].
    pub fn new() -> Self {
        Self::with_strategy(Arc::new(RpcPollStrategy::new()))
    }

    /// Create an agent using a specific strategy. The strategy name is recorded
    /// in the initial [`AgentState`].
    pub fn with_strategy(strategy: Arc<dyn Strategy>) -> Self {
        let mut state = AgentState::default();
        state.strategy = strategy.name().to_string();
        Self {
            state: Arc::new(Mutex::new(state)),
            strategy: Mutex::new(strategy),
        }
    }

    /// Return a cloned snapshot of the current state.
    pub fn state(&self) -> AgentState {
        self.state.lock().expect("agent state lock poisoned").clone()
    }

    /// Override the RPC endpoint (takes effect on the next strategy tick).
    pub fn set_rpc(&self, url: String) {
        self.state.lock().expect("agent state lock poisoned").rpc_endpoint = url;
    }

    /// Override the network label (e.g. `"mainnet"`, `"devnet"`).
    pub fn set_network(&self, network: String) {
        self.state.lock().expect("agent state lock poisoned").network = network;
    }

    /// Configure the agent's RPC endpoint and network from a Triton config.
    pub fn set_triton(&self, config: &crate::triton::TritonConfig) {
        let mut s = self.state.lock().expect("agent state lock poisoned");
        s.rpc_endpoint = config.rpc_url.clone();
        s.network = config.network.clone();
    }

    /// Append an action to the agent's log.
    pub fn record_action(&self, action: AgentAction) {
        self.state
            .lock()
            .expect("agent state lock poisoned")
            .actions
            .push(action);
    }

    /// Hot-swap the running strategy. The new strategy name is written to state
    /// immediately; the old strategy loop exits on its next `is_running` check.
    pub fn set_strategy(&self, strategy: Arc<dyn Strategy>) {
        let mut s = self.state.lock().expect("agent state lock poisoned");
        s.strategy = strategy.name().to_string();
        *self.strategy.lock().expect("strategy lock poisoned") = strategy;
    }

    /// Mark the agent as running and spawn the strategy loop on the Tokio runtime.
    pub async fn start_monitoring(&self) -> anyhow::Result<()> {
        self.state.lock().expect("agent state lock poisoned").is_running = true;

        let strategy = Arc::clone(&*self.strategy.lock().expect("strategy lock poisoned"));
        let state = Arc::clone(&self.state);
        tokio::spawn(async move {
            strategy.run(state).await;
        });

        Ok(())
    }

    /// Signal the strategy loop to exit by setting `is_running = false`.
    pub fn stop(&self) {
        self.state.lock().expect("agent state lock poisoned").is_running = false;
    }

    /// Return the live strategy so callers can dispatch messages to it.
    pub fn get_strategy(&self) -> Arc<dyn Strategy> {
        Arc::clone(&*self.strategy.lock().expect("strategy lock poisoned"))
    }

    /// Return the shared state Arc so strategy.handle_message can mutate it.
    pub fn state_arc(&self) -> Arc<Mutex<AgentState>> {
        Arc::clone(&self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state() {
        let agent = Agent::new();
        assert!(!agent.state().is_running);
    }

    #[tokio::test]
    async fn test_monitoring_starts() {
        let agent = Agent::new();
        agent.start_monitoring().await.unwrap();
        assert!(agent.state().is_running);
        agent.stop();
    }

    #[test]
    fn test_transfer_url_roundtrip() {
        let fields = crate::solana_pay::TransferUrlFields {
            recipient: "7xKF3rO1jW...".to_string(),
            amount: Some(0.01),
            spl_token: None,
            reference: None,
            label: Some("Demo".to_string()),
            message: Some("Thanks!".to_string()),
            memo: None,
        };
        let url = crate::solana_pay::encode_transfer_url(&fields);
        let parsed = crate::solana_pay::parse_url(&url).unwrap();
        match parsed {
            crate::solana_pay::ParsedUrl::Transfer(t) => {
                assert_eq!(t.recipient, fields.recipient);
                assert_eq!(t.amount, fields.amount);
                assert_eq!(t.label, fields.label);
                assert_eq!(t.message, fields.message);
            }
            _ => panic!("Expected transfer URL"),
        }
    }
}
