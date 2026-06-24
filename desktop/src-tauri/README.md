# src-tauri

The Tauri backend for the agent trading desk desktop application. It wraps `agent-core` behind a set of `#[tauri::command]` handlers, giving the React frontend (`src-ui`) access to the full multi-agent runtime through Tauri's type-safe IPC bridge.

---

## Table of Contents

1. [What it does](#what-it-does)
2. [AppState structure](#appstate-structure)
3. [Cargo.toml dependencies](#cargotoml-dependencies)
4. [Complete Tauri command reference](#complete-tauri-command-reference)
   - [Agent CRUD commands](#agent-crud-commands)
   - [Agent role commands](#agent-role-commands)
   - [Messaging commands](#messaging-commands)
   - [Shared state commands](#shared-state-commands)
   - [Workflow commands](#workflow-commands)
   - [Solana Pay commands](#solana-pay-commands)
   - [Pay Demo commands](#pay-demo-commands)
   - [CoralOS proxy commands](#coralos-proxy-commands)
   - [CoralOS MCP commands](#coralos-mcp-commands)
   - [Helius commands](#helius-commands)
5. [CoralOSClient](#coralosc lient)
6. [PaymentFlowRecord](#paymentflowrecord)
7. [Frontend invoke() examples](#frontend-invoke-examples)

---

## What it does

`src-tauri` is the Rust side of the Tauri desktop app. On startup it:

1. Creates a single `AgentManager` instance wrapped in `AppState`.
2. Creates a `CoralOSClient` pointed at `http://localhost:8080` (overridable at runtime).
3. Registers all `#[tauri::command]` handlers with `tauri::generate_handler![]`.
4. Serves the React frontend via Tauri's custom-protocol feature.

Every Tauri command takes a `State<AppState>` extractor to access the shared manager, CoralOS client, and payment-flow ring buffer. Async commands additionally take `State<'_, AppState>` with an explicit lifetime.

---

## AppState structure

```rust
struct AppState {
    manager: AgentManager,           // multi-agent runtime (internally Arc-based)
    coralos: CoralOSClient,          // HTTP client for the remote CoralOS server
    flows: Mutex<Vec<PaymentFlowRecord>>,  // ring buffer of payment flows (max 100)
}
```

---

## Cargo.toml dependencies

| Crate | Purpose |
|-------|---------|
| `tauri` 2 | Desktop app framework, IPC bridge |
| `tauri-plugin-shell` 2 | Shell command execution (used by python_agent module) |
| `serde` + `serde_json` 1 | JSON serialization across the IPC boundary |
| `tokio` 1 (full) | Async runtime for async commands and spawned background tasks |
| `chrono` 0.4 (serde) | Timestamps on payment flow records |
| `reqwest` 0.12 (json) | HTTP client inside `CoralOSClient` |
| `anyhow` 1 | Error propagation |
| `agent-core` (path) | The core multi-agent library |

---

## Complete Tauri command reference

### Agent CRUD commands

#### `create_agent`

Create a generic agent with the default `RpcPollStrategy`.

- **Args:** `id: String`
- **Returns:** `Result<AgentState, String>`
- **Error:** `"Agent with this ID already exists"` if `id` is taken.

#### `list_agents`

List all agents as `(id, state)` pairs sorted by ID.

- **Args:** none
- **Returns:** `Result<Vec<(String, AgentState)>, String>`

#### `delete_agent`

Remove an agent and its metadata.

- **Args:** `id: String`
- **Returns:** `Result<bool, String>` — `true` if the agent was found and removed.

#### `get_agent_state`

Snapshot the current state of a single agent.

- **Args:** `id: String`
- **Returns:** `Result<AgentState, String>`
- **Error:** `"Agent not found"` if `id` does not exist.

#### `set_agent_rpc`

Override an agent's RPC endpoint (takes effect on next strategy tick).

- **Args:** `id: String`, `url: String`
- **Returns:** `Result<bool, String>`

#### `set_agent_triton`

Configure an agent's RPC endpoint using a Triton/Helius PAYG x-token. If `grpc_endpoint` is provided it is used directly; otherwise the Triton mainnet endpoint is used.

- **Args:** `id: String`, `x_token: String`, `grpc_endpoint: Option<String>`
- **Returns:** `Result<bool, String>`

#### `start_agent`

Spawn the agent's strategy loop on the Tokio runtime.

- **Args:** `id: String`
- **Returns:** `Result<bool, String>`
- **Note:** This is `async`.

#### `stop_agent`

Signal the strategy loop to exit on its next `is_running` check.

- **Args:** `id: String`
- **Returns:** `Result<bool, String>`

#### `get_agent_actions`

Return the full action log for an agent.

- **Args:** `id: String`
- **Returns:** `Result<Vec<AgentAction>, String>`

---

### Agent role commands

#### `set_agent_role`

Assign a role to an agent. Accepted role strings: `"leader"`, `"worker"`, `"monitor"`, `"analyst"`, `"trader"`, `"coordinator"`.

- **Args:** `id: String`, `role: String`
- **Returns:** `Result<bool, String>`
- **Error:** `"Invalid role"` for unrecognized strings.

#### `get_agent_meta`

Return an agent's full `AgentMeta` (role, created_at, tags).

- **Args:** `id: String`
- **Returns:** `Result<AgentMeta, String>`

#### `list_agents_with_roles`

List all agents with their runtime state and full metadata.

- **Args:** none
- **Returns:** `Result<Vec<(String, AgentState, AgentMeta)>, String>`

---

### Messaging commands

#### `send_message`

Send a message from one agent to another (direct) or to all agents (broadcast).

- **Args:** `from: String`, `to: Option<String>`, `msg_type: String`, `payload: String`
- **Returns:** `Result<bool, String>`
- **Notes:** `to = null` sends a broadcast; `to = "agent-id"` sends a direct message.

#### `get_messages`

Return all messages visible to `agent_id` (broadcasts + direct).

- **Args:** `agent_id: String`
- **Returns:** `Result<Vec<AgentMessage>, String>`

#### `get_all_messages`

Return every message on the bus (admin/debug view).

- **Args:** none
- **Returns:** `Result<Vec<AgentMessage>, String>`

#### `get_conversation`

Return the direct-message thread between two agents.

- **Args:** `agent_a: String`, `agent_b: String`
- **Returns:** `Result<Vec<AgentMessage>, String>`

---

### Shared state commands

#### `set_shared_state`

Write a JSON value to the shared store. The writer must have `can_modify_shared_state` permission (checked via their role).

- **Args:** `key: String`, `value: Value` (any JSON), `changed_by: String`
- **Returns:** `Result<bool, String>` — `false` if `changed_by` lacks permission.

#### `get_shared_state`

Read a single entry.

- **Args:** `key: String`
- **Returns:** `Result<Option<SharedStateEntry>, String>`

#### `get_all_shared_state`

Snapshot the entire shared store.

- **Args:** none
- **Returns:** `Result<HashMap<String, SharedStateEntry>, String>`

#### `delete_shared_state`

Delete a key. Requires `can_modify_shared_state` permission.

- **Args:** `key: String`, `changed_by: String`
- **Returns:** `Result<bool, String>`

#### `get_state_history`

Return the bounded change-history log (newest last, max 500 entries).

- **Args:** none
- **Returns:** `Result<Vec<StateChange>, String>`

---

### Workflow commands

#### `create_workflow`

Register a new workflow with a list of steps.

- **Args:** `id: String`, `name: String`, `description: String`, `steps: Vec<WorkflowStep>`, `priority: u8` (clamped 1–10), `created_by: String`
- **Returns:** `Result<bool, String>`

#### `get_workflow`

Retrieve a workflow by ID.

- **Args:** `id: String`
- **Returns:** `Result<Option<Workflow>, String>`

#### `list_workflows`

List all registered workflows.

- **Args:** none
- **Returns:** `Result<Vec<Workflow>, String>`

#### `delete_workflow`

Delete a workflow.

- **Args:** `id: String`
- **Returns:** `Result<bool, String>`

#### `assign_workflow_step`

Assign an agent to a workflow step (transitions step to `Assigned`).

- **Args:** `workflow_id: String`, `step_id: String`, `agent_id: String`
- **Returns:** `Result<bool, String>`

#### `start_workflow_step`

Mark a step as `InProgress`.

- **Args:** `workflow_id: String`, `step_id: String`
- **Returns:** `Result<bool, String>`

#### `complete_workflow_step`

Mark a step as `Completed` with a result string. Automatically transitions the workflow to `Completed` when all steps are done.

- **Args:** `workflow_id: String`, `step_id: String`, `result: String`
- **Returns:** `Result<bool, String>`

#### `fail_workflow_step`

Mark a step (and the workflow) as `Failed`.

- **Args:** `workflow_id: String`, `step_id: String`, `reason: String`
- **Returns:** `Result<bool, String>`

#### `get_agent_workflows`

Return workflows that have `agent_id` assigned to at least one step.

- **Args:** `agent_id: String`
- **Returns:** `Result<Vec<Workflow>, String>`

#### `get_active_workflows`

Return workflows in `Draft`, `Running`, or `Paused` status.

- **Args:** none
- **Returns:** `Result<Vec<Workflow>, String>`

---

### Solana Pay commands

#### `create_solana_pay_agent`

Create a Solana Pay agent. `mode` must be `"transfer"` or `"payment"`.

- **Args:** `id: String`, `mode: String`
- **Returns:** `Result<AgentState, String>`

#### `get_agent_capabilities`

Return the list of capability strings for an agent's current strategy.

- **Args:** `id: String`
- **Returns:** `Result<Vec<String>, String>`

#### `solana_pay_create_url`

Encode a Solana Pay transfer URL. `amount` is in lamports (divided by 1e9 internally).

- **Args:** `recipient: String`, `amount: u64`, `label: String`, `message: String`
- **Returns:** `Result<String, String>`

#### `solana_pay_parse_url`

Parse a `solana:` URI.

- **Args:** `url: String`
- **Returns:** `Result<ParsedUrl, String>`

#### `solana_pay_validate`

Validate an on-chain transfer by tx signature. Uses the agent's configured RPC endpoint.

- **Args:** `id: String`, `signature: String`, `expected_recipient: Option<String>`
- **Returns:** `Result<ValidationResult, String>`
- **Note:** This is `async`.

#### `x402_parse_challenge`

Parse the headers of a 402 response into a `PaymentChallenge`.

- **Args:** `headers: Vec<(String, String)>`
- **Returns:** `Result<Option<PaymentChallenge>, String>`

#### `x402_demo_payment`

Run a full demo payment flow (no real signing). Stores a `PaymentFlowRecord` in the ring buffer.

- **Args:** `endpoint: String`, `budget: u64`
- **Returns:** `Result<DemoPaymentResult, String>`
- **Note:** This is `async`.

---

### Pay Demo commands

#### `create_triton_monitor_agent`

Create a Triton Yellowstone payment-monitor agent that watches for an SOL payment at `recipient`.

- **Args:** `id: String`, `recipient: String`, `amount_sol: f64`, `x_token: String`, `grpc_endpoint: Option<String>`, `label: Option<String>`
- **Returns:** `Result<AgentState, String>`

#### `generate_solana_pay_url`

Generate a Solana Pay transfer URL with `amount_sol` in SOL.

- **Args:** `recipient: String`, `amount_sol: f64`, `label: Option<String>`, `message: Option<String>`
- **Returns:** `Result<String, String>`

#### `get_pending_payment`

Check if a seller agent has received a `payment-received` action. Returns the tx signature if found.

- **Args:** `seller_id: String`
- **Returns:** `Result<Option<String>, String>`

#### `complete_sale`

Completes a sale: calls `pay.sh` via the full demo payment flow, records actions on both seller and buyer agents, sends a `"data-delivered"` direct message from seller to buyer, and writes the result to shared state under `"sale/{seller_id}/result"`.

- **Args:** `seller_id: String`, `buyer_id: String`, `tx_signature: Option<String>`
- **Returns:** `Result<String, String>` — the data payload returned from pay.sh.
- **Note:** This is `async`.

#### `get_payment_flows`

Return all stored payment flow records (max 100).

- **Args:** none
- **Returns:** `Result<Vec<PaymentFlowRecord>, String>`

---

### CoralOS proxy commands

#### `coralos_set_url`

Override the CoralOS server base URL at runtime.

- **Args:** `url: String`
- **Returns:** `Result<bool, String>`

#### `coralos_set_token`

Set the Bearer token used for CoralOS API calls.

- **Args:** `token: String`
- **Returns:** `Result<bool, String>`

#### `coralos_list_sessions`

List CoralOS sessions for a namespace, proxied through `CoralOSClient`.

- **Args:** `namespace: String`
- **Returns:** `Result<Vec<SessionStateExtended>, String>`
- **Note:** This is `async`.

#### `coralos_get_session`

Get a single CoralOS session by ID.

- **Args:** `namespace: String`, `session_id: String`
- **Returns:** `Result<SessionStateExtended, String>`
- **Note:** This is `async`.

---

### CoralOS MCP commands

#### `coralos_mcp_join`

Connect to a CoralOS MCP endpoint as an agent and spawn a background `run_loop`. Each incoming mention is recorded as a `"coral-mention"` action on the agent's log and acknowledged with a receipt message.

- **Args:** `connection_url: String`, `agent_id: String`
- **Returns:** `Result<bool, String>`
- **Note:** This is `async`. The background loop runs until the process exits.

#### `coralos_mcp_status`

Check whether an agent exists (proxy check for MCP session liveness).

- **Args:** `agent_id: String`
- **Returns:** `Result<bool, String>`

---

### Helius commands

#### `set_agent_helius`

Configure an agent's RPC endpoint to use a Helius devnet endpoint with an API key. Falls back to the public devnet RPC if `api_key` is empty.

- **Args:** `id: String`, `api_key: String`
- **Returns:** `Result<bool, String>`

#### `create_helius_monitor_agent`

Create a Triton payment-monitor agent using Helius devnet URLs.

- **Args:** `id: String`, `wallet: String`, `amount_sol: f64`, `api_key: String`, `label: Option<String>`
- **Returns:** `Result<AgentState, String>`

---

## CoralOSClient

`CoralOSClient` is a lightweight `reqwest`-backed HTTP client that communicates with a remote CoralOS server (or `coral-server`). The base URL and Bearer token are stored behind `Mutex` so they can be updated at runtime without restarting the app.

```rust
pub struct CoralOSClient {
    base_url: Arc<Mutex<String>>,
    api_token: Arc<Mutex<String>>,
    client: reqwest::Client,
}
```

**Methods:**

```rust
pub fn new(base_url: String, api_token: String) -> Self;
pub fn set_url(&self, url: String);     // strips trailing slash
pub fn set_token(&self, token: String);

// Agents
pub async fn list_agents(&self) -> anyhow::Result<Vec<(String, AgentState)>>;
pub async fn get_agent(&self, id: &str) -> anyhow::Result<AgentState>;
pub async fn create_agent(&self, id: &str) -> anyhow::Result<AgentState>;
pub async fn start_agent(&self, id: &str) -> anyhow::Result<bool>;
pub async fn stop_agent(&self, id: &str) -> anyhow::Result<bool>;
pub async fn delete_agent(&self, id: &str) -> anyhow::Result<bool>;
pub async fn get_agent_actions(&self, id: &str) -> anyhow::Result<Vec<AgentAction>>;

// Workflows
pub async fn list_workflows(&self) -> anyhow::Result<Vec<Workflow>>;
pub async fn get_workflow(&self, id: &str) -> anyhow::Result<Workflow>;

// Legacy session compatibility (maps agents → SessionStateExtended shape)
pub async fn list_sessions(&self, namespace: &str) -> anyhow::Result<Vec<SessionStateExtended>>;
pub async fn get_session(&self, namespace: &str, session_id: &str) -> anyhow::Result<SessionStateExtended>;
```

**Supporting types:**

```rust
pub struct CoralAgent {
    pub name: String,
    pub status: String,      // "running" | "stopped"
    pub description: String, // "RPC: <endpoint>"
    pub links: Vec<String>,
}

pub struct SessionStateExtended {
    pub id: String,
    pub namespace: String,
    pub status: String,      // "active" | "stopped"
    pub agents: Vec<CoralAgent>,
}
```

---

## PaymentFlowRecord

Captures the full lifecycle of a payment flow: request → 402 challenge → payment → data delivery.

```rust
struct PaymentFlowRecord {
    pub id: String,
    pub agent_id: String,
    pub endpoint: String,
    pub status: String,                 // "success" | "failed"
    pub protocol: Option<String>,       // "mpp" | "x402"
    pub amount: Option<u64>,            // smallest unit
    pub recipient: Option<String>,
    pub token: Option<String>,          // e.g. "USDC"
    pub payment_header: Option<String>, // Authorization header sent
    pub response_body: Option<String>,  // data returned after payment
    pub error: Option<String>,
    pub request_at: String,             // RFC 3339
    pub challenge_at: Option<String>,
    pub payment_at: Option<String>,
    pub delivery_at: Option<String>,
}
```

Up to 100 records are retained; older entries are dropped automatically.

---

## Frontend invoke() examples

All commands are called from TypeScript using Tauri's `invoke()` function, imported from `@tauri-apps/api/core`.

```typescript
import { invoke } from '@tauri-apps/api/core';

// Create an agent
const state = await invoke<AgentState>('create_agent', { id: 'alpha' });
console.log(state.strategy); // "rpc-poll"

// Start it
await invoke<boolean>('start_agent', { id: 'alpha' });

// Read its action log
const actions = await invoke<AgentAction[]>('get_agent_actions', { id: 'alpha' });

// Send a direct message
await invoke<boolean>('send_message', {
    from: 'alpha',
    to: 'beta',
    msgType: 'task-assigned',
    payload: JSON.stringify({ task: 'compute' }),
});

// Broadcast
await invoke<boolean>('send_message', {
    from: 'leader',
    to: null,
    msgType: 'announce',
    payload: 'system ready',
});

// Write shared state
await invoke<boolean>('set_shared_state', {
    key: 'market/AAPL',
    value: 189.42,
    changedBy: 'analyst',
});

// Create and drive a workflow
await invoke<boolean>('create_workflow', {
    id: 'wf-1',
    name: 'My Pipeline',
    description: 'Demo',
    steps: [
        { id: 'fetch', name: 'Fetch', description: 'Get data', status: 'Pending', assigned_to: null, dependencies: [], result: null, started_at: null, completed_at: null, timeout_secs: null },
    ],
    priority: 5,
    createdBy: 'leader',
});
await invoke<boolean>('start_workflow_step', { workflowId: 'wf-1', stepId: 'fetch' });
await invoke<boolean>('complete_workflow_step', { workflowId: 'wf-1', stepId: 'fetch', result: 'done' });

// Encode a Solana Pay URL
const url = await invoke<string>('solana_pay_create_url', {
    recipient: '7xKF3rO1jW...',
    amount: 10_000_000,   // lamports; divided by 1e9 internally → 0.01 SOL
    label: 'My Store',
    message: 'Order #42',
});

// Run a demo x402 payment flow
const result = await invoke<DemoPaymentResult>('x402_demo_payment', {
    endpoint: 'https://debugger.pay.sh/mpp/quote/AAPL',
    budget: 1_000_000,
});
console.log(result.success, result.response_body);

// Join a CoralOS MCP session
await invoke<boolean>('coralos_mcp_join', {
    connectionUrl: process.env.CORAL_CONNECTION_URL,
    agentId: 'my-rust-agent',
});
```
