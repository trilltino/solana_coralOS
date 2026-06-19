# TypeScript Workspace

The TypeScript workspace contains the `@solana/pay` library, documentation, examples, and the Solana Pay spec.

## Workspace Structure

```
typescript/
├── packages/
│   └── solana-pay/
│       ├── core/         # @solana/pay — URL encoding, QR codes, transfer validation
│       ├── docs/         # Documentation site
│       ├── examples/
│       │   ├── payment-flow-merchant/  # Transaction request example
│       │   └── point-of-sale/          # React retail POS app
│       └── spec/         # Solana Pay specification (SPEC.md)
├── package.json          # Root workspace manifest
├── pnpm-workspace.yaml   # pnpm workspace config
└── tsconfig.base.json    # Shared TypeScript config
```

## `@solana/pay` — Dual Nature

The NPM package `@solana/pay` serves **two roles**:

1. **Solana Pay URL Library** — `encodeURL`, `parseURL`, `createQR`, `findReference`, `validateTransfer` for merchant/wallet integrations.
2. **Rust CLI Distribution** — The `pay` binary is downloaded on first run based on your platform (macOS, Linux, Windows). The NPM package is primarily a wrapper that fetches the correct native artifact.

```json
// package.json (excerpt)
"bin": {
  "pay": "./run.cjs"
},
"supportedPlatforms": {
  "x86_64-pc-windows-msvc": {
    "artifact": "pay-x86_64-pc-windows-msvc.zip",
    "binary": "pay.exe"
  },
  // ... other platforms
}
```

When you run `npx pay`, the `run.cjs` script checks your platform, downloads the matching artifact if not cached, and delegates to the native binary.

## Build Commands

```sh
# Install dependencies
pnpm install

# Build core package
pnpm --filter @solana/pay build

# Run tests
pnpm --filter @solana/pay test

# Lint + format check
pnpm --filter @solana/pay lint

# Typecheck
pnpm --filter @solana/pay typecheck
```

## Key Dependencies

| Package | Why we use it |
|---------|---------------|
| `@solana/kit` v6 | Modern Solana TypeScript SDK (replaces `@solana/web3.js`) |
| `@solana/qr-code-styling` | QR code generation with styling options |
| `@solana-program/*` | Program-specific instruction builders (system, token, token-2022, memo) |

## Kit v6 Migration

`@solana/pay` v1.0 is built on `@solana/kit` v6. Key changes from v0.2 (`@solana/web3.js`):

| v0.2 | v1.0 |
|------|------|
| `PublicKey` | `Address` (branded string) |
| `Connection` | `Rpc` from `@solana/kit` |
| `BigNumber` (bignumber.js) | Plain `number` for human-readable amounts |
| `createTransfer()` returns `Transaction` | Returns `Instruction[]` — compose with kit's `pipe()` |
| `@solana/spl-token` | `@solana-program/token` |
| `Buffer` | `Uint8Array` / `TextEncoder` |

See the core README's Migration Guide for a full diff.

## Examples

- **`point-of-sale/`** — React app for in-person retail. Generates QR codes, accepts USDC. Deployable to Vercel.
- **`payment-flow-merchant/`** — Transaction request flow. The merchant server composes transactions dynamically instead of just requesting a transfer.

## Spec

`spec/SPEC.md` defines the Solana Pay URL scheme: `solana:<recipient>?amount=<amount>&spl-token=<mint>&reference=<pubkey>&label=<label>&message=<message>&memo=<memo>`.

The `@solana/pay` library is the reference implementation of this spec.
