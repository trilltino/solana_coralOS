//! `agent-core` — multi-agent orchestration library for Solana payment workflows.
//!
//! Provides [`AgentManager`] as the single entry point for creating and driving
//! agents, messaging, shared state, and DAG-based workflows. All public types
//! are `Serialize`/`Deserialize` so they can cross the Tauri IPC boundary.

pub mod agent;
pub mod agent_meta;
pub mod coral_mcp;
pub mod triton;
pub mod manager;
pub mod message_bus;
pub mod orchestrator;
pub mod role;
pub mod shared_state;
pub mod solana_pay;
pub mod strategy;

pub use agent::{Agent, AgentAction, AgentState};
pub use agent_meta::{AgentMeta, PayMode};
pub use coral_mcp::{CoralMcpSession, CoralMention};
pub use manager::AgentManager;
pub use message_bus::{AgentMessage, MessageBus};
pub use orchestrator::{StepStatus, Workflow, WorkflowEngine, WorkflowStatus, WorkflowStep};
pub use role::{AgentRole, RolePermissions};
pub use shared_state::{SharedState, SharedStateEntry, StateChange};
pub use solana_pay::*;
pub use strategy::{IdleStrategy, RpcPollStrategy, Strategy};

