# Contributing

Pay is developed in public and we appreciate contributions.

## Important: Branch Targeting

The `main` branch is the integration branch. All feature work and bug fixes should target `main`.

### Prerequisites

- [Just](https://github.com/casey/just) (command runner)
- Rust 1.86+
- Solana CLI 2.2+
- Node.js 20+ and pnpm (for SDK)

## Architecture & Where to Contribute

The monorepo has four main areas. Pick the one that matches your change:

| Area | Path | Typical changes |
|------|------|-----------------|
| **Rust CLI & Gateway** | `rust/` | New CLI commands, 402 protocol support, gateway middleware, keystore backends |
| **TypeScript SDK** | `typescript/` | Solana Pay URL library, QR codes, merchant/wallet client APIs |
| **Payment Debugger** | `pdb/` | UI components, sequence diagram rendering, flow correlation engine |
| **Provider Registry** | `pay-skills` (external repo) | New paid API listings, endpoint metadata, pricing, usage notes |

### Rust CLI & Gateway

The Rust workspace (`rust/`) contains 7 crates:

- **`cli`** — Command dispatcher. Add new commands here.
- **`core`** — Payment logic (MPP, x402, runner, server middleware, metering). Most gateway changes go here.
- **`types`** — Shared data types. Add cross-crate types here to avoid circular dependencies.
- **`keystore`** — OS secure storage. Add new platform backends here.
- **`mcp`** — MCP server for AI assistants. Add new MCP tools here.
- **`pdb`** — Embedded debugger UI assets. You usually edit `pdb/` directly instead.
- **`integration`** — Network-gated end-to-end tests.

```shell
just rs build              # Build release binary
just rs lint               # Clippy (warnings = errors)
just rs fmt                # Format check
just rs test               # Run all tests
just rs unit-test          # Unit tests only
just rs integration-test   # Integration tests only
just rs run -- --help      # Run the CLI
```

### TypeScript SDK

```shell
just ts install            # Install pnpm dependencies
just ts build              # Build the core package
just ts lint               # Check lint + formatting
just ts fmt                # Auto-fix formatting + lint
just ts typecheck          # Typecheck
just ts test               # Run tests
just ts test-watch         # Run tests in watch mode
```

### Payment Debugger

The debugger is a React SPA + Express backend that gets compiled into the Rust binary. Edit source in `pdb/`, then rebuild the Rust binary:

```shell
cd pdb && pnpm install --frozen-lockfile && pnpm build
cd ../rust && cargo build --release
```

Or use the placeholder for faster development builds:

```shell
$env:PAY_PDB_ALLOW_PLACEHOLDER="1"; cargo build --release
```

## Getting Started

Install all dependencies:

```shell
just install
```

## PR Workflow

1. Open an issue or comment on an existing one to discuss your change.
2. Fork the repo and create a feature branch from `main`.
3. Make your change. Add tests for new behavior.
4. Run `just ci` locally (full lint, typecheck, test, build for both Rust and TypeScript).
5. Use [conventional commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `chore:`, etc.).
6. Open a PR against `main`. CI will run `just ci` again.

## Code Style

- **Rust:** `cargo fmt` + `clippy --workspace --all-targets -- -D warnings`. Warnings are treated as errors in CI.
- **TypeScript:** `prettier` + `eslint`. Run `just ts fmt` to auto-fix.
- **Documentation:** READMEs should explain *why* a module exists, not just *what* it does. Include architecture context.

## Security

See [SECURITY.md](./SECURITY.md) for the security policy and vulnerability reporting process.
