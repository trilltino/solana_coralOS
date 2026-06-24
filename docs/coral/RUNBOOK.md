# Helius agent on real Coral — verified runbook

This is the **tested** end-to-end path that runs the Helius wallet monitor as a
first-class agent inside a real Coral Server. Every step below was executed and
confirmed on 2026-06-19 (Docker Desktop, Windows).

## What works (verified)

- Real Coral Server (`ghcr.io/coral-protocol/coral-server`) running on `:5555`.
- `helius-monitor` registered in the server's **local registry**.
- `POST /api/v1/local/session` → Coral **launches the agent as a Docker
  container**, injects the options + `CORAL_CONNECTION_URL`.
- The agent **connects over MCP** (streamable-HTTP), lists the `coral_*` tools,
  and **waits for mentions**, then runs the Solana PubSub detection and replies
  with `coral_send_message`.
- **Full e2e verified**: puppet `user-proxy` → mention → `helius-monitor` wakes,
  subscribes to Solana devnet websocket, ready to detect and report payments.

## Prerequisites

- Docker Desktop running.
- Both agent images built (the server launches them by tag):
  ```sh
  cd agent_demo/coral-agents/helius_monitor
  docker build -t helius-monitor:0.1.0 .

  cd ../user_proxy
  docker build -t user-proxy:0.1.0 .
  ```

## 1. Start the server (with the local registry mounted)

```sh
cd coral
docker compose up -d
```

`docker-compose.yml` mounts `../agent_demo/coral-agents` → `/agents` and
`config.toml`. Key config (`config.toml`):

```toml
[auth]
keys = ["dev"]

[registry]
localAgents = ["/agents/*"]      # scans /agents/*/coral-agent.toml

[docker]
address = "host.docker.internal" # MUST override the in-container 172.17.0.1
                                  # default, else launched agents can't dial back
```

Confirm the agent loaded:

```sh
curl -s -H "Authorization: Bearer dev" http://localhost:5555/api/v1/registry
# → ... "agents": [ { "name": "helius-monitor", "versions": ["0.1.0"] } ]
```

## 2. Create a session (launches both agents)

Use `session2.json` which includes both `helius-monitor` AND `user-proxy` (the
puppet agent that lets us inject mentions via the REST API):

```sh
curl -s -X POST -H "Authorization: Bearer dev" -H "Content-Type: application/json" \
  --data @session2.json  http://localhost:5555/api/v1/local/session
# → { "namespace": "default", "sessionId": "..." }
```

`session2.json` (both agents, exact schema):

```json
{
  "agentGraphRequest": {
    "agents": [
      {
        "id": { "name": "helius-monitor", "version": "0.1.0", "registrySourceId": { "type": "local" } },
        "name": "helius-monitor",
        "provider": { "type": "local", "runtime": "docker" },
        "proxies": {},
        "options": {
          "WALLET":  { "type": "string", "value": "<recipient pubkey>" },
          "AMOUNT_SOL": { "type": "f64", "value": 0.5 },
          "RPC_URL": { "type": "string", "value": "https://api.devnet.solana.com" },
          "WS_URL":  { "type": "string", "value": "wss://api.devnet.solana.com" }
        }
      },
      {
        "id": { "name": "user-proxy", "version": "0.1.0", "registrySourceId": { "type": "local" } },
        "name": "user-proxy",
        "provider": { "type": "local", "runtime": "docker" },
        "proxies": {},
        "options": {}
      }
    ]
  },
  "namespaceProvider": { "type": "create_if_not_exists", "namespaceRequest": { "name": "default" } },
  "execution": { "mode": "immediate" }
}
```

For **Helius mainnet**, drop RPC_URL/WS_URL and pass
`"HELIUS_API_KEY": { "type": "string", "value": "<key>" }` instead.

## 3. Watch it connect

```sh
docker logs coral-test --since 20s | grep -E "helius|user-proxy"
# [user-proxy]   connected; tools: [...]
# [user-proxy]   idle — puppet API is now active for this agent.
# [helius-monitor] connecting to Coral @ http://host.docker.internal:5555/mcp/v1/.../mcp
# [helius-monitor] available coral tools: ['coral_create_thread', ... 'coral_wait_for_mention', ...]
# [helius-monitor] using coral_wait_for_mention / coral_send_message
# (then it waits for a mention; on one, it watches the wallet and replies)
```

