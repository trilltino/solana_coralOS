# coral-server

Axum REST API that exposes `agent-core` over HTTP. Runs on `0.0.0.0:8080`. Uses `agent-core` as a path dependency from `../agent_demo/agent-core`.

## Endpoints

| Route | File | Purpose |
|-------|------|---------|
| `GET /health` | `main.rs` | Health check |
| `GET/POST /api/v1/agents` | `api/agents.rs` | List and create agents |
| `GET/POST /api/v1/workflows` | `api/workflows.rs` | List and trigger workflows |
| `POST /api/v1/messages` | `api/messaging.rs` | Publish a message to the bus |
| `GET/PUT /api/v1/state` | `api/shared_state.rs` | Read and write shared state |

CORS and request tracing are enabled via `tower-http` middleware.

## Source Layout

```
src/
  main.rs          # Server setup: Axum router, middleware, AppState
  api/
    mod.rs         # Module re-exports
    agents.rs      # Agent CRUD handlers
    workflows.rs   # Workflow handlers
    messaging.rs   # Message bus handlers
    shared_state.rs # State handlers
```

## AppState

`AgentManager` is wrapped in `Arc<Mutex<AgentManager>>` and injected via Axum's `State` extractor. All handlers take `State(state): State<AppState>`.

## Adding a New Endpoint

1. Add handler function(s) to the appropriate file in `api/`.
2. Register the route in `main.rs` under the `/api/v1/` router.
3. Ensure request/response types derive `Serialize + Deserialize`.
4. Return `axum::Json<T>` for JSON responses; use `StatusCode` for errors.

## Commands

```sh
# From this directory
cargo run               # start server on :8080
cargo build --release   # production binary

# From repo root
cd coral-server && cargo run
```

## Dependencies

- `axum 0.7` — HTTP framework
- `tower 0.5` / `tower-http 0.5` — CORS, request tracing middleware
- `tokio 1` (full features) — async runtime
- `agent-core` (path: `../agent_demo/agent-core`) — shared business logic
- `serde/serde_json`, `uuid`, `anyhow`, `tracing/tracing-subscriber`
