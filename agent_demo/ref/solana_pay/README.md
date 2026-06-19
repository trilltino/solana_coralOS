# Rust Workspace

The Rust workspace contains the Pay CLI, core payment library, gateway middleware, OS keystore abstraction, MCP server, embedded debugger assets, and integration tests.

## Workspace Structure

```
rust/
├── crates/
│   ├── cli/          # Command dispatcher — all user-facing commands
│   ├── core/         # Payment logic — MPP, x402, runner, server middleware, metering
│   ├── types/        # Shared types — avoids circular dependencies
│   ├── keystore/     # OS secure credential store abstraction
│   ├── mcp/          # MCP server — exposes pay as MCP tools for AI assistants
│   ├── pdb/          # Embedded debugger UI assets (compiled from ../pdb/)
│   └── integration/  # Network-gated end-to-end tests
├── Cargo.toml        # Workspace manifest
├── Justfile          # Rust-specific tasks (build, test, lint, run)
├── Dockerfile        # Production container image
└── config/           # Platform-specific config (polkit rules for Linux)
```

## Dependency Graph

```
cli
├── core
│   ├── types
│   ├── keystore
│   ├── solana-mpp
│   └── solana-x402
├── mcp
│   └── core
├── keystore
├── types
└── pdb

integration
└── core
```

`core` is the center of gravity. Most bug fixes and features touch it. `types` sits at the bottom to break circular dependencies between `core` and `keystore`.

## Feature Flags

| Flag | Crate | Purpose |
|------|-------|---------|
| `server` | `pay-core` | Enables axum-based payment gateway middleware, metering, accounting |
| `network_tests` | `pay-core` | Gates integration tests that hit live networks |
| `vendored-openssl` | `pay-core` | Builds OpenSSL from source for cross-compilation |
| `gcp_kms` | `cli` | Enables GCP KMS-backed signing for production gateways |

## Build Commands

```sh
# Release binary (all crates)
cargo build --release

# Fast debug build
cargo build

# Specific crate
cargo build -p pay

# With feature flags
cargo build --release -p pay-core --features server
```

## Testing

```sh
# Unit tests (exclude network-dependent integration tests)
cargo test --workspace --exclude pay-integration

# All tests including integration
cargo test --workspace

# Server-only tests (requires `server` feature)
cargo test -p pay-core --features server

# Integration tests only
cargo test -p pay-integration
```

## Key External Dependencies

| Crate | Why we use it |
|-------|---------------|
| `solana-mpp` | MPP protocol client + server implementation |
| `solana-x402` | x402 protocol client implementation |
| `axum` | Gateway HTTP server and payment middleware |
| `clap` | CLI argument parsing with derive macros |
| `rmcp` | MCP server framework (stdio transport) |
| `ratatui` + `crossterm` | Terminal UI for interactive commands |
| `reqwest` | Built-in HTTP client for `pay fetch` |
| `tokio` | Async runtime for server + charge building |
| `opentelemetry-*` | Observability export for production gateways |
| `ed25519-dalek` | Ed25519 signing for payment transactions |

## Solana Crate Pins

We pin to specific `solana-*` crate versions that match what `solana-mpp` and `solana-x402` expect. See `Cargo.toml` `[workspace.dependencies]` for the full list. Do not bump these independently — coordinate with the MPP/x402 SDK maintainers.
