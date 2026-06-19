# CoralOS — How It Actually Works

This document explains what "CoralOS" means in this repo, how the pieces fit
together, and walks the full request path from a button click in the desktop
app down to the in-memory data structures that hold agent state — with the
actual code.

> **TL;DR** — "CoralOS" is not a separate operating system or product living in
> this tree. It is the name for the **remote HTTP control plane** that the Tauri
> desktop app talks to. In this repo that control plane is implemented by
> [`coral-server`](../coral-server) (an Axum REST API) wrapping the shared
> [`agent-core`](../agent_demo/agent-core) library. The desktop app reaches it
> through a thin `reqwest` client called [`CoralOSClient`](../agent_demo/src-tauri/src/coralos.rs).

---

## 1. The three layers

```
┌──────────────────────────────────────────────────────────────┐
│  src-ui (React)                                                │
│    invoke('coralos_list_sessions', { namespace })             │
└───────────────┬──────────────────────────────────────────────┘
                │ Tauri IPC (JSON over the webview bridge)
┌───────────────▼──────────────────────────────────────────────┐
│  src-tauri  (coralos.rs : CoralOSClient)                       │
│    reqwest GET http://localhost:8080/api/v1/agents             │
│    Bearer <api_token>                                          │
└───────────────┬──────────────────────────────────────────────┘
                │ HTTP/JSON   (the "CoralOS" wire protocol)
┌───────────────▼──────────────────────────────────────────────┐
│  coral-server (Axum)                                           │
│    Router → handler → AppState.manager: Arc<AgentManager>      │
└───────────────┬──────────────────────────────────────────────┘
                │ in-process Rust calls
┌───────────────▼──────────────────────────────────────────────┐
│  agent-core (AgentManager, MessageBus, SharedState, …)        │
│    the actual business logic + agent runtime                   │
└──────────────────────────────────────────────────────────────┘
```

The key insight: **CoralOS is a boundary, not a codebase.** The same
`agent-core` logic can be driven two ways:

- **Locally** — `src-tauri/main.rs` owns its own `AgentManager` and calls it
  directly via `#[tauri::command]` handlers (no network).
- **Remotely** — `CoralOSClient` makes HTTP calls to a *different* process
  (`coral-server`) that owns *its own* `AgentManager`.

So the desktop app effectively has two backends: an embedded one and a remote
"CoralOS" one. They expose the same concepts (agents, workflows, messages,
state) over different transports.

---

## 2. The CoralOS client (`src-tauri/src/coralos.rs`)

This is the only thing in the desktop app that knows the word "CoralOS." It is a
~230-line `reqwest` wrapper. Its entire job is: build a URL, attach a bearer
token, send, deserialize the JSON into `agent-core` types.

### 2.1 Configuration & state

The client holds three things, two of them mutable behind a `Mutex` so they can
be reconfigured at runtime without rebuilding the client:

```rust
pub struct CoralOSClient {
    base_url: Arc<Mutex<String>>,   // e.g. "http://localhost:8080"
    api_token: Arc<Mutex<String>>,  // bearer token, empty by default
    client: reqwest::Client,        // connection-pooling HTTP client
}
```

It is constructed in `main.rs` with a default URL of `http://localhost:8080` and
**an empty token**:

```rust
coralos: CoralOSClient::new(
    "http://localhost:8080".to_string(),
    "".to_string(),
),
```

`set_url` deliberately strips a trailing slash so URL building stays clean:

```rust
pub fn set_url(&self, url: String) {
    let mut guard = self.base_url.lock().unwrap();
    *guard = url.trim_end_matches('/').to_string();
}
```

> **Important runtime gotcha:** even though the default URL is `localhost:8080`,
> the project docs describe the production default as *empty* — meaning CoralOS
> calls are inert until the UI calls `set_coralos_url` (and usually
> `set_coralos_token`). Until a real server is configured, every CoralOS call
> just fails to connect.

### 2.2 Every call follows the same shape

There is no clever abstraction — each method is hand-written and identical in
structure. The canonical example:

```rust
pub async fn list_agents(&self) -> anyhow::Result<Vec<(String, AgentState)>> {
    let url = format!("{}/api/v1/agents", self.url());
    let resp = self
        .client
        .get(&url)
        .bearer_auth(self.token())   // Authorization: Bearer <token>
        .send()
        .await?;
    resp.error_for_status_ref()?;    // turn 4xx/5xx into an Err
    let agents: Vec<(String, AgentState)> = resp.json().await?;
    Ok(agents)
}
```

Three things to notice, because they repeat across **every** method:

1. **URL is `{base}/api/v1/<resource>`** — the version prefix is hard-coded into
   each format string, not centralized.
