#!/usr/bin/env python3
"""
helius-monitor as a *real* Coral agent.

Unlike `agent.py` (the standalone sidecar that prints JSON to stdout), this
process is launched by Coral Server inside a session. Coral injects
`CORAL_CONNECTION_URL` (the MCP/SSE endpoint) and the options declared in
`coral-agent.toml` as environment variables. The agent then:

    connect (MCP/SSE) → list tools → loop:
        coral_wait_for_mentions   (block until another agent asks)
        watch_for_payment(...)    (reuse the Helius PubSub detection)
        coral_send_message        (report back into the thread)

No LLM — this is a deterministic worker agent.
"""

import asyncio
import json
import os
import sys
import urllib.request

LAMPORTS_PER_SOL = 1_000_000_000


def log(*a):
    print("[helius-monitor]", *a, file=sys.stderr, flush=True)


# ---------------------------------------------------------------------------
# Detection (same JSON-RPC + PubSub approach proven in agent.py)
# ---------------------------------------------------------------------------
def rpc_call(rpc_url: str, method: str, params: list):
    body = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    req = urllib.request.Request(
        rpc_url, data=body, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=20) as resp:
        return json.loads(resp.read()).get("result")


def get_balance(rpc_url: str, pubkey: str) -> int:
    res = rpc_call(rpc_url, "getBalance", [pubkey, {"commitment": "confirmed"}])
    return int(res["value"]) if isinstance(res, dict) else int(res or 0)


def latest_signature(rpc_url: str, pubkey: str):
    try:
        res = rpc_call(rpc_url, "getSignaturesForAddress", [pubkey, {"limit": 1}])
        if res:
            return res[0].get("signature"), res[0].get("slot")
    except Exception:
        pass
    return None, None


def endpoints():
    rpc = os.getenv("RPC_URL", "").strip()
    ws = os.getenv("WS_URL", "").strip()
    key = os.getenv("HELIUS_API_KEY", "").strip()
    if not rpc and key:
        rpc = f"https://mainnet.helius-rpc.com/?api-key={key}"
    if not ws and key:
        ws = f"wss://mainnet.helius-rpc.com/?api-key={key}"
    if not rpc:
        rpc = "https://api.devnet.solana.com"
    if not ws:
        ws = "wss://api.devnet.solana.com"
    return rpc, ws


async def watch_for_payment(wallet: str, expected_lamports: int) -> dict:
    """Block until a balance increase on `wallet` confirms; return the event."""
    import websockets

    rpc_url, ws_url = endpoints()
    try:
        last = get_balance(rpc_url, wallet)
    except Exception as e:
        log("baseline getBalance failed:", e)
        last = 0

    async with websockets.connect(ws_url, ping_interval=20) as w:
        await w.send(
            json.dumps(
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "accountSubscribe",
                    "params": [wallet, {"encoding": "base64", "commitment": "confirmed"}],
                }
            )
        )
        log("subscribed; watching", wallet)
        async for raw in w:
            msg = json.loads(raw)
            if msg.get("method") != "accountNotification":
                continue
            current = int(msg["params"]["result"]["value"]["lamports"])
            if current <= last:
                last = current
                continue
            received = current - last
            last = current
            sig, slot = latest_signature(rpc_url, wallet)
            return {
                "amount_sol": received / LAMPORTS_PER_SOL,
                "signature": sig,
                "slot": slot,
                "qualified": received >= expected_lamports,
            }


# ---------------------------------------------------------------------------
# MCP helpers — defensive, since tool arg schemas are discovered at runtime
# ---------------------------------------------------------------------------
def _result_text(result) -> str:
    try:
        parts = []
        for c in result.content:
            parts.append(getattr(c, "text", "") or "")
        return " ".join(p for p in parts if p)
    except Exception:
        return str(result)


def _parse_mention(text: str):
    """Best-effort: pull a threadId and a sender agent id out of the payload.

    The real Coral server returns shapes like:
      {"messages": [{"id": ..., "threadId": ..., "text": ...,
                     "senderName": ..., "mentionNames": [...]}]}
    or a single-message variant with top-level threadId/senderName.
    We probe all known key variants so the agent survives schema changes.
    """
    thread_id = None
    sender = None
    try:
        data = json.loads(text)
        # Top-level threadId (some versions)
        thread_id = (data.get("threadId") or data.get("thread_id"))
        # Top-level sender (various field names)
        sender = (
            data.get("senderName") or data.get("sender")
            or data.get("senderId") or data.get("from")
        )
        # Nested messages list (current Coral server format)
        if isinstance(data.get("messages"), list) and data["messages"]:
            m0 = data["messages"][0]
            thread_id = thread_id or m0.get("threadId") or m0.get("thread_id")
            sender = sender or (
                m0.get("senderName") or m0.get("sender")
                or m0.get("senderId") or m0.get("from")
            )
        # Single message wrapped under "message" key
        if isinstance(data.get("message"), dict):
            m = data["message"]
            thread_id = thread_id or m.get("threadId") or m.get("thread_id")
            sender = sender or (
                m.get("senderName") or m.get("sender") or m.get("senderId")
            )
    except Exception:
        pass
    return thread_id, sender


async def main():
    from mcp import ClientSession
    from mcp.client.streamable_http import streamablehttp_client

    url = os.environ.get("CORAL_CONNECTION_URL")
    if not url:
        log("CORAL_CONNECTION_URL not set — this agent must be launched by Coral.")
        sys.exit(1)

    wallet = os.environ.get("WALLET", "").strip()
    if not wallet:
        log("WALLET option not set.")
        sys.exit(1)
    expected = int(float(os.getenv("AMOUNT_SOL", "0.5")) * LAMPORTS_PER_SOL)

    # Coral's CORAL_CONNECTION_URL ends in /mcp — streamable-HTTP transport.
    log("connecting to Coral @", url)
    async with streamablehttp_client(url) as (read, write, _):
        async with ClientSession(read, write) as session:
            await session.initialize()
            tools = await session.list_tools()
            names = [t.name for t in tools.tools]
            log("available coral tools:", names)

            wait_tool = next(
                (n for n in names if "wait_for_mention" in n), "coral_wait_for_mentions"
            )
            send_tool = next(
                (n for n in names if n.endswith("send_message")), "coral_send_message"
            )
            log("using", wait_tool, "/", send_tool)

            while True:
                try:
                    res = await session.call_tool(wait_tool, {"maxWaitMs": 30000})
                    text = _result_text(res)
                    # Always log the raw payload so we can diagnose shape mismatches.
                    log("wait_for_mention raw:", text[:400])
                    thread_id, sender = _parse_mention(text)
                    if not text.strip() or text.strip() in ("null", "{}", "[]"):
                        # Genuine timeout / empty window — keep waiting.
                        continue
                    # Even if parsing is partial, proceed: we received *something*.
                    log("mention received — threadId=%s sender=%s" % (thread_id, sender))

                    ev = await watch_for_payment(wallet, expected)
                    kind = "payment-received" if ev["qualified"] else "partial-payment"
                    content = (
                        f"{kind} amount={ev['amount_sol']:.9f} SOL "
                        f"sig={ev['signature']} slot={ev['slot']}"
                    )
                    args = {"content": content}
                    if thread_id:
                        args["threadId"] = thread_id
                    if sender:
                        args["mentions"] = [sender]
                    await session.call_tool(send_tool, args)
                    log("reported:", content)
                except Exception as e:
                    log("loop error:", repr(e))
                    await asyncio.sleep(2)


if __name__ == "__main__":
    asyncio.run(main())