## 4. Trigger a mention (using the puppet API)

The puppet API requires `agentName` to be a registered session participant — that's
why `user-proxy` exists. It's an idle MCP agent whose sole purpose is to be
puppeted from outside the session.

```sh
SID="<sessionId from step 2>"

# Create a thread as user-proxy, with helius-monitor as participant
curl -s -X POST -H "Authorization: Bearer dev" -H "Content-Type: application/json" \
  -d '{"threadName":"payment-test","participantNames":["helius-monitor"]}' \
  http://localhost:5555/api/v1/puppet/default/$SID/user-proxy/thread
# → { "thread": { "id": "<threadId>", ... } }

TID="<threadId from above>"

# Send @helius-monitor mention in that thread
curl -s -X POST -H "Authorization: Bearer dev" -H "Content-Type: application/json" \
  -d "{\"threadId\":\"$TID\",\"content\":\"@helius-monitor watch for payment\",\"mentions\":[\"helius-monitor\"]}" \
  http://localhost:5555/api/v1/puppet/default/$SID/user-proxy/thread/message
```

You will then see in `docker logs coral-test`:
```
[helius-monitor] wait_for_mention raw: {"message":{"id":"...","threadId":"...",
  "text":"@helius-monitor watch for payment","senderName":"user-proxy",...}}
[helius-monitor] mention received — threadId=... sender=user-proxy
[helius-monitor] subscribed; watching <WALLET>
```

The agent is now watching the devnet websocket. When a transfer confirms it will
call `coral_send_message` back into the thread with:
```
payment-received amount=0.500000000 SOL sig=<sig> slot=<slot>
```

## Hard-won gotchas (so you don't rediscover them)

| Symptom | Cause | Fix |
|---|---|---|
| `MissingFieldException [readme, summary, license]` | agent info schema | add them to `[agent]`; license is `{type="spdx", expression="MIT"}` |
| `invalid edition '1', must be at least 3` | registry edition | `edition = 3` at top of `coral-agent.toml` |
| `Serializer for subclass 'number' not found` | option type | use `f64`/`i64`/`string`, not `number` |
| session `proxies` rejected as `[`…`]` | it's a map | `"proxies": {}` |
| `namespaceRequest … 'name' is required` | namespace req | `"namespaceRequest": { "name": "default" }` |
| agent `httpx.ConnectTimeout` to `172.17.0.1:5555` | server-in-container `isWindows()==false` | `[docker] address = "host.docker.internal"` |
| `Couldn't deserialize … unknown key 'timeoutMs'` | wrong arg | `coral_wait_for_mention` takes `maxWaitMs` |
| MCP URL ends in `/mcp` | transport is streamable-HTTP, not SSE | use `streamablehttp_client` in the agent |
| `Messages cannot mention the sender` | puppet API restriction | use a second agent (`user-proxy`) to send the mention |
| New agent dir not appearing in registry after rescan | `docker restart` required on first add | `docker restart coral-test` forces a fresh cold scan |
| `coral_wait_for_mention` returns `null`/empty → silent skip | agent silently `continue`s on parse failure | always log the raw text; response shape is `{"message":{"threadId":...,"senderName":...},"status":"Message received"}` |

## Stop / clean up

```sh
cd coral && docker compose down            # stop the server
docker ps -aq --filter ancestor=helius-monitor:0.1.0 | xargs docker rm -f   # stray agent containers
```

## Files

| File | Role |
|---|---|
| `coral/docker-compose.yml` · `coral/config.toml` | run the server + mount registry |
| `coral/session2.json` | session request that launches both agents |
| `coral/session.json` | single-agent session (no puppet; can't trigger mention externally) |
| `agent_demo/coral-agents/helius_monitor/coral-agent.toml` | registry definition for payment monitor |
| `agent_demo/coral-agents/helius_monitor/coral_agent.py` | the MCP agent (wait → watch → send) |
| `agent_demo/coral-agents/helius_monitor/Dockerfile` | docker-runtime image for helius-monitor |
| `agent_demo/coral-agents/user_proxy/coral-agent.toml` | registry definition for puppet agent |
| `agent_demo/coral-agents/user_proxy/agent.py` | idle MCP agent (connects, then blocks for puppet API) |
| `agent_demo/coral-agents/user_proxy/Dockerfile` | docker-runtime image for user-proxy |
| `coral/api_v1.json` | the server's OpenAPI (reference) |
