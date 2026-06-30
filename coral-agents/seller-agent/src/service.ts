// ← FORK HERE — replace deliverService() with your own service
//
// This is the only function you need to change to build your hackathon entry.
// Input:  the buyer's request string
// Output: your service's response string (JSON, plain text, whatever)
//
// Default: Jupiter DEX swap quote (SOL → USDC) — no API key needed

const KNOWN_SERVICES = new Set(['jupiter', 'coingecko', 'news', 'inference', 'claude', 'txline'])

export async function deliverService(request: string): Promise<string> {
  // The request may NAME a service as its first token — that's how the human checkout's
  // dropdown (and any buyer that picks a service) selects it per-order, e.g.
  //   "inference write a haiku about Solana"  → service=inference, prompt="write a haiku about Solana"
  //   "coingecko eth"                         → service=coingecko, payload="eth"
  // If the first token isn't a known service, fall back to the SERVICE env (single-service mode).
  const [first, ...rest] = request.trim().split(/\s+/)
  const named = KNOWN_SERVICES.has((first ?? '').toLowerCase())
  const service = named ? first.toLowerCase() : (process.env.SERVICE ?? 'jupiter')
  const payload = named ? rest.join(' ') : request

  switch (service) {
    case 'jupiter':
      return jupiterSwapQuote(payload)
    case 'coingecko':
      return coingeckoPrice(payload)
    case 'news':
      return newsHeadlines(payload)
    case 'inference':
    case 'claude':
      return claudeInference(payload)
    case 'txline':
      return txlineService(payload)
    default:
      return jupiterSwapQuote(payload)
  }
}

// Claude inference — resell LLM completions for SOL. This is the on-thesis
// agent-economy service: the buyer pays a micropayment, the seller runs a
// Claude completion and returns it. Calls the Anthropic Messages API over raw
// fetch (REST shape: x-api-key + anthropic-version: 2023-06-01) so the seller
// needs no SDK dependency — matching the other fetch-based services here.
//
// Model defaults to claude-opus-4-8 for maximum completion quality. For a
// micropayment reseller (~$0.015/call) where you want the economics to favour
// cost, set INFERENCE_MODEL=claude-haiku-4-5 ($1/$5 per MTok) instead.
async function claudeInference(request: string): Promise<string> {
  const key = process.env.ANTHROPIC_API_KEY
  if (!key) return JSON.stringify({ error: 'ANTHROPIC_API_KEY not set' })
  const model = process.env.INFERENCE_MODEL || 'claude-opus-4-8' // `||`: toml passes "" for unset options

  const res = await fetch('https://api.anthropic.com/v1/messages', {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-api-key': key,
      'anthropic-version': '2023-06-01',
    },
    body: JSON.stringify({
      model,
      max_tokens: 1024,
      messages: [{ role: 'user', content: request || 'Say hello.' }],
    }),
  })
  if (!res.ok) {
    return JSON.stringify({ error: `anthropic ${res.status}`, detail: (await res.text()).slice(0, 200) })
  }
  // Response content is an array of blocks; concatenate the text blocks.
  const data = await res.json() as { content?: Array<{ type: string; text?: string }> }
  const completion = (data.content ?? [])
    .filter(b => b.type === 'text')
    .map(b => b.text ?? '')
    .join('')
  return JSON.stringify({
    service: 'claude-inference',
    model,
    prompt: request,
    completion,
    timestamp: new Date().toISOString(),
  })
}

// Jupiter DEX — best swap route SOL → USDC
// Set JUPITER_API_KEY in .env for higher rate limits (free at jup.ag/developers)
async function jupiterSwapQuote(_request: string): Promise<string> {
  const SOL = 'So11111111111111111111111111111111111111112'
  const USDC = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'
  const headers: Record<string, string> = { 'Content-Type': 'application/json' }
  if (process.env.JUPITER_API_KEY) headers['x-api-key'] = process.env.JUPITER_API_KEY
  const res = await fetch(
    `https://api.jup.ag/swap/v1/quote?inputMint=${SOL}&outputMint=${USDC}&amount=1000000000&slippageBps=50`,
    { headers },
  )
  if (!res.ok) return JSON.stringify({ error: 'jupiter unavailable', status: res.status })
  const data = await res.json() as Record<string, unknown>
  return JSON.stringify({
    service: 'jupiter-swap-quote',
    pair: 'SOL→USDC',
    inAmount: '1 SOL',
    outAmount: `${(Number(data.outAmount) / 1_000_000).toFixed(4)} USDC`,
    priceImpact: data.priceImpactPct,
    timestamp: new Date().toISOString(),
  })
}

// CoinGecko — SOL price in USD (no API key)
async function coingeckoPrice(request: string): Promise<string> {
  const coin = request.toLowerCase().includes('eth') ? 'ethereum' : 'solana'
  const res = await fetch(
    `https://api.coingecko.com/api/v3/simple/price?ids=${coin}&vs_currencies=usd`,
  )
  if (!res.ok) return JSON.stringify({ error: 'coingecko unavailable' })
  const data = await res.json()
  return JSON.stringify({ coin, usd: data[coin]?.usd, timestamp: new Date().toISOString() })
}

// NewsAPI — top crypto headlines (requires NEWS_API_KEY)
async function newsHeadlines(request: string): Promise<string> {
  const key = process.env.NEWS_API_KEY
  if (!key) return JSON.stringify({ error: 'NEWS_API_KEY not set' })
  const q = encodeURIComponent(request || 'solana crypto')
  const res = await fetch(
    `https://newsapi.org/v2/everything?q=${q}&pageSize=5&sortBy=publishedAt&apiKey=${key}`,
  )
  if (!res.ok) return JSON.stringify({ error: 'newsapi unavailable' })
  const data = await res.json()
  const headlines = (data.articles ?? []).map((a: any) => ({
    title: a.title,
    source: a.source?.name,
    url: a.url,
  }))
  return JSON.stringify({ headlines, timestamp: new Date().toISOString() })
}

