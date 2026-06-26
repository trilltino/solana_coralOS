# Bridge — human → agent (Phantom checkout)

The **human front door** to the same agent economy. A person can't be an MCP agent, so this bridge
represents them: it injects their order into a CoralOS session **as the `user-proxy` agent** (the
puppet API), routed to the same `seller-agent` the autonomous buyer uses. The human pays the
seller's Solana Pay URL with Phantom; the seller verifies on-chain and delivers.

It also **serves the demo UI** — the React app in `../web` (built into the bridge image), so there's
no separate frontend to run in production.

## Run

```sh
# prereq: docker compose up -d coral   (from repo root)
npm install
SELLER_WALLET=<devnet pubkey> npm start     # bridge on :3010 (serves /web — the built React app)
# open http://localhost:3010 with Phantom (Devnet)
```

Running `npm start` bare-metal serves whatever is in `./web` (empty until the React build is copied
there). The normal path is `docker compose up -d bridge`, whose image builds the React UI into `/app/web`.
For live UI work, run `npm run dev` in `../web` (Vite proxies to this bridge).

(Via the root `docker compose up -d bridge`, the env comes from `.env` automatically.)

## Flow

```
Browser → POST /order { service }              → seller PAYMENT_REQUIRED (read from session state)
Phantom signs + sends the SOL transfer → sig
Browser → POST /order/:reference/paid { sig }       → seller verifies on-chain → DELIVERED
```

## Files

```
server.ts       puppet bridge (inject as user-proxy) + order/autonomous endpoints; serves ./web
web/            the built React UI (copied in by bridge/Dockerfile; source lives in ../web)
smoke.ts        headless test — pays from the .env keypair in place of the Phantom click
```

> The React source lives in `examples/agent-economy/web/`.

## Why read replies from session state

The coral **puppet API is send-only** — there's no GET to read a thread. So the bridge reads the
seller's replies from `GET /api/v1/local/session/{ns}/{sid}/extended`, scoped to the order's
`threadId`. (See `.claude/AGENT_ECONOMY_RESTRUCTURE.md`.)

## Headless check

```sh
npm run smoke     # order → pay from keypair → assert DELIVERED  (needs coral up + a funded .env wallet)
```

Fork point: `server.ts` — what the seller delivers still comes from
`coral-agents/seller-agent/src/service.ts`.
