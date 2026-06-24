# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Repo Is

A monorepo for a Solana payment infrastructure project. The root-level README and CONTRIBUTING.md describe `pay` — a CLI tool that handles HTTP 402 payment challenges (MPP and x402 protocols) with stablecoin signing. The active code in this working directory is the **desktop** and **api** components.

## Repo Layout

| Directory | Purpose |
|-----------|---------|
| `desktop/` | Tauri desktop app — multi-agent developer dashboard |
| `desktop/agent-core/` | Rust library: agent lifecycle, workflows, messaging, Solana Pay, Jito |
| `desktop/src-tauri/` | Tauri backend: Tauri IPC commands, CoralOS HTTP client |
| `desktop/src-ui/` | React frontend: Vite + Tailwind + @xyflow/react + zustand |
| `api/` | Axum REST API wrapping `agent-core` (runs on port 8080) |
| `api-ts/` | Node.js/Express REST API wrapping TypeScript strategies (runs on port 8081) |
| `sdk/` | TypeScript SDK — `agent-core-ts` mirrors Rust agent-core; `sdk/` is the CoralClient HTTP wrapper |
| `web/` | Next.js consumer marketplace — Phantom wallet payment flow |
| `claude-skills/` | Claude Code skills for this project |
| `docs/` | Design documents and CoralOS reference config |
| `ref/` | Reference implementations (payment debugger) — read-only |

## Commands

### desktop (Tauri)

```sh
# Install UI dependencies
cd desktop/src-ui && npm install

# Build agent-core
cd desktop && cargo build

# Run in dev mode (starts Vite dev server + Tauri)
cd desktop/src-tauri && cargo tauri dev
```

### api (Axum REST API — Rust)

```sh
# Run the server (listens on http://0.0.0.0:8080)
cd api && cargo run

# Build
cd api && cargo build --release
```

### api-ts (Express REST API — TypeScript)

```sh
# Install dependencies (only needed once)
cd api-ts && npm install

# Run in dev mode with hot reload
cd api-ts && npm run dev

# Server listens on http://0.0.0.0:8081
# Set NEXT_PUBLIC_CORAL_SERVER=http://localhost:8081 in web/.env.local to use it
```

### Rust workspace (desktop)

```sh
cd desktop

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

The central library used by both `src-tauri` and `api`. Key modules:

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

### api (Axum REST API)

Exposes `agent-core` over HTTP at `/api/v1/`:
- `/agents` — CRUD for agents
- `/workflows` — Workflow management
- `/messages` — Message bus access
- `/shared-state` — Shared state read/write
- `/payments` — Payment flow records
- `/swarm` — CoralOS swarm proxy
- `/weather` — Example: WeatherStrategy endpoint

### api-ts (Express REST API)

TypeScript mirror of `api` — same REST surface, runs strategies from `sdk/agent-core-ts`.
Students swap backends by changing `NEXT_PUBLIC_CORAL_SERVER`.

### src-ui (React frontend)

Single-file `App.tsx` using Tauri IPC (`invoke`) to call all Rust commands. State is managed locally with `useState`/`useEffect`. Uses `@xyflow/react` for workflow DAG visualization and `zustand` if global store is needed.

## Key Constraints

- **Tauri IPC boundary** — all Rust → UI data must be `Serialize`/`Deserialize`. Add `#[derive(Serialize, Deserialize)]` to any new types crossing the boundary.
- **Agent strategies are `Send + Sync`** — required by the `async_trait` `Strategy` trait. Use `Arc<Mutex<_>>` for any interior state.
- **`AgentManager` uses `Mutex` not `RwLock`** — write-heavy operations dominate; don't switch without profiling.
- **Yellowstone gRPC (Triton)** is stubbed out — the dependency is commented in `agent-core/Cargo.toml`. Don't uncomment without adding the client crate.
- **CoralOS integration** in `src-tauri` makes real HTTP calls to a configurable base URL — default is empty and must be set at runtime via `set_coralos_url`.
