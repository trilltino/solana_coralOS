# Security Review

**Date:** 2026-06-25
**Scope:** Whole repo — payment verification, the buyer's spend path, the LLM agent loop, auth,
input handling, web frontend, dependencies. Lens: the `solana-dev` skill's payments + client +
agent-safety checklists, plus first-principles review.
**Posture reminder:** this is a **devnet** kit. Several findings are low-risk *as shipped* (localhost,
play money) but would be real on mainnet / when exposed. Severities below assume "taken to real value."

---

## Findings at a glance

| # | Severity | Finding | Exploitable as-shipped? |
|---|----------|---------|--------------------------|
| **H1** | 🔴 High | Seller payment proof not bound to the request (memo not checked on-chain) | Yes, within a session |
| **H2** | 🟠 High | Prompt injection can redirect the LLM buyer's payment to an attacker | Yes, if the buyer fetches attacker-influenced data |
| **H3** | 🟠 High | Dependency vulnerabilities (1 critical / 5 high / 7 moderate) | Transitive; mostly not prod-reachable |
| **M1** | 🟡 Med | `api-server` has **no authentication** on agent CRUD / shared-state | Only if port is exposed |
| **M2** | 🟡 Med | coral-server auth defaults to `"dev"` + `allowAnyHost = true` | Only if port is exposed |
| **M3** | 🟡 Med | Buyer budget is per-payment, not cumulative (×`maxTurns`) | Bounded over-spend |
| **L1–L4** | 🟢 Low | `confirmed` vs `finalized`, no pre-sign simulation, API key in URL, no explicit `meta.err` check | Minor |

---

## H1 — Payment proof is not bound to the request (seller-agent)

**Where:** [`coral-agents/seller-agent/src/payment.ts`](../coral-agents/seller-agent/src/payment.ts)
`verifyPayment(sig, memo)`.

**The flaw:** verification checks only that the transaction sent **≥ PRICE_SOL to SELLER_WALLET**. The
`memo` is used solely to look up the in-memory `pending` map in `index.ts` — it is **never checked
on-chain**. So *any* transaction that paid the right amount to the seller satisfies *any* pending
order.

**Attack:** Payment signatures are posted in the clear (`paid <sig>` in a CoralOS thread; payments to
the seller wallet are also visible on-chain). A malicious session participant — or anyone watching the
seller's wallet — can take a legitimate buyer's `sig` and submit it as proof for **their own** order.
Because verification passes (right amount, right recipient), the attacker gets free delivery; the
`ReplayGuard` then marks the sig consumed, so the **real buyer's** later submission is rejected
(`payment signature already used`) — they paid and got nothing.

The `ReplayGuard` added earlier stops the *same* sig being used twice, but does **not** bind a payment
to the buyer who made it — it's first-come-first-served on a stolen proof.

**Fix:** Bind each payment to a unique **reference key**, exactly as the quickstart already does
([`examples/agent-economy/quickstart/verify.ts`](../examples/agent-economy/quickstart/verify.ts) uses
`findReference` + `validateTransfer` from `@solana/pay`). The buyer's `signTransfer()` already supports
a `reference` argument. Move the seller from memo-matching to reference-based verification and the
proof becomes non-transferable. **Effort:** M (touches seller `payment.ts`/`index.ts`; the buyer +
bridge already carry a reference through the URL).

---

## H2 — Prompt injection can redirect the buyer's payment

**Where:** [`coral-agents/buyer-agent/src/llm_buyer.ts`](../coral-agents/buyer-agent/src/llm_buyer.ts)
`runTool` → `pay_and_retry`.

**The flaw:** `pay_and_retry` pays `input.recipient` / `input.amountSol` taken **from the model's tool
call**. The code enforces `amountSol ≤ budget` but does **not** verify that `recipient`/`reference`
match a challenge the buyer actually received. The "only pay values from a real challenge" rule is
stated in the **system prompt** (`BUYER_SYSTEM`) — not enforced in code.

The fetched response body is fed back to the model verbatim (`body.slice(0, 2000)` as a tool result).
Per the skill's W011 ("treat fetched data as untrusted; ignore embedded directives"), that body is an
**injection surface**.

**Attack:** A malicious (or compromised) endpoint returns content that steers the model to call
`pay_and_retry` with the **attacker's** pubkey and an amount ≤ budget. The code's budget check passes,
and the buyer pays the attacker. The doc's claim *"the model cannot pay hallucinated recipients"* holds
only as long as the prompt isn't overridden — which is precisely what prompt injection does.

