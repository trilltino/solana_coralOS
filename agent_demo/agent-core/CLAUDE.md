# agent-core

Core Rust library shared by `src-tauri` and `coral-server`. Provides agent orchestration, inter-agent messaging, shared state, workflow execution, and Solana payment handling.

## Module Map

| Module | Key Types | Purpose |
|--------|-----------|---------|
| `agent.rs` | `Agent`, `AgentState` | Agent struct holding a pluggable `Strategy` and action log |
| `agent_meta.rs` | `AgentMeta`, `PayMode` | Role metadata attached to each agent |
| `manager.rs` | `AgentManager` | Creates/stores/drives agents; owns `MessageBus`, `SharedState`, `WorkflowEngine` |
| `strategy.rs` | `Strategy` (async_trait) | Pluggable behavior interface — all strategies implement this |
| `message_bus.rs` | `MessageBus`, `AgentMessage` | Broadcast and direct messaging between agents |
| `shared_state.rs` | `SharedState`, `SharedStateEntry` | Versioned key-value store accessible to all agents |
| `role.rs` | `AgentRole`, `RolePermissions` | Role enum (`Leader`, `Worker`, `Trader`, …) with permission sets |
| `orchestrator/workflow.rs` | `Workflow`, `WorkflowStep` | DAG of steps with explicit dependencies |
| `orchestrator/engine.rs` | `WorkflowEngine` | Dispatches steps to agents respecting dependency order |
| `solana_pay/` | — | URL parsing, MPP/x402 payment challenges, transfer strategies, validation, monitoring |
| `helius.rs` | `HeliusClient` | Helius RPC integration (wallet monitoring, transaction parsing) |
| `jito.rs` | `JitoClient` | Jito bundle submission for MEV-protected execution |

## Adding a New Strategy

1. Implement `async_trait Strategy for MyStrategy` in a new file.
2. Ensure `MyStrategy: Send + Sync` — wrap any interior mutable state in `Arc<Mutex<_>>`.
3. Export the type from `lib.rs`.

## Adding a New Type Crossing the IPC Boundary

Derive both `Serialize` and `Deserialize`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyType { ... }
```

Export from `lib.rs` so `src-tauri` and `coral-server` can import it.

## Solana Pay Sub-modules

| File | Purpose |
|------|---------|
| `url.rs` | Parse `solana:` URIs |
| `payment.rs` | MPP/x402 challenge structs and logic |
| `validation.rs` | Payment validation helpers |
| `strategies.rs` | `TransferStrategy`, `PaymentStrategy` implementations |
| `monitor.rs` | Confirmation monitoring loop |

## Testing

```sh
# From agent_demo/ workspace root
cargo test -p agent-core

# Single test by name
cargo test -p agent-core test_name
```

`dev-dependencies` include `tokio-test` for async test helpers.
