
# Workshop deck — Solana × CoralOS Agent Economy

> Condensed, **verified against the current code** (commit on `main`). Honest scope is called out so the
> demo holds up to questions. Two deployed devnet programs: escrow `R5NWNg9…CeXet`, arbiter `FJtuVXsy…ktXd`.

---

## 1 — Title

**Build an agent for the AI economy — it settles in SOL, coordinated by CoralOS.**

- Repo: `github.com/trilltino/solana_coralOS`
- Bounty: Imperial AI Agent Hackathon (Superteam Earn)

---

## 2 — What's actually in the repo

- **`packages/agent-runtime/`** — the SDK every agent imports. Four modules:
  `llm/` (provider-agnostic `complete()` over `fetch`, Anthropic default / OpenAI, no SDK) ·
  `solana/` (devnet guard + Solana Pay helpers) · `coral/` (CoralOS **MCP** client) ·
  `market/` (WANT/BID/AWARD protocol). *The web oracle uses `llm/` + `solana/`; `coral/` + `market/`
  power the multi-agent **CoralOS round** (`npm run coral`).*
- **`examples/txodds/`** — the **World Cup Oracle**: `agent/` (`edge.ts` = verified odds → fair odds + a
  read; `escrow.ts` + `arbiter.ts` = on-chain clients), `server/proxy.ts` (live data + settlement),
  `web/` (no-build React board), `coral/` (the **CoralOS round** launcher),
  **`escrow/` = the only Rust — TWO deployed Anchor programs: escrow + arbiter**.
- **`coral-agents/`** — the two agents coral-server launches for the round: `buyer-agent`, `seller-agent`
  (+ the `seller-worldcup` persona). **`docker-compose.yml`** runs coral-server.
- **`scripts/`** — `setup.js` (generates **three** devnet wallets: buyer, seller, arbiter), `txodds.js` (`npm run dev`).
- The **web demo needs no Docker**; the **CoralOS round needs Docker** (coral-server). CI is `.github/workflows/ci.yml`.

---

## 3 — Solana settlement: the reference binds the deal

- Every order gets a **reference** — a 32-byte key that seeds the escrow PDA: `seeds = [b"escrow", buyer, reference]`,
  so each order is its own on-chain account.
- **In this demo the reference is bound to the data:**
  `reference = sha256("txodds:<fixtureId>:<favourite>@<fairOdds>:<nonce>")`.
  So the on-chain escrow PDA **provably is the read you bought** — anyone with the preimage can verify it.
- The **Solana Pay** helpers (`generatePaymentUrl` / `verifyPayment` in `runtime/src/solana/pay.ts`)
  implement the canonical "reference as a read-only account" pattern and ship in the SDK for the
  pay-per-call path; the oracle settles through the **escrow + arbiter** directly.
- Code: `server/proxy.ts → boundReference()` · `agent/escrow.ts → escrowPda()` · `runtime/src/solana/pay.ts`.

---

## 3.5 — The contracts (the only Rust)

- **escrow** `R5NWNg9…CeXet` — the spine: `initialize(amount, reference, deadline)` → buyer deposits
  into a per-order PDA; `release()` pays the seller on delivery; `refund()` after a deadline.
  Security-checklist clean (`init` not `init_if_needed`, `has_one` both parties, `close = buyer`, checked
  math). **But buyer-released — it protects the buyer, not the seller.**
- **arbiter** `FJtuVXsy…ktXd` — the trustless fix, **deployed**: the buyer funds a **vault PDA** that
  becomes the escrow's buyer (can't claw back); a **neutral arbiter** is the only party that can
  `arbitrate_release` (pay the seller on verified delivery) or `arbitrate_refund`. A CPI wrapper over the
  live escrow — the escrow is unchanged.
- *Honest caveat: trustless **between buyer and seller**; the arbiter is still a trusted 3rd party (a
  production system stakes/decentralises it).*

---

## 4 — CoralOS (MCP) — and it actually runs

- CoralOS = agent coordination over Anthropic's **Model Context Protocol**. The runtime ships the client
  (`coral/mcp.ts`: connect, list tools, `waitForMention` / `waitForAgent`) + the market wire protocol
  (`market/protocol.ts`): `WANT → BID → AWARD → ESCROW_REQUIRED → DEPOSITED → DELIVERED → RELEASED`.
- **Two views of the same product:**
  - **Web oracle** (`npm run dev`) — the **single-agent** product view (proxy → read → arbiter settle).
  - **CoralOS round** (`docker compose up -d coral` → `npm run coral`) — the **multi-agent** version: a
    **buyer agent + a World Cup seller agent** trade the txodds edge **over coral-server (MCP)** and
    settle through the Solana escrow on devnet. **Verified live: a full `WANT → … → RELEASED` round with
    a real devnet release tx.** coral-server launches the agents as containers; they coordinate over a
    shared thread — no direct call between them.
- *Honest scope:* the round is one buyer + one seller (add personas to show competition — the specialist
  wins); it settles via the base escrow (the web view adds the arbiter + the order-bound reference).
- *MVP → at scale:* a couple of containers → a mesh of agents on a clustered coral-server · static
  graph → dynamic MCP registry/discovery · SOL → USDC + oracle-verified delivery.

---

## 5 — The problem: fair exchange (untrusted commerce)

- **The bind:** buyer pays first → seller may not deliver; seller delivers first → buyer may not pay.
  And raw odds are hard to act on.
- **LLM = the value:** verified de-margined odds → fair (break-even) odds + a one-line read.
- **Escrow + arbiter = trustless settlement:** funds locked in a per-order PDA; the buyer **can't claw
  back** and the seller is **only paid on delivery** — the neutral arbiter gates it.
- **The reference = the binding:** `sha256` ties the payment to that exact read.
- **Result:** verified analysis, paid out only on delivery, settled automatically on devnet.

---

## 5.5 — Demo (live): the World Cup Oracle

Pick a fixture → verified de-margined board with **fair odds** per outcome → the agent's **read** (LLM,
deterministic fallback) → **auto-settle through the arbiter** (buyer → vault → arbiter releases to the
seller), Explorer links, reference bound to the read.

---

## 6 — How I built it

Forked the reference repo and asked an LLM: *"How can I use agents to find and purchase odds data?"* →
pointed it at the TxODDS free World Cup tier → wrote **one transform** (`analyzeEdge`) + the proxy. The
rails (runtime, escrow) were already there; the arbiter + the bound reference were the upgrades.

---

## 7 — The surface you change (8 layers)

| # | Layer                 | In this demo                                                            |
| - | --------------------- | ----------------------------------------------------------------------- |
| 1 | Frontend              | the React board (`txodds/web`) — odds, the read, settlement links    |
| 2 | **The service** | `analyzeEdge` / `deliverService` — **the one real fork**     |
| 3 | Seller persona        | config (this demo is one specialist)                                    |
| 4 | The buyer             | what it wants + how it judges —*scaffolding (`market/`)*           |
| 5 | Solana Pay + escrow   | the reference binds the deal; pays on release —**+ the arbiter** |
| 6 | New agents            | **the arbiter is now shipped**; add a reseller / verifier         |
| 7 | The runtime           | import it, write behavior                                               |
| 8 | The contract          | the only Rust (escrow + arbiter) — the settlement spine                |

---

## 8 — Your turn + submit

- Ask an LLM: *"How can I use agents on Solana + CoralOS to do **X** — and which of the 8 layers do I change?"*
- Submit on **Superteam Earn**: 3-min demo video · pitch deck · public GitHub repo.

