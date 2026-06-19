# Making the Helius Agent a Real Coral Agent — Migration Plan

This is the concrete path from "a Rust `Strategy` that watches a wallet" to
"a first-class agent running inside a real Coral session, talking to other
agents over threads." It says exactly what to **keep**, what to **delete**, and
what to **build**, with the real Coral protocol calls.

> Read [`CORALOS.md`](./CORALOS.md) first for the layer model. The one-line
> premise here: **the real CoralOS is an MCP orchestration server (Kotlin,
> SSE/streamable-HTTP); the `coral-server/` in this repo is a REST stand-in.**
> The path to "real Coral" goes through the real server, not the stand-in.

---

## 0. What "your Helius agent" actually is

There is **no `helius.rs`** in the tree (the CLAUDE.md references one that does
not exist). The thing you call the Helius agent is the wallet-monitor strategy:

- [`solana_pay/monitor.rs`](../agent_demo/agent-core/src/solana_pay/monitor.rs)
  → `TritonPaymentMonitorStrategy`.

Its real behaviour (lines 58–140):

1. Snapshot the recipient's balance (`get_balance`) as a baseline.
2. Open a **Solana PubSub websocket** (`account_subscribe`) — this works against
   any Solana RPC/WS provider, **including Helius** (point `ws_url` at your
   Helius websocket).
3. On every account update, diff the lamports; when the delta ≥ the expected
   amount, emit a `payment-received` action (with tx signature + slot),
   otherwise `partial-payment`.
4. Reconnect with exponential backoff on stream errors.

This is a clean, event-driven source. **The Solana logic is 100% reusable.**
What is *not* reusable is how it reports: it pushes `AgentAction`s into a local
`Arc<Mutex<AgentState>>` (in-process), instead of speaking to an orchestrator.

---

## 1. The gap, precisely

A Coral agent is defined by four obligations. Here is where your agent stands on
each:

| Obligation | Real Coral agent | `TritonPaymentMonitorStrategy` today | Reusable? |
|---|---|---|---|
| **Transport** | MCP over SSE / streamable HTTP to Coral Server | none — runs in-process | ❌ build |
| **Identity** | `register` itself in a session | none | ❌ build |
| **Input** | `wait_for_mentions` → act on request | autonomous `loop` on `is_running` | ⚠️ rewire |
| **Output** | `send_message` into a **thread** | `state.actions.push(AgentAction{…})` | ⚠️ rewire |
| **Lifecycle** | launched by a **session** in a **runtime** (Docker/exe) | spawned by `AgentManager::start_agent` | ❌ replace |
| **Domain work** | watch wallet, detect payment | `run_stream()` PubSub diff loop | ✅ keep as-is |

So the migration is: **keep `run_stream`'s guts, replace everything around it.**

---

## 2. Two paths (and why we pick one)

### Path A — Coralize the agent against the *real* Coral Server ✅ recommended

Run the genuine Coral Server (Docker), and wrap the monitor logic as an MCP
agent that registers, waits for mentions, monitors, and reports into a thread.
The repo's Rust `coral-server` is retired (or demoted to "local tab only").

- **Effort:** ~a few hundred lines of glue + a Dockerfile + an agent config.
- **Risk:** low — you inherit Coral's sessions/threads/runtimes/payments.
- **Throwaway:** the REST `coral-server` and the `CoralOSClient` session shim.

### Path B — Turn the Rust `coral-server` into a real Coral-compatible server ❌

Implement MCP-over-SSE, sessions, threads, and all six tools in Rust.

- **Effort:** weeks. You are **reimplementing Coral** and chasing protocol parity
  forever.
- **Only worth it if** "a Rust-native Coral server" is itself the product goal.

**Decision: Path A.** The rest of this doc details it.

---

## 3. Target architecture (Path A)