2. **`bearer_auth(self.token())`** — the token is attached unconditionally, even
   when empty. An empty bearer token is sent as `Authorization: Bearer `.
3. **`error_for_status_ref()?`** — HTTP-level errors (404, 500…) become Rust
   `Err`s *before* JSON parsing, so a missing agent surfaces as an error rather
   than a malformed-JSON panic.

### 2.3 The full client surface

| Method | HTTP call | Returns |
|--------|-----------|---------|
| `list_agents()` | `GET /api/v1/agents` | `Vec<(String, AgentState)>` |
| `get_agent(id)` | `GET /api/v1/agents/{id}` | `AgentState` |
| `create_agent(id)` | `POST /api/v1/agents` `{ "id": id }` | `AgentState` |
| `start_agent(id)` | `POST /api/v1/agents/{id}/start` | `bool` |
| `stop_agent(id)` | `POST /api/v1/agents/{id}/stop` | `bool` |
| `delete_agent(id)` | `DELETE /api/v1/agents/{id}` | `bool` |
| `get_agent_actions(id)` | `GET /api/v1/agents/{id}/actions` | `Vec<AgentAction>` |
| `list_workflows()` | `GET /api/v1/workflows` | `Vec<Workflow>` |
| `get_workflow(id)` | `GET /api/v1/workflows/{id}` | `Workflow` |
| `list_sessions(ns)` | *(derived)* `GET /api/v1/agents` | `Vec<SessionStateExtended>` |
| `get_session(ns, id)` | *(derived)* `GET /api/v1/agents/{id}` | `SessionStateExtended` |

### 2.4 The "session" compatibility shim

CoralOS's vocabulary historically used **sessions** and **namespaces** (think:
a CoralOS server hosting many isolated sessions, each containing agents). This
repo's `coral-server` doesn't actually have sessions — it just has a flat list
of agents. So the client *fakes* the session API on top of the agent API:

```rust
/// List all agents as sessions (legacy compatibility).
pub async fn list_sessions(&self, namespace: &str)
    -> anyhow::Result<Vec<SessionStateExtended>>
{
    let agents = self.list_agents().await?;          // real call
    Ok(agents
        .into_iter()
        .map(|(id, state)| SessionStateExtended {     // reshape into a "session"
            id,
            namespace: namespace.to_string(),
            status: if state.is_running { "active" } else { "stopped" }.into(),
            agents: vec![CoralAgent {
                name: state.strategy.clone(),
                status: if state.is_running { "running" } else { "stopped" }.into(),
                description: format!("RPC: {}", state.rpc_endpoint),
                links: vec![],
            }],
        })
        .collect())
}
```

So a "session" here is a synthetic 1:1 wrapper around a single agent:

- the **session id** = the agent id,
- the **namespace** is whatever the caller passed in (purely echoed back, the
  server never sees it),
- each session contains exactly **one** `CoralAgent`, whose `name` is the
  agent's *strategy* string and whose `description` embeds the RPC endpoint.

These shapes are what the UI actually renders:

```rust
pub struct CoralAgent {
    pub name: String,
    pub status: String,
    pub description: String,
    pub links: Vec<String>,
}

pub struct SessionStateExtended {
    pub id: String,
    pub namespace: String,
    pub status: String,
    pub agents: Vec<CoralAgent>,
}
```

> **Takeaway:** "namespace" and "session" are vestigial CoralOS concepts kept
> for UI compatibility. On the wire there is only `/agents`.

---

## 3. How the desktop app exposes CoralOS to the UI

The React frontend cannot make these HTTP calls itself in the CoralOS model —
it goes through Tauri IPC. `src-tauri/main.rs` registers four thin proxy
commands. They lock nothing heavy; they just forward to the client and stringify
errors (Tauri commands must return `Result<T, String>`):

```rust
#[tauri::command]
fn coralos_set_url(state: State<AppState>, url: String) -> Result<bool, String> {
    state.coralos.set_url(url);
    Ok(true)
}

#[tauri::command]
async fn coralos_list_sessions(
    state: State<'_, AppState>,
    namespace: String,
) -> Result<Vec<SessionStateExtended>, String> {
    state.coralos
        .list_sessions(&namespace)
        .await
        .map_err(|e| e.to_string())
}
```

Registered in the builder:

```rust
.invoke_handler(tauri::generate_handler![
    /* … */
    coralos_set_url,
    coralos_set_token,
    coralos_list_sessions,
    coralos_get_session,
    /* … */
])
```

From the UI the flow is:

```ts
await invoke('coralos_set_url',   { url: 'https://my-coral-host:8080' });
await invoke('coralos_set_token', { token: 'secret' });
const sessions = await invoke('coralos_list_sessions', { namespace: 'default' });
```

