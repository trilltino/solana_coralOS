import express from 'express'
import cors from 'cors'
import { manager, makeStrategy } from './registry.js'
import { WeatherStrategy } from '../../sdk/agent-core-ts/src/strategies/weather.js'

const app = express()
app.use(cors())
app.use(express.json())

// ── Health ──────────────────────────────────────────────────────────────────

app.get('/health', (_req, res) => {
  res.json({ status: 'healthy', runtime: 'node', version: '0.1.0' })
})

// ── Agents ──────────────────────────────────────────────────────────────────

app.get('/api/v1/agents', (_req, res) => {
  res.json(manager.listAgents())
})

app.post('/api/v1/agents', (req, res) => {
  const { id, strategy = 'idle', config } = req.body as { id: string; strategy?: string; config?: unknown }
  if (!id) { res.status(400).json({ error: 'id is required' }); return }
  try {
    const s = makeStrategy(strategy, config)
    const state = manager.createAgent(id, s)
    if (!state) { res.status(409).json({ error: 'agent already exists' }); return }
    res.status(201).json(state)
  } catch (e) {
    res.status(400).json({ error: String(e) })
  }
})

app.get('/api/v1/agents/:id', (req, res) => {
  const state = manager.getAgentState(req.params.id)
  if (!state) { res.status(404).json({ error: 'not found' }); return }
  res.json(state)
})

app.delete('/api/v1/agents/:id', (req, res) => {
  res.json(manager.removeAgent(req.params.id))
})

app.post('/api/v1/agents/:id/start', async (req, res) => {
  const ok = await manager.startAgent(req.params.id)
  if (!ok) { res.status(404).json({ error: 'not found or already running' }); return }
  res.json(ok)
})

app.post('/api/v1/agents/:id/stop', (req, res) => {
  res.json(manager.stopAgent(req.params.id))
})

// Agent message dispatch — the TypeScript equivalent of coral-server's
// /coralos/mcp/join mention handler.
// POST /api/v1/agents/:id/handle   body: { "text": "London" }
// Returns the strategy's response string.
app.post('/api/v1/agents/:id/handle', async (req, res) => {
  const { text } = req.body as { text: string }
  if (typeof text !== 'string') { res.status(400).json({ error: 'text is required' }); return }
  const reply = await manager.handleMessage(req.params.id, text)
  if (reply === null) { res.status(404).json({ error: 'agent not found' }); return }
  res.json({ reply })
})

// ── Shared State ────────────────────────────────────────────────────────────

app.get('/api/v1/shared-state', (_req, res) => {
  res.json(manager.state.getAll())
})

app.post('/api/v1/shared-state/:key', (req, res) => {
  const { value, changed_by = 'api' } = req.body as { value: unknown; changed_by?: string }
  manager.state.set(req.params.key, value, changed_by)
  res.json(true)
})

app.delete('/api/v1/shared-state/:key', (req, res) => {
  manager.state.delete(req.params.key, 'api')
  res.json(true)
})

// ── Messages ────────────────────────────────────────────────────────────────

app.get('/api/v1/messages', (_req, res) => {
  res.json(manager.bus.getAll())
})

app.post('/api/v1/messages', (req, res) => {
  const { from, to, msg_type, payload } = req.body as {
    from: string; to?: string; msg_type: string; payload: string
  }
  if (to) {
    manager.direct(from, to, msg_type, payload)
  } else {
    manager.broadcast(from, msg_type, payload)
  }
  res.json(true)
})

// ── Weather ─────────────────────────────────────────────────────────────────
// POST /api/v1/weather   body: { city: "London" } | { lat: 51.5, lon: -0.1 }
//
// This is the endpoint the web result page calls after payment is confirmed.
// Runs WeatherStrategy directly — no agent required to be running.

app.post('/api/v1/weather', async (req, res) => {
  const { city, lat, lon } = req.body as { city?: string; lat?: number; lon?: number }
  if (!city && (lat === undefined || lon === undefined)) {
    res.status(400).json({ error: 'provide city or lat+lon' })
    return
  }

  const text = city
    ? JSON.stringify({ city })
    : JSON.stringify({ lat, lon })

  const strategy = new WeatherStrategy()
  const start = Date.now()

  // Minimal MutableAgentState for a one-shot call
  const mockState = {
    id: 'weather-agent',
    rpcEndpoint: 'https://api.devnet.solana.com',
    network: 'devnet',
    recordAction: (type: string, details: string) => {
      // Also record on the real agent if it exists
      manager.getAgent('weather-agent')?.recordAction(type, details)
    },
    snapshot: () => manager.getAgentState('weather-agent')!,
  }

  try {
    const resultStr = await strategy.handleMessage(text, mockState)
    const data = JSON.parse(resultStr) as unknown
    const latency_ms = Date.now() - start
    res.json({ ok: true, data, latency_ms })
  } catch (e) {
    res.status(500).json({ ok: false, error: String(e) })
  }
})

export default app
