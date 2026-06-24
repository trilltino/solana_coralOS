# coral-server

`coral-server` is an Axum REST API that exposes the full `agent-core` multi-agent runtime over HTTP. It provides the same capabilities as `src-tauri` but as a standalone server — making it suitable for headless environments, CI, or non-Tauri frontends.

The server listens on `http://0.0.0.0:8080` and enables CORS for all origins so any browser or CLI tool can reach it without preflight configuration.

---

## Table of Contents

1. [Running the server](#running-the-server)
2. [AppState fields](#appstate-fields)
3. [CORS configuration](#cors-configuration)
4. [REST API reference](#rest-api-reference)
   - [Health check](#health-check)
   - [Agents — `/api/v1/agents`](#agents)
   - [Workflows — `/api/v1/workflows`](#workflows)
   - [Messages — `/api/v1/messages`](#messages)
   - [Shared State — `/api/v1/state`](#shared-state)
   - [Solana Pay — `/api/v1/solana-pay`](#solana-pay)
   - [Pay Demo — `/api/v1/pay-demo`](#pay-demo)
   - [CoralOS — `/api/v1/coralos`](#coralos)

---

## Running the server

```sh
# Development (auto-rebuild on change not included, use cargo-watch separately)
cd coral-server && cargo run

# Release build
cd coral-server && cargo build --release
./target/release/coral-server

# The server prints:
# coral-server listening on http://0.0.0.0:8080
```

All routes are mounted at `/api/v1/`. The health check is at `/health`.

---

## AppState fields

```rust
pub struct AppState {
    pub manager: Arc<AgentManager>,          // multi-agent runtime (shared across handlers)
    pub flows: Arc<Mutex<Vec<PaymentFlowRecord>>>,  // payment flow ring buffer (max 100)
    pub coralos_url: Arc<Mutex<String>>,     // base URL of a remote CoralOS server
    pub coralos_token: Arc<Mutex<String>>,   // Bearer token for that server
    pub mcp_sessions: Arc<Mutex<HashMap<String, bool>>>,  // agent_name → loop active?
}
```

`AppState` is `Clone` (all fields behind `Arc`), so Axum's `State` extractor clones it cheaply for each handler.

---

## CORS configuration

```
Allow-Origin: *
Allow-Methods: *
Allow-Headers: *
```

All cross-origin requests are accepted. In production deployments restrict this to your frontend's origin.

---

## REST API reference

### Health check

#### `GET /health`

**Response:**

```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

```sh
curl http://localhost:8080/health
```

---

### Agents

Base path: `/api/v1/agents`

#### `GET /api/v1/agents`

List all agents as `[id, AgentState]` pairs sorted by ID.

**Response:**

```json
[
  ["alpha", { "is_running": true, "actions": [], "rpc_endpoint": "https://api.devnet.solana.com", "network": "devnet", "strategy": "rpc-poll" }]
]
```

```sh
curl http://localhost:8080/api/v1/agents
```

---

#### `POST /api/v1/agents`

Create a generic agent with the default `RpcPollStrategy`.

**Request body:**

```json
{ "id": "alpha" }
```

**Response:** `AgentState` (201-ish) or `409 Conflict` if `id` is taken.

```sh
curl -X POST http://localhost:8080/api/v1/agents \
  -H "Content-Type: application/json" \
  -d '{"id": "alpha"}'
```

---

#### `GET /api/v1/agents/with-roles`

List all agents with their full metadata (role, created_at, tags).

**Response:**

```json
[["alpha", { "is_running": false, "actions": [], "rpc_endpoint": "...", "network": "devnet", "strategy": "rpc-poll" }, { "role": "Worker", "created_at": "2026-01-01T00:00:00Z", "tags": [] }]]
```

```sh
curl http://localhost:8080/api/v1/agents/with-roles
```

---

#### `POST /api/v1/agents/solana-pay`

Create a Solana Pay agent. `mode` must be `"Transfer"` or `"Payment"`.

**Request body:**

```json
{ "id": "pay-agent", "mode": "Transfer" }
```

**Response:** `AgentState` or `409 Conflict`.

```sh
curl -X POST http://localhost:8080/api/v1/agents/solana-pay \
  -H "Content-Type: application/json" \
  -d '{"id": "pay-agent", "mode": "Transfer"}'
```

---

#### `POST /api/v1/agents/helius-monitor`

Create a Triton payment-monitor agent using Helius devnet URLs.

**Request body:**

```json
{
  "id": "monitor-1",
  "recipient": "7xKF3rO1jW...",
  "amount_sol": 0.01,
  "api_key": "your-helius-api-key",
  "label": "DataFeed"
}
```

**Response:** `AgentState` or `409 Conflict`. The agent records an initial `url-generated` action containing the Solana Pay URI.

```sh
curl -X POST http://localhost:8080/api/v1/agents/helius-monitor \
  -H "Content-Type: application/json" \
  -d '{"id":"monitor-1","recipient":"7xKF...","amount_sol":0.01,"api_key":"","label":"Demo"}'
```

---

#### `GET /api/v1/agents/:id`

Get the state of a single agent.

**Response:** `AgentState` or `404 Not Found`.

```sh
curl http://localhost:8080/api/v1/agents/alpha
```

---

#### `DELETE /api/v1/agents/:id`

Delete an agent and its metadata.

**Response:** `204 No Content` or `404 Not Found`.

```sh
curl -X DELETE http://localhost:8080/api/v1/agents/alpha
```

---

#### `POST /api/v1/agents/:id/start`

Start the agent's strategy loop.

**Response:**

```json
true
```

```sh
curl -X POST http://localhost:8080/api/v1/agents/alpha/start
```

---

#### `POST /api/v1/agents/:id/stop`

Stop the agent's strategy loop.

**Response:** `true` or `404 Not Found`.

```sh
curl -X POST http://localhost:8080/api/v1/agents/alpha/stop
```

---

#### `GET /api/v1/agents/:id/actions`

Return the agent's full action log.

**Response:**

```json
[
  {
    "timestamp": "2026-01-01T00:00:01Z",
    "action_type": "rpc-poll",
    "details": "Polled slot 12345 via https://api.devnet.solana.com",
    "tx_signature": null,
    "slot": 12345,
    "latency_ms": 42
  }
]
```

```sh
curl http://localhost:8080/api/v1/agents/alpha/actions
```

---

#### `POST /api/v1/agents/:id/rpc`

Override the agent's RPC endpoint.

**Request body:**

```json
{ "url": "https://devnet.helius-rpc.com/?api-key=abc123" }
```

**Response:** `true` or `404 Not Found`.

```sh
curl -X POST http://localhost:8080/api/v1/agents/alpha/rpc \
  -H "Content-Type: application/json" \
  -d '{"url":"https://api.mainnet-beta.solana.com"}'
```

---

#### `POST /api/v1/agents/:id/triton`

Configure a Triton PAYG x-token. If `grpc_endpoint` is provided it is used directly; otherwise the default Triton mainnet endpoint is used.

**Request body:**

```json
{ "x_token": "your-triton-token", "grpc_endpoint": null }
```

**Response:** `true` or `404 Not Found`.

```sh
curl -X POST http://localhost:8080/api/v1/agents/alpha/triton \
  -H "Content-Type: application/json" \
  -d '{"x_token":"tkn123","grpc_endpoint":null}'
```

---

#### `POST /api/v1/agents/:id/role`

Set the agent's role. Accepted values (case-insensitive): `leader`, `coordinator`, `monitor`, `analyst`, `trader`, `worker`.

**Request body:**

```json
{ "role": "trader" }
```

**Response:** `true` or `404 Not Found`.

```sh
curl -X POST http://localhost:8080/api/v1/agents/alpha/role \
  -H "Content-Type: application/json" \
  -d '{"role":"leader"}'
```

---

#### `POST /api/v1/agents/:id/helius`

Configure a Helius devnet RPC endpoint. Falls back to the public devnet RPC if `api_key` is empty.

**Request body:**

```json
{ "api_key": "your-helius-key" }
```

**Response:** `true` or `404 Not Found`.

```sh
curl -X POST http://localhost:8080/api/v1/agents/alpha/helius \
  -H "Content-Type: application/json" \
  -d '{"api_key":"abc123"}'
```

---

### Workflows

Base path: `/api/v1/workflows`

#### `GET /api/v1/workflows`

List all registered workflows.

**Response:** `Workflow[]`

```sh
curl http://localhost:8080/api/v1/workflows
```

---

#### `POST /api/v1/workflows`

Create a workflow. `priority` is clamped to 1–10.

**Request body:**

```json
{
  "id": "wf-1",
  "name": "Data Pipeline",
  "description": "Fetch, analyze, report",
  "steps": [
    {
      "id": "fetch",
      "name": "Fetch Data",
      "description": "Pull from API",
      "status": "Pending",
      "assigned_to": null,
      "dependencies": [],
      "result": null,
      "started_at": null,
      "completed_at": null,
      "timeout_secs": null
    },
    {
      "id": "analyze",
      "name": "Analyze",
      "description": "Run model",
      "status": "Pending",
      "assigned_to": "analyst",
      "dependencies": ["fetch"],
      "result": null,
      "started_at": null,
      "completed_at": null,
      "timeout_secs": 60
    }
  ],
  "priority": 7,
  "created_by": "leader"
}
```

**Response:** The created `Workflow` object.

```sh
curl -X POST http://localhost:8080/api/v1/workflows \
  -H "Content-Type: application/json" \
  -d '{"id":"wf-1","name":"Pipeline","description":"","steps":[],"priority":5,"created_by":"leader"}'
```

---

#### `GET /api/v1/workflows/:id`

Get a single workflow.

**Response:** `Workflow` or `404 Not Found`.

```sh
curl http://localhost:8080/api/v1/workflows/wf-1
```

---

#### `DELETE /api/v1/workflows/:id`

Delete a workflow.

**Response:** `204 No Content` or `404 Not Found`.

```sh
curl -X DELETE http://localhost:8080/api/v1/workflows/wf-1
```

---

#### `POST /api/v1/workflows/:id/steps/:step_id/assign`

Assign a step to an agent (transitions step status to `Assigned`).

**Request body:**

```json
{ "agent_id": "analyst" }
```

**Response:** `true`

```sh
curl -X POST http://localhost:8080/api/v1/workflows/wf-1/steps/fetch/assign \
  -H "Content-Type: application/json" \
  -d '{"agent_id":"worker-1"}'
```

---

#### `POST /api/v1/workflows/:id/steps/:step_id/start`

Mark a step as `InProgress`.

**Request body:** none

**Response:** `true`

```sh
curl -X POST http://localhost:8080/api/v1/workflows/wf-1/steps/fetch/start
```

---

#### `POST /api/v1/workflows/:id/steps/:step_id/complete`

Mark a step as `Completed`. Automatically sets the workflow to `Completed` when all steps are done.

**Request body:**

```json
{ "result": "{\"rows\": 1200}" }
```

**Response:** `true`

```sh
curl -X POST http://localhost:8080/api/v1/workflows/wf-1/steps/fetch/complete \
  -H "Content-Type: application/json" \
  -d '{"result":"done"}'
```

---

#### `POST /api/v1/workflows/:id/steps/:step_id/fail`

Mark a step and the workflow as `Failed`.

**Request body:**

```json
{ "reason": "timeout after 60s" }
```

**Response:** `true`

```sh
curl -X POST http://localhost:8080/api/v1/workflows/wf-1/steps/fetch/fail \
  -H "Content-Type: application/json" \
  -d '{"reason":"network error"}'
```

---

### Messages

Base path: `/api/v1/messages`

#### `GET /api/v1/messages`

Return every message on the bus (admin/debug view, max 1,000).

**Response:** `AgentMessage[]`

```sh
curl http://localhost:8080/api/v1/messages
```

---

#### `POST /api/v1/messages`

Send a message. `to: null` broadcasts; `to: "agent-id"` sends direct.

**Request body:**

```json
{
  "from": "leader",
  "to": "worker-1",
  "msg_type": "task-assigned",
  "payload": "{\"task\":\"compute\"}"
}
```

**Response:** `true` or `422 Unprocessable Entity` if `from` is empty.

```sh
curl -X POST http://localhost:8080/api/v1/messages \
  -H "Content-Type: application/json" \
  -d '{"from":"leader","to":null,"msg_type":"announce","payload":"go"}'
```

---

#### `GET /api/v1/messages/:agent_id`

Return all messages visible to `agent_id` (broadcasts + messages sent/received by this agent).

**Response:** `AgentMessage[]`

```sh
curl http://localhost:8080/api/v1/messages/worker-1
```

---

#### `GET /api/v1/messages/conversation/:a/:b`

Return the direct-message thread between agents `a` and `b`.

**Response:** `AgentMessage[]`

```sh
curl http://localhost:8080/api/v1/messages/conversation/leader/worker-1
```

---

### Shared State

Base path: `/api/v1/state`

#### `GET /api/v1/state`

Snapshot the entire shared key-value store.

**Response:**

```json
{
  "market/AAPL": {
    "value": 189.42,
    "last_modified": "2026-01-01T00:00:01Z",
    "modified_by": "analyst",
    "version": 3
  }
}
```

```sh
curl http://localhost:8080/api/v1/state
```

---

#### `GET /api/v1/state/history`

Return the bounded change-history log (max 500 entries, newest last).

**Response:** `StateChange[]`

```json
[
  {
    "key": "market/AAPL",
    "old_value": 189.42,
    "new_value": 190.00,
    "timestamp": "2026-01-01T00:00:02Z",
    "changed_by": "analyst"
  }
]
```

```sh
curl http://localhost:8080/api/v1/state/history
```

---

#### `GET /api/v1/state/:key`

Read a single entry.

**Response:** `SharedStateEntry | null`

```sh
curl http://localhost:8080/api/v1/state/market%2FAAPL
```

---

#### `POST /api/v1/state/:key`

Write a value. Returns `false` if `changed_by` lacks `can_modify_shared_state` permission for their role.

**Request body:**

```json
{ "value": 190.00, "changed_by": "analyst" }
```

**Response:** `true` or `false`, or `422 Unprocessable Entity` if `changed_by` is empty.

```sh
curl -X POST http://localhost:8080/api/v1/state/market%2FAAPL \
  -H "Content-Type: application/json" \
  -d '{"value":190.00,"changed_by":"analyst"}'
```

---

#### `DELETE /api/v1/state/:key`

Delete a key. Returns `false` if `changed_by` lacks permission.

**Request body:**

```json
{ "changed_by": "analyst" }
```

**Response:** `true` or `false`.

```sh
curl -X DELETE http://localhost:8080/api/v1/state/market%2FAAPL \
  -H "Content-Type: application/json" \
  -d '{"changed_by":"analyst"}'
```

---

### Solana Pay

Base path: `/api/v1/solana-pay`

#### `POST /api/v1/solana-pay/url`

Encode a Solana Pay transfer URL. `amount` is in SOL (float).

**Request body:**

```json
{
  "recipient": "7xKF3rO1jW...",
  "amount": 0.01,
  "label": "My Store",
  "message": "Order #42"
}
```

**Response:** The encoded `solana:` URI string.

```sh
curl -X POST http://localhost:8080/api/v1/solana-pay/url \
  -H "Content-Type: application/json" \
  -d '{"recipient":"7xKF3rO1jW...","amount":0.01,"label":"Demo","message":"Pay"}'
```

---

#### `POST /api/v1/solana-pay/parse`

Parse a `solana:` URI.

**Request body:**

```json
{ "url": "solana:7xKF3rO1jW...?amount=0.01&label=Demo" }
```

**Response:** `ParsedUrl` — a tagged enum:

```json
{ "Transfer": { "recipient": "7xKF3rO1jW...", "amount": 0.01, "spl_token": null, "reference": null, "label": "Demo", "message": null, "memo": null } }
```

or `422 Unprocessable Entity` if the URL is malformed.

```sh
curl -X POST http://localhost:8080/api/v1/solana-pay/parse \
  -H "Content-Type: application/json" \
  -d '{"url":"solana:7xKF3rO1jW...?amount=0.01"}'
```

---

#### `POST /api/v1/solana-pay/validate`

Validate an on-chain transfer by transaction signature. Uses the agent's RPC endpoint if `id` is provided; falls back to the provided `rpc_url` or `https://api.devnet.solana.com`.

**Request body:**

```json
{
  "id": "alpha",
  "signature": "5Q...",
  "expected_recipient": "7xKF3rO1jW...",
  "rpc_url": null
}
```

**Response:** `ValidationResult`

```json
{ "valid": true, "slot": 12345, "error": null }
```

```sh
curl -X POST http://localhost:8080/api/v1/solana-pay/validate \
  -H "Content-Type: application/json" \
  -d '{"id":"alpha","signature":"5Q...","expected_recipient":null,"rpc_url":null}'
```

---

#### `POST /api/v1/solana-pay/x402/parse`

Parse the headers of a 402 response into a `PaymentChallenge`.

**Request body:**

```json
{
  "headers": [
    ["www-authenticate", "Solana mpp=<base64-payload>"]
  ]
}
```

**Response:** `PaymentChallenge | null`

```json
{ "protocol": "mpp", "amount": 1000000, "recipient": "7xKF...", "token": "USDC", "payload": "<base64>" }
```

```sh
curl -X POST http://localhost:8080/api/v1/solana-pay/x402/parse \
  -H "Content-Type: application/json" \
  -d '{"headers":[["www-authenticate","Solana mpp=eyJyZXF1ZXN0Ijp7fX0="]]}'
```

---

#### `POST /api/v1/solana-pay/x402/demo`

Run a full demo payment flow against a sandbox endpoint (no real signing). Stores a flow record in the ring buffer.

**Request body:**

```json
{ "endpoint": "https://debugger.pay.sh/mpp/quote/AAPL", "budget": 1000000 }
```

**Response:** `DemoPaymentResult`

```json
{
  "success": true,
  "endpoint": "https://debugger.pay.sh/mpp/quote/AAPL",
  "challenge": { "protocol": "mpp", "amount": 1000, "recipient": "...", "token": "USDC", "payload": "..." },
  "payment_header": "Bearer demo-payment-mpp-to-...-amount-1000",
  "response_body": "{\"AAPL\":189.42}",
  "error": null
}
```

```sh
curl -X POST http://localhost:8080/api/v1/solana-pay/x402/demo \
  -H "Content-Type: application/json" \
  -d '{"endpoint":"https://debugger.pay.sh/mpp/quote/AAPL","budget":1000000}'
```

---

### Pay Demo

Base path: `/api/v1/pay-demo`

#### `GET /api/v1/pay-demo/flows`

Return all stored payment flow records (max 100).

**Response:** `PaymentFlowRecord[]`

```sh
curl http://localhost:8080/api/v1/pay-demo/flows
```

---

#### `POST /api/v1/pay-demo/complete-sale`

Complete a seller→buyer sale. Calls `pay.sh` via the demo payment flow, records actions on both agents, sends a `"data-delivered"` direct message from seller to buyer, and writes the result to shared state at `"sale/{seller_id}/result"`.

**Request body:**

```json
{
  "seller_id": "seller",
  "buyer_id": "buyer",
  "tx_signature": "5Q..."
}
```

**Response:** The data payload string returned from pay.sh (or a fallback JSON if unreachable).

```sh
curl -X POST http://localhost:8080/api/v1/pay-demo/complete-sale \
  -H "Content-Type: application/json" \
  -d '{"seller_id":"seller","buyer_id":"buyer","tx_signature":null}'
```

---

### CoralOS

Base path: `/api/v1/coralos`

#### `PUT /api/v1/coralos/config`

Update the CoralOS server URL and/or Bearer token. Both fields are optional.

**Request body:**

```json
{ "url": "https://my-coralos.example.com", "token": "Bearer abc123" }
```

**Response:** `true`

```sh
curl -X PUT http://localhost:8080/api/v1/coralos/config \
  -H "Content-Type: application/json" \
  -d '{"url":"https://my-coralos.example.com","token":""}'
```

---

#### `GET /api/v1/coralos/sessions/:ns`

List CoralOS sessions for namespace `ns`, proxied to the configured CoralOS URL. Returns `[]` if no CoralOS URL is set.

**Response:** `CoralSessionExtended[]`

```json
[
  {
    "id": "sess-abc",
    "namespace": "hackathon",
    "status": "active",
    "agentCount": 2,
    "paymentSessionId": null,
    "agents": []
  }
]
```

```sh
curl http://localhost:8080/api/v1/coralos/sessions/hackathon
```

---

#### `POST /api/v1/coralos/mcp/join`

Connect to a CoralOS MCP endpoint and spawn a background agent loop. The loop records each incoming mention as a `"coral-mention"` action on the named agent and replies with an acknowledgement.

**Request body:**

```json
{ "connection_url": "http://localhost:5555/mcp", "agent_name": "rust-agent" }
```

**Response:** `true` or `502 Bad Gateway` if the MCP connection fails.

```sh
curl -X POST http://localhost:8080/api/v1/coralos/mcp/join \
  -H "Content-Type: application/json" \
  -d '{"connection_url":"http://localhost:5555/mcp","agent_name":"rust-agent"}'
```

---

#### `GET /api/v1/coralos/mcp/status/:name`

Check whether the named agent has an active MCP loop.

**Response:** `true` or `false`

```sh
curl http://localhost:8080/api/v1/coralos/mcp/status/rust-agent
```
