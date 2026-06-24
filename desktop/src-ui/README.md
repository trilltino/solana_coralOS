# src-ui — Agent Demo React Frontend

Single-page React 18 application for the Solana multi-agent trading desk demo. Runs either inside a Tauri desktop window (via Tauri IPC) or as a standalone web app talking directly to coral-server over HTTP. The two modes are fully transparent to component code — the `transport.ts` module handles all routing.

## Table of Contents

- [Running the App](#running-the-app)
- [transport.ts — The IPC/HTTP Switch](#transportts--the-iphttp-switch)
- [Environment Variables](#environment-variables)
- [Tabs and Features](#tabs-and-features)
- [State Management](#state-management)
- [Adding a New Tab](#adding-a-new-tab)
- [Calling a Backend Command](#calling-a-backend-command)
- [Workflow DAG Visualization](#workflow-dag-visualization-xyflowreact)
- [Dependencies](#dependencies)

---

## Running the App

### Tauri mode (full desktop app)

Requires Rust + Cargo + `tauri-cli`. Starts Vite dev server on port 5173, then Tauri wraps it.

```bash
cd agent_demo/src-tauri
cargo tauri dev
```

### Web mode (coral-server backend)

Run coral-server first (listens on port 8080), then:

```bash
cd agent_demo/src-ui
npm install
npm run dev          # Vite dev server on http://localhost:5173
```

All Tauri IPC calls are automatically routed to HTTP requests against `http://localhost:8080` (or `VITE_API_URL`).

### Build

```bash
npm run build        # TypeScript check + Vite production bundle → dist/
npm run preview      # Preview the built bundle locally
```

---

## transport.ts — The IPC/HTTP Switch

`src/transport.ts` is the single seam between the two runtimes. It exports three symbols.

### `IS_TAURI: boolean`

`true` when `window.__TAURI__` is present (injected by the Tauri runtime). Use this flag when you need to conditionally show or hide Tauri-only features (e.g. the Python side-car agent tab).

```typescript
import { IS_TAURI } from './transport'

if (IS_TAURI) {
  // show Python agent controls
}
```

### `invoke<T>(cmd, args?): Promise<T>`

Drop-in replacement for `@tauri-apps/api/core`'s `invoke`. In Tauri mode it lazy-imports and calls the Tauri IPC bridge. In web mode it dispatches through an internal HTTP routing table.

```typescript
import { invoke } from './transport'

// Works identically in both modes:
const agents = await invoke<AgentTuple[]>('list_agents')
const state  = await invoke<AgentState>('get_agent_state', { id: 'trader-1' })
```

**Command → HTTP mapping examples:**

| Tauri command | HTTP equivalent |
|---------------|----------------|
| `list_agents` | `GET /api/v1/agents` |
| `create_agent` | `POST /api/v1/agents` |
| `start_agent` | `POST /api/v1/agents/:id/start` |
| `send_message` | `POST /api/v1/messages` |
| `get_all_shared_state` | `GET /api/v1/state` |
| `list_workflows` | `GET /api/v1/workflows` |
| `solana_pay_create_url` | `POST /api/v1/solana-pay/url` |
| `x402_demo_payment` | `POST /api/v1/solana-pay/x402/demo` |
| `complete_sale` | `POST /api/v1/pay-demo/complete-sale` |
| `coralos_set_url` | `PUT /api/v1/coralos/config` |
| `coralos_list_sessions` | `GET /api/v1/coralos/sessions/:namespace` |

**Python agent commands** (`python_agent_start`, `python_agent_status`, `python_agent_stop`) are Tauri-only. In web mode, `python_agent_status` returns `false`, `python_agent_stop` is a no-op, and `python_agent_start` throws an error.

### `listenEvent<T>(event, handler): Promise<() => void>`

Tauri event listener. In web mode returns a no-op unlisten function. Use for streaming events from Tauri background processes (e.g. Python side-car output).

```typescript
import { listenEvent } from './transport'

const unlisten = await listenEvent<PythonAgentEvent>('python-agent-event', (e) => {
  console.log(e.payload)
})

// Cleanup:
unlisten()
```

---

## Environment Variables

Set in `.env` (copy `.env.example` if present) or export before running.

| Variable | Where used | Description |
|----------|-----------|-------------|
| `VITE_API_URL` | `transport.ts` | Base URL for HTTP mode. Defaults to `http://localhost:8080`. |
| `VITE_HELIUS_API_KEY` | `App.tsx` | Helius API key. Used in the Pay Demo tab and Helius agent creation. Falls back to a bundled demo key. |
| `TAURI_DEV_HOST` | `vite.config.ts` | Set by Tauri CLI for remote device development. |

```bash
# .env
VITE_API_URL=http://localhost:8080
VITE_HELIUS_API_KEY=your-helius-key-here
```

---

## Tabs and Features

The app is a single `App` component with a tab bar. The active tab is stored in `useState<Tab>`. Each tab has its own refresh loop (via `setInterval` in a `useEffect`) that starts/stops based on whether the tab is active.

### `Tab` type

```typescript
type Tab =
  | 'local'
  | 'coralos'
  | 'messaging'
  | 'shared-state'
  | 'workflows'
  | 'solana-pay'
  | 'pay-demo'
  | 'payment-flows'
  | 'python-agent'
```

### Local Agents tab (`"local"`)

**Refresh interval:** 2 seconds.

Displays all agents from `list_agents` and `list_agents_with_roles`. Shows per-agent:
- Running state (green/red badge)
- Role assignment dropdown (leader / coordinator / worker / monitor / analyst / trader)
- Start / Stop buttons with per-agent loading state
- Helius API key configuration button
- Full action log (timestamp, action_type, details, tx_signature, slot)

Handlers: `handleCreate`, `handleDelete`, `handleStart`, `handleStop`, `handleSetRole`, `handleHelius`.

### CoralOS tab (`"coralos"`)

**Refresh interval:** 5 seconds (only while tab is active).

Connects to a CoralOS server by setting URL and auth token via `coralos_set_url` / `coralos_set_token`. Lists sessions from `coralos_list_sessions` for the configured namespace, showing each session's agents.

State: `coralUrl`, `coralToken`, `coralNamespace`, `coralSessions`, `selectedCoralSession`.

### Messaging tab (`"messaging"`)

**Refresh interval:** 2 seconds.

Shows all messages from `get_all_messages`. Supports:
- Compose and send (broadcast or direct)
- Filter by agent (shows messages to/from a specific agent)
- Displays `from`, `to`, `msg_type`, `payload`, `timestamp` for each message

### Shared State tab (`"shared-state"`)

**Refresh interval:** 3 seconds.

Shows the full state store and change history. Allows setting new key-value pairs. Values are parsed as JSON if possible, otherwise stored as strings.

### Workflows tab (`"workflows"`)

**Refresh interval:** 3 seconds.

Create workflows with multiple steps (name + description per step). For each step on an existing workflow: assign to an agent, mark as started, mark as completed. Selected workflow shows full step DAG with status badges.

### Solana Pay tab (`"solana-pay"`)

No automatic refresh. UI-driven actions only.

Four panels:
1. **Create Agent** — creates a `Transfer` or `Payment` mode Solana Pay agent
2. **Generate URL** — calls `solana_pay_create_url` with recipient, amount, label, message
3. **Parse URL** — calls `solana_pay_parse_url` on any `solana:` URL
4. **Validate Transaction** — calls `solana_pay_validate` with a transaction signature
5. **Parse 402 Headers** — calls `x402_parse_challenge` on raw headers JSON
6. **Demo Payment** — calls `x402_demo_payment` against a live 402-gated endpoint

### Pay Demo tab (`"pay-demo"`)

**Refresh interval:** 2 seconds.

Guided walkthrough of a seller/buyer payment flow:
1. Creates `pd-seller` (HeliusMonitor) and `pd-buyer` (idle) agents
2. Seller auto-generates a Solana Pay URL on start
3. Polls seller state every 2 s — when `payment-received` action appears, automatically calls `complete_sale`
4. Displays the gated data returned by `complete_sale`

Fixed agent IDs: `pd-seller` and `pd-buyer`.

### Payment Flows tab (`"payment-flows"`)

**Refresh interval:** 2 seconds.

Displays `PaymentFlowRecord` entries from `get_payment_flows`. Selecting a flow shows its full lifecycle: `request_at → challenge_at → payment_at → delivery_at`, along with protocol, amount, recipient, token, and error details.

### Python Agent tab (`"python-agent"`)

**Tauri-only** (hidden or non-functional in web mode).

Controls a Python side-car process managed by `src-tauri`. Configuration fields:
- Agent type (`helius-monitor` is the default)
- Wallet public key
- Amount SOL
- Helius API key
- RPC URL / WebSocket URL override
- Mode (`standalone` or `coral`)

Events streamed from the Python process via the `python-agent-event` Tauri event are displayed in a live log. The `listenEvent` listener is set up in a global `useEffect` on mount.

---

## State Management

The app uses plain React `useState` and `useEffect` — no external state library for core state. All state is local to the `App` component.

**Polling pattern:** Each tab has a `useEffect` that runs `setInterval` only while the relevant tab is active:

```typescript
useEffect(() => {
  if (tab !== 'messaging') return    // skip if not on this tab
  refreshMessages()
  const id = setInterval(refreshMessages, 2000)
  return () => clearInterval(id)     // cleanup on unmount or tab change
}, [tab])
```

**`zustand`** is a listed dependency (`^4`) and is available for use in new features, but the existing `App.tsx` does not use it yet.

**`@tanstack/react-query`** (`^5`) is also available for more structured server state — use it for new features that need caching, background refetch, or optimistic updates.

---

## Adding a New Tab

1. **Add the tab key** to the `Tab` union type:

```typescript
type Tab =
  | 'local'
  // ... existing tabs ...
  | 'my-new-tab'
```

2. **Add a tab button** in the nav bar section of `App.tsx`:

```typescript
<button onClick={() => setTab('my-new-tab')} className={tab === 'my-new-tab' ? 'active' : ''}>
  My New Tab
</button>
```

3. **Add state variables** for the tab's data:

```typescript
const [myData, setMyData] = useState<MyDataType | null>(null)
```

4. **Add a refresh function** and polling `useEffect`:

```typescript
const refreshMyData = async () => {
  const data = await invoke<MyDataType>('my_command')
  setMyData(data)
}

useEffect(() => {
  if (tab !== 'my-new-tab') return
  refreshMyData()
  const id = setInterval(refreshMyData, 3000)
  return () => clearInterval(id)
}, [tab])
```

5. **Add the tab body** in the JSX:

```typescript
{tab === 'my-new-tab' && (
  <div>
    <h2>My New Tab</h2>
    {myData && <pre>{JSON.stringify(myData, null, 2)}</pre>}
    <button onClick={refreshMyData}>Refresh</button>
  </div>
)}
```

6. **Add the HTTP mapping** in `transport.ts` if you are adding a new backend command and need web-mode support:

```typescript
case 'my_command':
  return httpGet('/api/v1/my-endpoint')
```

---

## Calling a Backend Command

Always go through `invoke` from `transport.ts`, never call `fetch` or Tauri directly in components.

```typescript
import { invoke } from './transport'

// No args
const agents = await invoke<AgentTuple[]>('list_agents')

// With args — arg keys are camelCase (Tauri convention); transport.ts converts to snake_case for HTTP
const state = await invoke<AgentState>('get_agent_state', { id: 'trader-1' })

// POST with a body
await invoke('send_message', {
  from: 'user',
  to: 'trader-1',
  msgType: 'task',
  payload: '{"cmd":"rebalance"}',
})
```

Error handling: `invoke` throws on non-2xx HTTP or Tauri errors. Wrap calls in `try/catch` for user-facing flows.

---

## Workflow DAG Visualization (@xyflow/react)

`@xyflow/react` (`^12`) is available for rendering workflow step DAGs. It is not yet wired into `App.tsx`'s Workflows tab (which uses a plain list), but the dependency is ready to use.

Example of rendering a `Workflow`'s steps as a flow graph:

```typescript
import { ReactFlow, type Node, type Edge } from '@xyflow/react'
import '@xyflow/react/dist/style.css'

function WorkflowDag({ workflow }: { workflow: Workflow }) {
  const nodes: Node[] = workflow.steps.map((step, i) => ({
    id: step.id,
    position: { x: 200 * i, y: 0 },
    data: { label: `${step.name} (${step.status})` },
  }))

  const edges: Edge[] = workflow.steps.flatMap(step =>
    step.dependencies.map(depId => ({
      id: `${depId}-${step.id}`,
      source: depId,
      target: step.id,
    }))
  )

  return (
    <div style={{ height: 300 }}>
      <ReactFlow nodes={nodes} edges={edges} fitView />
    </div>
  )
}
```

---

## Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| `react` | `^18.2` | UI framework |
| `react-dom` | `^18.2` | DOM renderer |
| `@tauri-apps/api` | `^2` | Tauri IPC (lazy-imported by transport.ts) |
| `@tauri-apps/plugin-shell` | `^2` | Shell access (Python side-car) |
| `@tanstack/react-query` | `^5` | Server state (available, not yet used in App.tsx) |
| `@xyflow/react` | `^12` | Flow/DAG graph visualization |
| `zustand` | `^4` | Global state store (available, not yet used in App.tsx) |
| `vite` | `^5` | Build tool and dev server |
| `tailwindcss` | `^3.4` | Utility CSS |
| `typescript` | `^5.0` | Type checking |