// TxODDS TxLINE — verified World Cup data (free devnet tier). Needs TXLINE_API_KEY, minted by the
// one-time on-chain subscribe (see examples/txodds). Verbs (the buyer's request after `txline`):
//   "fixtures"          → upcoming World Cup / Int Friendlies fixtures
//   "odds <fixtureId>"  → de-margined StablePrice odds for a fixture
//   "edge <fixtureId>"  → odds + an LLM value call (the on-thesis, all-three-pillars product)
// Verified live on devnet (2026-06): host txline-dev.txodds.com; odds path is /api/odds/snapshot/{id};
// every call needs the guest JWT *and* the activated X-Api-Token.
// `||` not `??`: coral passes an empty-string default for unset options, which `??` would not catch.
const TXLINE_BASE = process.env.TXLINE_BASE_URL || 'https://txline-dev.txodds.com'

async function txlineGet(path: string): Promise<unknown> {
  const apiToken = process.env.TXLINE_API_KEY
  if (!apiToken) return { error: 'TXLINE_API_KEY not set — run the one-time subscribe (see examples/txodds)' }
  const auth = await fetch(`${TXLINE_BASE}/auth/guest/start`, { method: 'POST' })
  if (!auth.ok) return { error: `txline auth ${auth.status}` }
  const jwt = ((await auth.json()) as { token: string }).token
  const res = await fetch(`${TXLINE_BASE}${path}`, {
    headers: { Authorization: `Bearer ${jwt}`, 'X-Api-Token': apiToken },
  })
  if (!res.ok) return { error: `txline ${path} ${res.status}` }
  return res.json()
}

async function txlineService(request: string): Promise<string> {
  const tokens = request.trim().split(/\s+/).filter(Boolean)
  // A bare fixture id (single numeric token) is treated as `edge <id>` — the on-thesis product, and it
  // survives the single-token WANT `arg` the marketplace broadcasts (e.g. BUYER_ARG=17588245).
  let verb = (tokens[0] ?? 'fixtures').toLowerCase()
  let fixtureId = tokens[1]
  if (/^\d+$/.test(verb)) { fixtureId = verb; verb = 'edge' }
  switch (verb) {
    case 'odds':
      return JSON.stringify({ service: 'txline-odds', fixtureId, odds: await txlineGet(`/api/odds/snapshot/${fixtureId}`) })
    case 'edge': {
      const [odds, fixtures] = await Promise.all([
        txlineGet(`/api/odds/snapshot/${fixtureId}`),
        txlineGet('/api/fixtures/snapshot'),
      ])
      // Pull the 1X2 de-margined market so the buyer/UI gets the odds board, not just the call.
      const m = Array.isArray(odds) ? (odds as Array<Record<string, unknown>>).find((x) => String(x.SuperOddsType ?? '').includes('1X2')) : undefined
      const market = m ? { names: m.PriceNames, pct: m.Pct } : undefined
      // Resolve team names from the fixtures snapshot so each round shows a real matchup.
      const fx = Array.isArray(fixtures) ? (fixtures as Array<Record<string, unknown>>).find((f) => String(f.FixtureId) === String(fixtureId)) : undefined
      const teams = fx ? { home: fx.Participant1, away: fx.Participant2, competition: fx.Competition } : undefined
      const matchup = teams ? `${teams.home} v ${teams.away}` : `fixture ${fixtureId}`
      const raw = await claudeInference(
        `You are a football trading analyst. For ${matchup}, from these de-margined World Cup odds return ` +
          `JSON {call, confidence} — a one-line value call and a 0-1 confidence. Odds: ${JSON.stringify(odds).slice(0, 1500)}`,
      )
      // Prefer the LLM value call; if the model is unavailable (no key/credits), fall back to a
      // deterministic odds-based pick so the demo always renders a clean edge.
      const llm = ((): Record<string, unknown> | undefined => { try { return JSON.parse(raw) } catch { return undefined } })()
      const completion = typeof llm?.completion === 'string' ? llm.completion.trim() : ''
      let analysis: unknown
      if (completion) {
        try { analysis = JSON.parse(completion) } catch { analysis = { call: completion } }
      } else {
        const names = (market?.names ?? []) as string[]
        const pcts = (market?.pct ?? []) as string[]
        let bi = -1, bp = -1
        names.forEach((_, i) => { const p = Number(pcts[i]); if (Number.isFinite(p) && p > bp) { bp = p; bi = i } })
        const label = bi < 0 ? '—' : names[bi] === 'part1' ? (teams?.home ?? 'Home') : names[bi] === 'part2' ? (teams?.away ?? 'Away') : 'Draw'
        analysis = bi >= 0
          ? { call: `Odds favour ${label} (${bp.toFixed(0)}%)`, confidence: Number((bp / 100).toFixed(2)), note: 'deterministic — add Anthropic credits for an LLM call' }
          : { call: 'odds unavailable' }
      }
      return JSON.stringify({ service: 'txline-edge', fixtureId, teams, market, analysis })
    }
    case 'fixtures':
    default: {
      const fixtures = await txlineGet('/api/fixtures/snapshot')
      const list = Array.isArray(fixtures) ? fixtures : []
      return JSON.stringify({ service: 'txline-fixtures', count: list.length, fixtures: list.slice(0, 10) })
    }
  }
}
