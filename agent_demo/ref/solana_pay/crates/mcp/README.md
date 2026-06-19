# `pay-mcp`

MCP (Model Context Protocol) server implementation for Pay. Exposes Pay's client functionality as MCP tools that AI assistants like Claude, Cursor, and Codex can call.

## What is MCP?

[MCP](https://modelcontextprotocol.io/) is an open protocol that standardizes how AI assistants discover and call tools. Instead of each assistant vendor building custom integrations, they speak MCP. Pay ships an MCP server so any MCP-compatible assistant can request paid API calls.

## Exposed Tools

| Tool | Maps to | Purpose |
|------|---------|---------|
| `search_catalog` | `pay skills search` | Find providers for a task |
| `get_catalog_entry` | `pay skills endpoints` | Get full endpoint details for a provider |
| `list_catalog` | `pay skills list` | Browse all available providers |
| `curl` | `pay-core::client::runner` | Make HTTP requests with 402 handling |
| `get_balance` | `pay-core::client::balance` | Check stablecoin balances |
| `create_skill` | `pay skills provider validate` | Validate a provider markdown file |

## Architecture

```
Claude / Cursor / Codex (MCP Client)
           │
           │ stdio transport
           ▼
    ┌─────────────┐
    │   pay-mcp    │
    │   (rmcp)     │
    └──────┬──────┘
           │
           ▼
    ┌─────────────┐
    │   pay-core   │
    │  (client)    │
    └─────────────┘
```

The MCP server uses **stdio transport** — it reads JSON-RPC messages from stdin and writes responses to stdout. This is the standard transport for local MCP servers launched by AI assistant clients.

## How Agents Use Pay via MCP

When a user says "get me stock quotes for AAPL," the agent:

1. Calls `search_catalog({query: "stock quotes"})`.
2. Gets back ranked providers with endpoint candidates.
3. Calls `get_catalog_entry({fqn: "provider/stock-api"})` for full usage notes.
4. Calls `curl({url: "https://gateway..."})` — Pay prepares the payment, the user approves via Touch ID, and the response is returned to the agent.

The agent never sees private keys, API keys, or payment credentials.

## Adding a New MCP Tool

1. Add the tool definition in `src/tools.rs`.
2. Implement the handler in `src/server.rs`.
3. The handler delegates to `pay-core` — no payment logic lives in this crate.
4. Add tests for the tool schema and handler.

## Launching the MCP Server

```sh
# Standalone
pay mcp

# Injected into Claude Code
pay claude

# Injected into Codex
pay codex
```