```
┌─────────────────────────────────────────────────────────────────┐
│  Real Coral Server  (Kotlin / Ktor, MCP host)                    │
│    • Session: "payments-demo"   (namespace-scoped)               │
│    • Thread:  "settlement"                                        │
│    • Tools:   register · list_agents · create_thread ·           │
│               add_participant · send_message · wait_for_mentions  │
└───────────▲───────────────────────────────────────▲──────────────┘
            │ MCP / SSE                              │ MCP / SSE
   ┌────────┴─────────┐                     ┌────────┴───────────┐
   │ helius-monitor   │                     │ any other Coral    │
   │ agent (NEW)      │                     │ agent (planner,    │
   │  • register      │                     │  trader, …)        │
   │  • wait_for_     │  "watch WALLET for  │                    │
   │    mentions ─────┼──  0.5 SOL" ───────▶│                    │
   │  • run_stream()  │                     │                    │
   │    (REUSED)      │  "payment-received  │                    │
   │  • send_message ─┼──  sig=… slot=…" ──▶│                    │
   └──────────────────┘                     └────────────────────┘
        runtime: Docker container / native executable
```

The agent process is launched **by the session**, dials back to the server over
SSE, and from then on is just a request/response participant on a thread.

---

## 4. The new agent: structure

Coral's first-party agent examples are Python (MCP SDK + `langchain-mcp-adapters`),
but a Rust agent works too via an MCP client crate. Either way the shape is the
same control loop:

```
connect(SSE) ──▶ register(agentId="helius-monitor", description=…)
      │
      ▼
loop {
    req = wait_for_mentions(timeoutMs = 30_000)      // block for work
    if req is "watch <WALLET> for <AMOUNT> SOL" {
        // ── REUSED CORE ──────────────────────────────────────────
        result = run_stream(wallet, amount, helius_ws_url)   // PubSub diff
        // ─────────────────────────────────────────────────────────
        send_message(
            threadId = req.threadId,
            mentions = [req.sender],
            content  = "payment-received amount=… sig=… slot=…"
        )
    }
}
```

### The reuse seam

Refactor `run_stream` so it does not depend on `Arc<Mutex<AgentState>>`. Today
it both *detects* the payment and *records* it into agent state. Split those:

```rust
// agent-core: pure detection, framework-agnostic.
// Returns the first qualifying payment instead of pushing into AgentState.
pub async fn watch_for_payment(
    rpc_url: &str,
    ws_url: &str,          // ← your Helius websocket
    recipient: &str,
    expected_lamports: u64,
) -> anyhow::Result<PaymentEvent>;   // { amount_lamports, signature, slot }
```

`TritonPaymentMonitorStrategy::run_stream` (monitor.rs:58) becomes a thin caller
of `watch_for_payment` that adapts the result into `AgentAction` — so the **local
Tauri tab keeps working unchanged**, and the new Coral agent calls
`watch_for_payment` directly and adapts the result into `send_message`. One
detection function, two front-ends.

---

## 5. Coral tool calls you will make

These are the real MCP tools the agent invokes against the session (names from
Coral Server). Payloads are illustrative:

| Step | Tool | Purpose |
|---|---|---|
| startup | `register` | announce `helius-monitor` + its capability description |
| optional | `list_agents` | discover who else is in the session |
| optional | `create_thread` / `add_participant` | open a "settlement" thread, add the requester |
| block | `wait_for_mentions` | park until another agent asks for a watch |
| reply | `send_message` | post `payment-received` (+ sig/slot) mentioning the requester |

The agent is **reactive**: it does nothing until mentioned, then runs the Helius
subscription, then answers. That maps cleanly onto the existing event loop —
you are swapping the `is_running` poll for `wait_for_mentions`, and the
`actions.push` for `send_message`.

---

## 6. Packaging it as a session-launchable runtime

Coral launches agents from a registry/config entry that points at a runtime.
Minimum deliverables:

