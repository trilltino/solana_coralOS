# agent-core

`agent-core` is the central Rust library that powers the Solana multi-agent payment demo. It provides agent lifecycle management, pluggable strategy execution, inter-agent messaging, shared key-value state, DAG-based workflow orchestration, Solana Pay URL encoding/validation, HTTP 402 payment challenge handling (MPP and x402), and a CoralOS MCP client.

Both `src-tauri` (the Tauri desktop backend) and `coral-server` (the Axum REST API) depend on this crate as a library. All public types implement `Serialize`/`Deserialize` so they can cross the Tauri IPC boundary without additional wrappers.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Cargo.toml Dependencies](#cargotoml-dependencies)
3. [Module Reference](#module-reference)
   - [agent](#agent)
   - [agent_meta](#agent_meta)
   - [strategy](#strategy)
   - [manager](#manager)
   - [message_bus](#message_bus)
   - [shared_state](#shared_state)
   - [role](#role)
   - [orchestrator](#orchestrator)
   - [solana_pay](#solana_pay)
   - [coral_mcp](#coral_mcp)
4. [Usage Examples](#usage-examples)
   - [Creating and starting agents](#creating-and-starting-agents)
   - [Custom Strategy implementation](#custom-strategy-implementation)
   - [MessageBus send/receive](#messagebus-sendreceive)
   - [SharedState get/set/history](#sharedstate-getsethistory)
   - [WorkflowEngine DAG execution](#workflowengine-dag-execution)
   - [CoralMcpSession run_loop pattern](#coralmcpsession-run_loop-pattern)
   - [Solana Pay URL encoding and validation](#solana-pay-url-encoding-and-validation)
   - [HTTP 402 payment challenge flow](#http-402-payment-challenge-flow)

---

## Architecture Overview

```
agent-core
├── AgentManager          ← single entry point; owns all subsystems
│   ├── BTreeMap<id, Arc<Agent>>  ← agent registry
│   ├── HashMap<id, AgentMeta>    ← role + lifecycle metadata
│   ├── MessageBus                ← broadcast + direct messaging
│   ├── SharedState               ← global KV store with history
│   └── WorkflowEngine            ← DAG workflow store
├── Agent                 ← holds Strategy + action log
│   └── Arc<dyn Strategy> ← pluggable async behaviour
├── solana_pay/           ← URL encoding, 402 parsing, validation
├── coral_mcp             ← MCP client for CoralOS sessions
├── role                  ← AgentRole enum + RolePermissions
└── orchestrator/         ← Workflow + WorkflowStep + WorkflowEngine
```

`AgentManager::clone()` is cheap — it clones inner `Arc` handles so multiple owners (e.g., the Tauri command handlers and a background MCP task) all share the same live state.

---

## Cargo.toml Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `solana-sdk` | 2.1 | Solana core types (pubkeys, transactions, etc.) |
| `solana-client` | 2.1 | JSON-RPC client (`RpcClient`) used by `RpcPollStrategy` |
| `solana-transaction-status` | 2.1 | Decoded transaction information for payment validation |
| `tokio` | 1 (full) | Async runtime; `tokio::spawn` drives strategy loops |
| `anyhow` | 1 | Ergonomic error propagation throughout the library |
| `serde` + `serde_json` | 1 | Serialization for all public types (IPC boundary) |
| `tracing` | 0.1 | Structured logging (`tracing::info!`, `tracing::error!`) |
| `chrono` | 0.4 (serde) | UTC timestamps on actions, messages, state changes |
| `uuid` | 1 (v4) | Random IDs for messages and workflow steps |
| `futures` | 0.3 | Async combinators used in strategy implementations |
| `reqwest` | 0.12 (json) | HTTP client for 402 payment flows and CoralOS API calls |
| `async-trait` | 0.1 | Enables `async fn` in trait definitions (`Strategy` trait) |
| `url` | 2 | URL parsing for Solana Pay URI validation |
| `urlencoding` | 2 | Percent-encoding of Solana Pay query parameters |
| `base64` | 0.22 | Decoding MPP challenge payloads |
| `rmcp` | 1.8.0 (client, streamable-http-client-reqwest, reqwest-native-tls) | MCP protocol client for CoralOS sessions |

---

## Module Reference

### agent

**File:** `src/agent.rs`

The `Agent` struct is a single autonomous unit. Its mutable state is protected by `Arc<Mutex<AgentState>>` so it can be shared across threads safely.

#### `AgentAction`

A single event recorded in an agent's log.

```rust
pub struct AgentAction {
    pub timestamp: DateTime<Utc>,
    pub action_type: String,   // e.g. "rpc-poll", "url-generated", "payment-received"
    pub details: String,       // human-readable description
    pub tx_signature: Option<String>, // Solana tx signature if relevant
    pub slot: Option<u64>,     // slot number if relevant
    pub latency_ms: u64,       // wall-clock duration of the operation
}
```

#### `AgentState`

A serializable snapshot of an agent — safe to send over IPC.

```rust
pub struct AgentState {
    pub is_running: bool,
    pub actions: Vec<AgentAction>,
    pub rpc_endpoint: String,   // default: "https://api.devnet.solana.com"
    pub network: String,        // "devnet" | "mainnet-beta" | etc.
    pub strategy: String,       // name of the active Strategy implementation
}
```

#### `Agent`

```rust
impl Agent {
    // Create with the default RpcPollStrategy.
    pub fn new() -> Self;

    // Create with a specific strategy.
    pub fn with_strategy(strategy: Arc<dyn Strategy>) -> Self;

    // Clone a snapshot of the current state.
    pub fn state(&self) -> AgentState;

    // Override the RPC endpoint.
    pub fn set_rpc(&self, url: String);

    // Override the network label.
    pub fn set_network(&self, network: String);

    // Apply a TritonConfig to both rpc_endpoint and network.
    pub fn set_triton(&self, config: &TritonConfig);

    // Append an action to the log.
    pub fn record_action(&self, action: AgentAction);

    // Hot-swap the running strategy (old loop exits on its next is_running check).
    pub fn set_strategy(&self, strategy: Arc<dyn Strategy>);

    // Mark is_running = true and spawn the strategy loop in tokio.
    pub async fn start_monitoring(&self) -> anyhow::Result<()>;

    // Signal the strategy loop to stop by setting is_running = false.
    pub fn stop(&self);
}
```

---

### agent_meta

**File:** `src/agent_meta.rs`

Lightweight metadata stored alongside an agent's runtime state. Kept separate from `AgentState` so IPC snapshots remain lean.

#### `PayMode`

```rust
pub enum PayMode {
    Transfer,  // encodes transfer URLs (stateless)
    Payment,   // handles x402/MPP payment challenges
}
```

#### `AgentMeta`

```rust
pub struct AgentMeta {
    pub role: AgentRole,              // default: AgentRole::Worker
    pub created_at: DateTime<Utc>,
    pub tags: Vec<String>,            // free-form labels for filtering
}
```

---

### strategy

**File:** `src/strategy.rs`

The pluggable behaviour interface for agents.

#### `Strategy` trait

```rust
#[async_trait]
pub trait Strategy: Send + Sync {
    // Main loop. Must poll `state.is_running` and return when false.
    async fn run(&self, state: Arc<Mutex<AgentState>>);

    // Short stable identifier written to AgentState::strategy.
    fn name(&self) -> &'static str;
}
```

#### `RpcPollStrategy`

The default strategy. Polls the Solana RPC for the current slot every 5 seconds and appends an `AgentAction` with `action_type = "rpc-poll"` (or `"rpc-error"` on failure).

- `name()` returns `"rpc-poll"`

#### `IdleStrategy`

A no-op strategy that spins at 1 Hz checking `is_running`. Used for agents that react to external commands rather than time-based polling.

- `name()` returns `"idle"`

---

### manager

**File:** `src/manager.rs`

`AgentManager` is the single entry point for all agent operations. It owns the agent map, metadata map, message bus, shared state, and workflow engine.

```rust
#[derive(Clone)]
pub struct AgentManager { /* ... */ }
```

`Clone` is cheap — it clones the inner `Arc` handles only.

#### Agent CRUD

```rust
// Create a generic agent. Returns None if id is already taken.
pub fn create_agent(&self, id: String) -> Option<AgentState>;

// Create a Solana Pay agent (Transfer or Payment mode).
pub fn create_solana_pay_agent(&self, id: String, mode: PayMode) -> Option<AgentState>;

// Create a Triton Yellowstone payment-monitor agent.
// Records an initial "url-generated" action with the Solana Pay URI.
pub fn create_triton_monitor_agent(
    &self,
    id: String,
    recipient: String,
    amount_sol: f64,
    config: TritonConfig,
    label: Option<String>,
) -> Option<AgentState>;

pub fn get_agent_state(&self, id: &str) -> Option<AgentState>;
pub fn list_agents(&self) -> Vec<(String, AgentState)>;  // sorted by id
pub fn remove_agent(&self, id: &str) -> bool;

pub fn set_rpc(&self, id: &str, url: String) -> bool;
pub fn set_triton(&self, id: &str, config: &TritonConfig) -> bool;
pub fn set_strategy(&self, id: &str, strategy: Arc<dyn Strategy>) -> bool;
pub fn get_agent_capabilities(&self, id: &str) -> Vec<String>;

pub async fn start_agent(&self, id: &str) -> anyhow::Result<bool>;
pub fn stop_agent(&self, id: &str) -> bool;

pub fn record_action(&self, id: &str, action: AgentAction) -> bool;
pub fn get_actions(&self, id: &str) -> Option<Vec<AgentAction>>;
```

#### Role methods

```rust
pub fn set_agent_role(&self, id: &str, role: AgentRole) -> bool;
pub fn get_agent_role(&self, id: &str) -> Option<AgentRole>;
pub fn get_agent_meta(&self, id: &str) -> Option<AgentMeta>;
pub fn list_agents_with_roles(&self) -> Vec<(String, AgentState, AgentMeta)>;
```

#### Messaging methods

```rust
// Deliver a pre-built AgentMessage to the bus.
pub fn send_message(&self, msg: AgentMessage);

// Return all messages visible to agent_id (broadcasts + direct).
pub fn get_messages(&self, agent_id: &str) -> Vec<AgentMessage>;

// Return every message on the bus (admin/debug view).
pub fn get_all_messages(&self) -> Vec<AgentMessage>;

// Return the direct-message thread between two agents.
pub fn get_conversation(&self, a: &str, b: &str) -> Vec<AgentMessage>;

// Broadcast — requires can_broadcast permission on the sender's role.
pub fn broadcast(&self, from: &str, msg_type: &str, payload: &str) -> bool;

// Direct message — no permission check.
pub fn send_direct(&self, from: &str, to: &str, msg_type: &str, payload: &str);
```

#### Shared state methods

```rust
// Write — requires can_modify_shared_state on changed_by's role.
pub fn set_shared_state(&self, key: &str, value: Value, changed_by: &str) -> bool;
pub fn get_shared_state(&self, key: &str) -> Option<SharedStateEntry>;
pub fn get_all_shared_state(&self) -> HashMap<String, SharedStateEntry>;
pub fn delete_shared_state(&self, key: &str, changed_by: &str) -> bool;
pub fn get_state_history(&self) -> Vec<StateChange>;
```

#### Workflow methods

```rust
pub fn create_workflow(&self, workflow: Workflow);
pub fn get_workflow(&self, id: &str) -> Option<Workflow>;
pub fn list_workflows(&self) -> Vec<Workflow>;
pub fn delete_workflow(&self, id: &str) -> bool;
pub fn assign_workflow_step(&self, workflow_id: &str, step_id: &str, agent_id: &str) -> bool;
pub fn start_workflow_step(&self, workflow_id: &str, step_id: &str) -> bool;
pub fn complete_workflow_step(&self, workflow_id: &str, step_id: &str, result: String) -> bool;
pub fn fail_workflow_step(&self, workflow_id: &str, step_id: &str, reason: String) -> bool;
pub fn get_agent_workflows(&self, agent_id: &str) -> Vec<Workflow>;
pub fn get_active_workflows(&self) -> Vec<Workflow>;  // Draft | Running | Paused
```

---

### message_bus

**File:** `src/message_bus.rs`

In-memory ring buffer (capacity 1,000) of inter-agent messages.

#### `AgentMessage`

```rust
pub struct AgentMessage {
    pub id: String,           // UUID v4
    pub from: String,         // sender agent id
    pub to: Option<String>,   // None = broadcast; Some(id) = direct
    pub msg_type: String,     // e.g. "task-assigned", "data-ready"
    pub payload: String,      // arbitrary string content
    pub timestamp: DateTime<Utc>,
}
```

**Constructors:**

```rust
AgentMessage::broadcast(from: String, msg_type: String, payload: String) -> Self;
AgentMessage::direct(from: String, to: String, msg_type: String, payload: String) -> Self;
```

**Visibility rule:** a message is visible to `agent_id` if `to` is `None` (broadcast), `from == agent_id`, or `to == Some(agent_id)`.

#### `MessageBus`

```rust
pub struct MessageBus { /* Arc-wrapped internals */ }

impl MessageBus {
    pub fn new() -> Self;
    pub fn send(&self, msg: AgentMessage);                         // increments unread for direct msgs
    pub fn get_messages_for(&self, agent_id: &str) -> Vec<AgentMessage>;
    pub fn get_all_messages(&self) -> Vec<AgentMessage>;
    pub fn get_unread_count(&self, agent_id: &str) -> usize;
    pub fn clear_unread(&self, agent_id: &str);
    pub fn get_conversation(&self, agent_a: &str, agent_b: &str) -> Vec<AgentMessage>;
}
```

---

### shared_state

**File:** `src/shared_state.rs`

A versioned key-value store shared across all agents. History is capped at 500 entries.

#### `SharedStateEntry`

```rust
pub struct SharedStateEntry {
    pub value: Value,            // serde_json::Value
    pub last_modified: DateTime<Utc>,
    pub modified_by: String,
    pub version: u64,            // starts at 1, increments on each write
}
```

#### `StateChange`

An audit log entry created on every write or delete.

```rust
pub struct StateChange {
    pub key: String,
    pub old_value: Option<Value>,  // None on first write
    pub new_value: Value,          // Value::Null on deletion
    pub timestamp: DateTime<Utc>,
    pub changed_by: String,
}
```

#### `SharedState`

```rust
pub struct SharedState { /* Arc-wrapped internals */ }

impl SharedState {
    pub fn new() -> Self;
    pub fn set(&self, key: String, value: Value, changed_by: String);
    pub fn get(&self, key: &str) -> Option<SharedStateEntry>;
    pub fn get_all(&self) -> HashMap<String, SharedStateEntry>;
    pub fn delete(&self, key: &str, changed_by: String);  // no-op if key absent
    pub fn get_history(&self) -> Vec<StateChange>;         // newest last
    pub fn get_keys(&self) -> Vec<String>;
}
```

---

### role

**File:** `src/role.rs`

Controls what operations each agent is permitted to perform.

#### `AgentRole`

```rust
pub enum AgentRole {
    Leader,       // full control over everything
    Worker,       // can modify shared state
    Monitor,      // can broadcast and view all messages
    Analyst,      // can modify shared state
    Trader,       // can modify shared state + initiate payments
    Coordinator,  // manage workflows + assign tasks (no stop-agent)
}
```

Default: `AgentRole::Worker`.

#### `RolePermissions`

```rust
pub struct RolePermissions {
    pub can_broadcast: bool,
    pub can_assign_tasks: bool,
    pub can_modify_shared_state: bool,
    pub can_start_workflows: bool,
    pub can_stop_other_agents: bool,
    pub can_view_all_messages: bool,
    pub can_initiate_payments: bool,
    pub can_create_payment_requests: bool,
}
```

Permission matrix by role:

| Permission | Leader | Coordinator | Worker | Monitor | Analyst | Trader |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| can_broadcast | Y | Y | | Y | | |
| can_assign_tasks | Y | Y | | | | |
| can_modify_shared_state | Y | Y | Y | | Y | Y |
| can_start_workflows | Y | Y | | | | |
| can_stop_other_agents | Y | | | | | |
| can_view_all_messages | Y | Y | | Y | | |
| can_initiate_payments | Y | Y | | | | Y |
| can_create_payment_requests | Y | Y | | | | Y |

Obtain permissions for a role:

```rust
let perms: RolePermissions = AgentRole::Trader.permissions();
assert!(perms.can_initiate_payments);
```

---

### orchestrator

**Files:** `src/orchestrator/workflow.rs`, `src/orchestrator/engine.rs`, `src/orchestrator/mod.rs`

DAG-based workflow orchestration. Each `Workflow` is a collection of `WorkflowStep`s with dependency edges. Steps become "ready" when all their dependencies are `Completed`.

#### `StepStatus`

```rust
pub enum StepStatus {
    Pending,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Skipped,
}
```

#### `WorkflowStatus`

```rust
pub enum WorkflowStatus {
    Draft,
    Running,
    Paused,
    Completed,  // set automatically when all steps complete
    Failed,     // set when any step fails
    Cancelled,
}
```

#### `WorkflowStep`

```rust
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: StepStatus,
    pub assigned_to: Option<String>,       // agent id
    pub dependencies: Vec<String>,         // step ids that must complete first
    pub result: Option<String>,            // output string written on completion
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub timeout_secs: Option<u64>,
}

impl WorkflowStep {
    pub fn new(id: &str, name: &str, description: &str) -> Self;
    pub fn with_assignee(self, agent_id: &str) -> Self;   // builder
    pub fn depends_on(self, step_id: &str) -> Self;        // builder
}
```

#### `Workflow`

```rust
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: WorkflowStatus,
    pub steps: Vec<WorkflowStep>,
    pub current_step: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub assigned_agents: Vec<String>,
    pub priority: u8,   // 1–10; higher = more important
    pub tags: Vec<String>,
}

impl Workflow {
    pub fn new(id: &str, name: &str, description: &str, created_by: &str) -> Self;
    pub fn add_step(&mut self, step: WorkflowStep);
    pub fn get_ready_steps(&self) -> Vec<&WorkflowStep>;  // unblocked pending steps
    pub fn assign_step(&mut self, step_id: &str, agent_id: &str) -> bool;
    pub fn start_step(&mut self, step_id: &str) -> bool;
    pub fn complete_step(&mut self, step_id: &str, result: String) -> bool;
    pub fn fail_step(&mut self, step_id: &str, reason: String) -> bool;
    pub fn progress_pct(&self) -> u8;  // 0–100

    // Pre-built templates:
    pub fn solana_pay_checkout(recipient: &str, amount: u64, label: &str) -> Self;
    pub fn x402_api_call(endpoint: &str, budget: u64) -> Self;
}
```

#### `WorkflowEngine`

```rust
pub struct WorkflowEngine { /* Arc-wrapped HashMap */ }

impl WorkflowEngine {
    pub fn new() -> Self;
    pub fn create_workflow(&self, workflow: Workflow);       // overwrites on id collision
    pub fn get_workflow(&self, id: &str) -> Option<Workflow>;
    pub fn list_workflows(&self) -> Vec<Workflow>;
    pub fn update_workflow(&self, id: &str, f: impl FnOnce(&mut Workflow)) -> bool;
    pub fn delete_workflow(&self, id: &str) -> bool;
    pub fn get_workflows_for_agent(&self, agent_id: &str) -> Vec<Workflow>;
    pub fn get_active_workflows(&self) -> Vec<Workflow>;  // Draft | Running | Paused
}
```

---

### solana_pay

**Files:** `src/solana_pay/`

Provides Solana Pay URL encoding/parsing, HTTP 402 challenge handling (MPP and x402), on-chain transfer validation, and agent strategies for payment flows.

#### Public exports from `src/solana_pay/mod.rs`

```rust
pub use url::{encode_transfer_url, encode_transaction_request_url, parse_url, ParsedUrl, TransferUrlFields, TransactionRequestUrlFields};
pub use payment::{demo_payment_flow, parse_402_response, PaymentChallenge, PaymentProtocol, DemoPaymentResult};
pub use strategies::{PaymentStrategy, TransferStrategy};
pub use monitor::TritonPaymentMonitorStrategy;
pub use validation::{validate_transfer, ValidationResult};
```

#### `TransferUrlFields`

```rust
pub struct TransferUrlFields {
    pub recipient: String,          // base58 pubkey
    pub amount: Option<f64>,        // SOL amount (not lamports)
    pub spl_token: Option<String>,  // SPL token mint pubkey
    pub reference: Option<String>,  // reference pubkey for tracking
    pub label: Option<String>,
    pub message: Option<String>,
    pub memo: Option<String>,
}
```

#### `ParsedUrl`

```rust
pub enum ParsedUrl {
    Transfer(TransferUrlFields),
    TransactionRequest(TransactionRequestUrlFields),
}
```

#### `PaymentProtocol`

```rust
pub enum PaymentProtocol {
    Mpp,   // serialized as "mpp"
    X402,  // serialized as "x402"
}
```

#### `PaymentChallenge`

Extracted from a 402 response.

```rust
pub struct PaymentChallenge {
    pub protocol: PaymentProtocol,
    pub amount: u64,        // smallest unit (lamports or raw SPL)
    pub recipient: String,
    pub token: String,      // e.g. "USDC"
    pub payload: String,    // raw base64 (MPP) or JSON (x402)
}
```

#### `DemoPaymentResult`

```rust
pub struct DemoPaymentResult {
    pub success: bool,
    pub endpoint: String,
    pub challenge: Option<PaymentChallenge>,
    pub payment_header: Option<String>,
    pub response_body: Option<String>,
    pub error: Option<String>,
}
```

#### Free functions

```rust
// Encode a solana: transfer URI.
pub fn encode_transfer_url(fields: &TransferUrlFields) -> String;

// Encode a solana: transaction-request URI.
pub fn encode_transaction_request_url(fields: &TransactionRequestUrlFields) -> String;

// Parse a solana: URI into a ParsedUrl.
pub fn parse_url(url: &str) -> anyhow::Result<ParsedUrl>;

// Parse a 402 response's headers and extract a PaymentChallenge.
// Supports www-authenticate (MPP) and x-payment-required/x-payment (x402).
pub fn parse_402_response(headers: &[(String, String)]) -> Option<PaymentChallenge>;

// Run a full demo payment flow: GET → parse 402 → build mock header → retry.
// No real signing occurs in demo mode.
pub async fn demo_payment_flow(endpoint: &str, budget: u64) -> DemoPaymentResult;

// Validate an on-chain transfer by transaction signature.
pub async fn validate_transfer(
    rpc_url: &str,
    signature: &str,
    expected_recipient: Option<&str>,
) -> ValidationResult;
```

---

### coral_mcp

**File:** `src/coral_mcp.rs`

MCP client that connects Rust agents to CoralOS sessions as first-class participants — the same pattern used by the Python `coral_agent.py`.

#### `CoralMention`

Parsed fields from a `coral_wait_for_mentions` response.

```rust
pub struct CoralMention {
    pub thread_id: Option<String>,  // CoralOS thread ID
    pub sender: Option<String>,     // name/id of the sending agent
    pub text: String,               // raw response payload
}
```

#### `CoralMcpSession`

```rust
pub struct CoralMcpSession { /* connected MCP client */ }

impl CoralMcpSession {
    // Connect via Streamable-HTTP transport and perform the MCP handshake.
    // `url` = value of CORAL_CONNECTION_URL; `agent_name` = MCP client identity.
    pub async fn connect(url: &str, agent_name: &str) -> Result<Self>;

    // Block up to max_wait_ms for an incoming mention.
    // Returns None on timeout or empty response.
    pub async fn wait_for_mention(&self, max_wait_ms: u64) -> Result<Option<CoralMention>>;

    // Send a message into a CoralOS thread.
    // `mentions` is a list of agent names to @-mention.
    pub async fn send_message(
        &self,
        content: &str,
        thread_id: Option<&str>,
        mentions: &[&str],
    ) -> Result<()>;

    // High-level loop: wait_for_mention(30s) → call handler → send_message(response).
    // Retries on transport error after 2 s. Runs until the future is cancelled.
    pub async fn run_loop<F, Fut>(&self, handler: F)
    where
        F: Fn(CoralMention) -> Fut,
        Fut: Future<Output = String>;
}
```

---

## Usage Examples

### Creating and starting agents

```rust
use agent_core::AgentManager;

let manager = AgentManager::new();

// Create a generic RPC-polling agent.
let state = manager.create_agent("alpha".to_string()).expect("id must be unique");
println!("strategy: {}", state.strategy); // "rpc-poll"

// Start the agent's strategy loop in the current tokio runtime.
manager.start_agent("alpha").await.unwrap();

// After some work...
manager.stop_agent("alpha");

// Remove the agent.
manager.remove_agent("alpha");
```

### Custom Strategy implementation

```rust
use agent_core::strategy::Strategy;
use agent_core::{AgentAction, AgentState};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

pub struct PingStrategy {
    pub target: String,
}

#[async_trait]
impl Strategy for PingStrategy {
    fn name(&self) -> &'static str {
        "ping"
    }

    async fn run(&self, state: Arc<Mutex<AgentState>>) {
        let mut ticker = interval(Duration::from_secs(10));
        loop {
            ticker.tick().await;
            let is_running = state.lock().unwrap().is_running;
            if !is_running { break; }

            // ... do work ...
            state.lock().unwrap().actions.push(AgentAction {
                timestamp: chrono::Utc::now(),
                action_type: "ping".to_string(),
                details: format!("pinged {}", self.target),
                tx_signature: None,
                slot: None,
                latency_ms: 5,
            });
        }
    }
}

// Install it on an existing agent:
let manager = AgentManager::new();
manager.create_agent("my-agent".to_string());
manager.set_strategy("my-agent", std::sync::Arc::new(PingStrategy { target: "api.example.com".to_string() }));
```

### MessageBus send/receive

```rust
use agent_core::{AgentManager, AgentMessage};

let manager = AgentManager::new();
manager.create_agent("leader".to_string());
manager.set_agent_role("leader", agent_core::AgentRole::Leader);
manager.create_agent("worker-1".to_string());

// Broadcast from leader (requires can_broadcast permission).
manager.broadcast("leader", "task-assigned", r#"{"task":"compute"}"#);

// Direct message from leader to worker-1 (no permission check).
manager.send_direct("leader", "worker-1", "data-ready", "payload");

// Worker-1 reads its messages.
let messages = manager.get_messages("worker-1");
for msg in messages {
    println!("{}: {} → {:?}: {}", msg.from, msg.msg_type, msg.to, msg.payload);
}

// Read the conversation thread between two agents.
let thread = manager.get_conversation("leader", "worker-1");
```

### SharedState get/set/history

```rust
use agent_core::{AgentManager, AgentRole};
use serde_json::json;

let manager = AgentManager::new();
manager.create_agent("analyst".to_string());
manager.set_agent_role("analyst", AgentRole::Analyst);

// Write a value (requires can_modify_shared_state).
let ok = manager.set_shared_state("market/AAPL", json!(189.42), "analyst");
assert!(ok);

// Read it back.
let entry = manager.get_shared_state("market/AAPL").unwrap();
println!("v{}: {:?}", entry.version, entry.value); // v1: Number(189.42)

// Update it.
manager.set_shared_state("market/AAPL", json!(190.00), "analyst");

// Read change history.
let history = manager.get_state_history();
for change in history {
    println!("{}: {} -> {}", change.key, change.old_value.unwrap_or_default(), change.new_value);
}
```

### WorkflowEngine DAG execution

```rust
use agent_core::{AgentManager, Workflow, WorkflowStep};

let manager = AgentManager::new();

// Use the built-in Solana Pay checkout template.
let wf = Workflow::solana_pay_checkout("7xKF3rO1jW...", 1_000_000, "Demo");
manager.create_workflow(wf);

let wf_id = "pay-checkout-..."; // captured from wf.id before moving

// Or build a custom workflow:
let mut wf = Workflow::new("my-wf", "Data Pipeline", "3-step pipeline", "leader");
wf.add_step(WorkflowStep::new("fetch", "Fetch Data", "fetch from API"));
wf.add_step(
    WorkflowStep::new("analyze", "Analyze", "run model")
        .with_assignee("analyst")
        .depends_on("fetch"),
);
wf.add_step(
    WorkflowStep::new("report", "Report", "write results")
        .depends_on("analyze"),
);
manager.create_workflow(wf);

// Drive the DAG:
manager.assign_workflow_step("my-wf", "fetch", "worker-1");
manager.start_workflow_step("my-wf", "fetch");
manager.complete_workflow_step("my-wf", "fetch", r#"{"rows": 1200}"#.to_string());

// "analyze" is now ready (dependency satisfied).
let ready = manager.get_workflow("my-wf").unwrap();
let ready_steps = ready.get_ready_steps();
println!("{} steps ready", ready_steps.len()); // 1
```

### CoralMcpSession run_loop pattern

```rust
use agent_core::coral_mcp::CoralMcpSession;

#[tokio::main]
async fn main() {
    let session = CoralMcpSession::connect(
        "http://localhost:5555/mcp",
        "rust-agent",
    )
    .await
    .expect("connect failed");

    // run_loop blocks until the future is cancelled.
    session.run_loop(|mention| async move {
        println!("Got mention from {:?}: {}", mention.sender, mention.text);
        format!("rust-agent acknowledged: {}", mention.text)
    })
    .await;
}
```

To integrate with `AgentManager` (as done in `src-tauri/src/main.rs`):

```rust
let manager = manager.clone(); // cheap Arc clone
let agent_id = "my-coral-agent".to_string();
tokio::spawn(async move {
    session.run_loop(move |mention| {
        let mgr = manager.clone();
        let aid = agent_id.clone();
        async move {
            mgr.record_action(&aid, AgentAction {
                timestamp: chrono::Utc::now(),
                action_type: "coral-mention".to_string(),
                details: mention.text.chars().take(200).collect(),
                tx_signature: None,
                slot: None,
                latency_ms: 0,
            });
            format!("agent={} received from {:?}", aid, mention.sender)
        }
    }).await;
});
```

### Solana Pay URL encoding and validation

```rust
use agent_core::solana_pay::{encode_transfer_url, parse_url, TransferUrlFields, ParsedUrl};

// Encode a transfer URL.
let url = encode_transfer_url(&TransferUrlFields {
    recipient: "7xKF3rO1jW...".to_string(),
    amount: Some(0.01),     // SOL
    spl_token: None,
    reference: None,
    label: Some("My Store".to_string()),
    message: Some("Order #42".to_string()),
    memo: None,
});
// => "solana:7xKF3rO1jW...?amount=0.01&label=My+Store&message=Order+%2342"

// Parse it back.
match parse_url(&url).unwrap() {
    ParsedUrl::Transfer(t) => println!("recipient: {}", t.recipient),
    ParsedUrl::TransactionRequest(t) => println!("link: {}", t.link),
}

// Validate a completed transfer on-chain.
use agent_core::solana_pay::validate_transfer;
let result = validate_transfer(
    "https://api.devnet.solana.com",
    "5Q...",                        // transaction signature
    Some("7xKF3rO1jW..."),         // expected recipient pubkey
).await;
println!("valid: {}, slot: {:?}", result.valid, result.slot);
```

### HTTP 402 payment challenge flow

```rust
use agent_core::solana_pay::{parse_402_response, demo_payment_flow};

// Parse a 402 response manually.
let headers = vec![
    ("www-authenticate".to_string(), "Solana mpp=<base64-payload>".to_string()),
];
if let Some(challenge) = parse_402_response(&headers) {
    println!("protocol: {}, amount: {}, recipient: {}", challenge.protocol, challenge.amount, challenge.recipient);
}

// Run the full demo flow against a sandbox endpoint.
let result = demo_payment_flow("https://debugger.pay.sh/mpp/quote/AAPL", 1_000_000).await;
if result.success {
    println!("data: {}", result.response_body.unwrap());
} else {
    println!("failed: {}", result.error.unwrap());
}
```
