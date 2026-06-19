# `pay-integration`

Network-gated integration tests. These are **not** run by default because they require:

- Live Solana RPC access
- Upstream API availability
- Wallet with real or sandbox funds

## When to Run

Run these before releases, in CI, or when modifying:
- OpenAPI discovery and filtering
- Gateway proxy behavior
- Payment protocol end-to-end flows
- Provider catalog probing

## When to Skip

Skip these during rapid development. Use `cargo test --workspace --exclude pay-integration` or `just rs unit-test`.

## Test Scope

| Test Category | What it covers |
|---------------|----------------|
| OpenAPI smoke tests | Hit real gateways, verify `/openapi.json` returns valid filtered specs |
| End-to-end payment flows | Full 402 challenge → payment → retry → response cycle against sandbox |
| Provider probing | `pay skills probe` against live provider endpoints |

## Environment

Tests use the `network_tests` feature gate in `pay-core` to conditionally compile network-dependent code. Integration tests themselves may require:

- `PAY_SANDBOX=1` to force sandbox mode
- Specific provider API keys (injected as env vars, never committed)

## Running

```sh
# Integration tests only
cargo test -p pay-integration

# With sandbox mode
PAY_SANDBOX=1 cargo test -p pay-integration
```