1. **Agent config** (Coral's `coral-agent.toml`-style descriptor): id,
   description, the option schema (`wallet`, `amount`, `heliusWsUrl`,
   `heliusApiKey`), and the runtime.
2. **Runtime** — pick one:
   - **Docker** (most portable): a `Dockerfile` that builds the agent binary and
     runs it; the server injects the MCP connection URL + options as env vars.
   - **Executable**: a built binary Coral spawns directly.
3. **Secrets**: `HELIUS_API_KEY` supplied as a session option / env var, never
   committed.

Then a session spec wires it in:

```
session "payments-demo"
  agent helius-monitor { wallet = "…", amount = 0.5, heliusWsUrl = "wss://…helius…" }
  agent planner        { … }
  # Coral boots both runtimes, connects them over SSE, they coordinate on threads
```

---

## 7. Staged execution plan

| Stage | Deliverable | Done when |
|---|---|---|
| **S1** | Run real Coral Server locally (Docker), hit `list_agents` with a hello-world agent | a dummy agent registers & shows in a session |
| **S2** | Extract `watch_for_payment` from `run_stream`; keep `TritonPaymentMonitorStrategy` green | `cargo test -p agent-core` passes; local Tauri tab still detects payments |
| **S3** | Build `helius-monitor` MCP agent: register → `wait_for_mentions` → `watch_for_payment` → `send_message` | mention it manually, it reports a real devnet payment back on the thread |
| **S4** | Dockerize + agent config; launch it from a session spec (not by hand) | the **session** boots it; no manual `cargo run` |
| **S5** | Point the Tauri "CoralOS" tab at the real server; retire/relabel the Rust `coral-server` | the tab lists the real session's agents over the real protocol |
| **S6** (optional) | Wire Coral **payments** so the data is gated behind on-chain settlement | a paying agent receives data only after `payment-received` |

S2 is the only change inside this repo's Rust; S1/S3/S4 are new, mostly-glue
code; S5 is config; S6 is the actual product story (pay-to-receive-data, which
your `solana_pay_url` builder already hints at).

---

## 8. What gets kept vs. deleted

**Keep (reused, valuable):**
- `run_stream` / `watch_for_payment` — the Helius PubSub detection loop.
- The Solana Pay URL builder (`encode_transfer_url`) — useful for S6 payments.
- `agent-core` types — still the local Tauri backend.

**Delete or demote (after S5):**
- [`coral-server/`](../coral-server) REST API — replaced by the real Coral Server.
- The session/namespace shim in
  [`coralos.rs`](../agent_demo/src-tauri/src/coralos.rs) (`list_sessions` /
  `get_session` faking sessions from a flat agent list) — the real server
  returns real sessions.
- The unenforced bearer-token handling — the real server does auth (note the
  published *session-creation auth-bypass* advisory; use a current version).

---

## 9. Honest risk notes

- **Language**: Coral's reference agents are Python. A Rust MCP agent is viable
  but you'll be on a less-trodden path for the SSE/MCP client. If speed matters,
  a thin Python agent that shells/binds to the Rust detection (or just calls
  Helius directly) may reach S3 faster than a pure-Rust agent.
- **Helius specifics**: the current code uses standard `account_subscribe`. That
  already works with Helius. If you want Helius *enhanced* webhooks/transactions
  instead of raw PubSub, that's an additional adapter behind the same
  `watch_for_payment` seam — not a rearchitecture.
- **It is a real path, not a toggle.** S1–S5 is days of work, not hours. The
  reason it's *achievable* is that your detection logic is already isolated and
  event-shaped; you are wrapping it, not rewriting it.

---

## 10. One-paragraph answer

Yes — there's a clean path to a fully working Coral integration with the Helius
monitor. You don't extend the repo's Rust `coral-server`; you run the **real**
Coral Server and turn the monitor into a Coral agent: extract its PubSub
detection into a framework-agnostic `watch_for_payment`, wrap it in an MCP loop
(`register` → `wait_for_mentions` → detect → `send_message`), package it as a
Docker/executable runtime with a Coral agent config, and launch it from a
session. The Solana/Helius logic survives intact; only the transport and
lifecycle around it change.
