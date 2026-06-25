/**
 * Human → user-proxy bridge — the human front door to the agent economy.
 *
 * A person can't be an MCP agent, so this bridge represents them: it injects their order into a
 * CoralOS session *as* the `user-proxy` agent (the exact puppet-API pattern proven by smoke-mcp),
 * routed to the same `seller-agent` the autonomous buyer uses. The human pays the seller's Solana
 * Pay URL with Phantom; the seller verifies on-chain and delivers. One protocol, two front doors.
 *
 *   Browser → POST /order            { service }       → { memo, amountSol, solanaPayUrl }
 *   (Phantom signs + sends the SOL transfer → sig)
 *   Browser → POST /order/:memo/paid { sig }           → { status: 'delivered', data }
 *
 * Env: CORAL_SERVER_URL, CORAL_TOKEN, SELLER_WALLET (required), PRICE_SOL, SERVICE,
 *      SOLANA_RPC_URL, ANTHROPIC_API_KEY, PORT (default 3010).
 */
import express from 'express'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const BASE = process.env.CORAL_SERVER_URL ?? 'http://localhost:5555'
const TOKEN = process.env.CORAL_TOKEN ?? 'dev'
const NS = 'default'
const PORT = Number(process.env.PORT ?? 3010)
const AUTH = { Authorization: `Bearer ${TOKEN}`, 'Content-Type': 'application/json' }

const SELLER_WALLET = process.env.SELLER_WALLET ?? ''
const PRICE_SOL = process.env.PRICE_SOL ?? '0.0001'
const SERVICE = process.env.SERVICE ?? 'jupiter'
const RPC = process.env.SOLANA_RPC_URL ?? 'https://api.devnet.solana.com'
const ANTHROPIC = process.env.ANTHROPIC_API_KEY ?? ''

// ── Typed coral option values: { type: "string" | "f64", value } ──
const str = (value: string) => ({ type: 'string', value })

// ── Lazy long-lived session [seller-agent, user-proxy]; one thread per order ──
let session: Promise<string> | null = null
const orders = new Map<string, { threadId: string }>()

function ensureSession(): Promise<string> {
  if (session) return session
  session = (async () => {
    if (!SELLER_WALLET) throw new Error('SELLER_WALLET not set — the seller has no wallet to receive payments')
    const sellerOpts: Record<string, unknown> = {
      SELLER_WALLET: str(SELLER_WALLET),
      SOLANA_RPC_URL: str(RPC),
      SERVICE: str(SERVICE),
    }
    if (ANTHROPIC) sellerOpts.ANTHROPIC_API_KEY = str(ANTHROPIC)

    const localAgent = (name: string, options: Record<string, unknown> = {}) => ({
      id: { name, version: '0.1.0', registrySourceId: { type: 'local' } },
      name, provider: { type: 'local', runtime: 'docker' }, options,
    })

    const res = await fetch(`${BASE}/api/v1/local/session`, {
      method: 'POST', headers: AUTH,
      body: JSON.stringify({
        agentGraphRequest: { agents: [localAgent('seller-agent', sellerOpts), localAgent('user-proxy')] },
        namespaceProvider: { type: 'create_if_not_exists', namespaceRequest: { name: NS } },
        execution: { mode: 'immediate' },
      }),
    })
    if (!res.ok) { session = null; throw new Error(`session create failed: ${res.status} ${await res.text()}`) }
    const { sessionId } = await res.json() as { sessionId: string }
    console.error(`[bridge] session ${sessionId} created — waiting for agents to spawn`)
    await new Promise(r => setTimeout(r, 8000)) // let coral spawn + connect the containers
    return sessionId
  })()
  return session
}

/** Inject a message into a thread as user-proxy. */
async function inject(sid: string, threadId: string, content: string) {
  const res = await fetch(`${BASE}/api/v1/puppet/${NS}/${sid}/user-proxy/thread/message`, {
    method: 'POST', headers: AUTH,
    body: JSON.stringify({ threadId, content, mentions: ['seller-agent'] }),
  })
  if (!res.ok) throw new Error(`inject failed: ${res.status} ${await res.text()}`)
}

