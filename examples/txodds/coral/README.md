# CoralOS round — the multi-agent story

The web oracle (`npm run dev`) is the **single-agent product view**: one proxy fetches verified odds,
produces the read, and settles. This folder is the **multi-agent** version: two agents trade the same
TxODDS edge **over CoralOS (MCP)** and settle through the Solana escrow on devnet — coordinated by
coral-server, with no direct call between them.

```
buyer-agent                                            seller-worldcup
   │ ── WANT  service=txline arg="edge <fixtureId>" ──────> │ (bids only on txline)
   │ <─ BID   price=0.0005 ──────────────────────────────── │
   │ ── AWARD ────────────────────────────────────────────> │
   │ <─ ESCROW_REQUIRED reference=… amount=0.0005 ────────── │
   │ ── DEPOSITED  (escrow PDA funded on devnet) ──────────> │ (verifies on-chain)
   │ <─ DELIVERED  {teams, odds, the LLM call} ──────────── │  (fetches TxLINE odds → LLM)
   │ ── RELEASED   (escrow pays the seller) ──────────────> │
```

Verified on devnet — a real `RELEASED` tx is printed by the buyer (e.g. `explorer.solana.com/tx/…`).

## Run it

Needs **Docker** (for coral-server) and the repo `.env` with `BUYER_KEYPAIR_B58` (funded), `WALLET`
(seller payout), `ANTHROPIC_API_KEY`, and `TXLINE_API_KEY` (mint one: `npm run mint`).

```sh
docker compose up -d coral        # start coral-server (the MCP coordinator) — from the repo root
cd examples/txodds && npm run coral
```

`round.ts` creates a CoralOS session graph (`POST /api/v1/local/session`); coral-server launches the
buyer + seller as containers, injects each one's `CORAL_CONNECTION_URL`, and they run the round above.
It uses a live fixture id from the running proxy (`/api/board`) so the seller has real odds to deliver.

### Watch it
coral names the agent containers by **UUID**, so find + tail them:
```sh
docker logs -f $(docker ps -qf ancestor=buyer-agent:0.1.0  | head -1)   # WANT → AWARD → DEPOSITED → RELEASED
docker logs -f $(docker ps -qf ancestor=seller-agent:0.1.0 | head -1)   # BID → ESCROW_REQUIRED → DELIVERED
```
Set `TRACE=1` in `.env` for the `coral_*` MCP calls + Explorer links. Clean up spawned agents with
`docker ps -q --filter ancestor=buyer-agent:0.1.0 --filter ancestor=seller-agent:0.1.0 | xargs docker rm -f`.

## What's here
- `coral.toml` — coral-server config (pure MCP coordinator, discovers `/agents/*`).
- `round.ts` — the session launcher (one buyer + one World Cup seller).
- The agents live in [`../../../coral-agents/`](../../../coral-agents) (`buyer-agent`, `seller-agent`,
  and the `seller-worldcup` persona that reuses the seller image with `SERVICES=txline`).

## Scope, honestly
- The round settles through the **base escrow** (`deposit → release`); the web demo's order-bound
  reference + the **arbiter** are the newer settlement path — aligning the agents to them is a follow-up.
- One buyer + one seller is the minimal round. Add the generic seller personas to show *competition*
  (they decline `txline`, the specialist wins) — that's the full marketplace the runtime's `market/`
  protocol supports.
