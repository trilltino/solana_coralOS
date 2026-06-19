# `pay` CLI

The command-line interface for Pay. Thin dispatch layer over `pay-core`. Handles argument parsing, subcommand routing, interactive prompts, and wrapping external tools.

## Command Taxonomy

Commands are organized by who uses them and what they do:

| Category | Commands | Who uses them |
|----------|----------|---------------|
| **HTTP wrappers** | `curl`, `wget`, `http`, `fetch` | Developers calling paid APIs from scripts |
| **AI agents** | `claude`, `codex` | Users injecting Pay into agent sessions |
| **Wallet** | `setup`, `whoami`, `send`, `topup` | Everyone — account lifecycle |
| **Gateway** | `server start`, `server demo`, `server scaffold` | API providers monetizing endpoints |
| **Catalog** | `skills search`, `skills list`, `skills add` | Agents and users discovering paid APIs |
| **MCP** | `mcp` | AI assistant runtimes (stdio transport) |

## The 402 Retry Loop

When you run `pay curl <url>` and the server returns `402 Payment Required`:

1. **Detect** — CLI sees `402` status + `www-authenticate` (MPP) or `X-PAYMENT-REQUIRED` (x402) header.
2. **Parse** — `pay-core::mpp` or `pay-core::x402` extracts the challenge (price, recipient, network, currency).
3. **Build** — Core constructs a Solana transaction for the exact amount.
4. **Prompt** — Keystore asks OS for biometric/password authorization.
5. **Sign** — If approved, keystore signs the transaction.
6. **Submit** — Transaction is sent to Solana RPC.
7. **Retry** — CLI retries the original HTTP request with the payment proof in headers.
8. **Deliver** — Gateway verifies payment and returns the actual response.

All of this happens inside `run_curl_with_headers` in `pay-core`; the CLI just calls it.

## Account Lifecycle

```sh
pay setup              # Generate keypair, store in OS secure storage
pay whoami             # Show active account + stablecoin balances
pay account new work   # Create named account
pay account list       # List all accounts
pay send <recipient> <amount>  # Transfer stablecoins
pay topup              # Add funds via onramp or mobile wallet
```

Accounts are stored in `~/.config/pay/accounts.yml` (metadata only — keys are in OS keystore). Use `--account <name>` to select a non-default account per command.

## Adding a New Command

1. Add a module in `src/commands/<name>.rs`.
2. Register it in `src/commands/mod.rs` under the `Command` enum.
3. If it needs an account, return `true` from `requires_account()`.
4. Add to `ToolKind` if it wraps an external tool.

## Global Flags

| Flag | Purpose |
|------|---------|
| `--sandbox` / `-s` | Use ephemeral devnet wallet + Surfpool RPC |
| `--mainnet` | Force mainnet regardless of challenge |
| `--local` | Use localhost Surfpool instead of hosted |
| `--account <name>` | Use specific named account |
| `--debugger` | Route MCP curl through Payment Debugger proxy |
| `--verbose` | Show tracing logs and payment details |
| `--no-dna` | Machine-readable output, non-interactive defaults |