/** Recursively collect message objects ({ threadId, text }) from the extended session state. */
function collectMessages(node: unknown, out: { threadId: string; text: string }[] = []): { threadId: string; text: string }[] {
  if (Array.isArray(node)) {
    for (const v of node) collectMessages(v, out)
  } else if (node && typeof node === 'object') {
    const o = node as Record<string, unknown>
    if (typeof o.threadId === 'string' && typeof o.text === 'string') out.push({ threadId: o.threadId, text: o.text })
    for (const v of Object.values(o)) collectMessages(v, out)
  }
  return out
}

/**
 * Poll the extended session state until a message in `threadId` contains `marker`; return its text.
 * The puppet API is send-only (no read), so replies are read from the session state instead.
 */
async function pollThread(sid: string, threadId: string, marker: string, timeoutMs = 35000): Promise<string> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    await new Promise(r => setTimeout(r, 1500))
    const res = await fetch(`${BASE}/api/v1/local/session/${NS}/${sid}/extended`, { headers: AUTH })
    if (res.ok) {
      const msg = collectMessages(await res.json()).find(m => m.threadId === threadId && m.text.includes(marker))
      if (msg) return msg.text
    }
  }
  throw new Error(`timed out waiting for "${marker}"`)
}

const app = express()
app.use(express.json())

// Serve the Phantom checkout UI (same origin → no CORS).
const webDir = join(dirname(fileURLToPath(import.meta.url)), 'web')
app.use(express.static(webDir))

app.get('/health', (_req, res) => res.json({ ok: true, seller: SELLER_WALLET, service: SERVICE }))

// 1. Start an order — open a thread, ask the seller, return the Solana Pay URL.
app.post('/order', async (req, res) => {
  try {
    const service = String(req.body?.service ?? SERVICE)
    const sid = await ensureSession()

    const tres = await fetch(`${BASE}/api/v1/puppet/${NS}/${sid}/user-proxy/thread`, {
      method: 'POST', headers: AUTH,
      body: JSON.stringify({ threadName: `order-${Date.now()}`, participantNames: ['seller-agent'] }),
    })
    if (!tres.ok) throw new Error(`thread create failed: ${tres.status} ${await tres.text()}`)
    const threadId = (await tres.json() as { thread: { id: string } }).thread.id

    await inject(sid, threadId, `request ${service}`)
    const text = await pollThread(sid, threadId, 'PAYMENT_REQUIRED')

    const memo = text.match(/memo=([A-Za-z0-9-]+)/)?.[1]
    const amountSol = text.match(/amount=([\d.]+)/)?.[1]
    const solanaPayUrl = text.match(/url=(solana:[^\s"\\]+)/)?.[1]
    if (!memo || !solanaPayUrl) throw new Error('could not parse seller PAYMENT_REQUIRED')

    orders.set(memo, { threadId })
    res.json({ memo, amountSol: amountSol ?? PRICE_SOL, solanaPayUrl, recipient: SELLER_WALLET })
  } catch (e) {
    console.error(`[bridge] /order error: ${e}`)
    res.status(502).json({ error: (e as Error).message })
  }
})

// 2. Submit payment proof — tell the seller, wait for delivery.
app.post('/order/:memo/paid', async (req, res) => {
  try {
    const memo = req.params.memo
    const sig = String(req.body?.sig ?? '')
    const order = orders.get(memo)
    if (!order) return res.status(404).json({ error: `unknown memo ${memo}` })
    if (!sig) return res.status(400).json({ error: 'sig required' })

    const sid = await ensureSession()
    await inject(sid, order.threadId, `paid ${sig} memo=${memo}`)
    const text = await pollThread(sid, order.threadId, 'DELIVERED')

    // DELIVERED <data> — grab the seller's delivered payload (rest of the message).
    const data = text.match(/DELIVERED\s+([\s\S]+)/)?.[1]?.trim()
    orders.delete(memo)
    res.json({ status: 'delivered', sig, data: data ?? '(delivered)' })
  } catch (e) {
    console.error(`[bridge] /paid error: ${e}`)
    res.status(502).json({ error: (e as Error).message })
  }
})

app.listen(PORT, () => {
  console.error(`[bridge] human front door on :${PORT} — seller=${SELLER_WALLET || '(SELLER_WALLET unset!)'} service=${SERVICE}`)
})
