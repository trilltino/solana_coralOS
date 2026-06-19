# `pay-types`

Shared data types for the Pay Rust workspace. Lives at the bottom of the dependency graph so `core` and `keystore` (and any future crates) can depend on it without circularity.

## Why a Separate Crate?

Both `pay-core` and `pay-keystore` need to agree on what a `Stablecoin` or `Amount` is. Without `pay-types`, either:
- `core` depends on `keystore` (wrong direction — core shouldn't know about storage)
- `keystore` depends on `core` (wrong direction — storage shouldn't know about payment logic)
- Types are duplicated and drift apart

`pay-types` breaks the cycle.

## Key Types

| Type | Purpose |
|------|---------|
| `Stablecoin` | USDC, USDT, CASH, PYUSD, USDG — the settlement rails |
| `Amount` | Decimal-safe stablecoin amounts (6-decimal precision for USDC-style tokens) |
| `MeteringConfig` | Dimension-based pricing: unit, scale, tiered rates |
| `ApiSpec` | Gateway endpoint allowlist: method, path, resource, pricing, splits |
| `Challenge` | Parsed 402 challenge: protocol, price, recipient, network, currency |
| `Receipt` | Payment proof for retry: signed transaction, signature |

## Stability

Types in this crate change slowly. They are the contract between crates. When adding a new type, consider whether multiple crates need it. If only `core` needs it, keep it in `core`. If `core` + `keystore` + `cli` all need it, it belongs here.
