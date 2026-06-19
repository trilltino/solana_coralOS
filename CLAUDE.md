# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Repo Is

A monorepo for a Solana payment infrastructure project. The root-level README and CONTRIBUTING.md describe `pay` — a CLI tool that handles HTTP 402 payment challenges (MPP and x402 protocols) with stablecoin signing. The active code in this working directory is the **agent_demo** and **coral-server** components.

## Repo Layout

| Directory | Purpose |
|-----------|---------|
| `agent_demo/` | Tauri desktop app — multi-agent trading desk demo |
| `agent_demo/agent-core/` | Rust library: agent lifecycle, workflows, messaging, Solana Pay, Jito |
| `agent_demo/src-tauri/` | Tauri backend: Tauri IPC commands, CoralOS HTTP client |
| `agent_demo/src-ui/` | React frontend: Vite + Tailwind + @xyflow/react + zustand |
| `coral-server/` | Axum REST API wrapping `agent-core` (runs on port 8080) |
| `ref/` | Reference implementations (payment debugger, coral-server) — read-only reference |

## Commands

### agent_demo (Tauri)

```sh
# Install UI dependencies
cd agent_demo/src-ui && npm install

# Build agent-core
cd agent_demo && cargo build

# Run in dev mode (starts Vite dev server + Tauri)
cd agent_demo/src-tauri && cargo tauri dev
```

### coral-server (Axum API)

```sh
# Run the server (listens on http://0.0.0.0:8080)
cd coral-server && cargo run

# Build
cd coral-server && cargo build --release
```

### Rust workspace (agent_demo)

```sh
cd agent_demo

cargo build              # debug build
cargo build --release    # release build
cargo test               # all tests
cargo test -p agent-core # single crate tests
cargo clippy --workspace --all-targets -- -D warnings   # lint
cargo fmt --check        # format check
cargo fmt                # auto-format
```

## Architecture

### agent-core (Rust library)

The central library used by both `src-tauri` and `coral-server`. Key modules:

- **`agent.rs` / `AgentState`** — An agent holds a pluggable `Strategy` and an action log. Strategies are `async_trait` objects (`RpcPollStrategy`, `IdleStrategy`, Solana Pay strategies).
- **`manager.rs` / `AgentManager`** — Creates, stores, and drives multiple agents. Uses `BTreeMap` keyed by string ID. Also owns `MessageBus`, `SharedState`, and `WorkflowEngine`.
- **`message_bus.rs`** — Broadcast/direct messaging between agents (`AgentMessage`).
- **`shared_state.rs`** — Key-value store (`SharedStateEntry`) with versioning and change history, accessible to all agents.
- **`orchestrator/`** — `Workflow` (DAG of `WorkflowStep`s with dependencies) + `WorkflowEngine` that dispatches steps to agents.
- **`role.rs`** — `AgentRole` enum (`Leader`, `Worker`, `Trader`, etc.) with associated `RolePermissions`.
- **`solana_pay/`** — Solana Pay URL parsing, MPP/x402 payment challenge logic, transfer/payment strategies, and validation helpers.
- **`helius.rs` / `jito.rs`** — Helius RPC and Jito bundle integrations.

### src-tauri (Tauri backend)

- **`main.rs`** — All `#[tauri::command]` handlers. Wraps `AgentManager` in `Mutex<AgentManager>` and `CoralOSClient` in `Mutex<CoralOSClient>`.
- **`coralos.rs`** — Lightweight HTTP client that talks to a remote CoralOS server (session/agent APIs via `reqwest`).

### coral-server (Axum REST API)

Exposes `agent-core` over HTTP at `/api/v1/`:
- `/agents` — CRUD for agents
- `/workflows` — Workflow management
- `/messages` — Message bus access
- `/state` — Shared state read/write

### src-ui (React frontend)

Single-file `App.tsx` using Tauri IPC (`invoke`) to call all Rust commands. State is managed locally with `useState`/`useEffect`. Uses `@xyflow/react` for workflow DAG visualization and `zustand` if global store is needed.

## Key Constraints

- **Tauri IPC boundary** — all Rust → UI data must be `Serialize`/`Deserialize`. Add `#[derive(Serialize, Deserialize)]` to any new types crossing the boundary.
- **Agent strategies are `Send + Sync`** — required by the `async_trait` `Strategy` trait. Use `Arc<Mutex<_>>` for any interior state.
- **`AgentManager` uses `Mutex` not `RwLock`** — write-heavy operations dominate; don't switch without profiling.
- **Yellowstone gRPC (Triton)** is stubbed out — the dependency is commented in `agent-core/Cargo.toml`. Don't uncomment without adding the client crate.
- **CoralOS integration** in `src-tauri` makes real HTTP calls to a configurable base URL — default is empty and must be set at runtime via `set_coralos_url`.
