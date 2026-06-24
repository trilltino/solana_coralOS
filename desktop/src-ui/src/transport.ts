/**
 * transport.ts — runtime switch between Tauri IPC and HTTP.
 *
 * Drop-in replacement for `@tauri-apps/api/core` invoke.
 * When running inside Tauri (window.__TAURI__ present) all calls go through
 * the Tauri IPC bridge. When running as a plain web app they are routed to
 * the api server (default http://localhost:8080).
 *
 * Usage in App.tsx:
 *   import { invoke, IS_TAURI, listenEvent } from "./transport"
 */

// Detect Tauri at runtime — the Tauri runtime injects __TAURI__ onto window.
export const IS_TAURI: boolean =
  typeof window !== 'undefined' && !!(window as unknown as Record<string, unknown>).__TAURI__

// Base URL for HTTP mode — set VITE_API_URL in .env to override.
const BASE =
  (typeof import.meta !== 'undefined' && (import.meta as unknown as { env?: Record<string, string> }).env?.VITE_API_URL) ||
  'http://localhost:8080'

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

async function httpGet<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`)
  if (!res.ok) throw new Error(`GET ${path} → ${res.status}`)
  return res.json() as Promise<T>
}

async function httpPost<T>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: body != null ? JSON.stringify(body) : undefined,
  })
  if (!res.ok) throw new Error(`POST ${path} → ${res.status}`)
  if (res.status === 204) return undefined as unknown as T
  return res.json() as Promise<T>
}

async function httpPut<T>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: body != null ? JSON.stringify(body) : undefined,
  })
  if (!res.ok) throw new Error(`PUT ${path} → ${res.status}`)
  if (res.status === 204) return undefined as unknown as T
  return res.json() as Promise<T>
}

async function httpDelete<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`, { method: 'DELETE' })
  if (res.status === 204) return true as unknown as T
  if (!res.ok) throw new Error(`DELETE ${path} → ${res.status}`)
  return res.json() as Promise<T>
}

// ---------------------------------------------------------------------------
// HTTP dispatch table — maps Tauri command names to HTTP calls
// ---------------------------------------------------------------------------

type Args = Record<string, unknown>

