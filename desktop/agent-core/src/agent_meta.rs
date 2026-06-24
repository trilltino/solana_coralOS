use crate::role::AgentRole;

/// Which Solana Pay protocol variant an agent is configured for.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PayMode {
    /// Stateless transfer URL — encodes recipient + amount into a `solana:` URI.
    Transfer,
    /// x402 / MPP payment-challenge flow — handles HTTP 402 negotiation.
    Payment,
}


/// Role and lifecycle metadata stored alongside an agent's runtime state.
///
/// Kept separate from [`AgentState`](crate::AgentState) so that IPC snapshots
/// remain lightweight while this richer context is only fetched when needed.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AgentMeta {
    pub role: AgentRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Free-form labels for filtering/grouping agents.
    pub tags: Vec<String>,
}


impl Default for AgentMeta {
    fn default() -> Self {
        Self {
            role: AgentRole::Worker,
            created_at: chrono::Utc::now(),
            tags: Vec::new(),
        }
    }
}