Note the `AppState` holds the client by value (not behind a `Mutex`), which is
fine because `CoralOSClient`'s mutable fields are *internally* synchronized with
their own `Arc<Mutex<…>>`:

```rust
.manage(AppState {
    manager: AgentManager::new(),
    coralos: CoralOSClient::new("http://localhost:8080".into(), "".into()),
    flows: std::sync::Mutex::new(Vec::new()),
})
```

---

## 4. The server side: what CoralOS actually *is* in this repo

When the client hits `http://…:8080`, it's talking to `coral-server`. This is
the concrete implementation of the "CoralOS" control plane.

### 4.1 Boot & routing (`coral-server/src/main.rs`)

```rust
#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<AgentManager>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let state = AppState { manager: Arc::new(AgentManager::new()) };

    let app = Router::new()
        .route("/health", get(health_check))
        .nest("/api/v1/agents",    api::agents::routes())
        .nest("/api/v1/workflows", api::workflows::routes())
        .nest("/api/v1/messages",  api::messaging::routes())
        .nest("/api/v1/state",     api::shared_state::routes())
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

What this tells you about how CoralOS *really* behaves:

- **One process, one `AgentManager`.** All state lives in
  `Arc<AgentManager>` shared across every request handler. There is no database
  and no persistence — restart the server and all agents/workflows/state are
  gone.
- **No authentication is actually enforced.** The client sends
  `Authorization: Bearer <token>`, but the server never reads it. CORS is wide
  open (`Any` origin/method/header). This is a demo control plane, not a
  hardened one.
- **Four resource groups** nested under `/api/v1`, mirroring the four pillars of
  `agent-core`: agents, workflows, messages, shared state.

### 4.2 Resource group: agents (`api/agents.rs`)

This is the most-used group and the one the CoralOS client mirrors. Routes:

```rust
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/",              get(list_agents).post(create_agent))
        .route("/:id",           get(get_agent).delete(delete_agent))
        .route("/:id/start",     post(start_agent))
        .route("/:id/stop",      post(stop_agent))
        .route("/:id/actions",   get(get_actions))
        .route("/:id/rpc",       post(set_rpc))
        .route("/:id/triton",    post(set_triton))
}
```

Each handler is a near-direct pass-through to `AgentManager`, translating the
result into an HTTP status code. Creation rejects blank ids and reports a name
collision as `409 Conflict`:

```rust
async fn create_agent(
    State(state): State<AppState>,
    Json(req): Json<CreateAgentRequest>,
) -> Result<Json<AgentState>, StatusCode> {
    if req.id.trim().is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);   // 422
    }
    state.manager
        .create_agent(req.id)
        .map(Json)
        .ok_or(StatusCode::CONFLICT)                    // 409
}
```

`start_agent` is the only `async` manager call (it spawns the strategy loop);
the rest are synchronous in-memory mutations:

```rust
async fn start_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<bool>, StatusCode> {
    state.manager
        .start_agent(&id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

Note the server exposes **more** than the CoralOS client consumes — e.g.
`POST /:id/rpc` (change RPC endpoint) and `POST /:id/triton` (configure a Triton
gRPC PAYG key). The client in `coralos.rs` simply hasn't wired those up yet.

### 4.3 Resource group: workflows (`api/workflows.rs`)

Workflows are DAGs of steps. The server exposes lifecycle transitions on
individual steps:

```rust
.route("/",                             get(list_workflows).post(create_workflow))
.route("/:id",                          get(get_workflow).delete(delete_workflow))
.route("/:id/steps/:step_id/assign",    post(assign_step))
.route("/:id/steps/:step_id/start",     post(start_step))
.route("/:id/steps/:step_id/complete",  post(complete_step))
.route("/:id/steps/:step_id/fail",      post(fail_step))
```

Creation clamps priority into the documented 1–10 range rather than rejecting
out-of-range input:

```rust
let mut workflow = Workflow::new(&req.id, &req.name, &req.description, &req.created_by);
workflow.priority = req.priority.clamp(1, 10);
for step in req.steps {
    workflow.add_step(step);
}
state.manager.create_workflow(workflow.clone());
```

### 4.4 Resource group: messages (`api/messaging.rs`)

The message bus supports both direct and broadcast messages. The `to` field
being `None` is the discriminator:

```rust
let msg = match req.to {
    Some(to_id) => AgentMessage::direct(req.from, to_id, req.msg_type, req.payload),
    None        => AgentMessage::broadcast(req.from, req.msg_type, req.payload),
};
state.manager.send_message(msg);
```

Reads come back per-agent or as a two-party conversation thread:

```rust
.route("/",                     post(send_message))
.route("/:agent_id",            get(get_messages))
.route("/conversation/:a/:b",   get(get_conversation))
```

### 4.5 Resource group: shared state (`api/shared_state.rs`)

A versioned key-value store with a change-history log. Every write requires a
`changed_by` attribution string, which is what feeds the audit history:

```rust
pub struct SetStateRequest {
    pub value: Value,        // arbitrary JSON
    pub changed_by: String,  // who made the change (required, non-empty)
}
```

```rust
.route("/",         get(get_all_state))
.route("/history",  get(get_state_history))
.route("/:key",     get(get_state).post(set_state).delete(delete_state))
```

---

## 5. The data that crosses the CoralOS boundary

Because the same `agent-core` types are serialized on the server and
deserialized on the client, the wire format *is* the Rust type. The central one
is `AgentState` ([`agent-core/src/agent.rs`](../agent_demo/agent-core/src/agent.rs)):

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentState {
    pub is_running: bool,
    pub actions: Vec<AgentAction>,
    pub rpc_endpoint: String,
    pub network: String,
    pub strategy: String,       // name of the active Strategy impl
}
```

…and the per-action log entry:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentAction {
    pub timestamp: DateTime<Utc>,
    pub action_type: String,     // "rpc-poll", "url-generated", …
    pub details: String,
    pub tx_signature: Option<String>,
    pub slot: Option<u64>,
    pub latency_ms: u64,
}
```

This is why `coralos.rs` can write `let agents: Vec<(String, AgentState)> =
resp.json().await?;` — the server literally returns `Json(manager.list_agents())`
where `list_agents()` is `Vec<(String, AgentState)>`. The boundary is type-safe
by construction because both sides depend on the same crate.

---

## 6. End-to-end walkthrough: "show me the sessions"

Putting it all together, here is the complete path of one CoralOS call:

1. **UI** calls `invoke('coralos_list_sessions', { namespace: 'default' })`.
2. **Tauri** routes to the `coralos_list_sessions` command in `main.rs`, which
   calls `state.coralos.list_sessions("default")`.
3. **CoralOSClient** can't list sessions for real, so it calls
   `self.list_agents()`, which issues
   `GET http://localhost:8080/api/v1/agents` with a bearer header.
4. **coral-server** matches the route to `list_agents`, which returns
   `Json(state.manager.list_agents())` — a snapshot of every agent in the
   single in-process `AgentManager`.
5. **agent-core** serializes `Vec<(String, AgentState)>` to JSON.
6. Back in **CoralOSClient**, each `(id, AgentState)` is reshaped into a
   synthetic `SessionStateExtended` (one agent per session, status derived from
   `is_running`, RPC endpoint folded into the description).
7. **Tauri** serializes that to JSON across the IPC bridge.
8. **UI** renders the session list.

No state was persisted; everything was read live from the server's memory.

---

## 7. What CoralOS does and does *not* do (summary)

**Does:**

- Act as a remote, HTTP/JSON control plane for the same agent runtime the
  desktop app embeds — agents, workflows, message bus, shared state.
- Keep all state in a single in-memory `AgentManager` behind `Arc`.
- Let the desktop app point at *any* CoralOS host at runtime
  (`coralos_set_url` / `coralos_set_token`).
- Preserve legacy "session/namespace" vocabulary by synthesizing sessions from
  the flat agent list on the client side.

**Does not:**

- Persist anything (no DB; restart = clean slate).
- Enforce the bearer token it accepts, or restrict CORS.
- Implement real multi-tenant sessions/namespaces server-side — those are a
  client-side fiction.
- Cover the full server API from the client — `set_rpc` / `set_triton` /
  workflow-step transitions / messaging / shared-state endpoints exist on the
  server but aren't all surfaced through `CoralOSClient` yet.

---

## 8. File reference

| Concern | File |
|---------|------|
| CoralOS HTTP client | [`agent_demo/src-tauri/src/coralos.rs`](../agent_demo/src-tauri/src/coralos.rs) |
| Tauri proxy commands | [`agent_demo/src-tauri/src/main.rs`](../agent_demo/src-tauri/src/main.rs) (`coralos_*`) |
| Server bootstrap & routing | [`coral-server/src/main.rs`](../coral-server/src/main.rs) |
| Agent endpoints | [`coral-server/src/api/agents.rs`](../coral-server/src/api/agents.rs) |
| Workflow endpoints | [`coral-server/src/api/workflows.rs`](../coral-server/src/api/workflows.rs) |
| Message endpoints | [`coral-server/src/api/messaging.rs`](../coral-server/src/api/messaging.rs) |
| Shared-state endpoints | [`coral-server/src/api/shared_state.rs`](../coral-server/src/api/shared_state.rs) |
| Wire types | [`agent_demo/agent-core/src/agent.rs`](../agent_demo/agent-core/src/agent.rs) |
