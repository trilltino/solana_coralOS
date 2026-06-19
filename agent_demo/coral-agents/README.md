# coral-agents

Thin, standalone Python agents that run **alongside** the Tauri app and can be
launched from it (Local Agents → "Python Agent" tab). They are the on-ramp to
real Coral integration described in
[`../../docs/HELIUS-AGENT-TO-CORAL.md`](../../docs/HELIUS-AGENT-TO-CORAL.md).

## helius_monitor

The Python sibling of `agent-core`'s `TritonPaymentMonitorStrategy`. Opens a
Solana PubSub websocket (works with **Helius** — pass a Helius key or full
Helius URLs) and streams JSON payment events to stdout, which the Tauri backend
forwards to the UI.

### Setup

```sh
cd agent_demo/coral-agents/helius_monitor
python -m venv .venv
.venv\Scripts\activate          # Windows
pip install -r requirements.txt
```

### Run by hand (without the app)

```sh
# Helius (mainnet): just give it your key
python agent.py --wallet <PUBKEY> --amount 0.5 --helius-api-key <KEY>

# Or explicit endpoints (e.g. devnet)
python agent.py --wallet <PUBKEY> --amount 0.5 \
  --rpc-url https://api.devnet.solana.com \
  --ws-url  wss://api.devnet.solana.com
```

Each stdout line is one JSON event: `started`, `baseline`, `stream-connected`,
`payment-received`, `partial-payment`, `error`, `exited`.

### Launch from the Tauri app

The app spawns `python agent.py …` for you and shows a live event log. It finds
this script relative to the app, or via the `PAY_AGENTS_DIR` env var. Set
`PYTHON` to choose a specific interpreter (defaults to `python`).

### Coral mode (scaffold)

`--mode coral --coral-url <url>` is a stub showing where the real MCP
register / `wait_for_mentions` / `send_message` loop goes. The detection logic
is framework-agnostic so it drops straight into that loop. See the migration
doc for the full path.
