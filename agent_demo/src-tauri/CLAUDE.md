# src-tauri

Tauri backend for the agent_demo desktop app. Bridges the React frontend to `agent-core` via Tauri IPC commands. Also owns the `CoralOSClient` HTTP integration.

## Source Files

| File | Purpose |
|------|---------|
| `main.rs` | All `#[tauri::command]` handlers; wraps `AgentManager` and `CoralOSClient` in `Mutex`; defines `PaymentFlowRecord` for flow tracking |
| `coralos.rs` | `CoralOSClient` — lightweight `reqwest`-based HTTP client for a remote CoralOS server (session/agent APIs) |

## State Management

Both `AgentManager` and `CoralOSClient` are stored in Tauri's managed state as `Mutex<T>`:

```rust
tauri::Builder::default()
    .manage(Mutex::new(AgentManager::new()))
    .manage(Mutex::new(CoralOSClient::default()))
```

Always lock the mutex, perform the operation, then drop the guard before any `await` to avoid deadlocks.

## Adding a New IPC Command

1. Write the handler function with `#[tauri::command]` in `main.rs`.
2. Ensure all argument and return types derive `Serialize + Deserialize`.
3. Register the command in the `.invoke_handler(tauri::generate_handler![...])` call.
4. Call it from the frontend with `invoke('command_name', { arg: value })`.

## CoralOS Client

`CoralOSClient` makes HTTP calls to a configurable base URL. The URL is empty by default and must be set at runtime via the `set_coralos_url` IPC command before any CoralOS calls succeed.

## Build & Run

```sh
# From this directory
cargo tauri dev    # dev mode with hot reload

# From agent_demo/ workspace root
cargo build -p triton-agent-demo
```

## Configuration

- `tauri.conf.json` — window size 1400×900, identifier `com.tritonagent.demo`, CSP set to null (dev only).
- `icons/` — application icon assets; update with `cargo tauri icon <image>`.
