#!/usr/bin/env python3
"""
helius-monitor — a thin Solana wallet-payment monitor agent.

It is the Python sibling of `agent-core`'s `TritonPaymentMonitorStrategy`:
it opens a Solana PubSub websocket (works against any RPC, including Helius),
watches a recipient wallet, and reports the moment a transfer of at least the
expected amount confirms.

Two modes:
  --mode standalone   Run the monitor and stream JSON events to stdout.
                      This is what the Tauri app launches and listens to.
  --mode coral        (Scaffold) Connect to a real Coral Server over MCP and
                      act as a Coral agent (register / wait_for_mentions /
                      send_message). Requires the `mcp` package + a server;
                      see `coralize()` below.

Every line printed to stdout is a single JSON object — the Tauri backend reads
these line-by-line and forwards them to the UI as `python-agent-event`s.

Event shapes:
  {"type":"started","wallet":...,"amount_sol":...,"mode":...,"rpc_url":...}
  {"type":"baseline","balance_sol":...}
  {"type":"stream-connected"}
  {"type":"payment-received","amount_sol":...,"signature":...,"slot":...}
  {"type":"partial-payment","amount_sol":...}
  {"type":"error","message":...}
  {"type":"exited"}
"""

import argparse
import json
import os
import sys
import time
import urllib.request

LAMPORTS_PER_SOL = 1_000_000_000


def emit(event: dict) -> None:
    """Print one JSON event line and flush so Tauri sees it immediately."""
    sys.stdout.write(json.dumps(event) + "\n")
    sys.stdout.flush()


def rpc_call(rpc_url: str, method: str, params: list):
    """Minimal synchronous JSON-RPC call using stdlib only."""
    body = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    req = urllib.request.Request(
        rpc_url, data=body, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=20) as resp:
        return json.loads(resp.read()).get("result")


def get_balance(rpc_url: str, pubkey: str) -> int:
    result = rpc_call(rpc_url, "getBalance", [pubkey, {"commitment": "confirmed"}])
    return int(result["value"]) if isinstance(result, dict) else int(result or 0)


def latest_signature(rpc_url: str, pubkey: str):
    try:
        result = rpc_call(rpc_url, "getSignaturesForAddress", [pubkey, {"limit": 1}])
        if result:
            top = result[0]
            return top.get("signature"), top.get("slot")
    except Exception:
        pass
    return None, None


def resolve_endpoints(args) -> tuple[str, str]:
    """Build rpc/ws URLs. If a Helius key is present and URLs are blank,
    default to Helius mainnet endpoints — making this a real 'Helius agent'."""
    rpc_url = args.rpc_url
    ws_url = args.ws_url
    key = os.environ.get("HELIUS_API_KEY") or args.helius_api_key
    if not rpc_url and key:
        rpc_url = f"https://mainnet.helius-rpc.com/?api-key={key}"
    if not ws_url and key:
        ws_url = f"wss://mainnet.helius-rpc.com/?api-key={key}"
    if not rpc_url:
        rpc_url = "https://api.devnet.solana.com"
    if not ws_url:
        ws_url = "wss://api.devnet.solana.com"
    return rpc_url, ws_url


def run_standalone(args) -> None:
    try:
        import websockets  # noqa: F401  (async client)
        import asyncio
    except ImportError:
        emit(
            {
                "type": "error",
                "message": "missing dependency 'websockets'. "
                "Run: pip install -r requirements.txt",
            }
        )
        sys.exit(1)

    rpc_url, ws_url = resolve_endpoints(args)
    expected_lamports = int(args.amount * LAMPORTS_PER_SOL)

    emit(
        {
            "type": "started",
            "wallet": args.wallet,
            "amount_sol": args.amount,
            "mode": "standalone",
            "rpc_url": rpc_url,
        }
    )

    async def loop():
        import websockets

        try:
            baseline = get_balance(rpc_url, args.wallet)
        except Exception as e:
            emit({"type": "error", "message": f"getBalance failed: {e}"})
            baseline = 0
        emit({"type": "baseline", "balance_sol": baseline / LAMPORTS_PER_SOL})

        last = baseline
        backoff = 1
        while True:
            try:
                async with websockets.connect(ws_url, ping_interval=20) as ws:
                    await ws.send(
                        json.dumps(
                            {
                                "jsonrpc": "2.0",
                                "id": 1,
                                "method": "accountSubscribe",
                                "params": [
                                    args.wallet,
                                    {"encoding": "base64", "commitment": "confirmed"},
                                ],
                            }
                        )
                    )
                    emit({"type": "stream-connected"})
                    backoff = 1
                    async for raw in ws:
                        msg = json.loads(raw)
                        if msg.get("method") != "accountNotification":
                            continue
                        current = int(
                            msg["params"]["result"]["value"]["lamports"]
                        )
                        if current <= last:
                            last = current
                            continue
                        received = current - last
                        last = current
                        sig, slot = latest_signature(rpc_url, args.wallet)
                        etype = (
                            "payment-received"
                            if received >= expected_lamports
                            else "partial-payment"
                        )
                        emit(
                            {
                                "type": etype,
                                "amount_sol": received / LAMPORTS_PER_SOL,
                                "signature": sig,
                                "slot": slot,
                            }
                        )
            except Exception as e:
                emit(
                    {
                        "type": "error",
                        "message": f"stream error: {e} — retrying in {backoff}s",
                    }
                )
                time.sleep(backoff)
                backoff = min(backoff * 2, 30)

    try:
        asyncio.run(loop())
    except KeyboardInterrupt:
        pass
    finally:
        emit({"type": "exited"})


def coralize(args) -> None:
    """Scaffold for running as a real Coral agent over MCP.

    To complete this you would:
      1. pip install the Coral/MCP client SDK.
      2. Connect to args.coral_url over SSE / streamable HTTP.
      3. `register` this agent in the session.
      4. Loop: `wait_for_mentions` -> parse "watch <WALLET> for <AMOUNT>" ->
         reuse the detection in run_standalone's inner loop -> `send_message`
         back into the thread mentioning the requester.
    The detection logic above is intentionally framework-agnostic so it can be
    dropped straight into that loop.
    """
    emit(
        {
            "type": "error",
            "message": "coral mode is a scaffold — connect to a real Coral "
            "Server and wire register/wait_for_mentions/send_message. "
            "Falling back to standalone monitoring.",
        }
    )
    run_standalone(args)


def main() -> None:
    p = argparse.ArgumentParser(description="helius-monitor agent")
    p.add_argument("--wallet", required=True, help="recipient wallet pubkey")
    p.add_argument("--amount", type=float, default=0.0, help="expected SOL")
    p.add_argument("--rpc-url", dest="rpc_url", default="")
    p.add_argument("--ws-url", dest="ws_url", default="")
    p.add_argument("--helius-api-key", dest="helius_api_key", default="")
    p.add_argument("--mode", default="standalone", choices=["standalone", "coral"])
    p.add_argument("--coral-url", dest="coral_url", default="")
    args = p.parse_args()

    if args.mode == "coral":
        coralize(args)
    else:
        run_standalone(args)


if __name__ == "__main__":
    main()
