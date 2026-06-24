# Agent Demos

A Solana agent trading desk scaffolded with **Tauri** (Rust + React) that demonstrates real-time on-chain data streaming via **Triton One RPC** and MEV-protected execution via **Jito bundles**.

## Architecture

```
┌─────────────────┐
│   Tauri UI      │  React frontend recording agent actions
│  (src-ui/)      │
└────────┬────────┘
         │ Tauri IPC
┌────────▼────────┐
│  Tauri Backend  │  Rust: agent orchestration + wallet + Jito
│  (src-tauri/)   │
└────────┬────────┘
         │
┌────────▼────────┐
│   agent-core    │  Rust lib: Triton gRPC streaming, signal logic
│  (agent-core/)  │
└────────┬────────┘
         │ gRPC
┌────────▼────────┐
│  Triton One     │  Yellowstone gRPC streaming
│   (devnet)      │
└─────────────────┘
```

## Quick Start

```bash
# 1. Install dependencies
cd agent-core && cargo build
cd ../src-ui && npm install

# 2. Run the Tauri app
cd ../src-tauri && cargo tauri dev
```

## Docs

- [How to Make This a Demo](docs/demo-guide.md)
- [Why Jito Is a Superior Choice](docs/why-jito.md)

## Tracks Supported

- **Track 1** — Agents serving agents (via x402/CoralOS integration points)
- **Track 2** — Agents serving humans (delegated trading with budget controls)
- **Track 3** — Agent-accessible services (Triton gRPC wrapper exposed as paid API)

## License

MIT
