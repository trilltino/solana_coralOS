# seller-agent

The **fulfillment agent** — an LLM-driven seller that competes for a buyer's business in the open
marketplace and gets paid **through the escrow contract** on delivery. coral-server launches it as a
container; the three personas (`seller-cheap` / `seller-premium` / `seller-lazy`) reuse this same
image with different `PERSONA` / `FLOOR_SOL` / `SERVICES`.

## Market protocol (over a shared CoralOS thread)

```
WANT round=… service=… arg=… budget=…              → decide whether/at-what-price to bid (LLM, guarded)
  → BID round=… price=… by=<me> [note=…]            (or stay silent — self-selection)
AWARD round=… to=<me>                               → mint a reference → ESCROW_REQUIRED …
DEPOSITED round=… reference=… buyer=… sig=…         → verify the escrow is funded on-chain (isFunded)
  → deliverService() → DELIVERED round=… <data>
```

The legacy 1:1 direct-pay protocol is still handled for the HTTP/402 on-ramp:

```
request <query>           → PAYMENT_REQUIRED reference=<R> amount=<sol> url=solana:…
paid <sig> reference=<R>   → (verify on-chain) → DELIVERED <data>   |   ERROR …
```

## How it's secured

- **Code-enforced bidding** (`bidder.ts`): the LLM *proposes* a bid; the code *enforces* the
  economics — never bid on a service it doesn't carry, never below its cost floor, never above the
  buyer's budget. A prompt injection inside a `WANT` can't make it bid at a loss.
- **Deliver only against funded escrow** (`escrow.ts` `isFunded`): the seller delivers only after
  confirming on-chain that the escrow PDA for `(buyer, reference)` names it and holds the amount.
- **Reference-bound payments** (`payment.ts`, legacy path): `generatePaymentUrl` mints a unique
  single-use reference; `verifyPayment` uses Solana Pay's `validateTransfer` to confirm the right
  amount reached the right wallet **carrying that reference** — a proof can't be stolen or reused.
- **Replay guard** (`replay.ts`): consumed signatures are rejected.

## The fork point

```ts
// src/service.ts
export async function deliverService(request: string) { /* ← what you sell */ }
```
Built-ins via the `SERVICE` env: `jupiter` (default) · `coingecko` · `news` · `inference` (a Claude completion).

## Files

| File             | Role                                                                       |
| ---------------- | -------------------------------------------------------------------------- |
| `src/index.ts`   | the agent loop — market protocol + legacy 1:1 routing, verify, deliver     |
| `src/bidder.ts`  | `decideBid` / `sellerConfigFromEnv` — LLM bid, code-enforced floor/budget  |
| `src/escrow.ts`  | seller-side escrow client — read-only `isFunded` check before delivery     |
| `src/payment.ts` | `generatePaymentUrl` (reference) + `verifyPayment` (`validateTransfer`)    |
| `src/replay.ts`  | `ReplayGuard` — rejects reused payment signatures                          |
| `src/service.ts` | `deliverService` — **the fork point**                                      |

## Env

`SELLER_WALLET` (recipient pubkey, required) · `AGENT_NAME` (market identity) ·
`SERVICES` / `FLOOR_SOL` / `PERSONA` (bidding) · `SERVICE` (what `deliverService` returns) ·
`ESCROW_DEADLINE_SECS` · `SOLANA_RPC_URL` · `ANTHROPIC_API_KEY` | `OPENAI_API_KEY` (+ `LLM_PROVIDER`) ·
per-service keys (`JUPITER_API_KEY`, `NEWS_API_KEY`). Devnet only.

## Test

```sh
npm install && npm run typecheck && npm test   # bidder + replay + payment + service (17 cases)
```

The `isFunded` read hits the escrow program deployed to devnet; it needs live RPC, so it runs in a
live market session rather than in `npm test`.

Built into a Docker image by `bash build-agents.sh seller`; launched by coral-server per session.