async function httpDispatch<T>(cmd: string, args: Args): Promise<T> {
  switch (cmd) {
    // ── Agents ──────────────────────────────────────────────────────────────
    case 'list_agents':
      return httpGet('/api/v1/agents')

    case 'list_agents_with_roles':
      return httpGet('/api/v1/agents/with-roles')

    case 'create_agent':
      return httpPost('/api/v1/agents', { id: args.id })

    case 'get_agent_state':
      return httpGet(`/api/v1/agents/${args.id}`)

    case 'delete_agent':
      return httpDelete(`/api/v1/agents/${args.id}`)

    case 'start_agent':
      return httpPost(`/api/v1/agents/${args.id}/start`)

    case 'stop_agent':
      return httpPost(`/api/v1/agents/${args.id}/stop`)

    case 'set_agent_role':
      return httpPost(`/api/v1/agents/${args.id}/role`, { role: args.role })

    case 'set_agent_helius':
      return httpPost(`/api/v1/agents/${args.id}/helius`, { api_key: args.apiKey })

    case 'create_solana_pay_agent':
      return httpPost('/api/v1/agents/solana-pay', { id: args.id, mode: args.mode })

    case 'create_helius_monitor_agent':
      return httpPost('/api/v1/agents/helius-monitor', {
        id: args.id,
        recipient: args.recipient ?? args.wallet,
        amount_sol: args.amountSol,
        api_key: args.apiKey,
        label: args.label ?? null,
      })

    // ── Messaging ────────────────────────────────────────────────────────────
    case 'get_all_messages':
      return httpGet('/api/v1/messages')

    case 'send_message':
      return httpPost('/api/v1/messages', {
        from: args.from,
        to: args.to ?? null,
        msg_type: args.msgType,
        payload: args.payload,
      })

    // ── Shared State ─────────────────────────────────────────────────────────
    case 'get_all_shared_state':
      return httpGet('/api/v1/shared-state')

    case 'get_state_history':
      return httpGet('/api/v1/shared-state/history')

    case 'set_shared_state':
      return httpPost(`/api/v1/shared-state/${args.key}`, {
        value: args.value,
        changed_by: args.changedBy,
      })

    // ── Workflows ────────────────────────────────────────────────────────────
    case 'list_workflows':
      return httpGet('/api/v1/workflows')

    case 'create_workflow':
      return httpPost('/api/v1/workflows', {
        id: args.id,
        name: args.name,
        description: args.description,
        steps: args.steps,
        priority: args.priority,
        created_by: args.createdBy,
      })

    case 'assign_workflow_step':
      return httpPost(
        `/api/v1/workflows/${args.workflowId}/steps/${args.stepId}/assign`,
        { agent_id: args.agentId },
      )

    case 'start_workflow_step':
      return httpPost(`/api/v1/workflows/${args.workflowId}/steps/${args.stepId}/start`)

    case 'complete_workflow_step':
      return httpPost(
        `/api/v1/workflows/${args.workflowId}/steps/${args.stepId}/complete`,
        { result: args.result },
      )

    // ── Solana Pay ───────────────────────────────────────────────────────────
    case 'solana_pay_create_url':
      return httpPost('/api/v1/solana-pay/url', {
        recipient: args.recipient,
        amount: args.amount,
        label: args.label ?? null,
        message: args.message ?? null,
      })

    case 'solana_pay_parse_url':
      return httpPost('/api/v1/solana-pay/parse', { url: args.url })

    case 'solana_pay_validate':
      return httpPost('/api/v1/solana-pay/validate', {
        id: args.id,
        signature: args.signature,
        expected_recipient: args.expectedRecipient ?? null,
      })

    case 'x402_parse_challenge':
      return httpPost('/api/v1/solana-pay/x402/parse', { headers: args.headers })

    case 'x402_demo_payment':
      return httpPost('/api/v1/solana-pay/x402/demo', {
        endpoint: args.endpoint,
        budget: args.budget,
      })

    // ── Pay Demo ─────────────────────────────────────────────────────────────
    case 'complete_sale':
      return httpPost('/api/v1/payments/complete-sale', {
        seller_id: args.sellerId,
        buyer_id: args.buyerId,
        tx_signature: args.txSignature ?? null,
      })

    case 'get_payment_flows':
      return httpGet('/api/v1/payments/flows')

    // ── CoralOS / Swarm ───────────────────────────────────────────────────────
    case 'coralos_set_url':
      return httpPut('/api/v1/swarm/config', { url: args.url })

    case 'coralos_set_token':
      return httpPut('/api/v1/swarm/config', { token: args.token })

    case 'coralos_list_sessions':
      return httpGet(`/api/v1/swarm/sessions/${args.namespace}`)

    case 'coralos_mcp_join':
      return httpPost('/api/v1/swarm/mcp/join', {
        connection_url: (args as Record<string, string>).connectionUrl,
        agent_name: (args as Record<string, string>).agentName,
      })

    case 'coralos_mcp_status':
      return httpGet(`/api/v1/swarm/mcp/status/${(args as Record<string, string>).name}`)

    // ── Weather agent ─────────────────────────────────────────────────────────
    case 'weather_query':
      return httpPost('/api/v1/weather', { city: (args as Record<string, string>).city })
        .then((res: unknown) => (res as { data: unknown }).data) as Promise<T>

    // ── Python agent (Tauri-only — no-op in web mode) ────────────────────────
    case 'python_agent_status':
      return Promise.resolve(false as unknown as T)

    case 'python_agent_stop':
      return Promise.resolve(undefined as unknown as T)

    case 'python_agent_start':
      throw new Error('Python agent is only available in Tauri desktop mode')

    default:
      throw new Error(`Unknown command in web mode: ${cmd}`)
  }
}

// ---------------------------------------------------------------------------
// Public API — mirrors @tauri-apps/api/core exactly
// ---------------------------------------------------------------------------

/**
 * invoke<T>(cmd, args?) — call a Tauri command or its HTTP equivalent.
 * Swap the import in App.tsx and the rest is automatic.
 */
export async function invoke<T>(
  cmd: string,
  args?: Args,
): Promise<T> {
  if (IS_TAURI) {
    // Lazy-import Tauri to avoid bundling it when running as a web app.
    const { invoke: tauriInvoke } = await import('@tauri-apps/api/core')
    return tauriInvoke<T>(cmd, args)
  }
  return httpDispatch<T>(cmd, args ?? {})
}

/**
 * listenEvent — Tauri event listener, no-op in web mode.
 * Returns a Promise<unlisten fn> exactly like Tauri's listen().
 */
export async function listenEvent<T>(
  event: string,
  handler: (e: { payload: T }) => void,
): Promise<() => void> {
  if (IS_TAURI) {
    const { listen } = await import('@tauri-apps/api/event')
    return listen<T>(event, handler)
  }
  // Web: return a no-op unlisten function
  return () => {}
}
