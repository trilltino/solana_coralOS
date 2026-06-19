# `pay-core`

The heart of Pay. Contains both **client** logic (what the CLI uses to pay for APIs) and **server** logic (what gateway operators use to monetize APIs).

## Dual Personality

### Client Modules (`client/`)

Used by `crates/cli`, `crates/mcp`, and any Rust consumer that wants to pay for HTTP APIs.

| Module | Purpose |
|--------|---------|
| `client::mpp` | Parse MPP `www-authenticate` challenges, build charge transactions, validate receipts |
| `client::x402` | Interact with x402 facilitators — verify payment, request settlement |
| `client::runner` | Wrap external tools (`curl`, `wget`, `httpie`) with header injection and 402 retry |
| `client::sandbox` | Ephemeral devnet wallet generation, auto-funding via Surfpool cheatcodes |
| `client::session` | MPP session voucher management — batched approvals without re-prompting |
| `client::send` | Direct stablecoin transfers |
| `client::balance` | Query USDC/USDT/CASH balances |

### Server Modules (`server/`)

Enabled by the `server` feature flag. Used by `pay server start` and any axum-based gateway.

| Module | Purpose |
|--------|---------|
| `server::metering` | Dimension-based pricing: per-request, per-token, per-page, tiered rates |
| `server::accounting` | `AccountingKey`/`AccountingStore` — tracks usage per caller, period windows |
| `server::middleware` | axum layer that intercepts requests, returns 402 for unpaid metered endpoints |
| `server::session` | Voucher-based repeated calls — issue a session token after first payment |
| `server::telemetry` | Fee-payer wallet tracking, OTLP observability export |

## The `PaymentState` Trait

Any axum app that wants to use Pay's payment middleware must implement `PaymentState`:

```rust
pub trait PaymentState: Clone + Send + Sync + 'static {
    fn apis(&self) -> &[ApiSpec];           // Endpoint allowlist + pricing
    fn mpp(&self) -> Option<&Mpp>;          // MPP challenge generator
    fn session_mpp(&self) -> Option<&SessionMpp>;  // Session voucher support
    fn fee_payer_wallet(&self) -> Option<&FeePayerWallet>;  // Fee coverage
}
```

The gateway reads your YAML spec, builds `ApiSpec` structs, and implements this trait on app state.

## Metering Dimensions

Pricing is dimension-based:

```yaml
metering:
  dimensions:
    - direction: usage
      unit: requests
      scale: 1
      tiers:
        - price_usd: 0.01
        - threshold: 100
          price_usd: 0.005
```

Supported units: `requests`, `tokens`, `characters`, `pages`, `minutes`, `bytes`, `queries`. The `scale` divides the measured quantity before pricing.

## OpenAPI / Discovery Integration

When `pay server start --openapi openapi.json` is used:

1. Gateway reads the OpenAPI 3 or Google Discovery document at startup.
2. Filters to only paths/methods listed in `endpoints[]` (the allowlist).
3. Rewrites `servers[].url` from the incoming `Host` header.
4. Serves the filtered spec at `GET /openapi.json`.

Agents can drive the proxy by reading `/openapi.json` — they don't need to know the upstream URL.

## Why These Solana Crates?

We pin to specific `solana-*` crate versions because `solana-mpp` and `solana-x402` depend on them. The workspace declares:

- `solana-hash`, `solana-pubkey`, `solana-signature` — primitives
- `solana-instruction`, `solana-message`, `solana-transaction` — transaction building
- `solana-system-interface` — transfer instructions
- `bincode` — serialization (must match Solana's bincode version)

**Do not bump these independently.** Coordinate with the MPP/x402 SDK maintainers to keep versions aligned.

## Feature Flags

| Flag | Effect |
|------|--------|
| `server` | Enables axum middleware, metering, accounting, telemetry |
| `network_tests` | Gates tests that hit live Solana RPC or upstream APIs |
| `vendored-openssl` | Builds OpenSSL from source; needed for cross-compilation |

## Testing

```sh
# Unit tests
cargo test -p pay-core

# Server tests (requires `server` feature)
cargo test -p pay-core --features server --test server_tests

# Metering tests
cargo test -p pay-core --features server --test metering_tests

# Surfpool sandbox tests
cargo test -p pay-core --features server --test surfpool_tests

# Session tests
cargo test -p pay-core --features server --test session_surfpool_sdk_tests
```