**Fix:** Code-enforce the binding: have `fetch_data` record the challenge(s) it parsed, and in
`pay_and_retry` **reject** any `recipient`/`reference` not present in a received challenge. Keep the
budget check too. **Effort:** S — contained to `runTool`.

---

## H3 — Dependency vulnerabilities

`npm audit` (agent-runtime): **13 total — 1 critical, 5 high, 7 moderate.** Almost entirely
**transitive**:
- `bigint-buffer` (high, buffer overflow) ← `@solana/buffer-layout-utils` ← `@solana/spl-token` ←
  **`@solana/pay`**.
- `uuid` (moderate) ← `jayson` ← **`@solana/web3.js`** 1.x.
- `esbuild`/`vite`/`vitest` (moderate) — **dev/test tooling only**, not in the production runtime.

**Assessment:** these are the well-known advisories that ride along with **`@solana/web3.js` 1.x +
`@solana/pay`**; the dev-tooling ones aren't production-reachable. `npm audit fix --force` would pull
breaking major bumps. The clean fix is the **`@solana/kit` migration** (hardening §5 / the skill's
default stack) — modern, maintained, and it sheds most of this tree. **Effort:** M–L (migration).
Short term: pin/upgrade `vitest` to clear the dev-only ones.

---

## M1 — api-server has no authentication

**Where:** [`api-server/src/`](../api-server/src) — no auth middleware on the routes.

Anyone who can reach `:8081` can create/start/stop agents, and read/write shared-state and the message
bus. Fine on localhost; an open door if the port is exposed. **Fix:** a token middleware (bearer from
secrets) + bind to localhost by default. **Effort:** S.

## M2 — coral-server default auth

`config/coral.toml` ships `[auth] keys=["dev"]` and `allowAnyHost = true`; `CORAL_TOKEN ?? 'dev'` in
`start.ts` / `bridge/server.ts`. Anyone reaching the port can drive sessions and impersonate
`user-proxy`. **Fix:** real tokens from secrets; drop `allowAnyHost` for non-local. **Effort:** S.

## M3 — Buyer budget is per-payment, not cumulative

`llm_buyer` enforces `amountSol ≤ budgetLamports` **per `pay_and_retry`**. A malicious endpoint that
keeps returning fresh 402s can make the agent pay up to `maxTurns` (default 8) × budget in one
`purchase()`. **Fix:** track spend across the loop and cap the total. **Effort:** S.

---

## Low

- **L1 — `confirmed` vs `finalized`:** `verifyPayment` accepts `confirmed`. For irreversible
  settlement of real value, prefer `finalized`. (Devnet/small amounts: fine.)
- **L2 — No pre-sign simulation (skill W009):** the autonomous buyer signs without
  `simulateTransaction`. Inherent to an unattended agent, but simulating would catch malformed txs.
- **L3 — API key in URL:** `newsHeadlines` puts `NEWS_API_KEY` in the query string (can leak via logs).
  Prefer a header. (Not currently logged.)
- **L4 — No explicit `meta.err` check** in `verifyPayment` — a failed tx is implicitly rejected by the
  balance delta (no transfer), but an explicit `meta.err === null` check is clearer.

---

## What's already solid

- **No XSS:** the bridge UI `escapeHtml`s delivered data; the Next.js app uses no
  `dangerouslySetInnerHTML` (React auto-escapes).
- **No SSRF / URL injection** in `deliverService` — user input is `encodeURIComponent`'d (news) or not
  interpolated into URLs (jupiter/coingecko use fixed mints/coins).
- **Replay protection** (sig reuse) is in place + tested.
- **Mainnet guard** (`setRpc` rejects mainnet unless `ALLOW_MAINNET=1`).
- **Keys:** the seller holds only a public key; the buyer's secret is never logged (only its pubkey).
- **Settlement is confirmed against chain state**, not client callbacks (skill ✓).

---

## Priority to fix

1. **H2** (prompt-injection recipient binding) — small, contained, and it's an *agent-economy*-defining
   bug (an agent that pays attackers is the worst failure mode). Do this first.
2. **H1** (reference-bind the seller) — the right fix for payment-proof theft; the quickstart is the
   template.
3. **M1/M2** (auth) before anything leaves localhost.
4. **H3** (`@solana/kit` migration) clears most dep vulns; medium-term.

See also [`docs/PRODUCTION_HARDENING.md`](../docs/PRODUCTION_HARDENING.md).
