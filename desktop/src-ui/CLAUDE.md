# src-ui

React/TypeScript frontend for the agent_demo Tauri app. Built with Vite and Tailwind CSS.

## Stack

| Tool | Version | Purpose |
|------|---------|---------|
| React | 18.2 | UI framework |
| TypeScript | 5 | Type safety |
| Vite | 5 | Dev server and build |
| Tailwind CSS | 3.4 | Utility-first styling |
| @xyflow/react | 12 | Workflow DAG visualization |
| @tauri-apps/api | 2 | IPC bridge to Rust backend |
| zustand | 4 | Global state store |
| @tanstack/react-query | 5 | Server state / async data |

## Structure

```
src/
  App.tsx      # Single-component app — all UI logic lives here
  main.tsx     # Entry point — mounts <App />
  index.css    # Global styles and Tailwind directives
```

`App.tsx` is intentionally monolithic for this demo. Do not split into smaller components unless the file becomes unmanageable.

## Key Frontend Types

Defined inline in `App.tsx` to mirror Rust structs:

- `AgentState` — agent snapshot (id, role, status, action log)
- `AgentAction` — individual action entry
- `AgentMeta` — role metadata
- `AgentMessage` — message bus entry
- `WorkflowStep` / `Workflow` — orchestrator structures

When the Rust side changes a struct, update the matching TypeScript interface here.

## Calling Rust Commands

Use the Tauri `invoke` helper:

```ts
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<ReturnType>('command_name', { arg: value });
```

The command name must exactly match the Rust `#[tauri::command]` function name (snake_case).

## Commands

```sh
npm install        # install dependencies
npm run dev        # start Vite dev server (used by cargo tauri dev)
npm run build      # tsc + Vite production build
npm run preview    # preview production build locally
```

## Styling Notes

- Use Tailwind utility classes directly in JSX.
- Avoid custom CSS unless Tailwind cannot express the style.
- Dark/light theme toggling is handled via Tailwind's `dark:` variant and a class on `<html>`.
