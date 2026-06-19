# agent_demo

Tauri desktop application — multi-agent trading desk demo for Solana.

## Layout

| Path | Purpose |
|------|---------|
| `agent-core/` | Rust library: agent lifecycle, messaging, workflows, Solana Pay, Jito |
| `src-tauri/` | Tauri backend: `#[tauri::command]` handlers, CoralOS HTTP client |
| `src-ui/` | React frontend: Vite + Tailwind + @xyflow/react |
| `ref/` | Read-only reference implementations — do not modify |

## Workspace

`Cargo.toml` at this level defines the Rust workspace with two members: `agent-core` and `src-tauri`. All `cargo` commands run from this directory unless stated otherwise.

## Commands

```sh
# Install UI dependencies
cd src-ui && npm install

# Build the entire workspace
cargo build

# Run in dev mode (starts Vite dev server + Tauri hot reload)
cd src-tauri && cargo tauri dev

# Run tests
cargo test

# Single crate tests
cargo test -p agent-core

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Format
cargo fmt
```

## Key Constraints

- All types that cross the Tauri IPC boundary must derive `Serialize` and `Deserialize`.
- `Strategy` implementations must be `Send + Sync` — use `Arc<Mutex<_>>` for interior state.
- Helius wallet monitoring runs against Solana devnet; configure your Helius API key before running payment strategies.
- The `ref/` directory is read-only reference material — never modify it.
