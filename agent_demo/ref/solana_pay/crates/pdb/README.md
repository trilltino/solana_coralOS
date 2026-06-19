# `pay-pdb`

Embedded Payment Debugger assets. This crate does not contain source code — it bundles the compiled React/Express UI from `../../pdb/` into the Rust binary at build time.

## How the Embedding Works

1. The source lives in `pdb/` at the repo root — a React SPA + Express backend.
2. `pdb/` is built with `pnpm build`, producing static assets in `pdb/dist/`.
3. `crates/pdb/build.rs` reads `pdb/dist/` and embeds it as static bytes using `include_dir`.
4. The `pay` binary serves these assets at runtime when `--debugger` is passed.

## Build Integration

### Full build (includes debugger UI)

```sh
cd pdb && pnpm install --frozen-lockfile && pnpm build
cd ../rust && cargo build --release
```

### Fast build (placeholder instead of real UI)

```sh
# On Windows (PowerShell)
$env:PAY_PDB_ALLOW_PLACEHOLDER="1"; cargo build --release

# On macOS/Linux
PAY_PDB_ALLOW_PLACEHOLDER=1 cargo build --release
```

### Prebuilt distribution

Release builds publish `pay-pdb-dist-<version>.tar.gz`. Packagers (Homebrew, etc.) can unpack it and set `PAY_PDB_DIST=/path/to/dist` before running Cargo. This avoids requiring Node.js/pnpm in the build environment.

`build.rs` intentionally **does not** fetch from GitHub releases — builds must be pinned and reproducible, compatible with offline build systems.

## Runtime

When the debugger is active (e.g., `pay --debugger curl ...`):

1. The CLI starts a local proxy on port `1402`.
2. MCP `curl` requests are routed through this proxy.
3. The proxy intercepts the 402 challenge, payment, and retry.
4. Each step is logged as an SSE event to `http://127.0.0.1:1402/__debugger/logs/stream`.
5. The React frontend renders a sequence diagram from these events.

## Why a Separate Crate?

Embedding frontend assets in `crates/cli/build.rs` would couple the CLI crate to the PDB build process. A dedicated crate:

- Isolates the `include_dir` dependency
- Lets `cli` depend on `pdb` optionally (future: `--no-debugger` builds)
- Makes the build.rs failure mode clear (missing `pdb/dist` vs. other build issues)
