# Building the React Frontend — end to end

A complete, copy-paste guide to replacing the single-file demo UI with a proper **Vite + React +
TypeScript + Solana wallet-adapter** app — the polished showcase. Every file is here.

**The one principle:** the backend does not change. React calls the exact same bridge endpoints the
single-file UI does. You're swapping *how it looks*, not *how it works*.

```
React app (:5173 dev / served by bridge in prod)
   └─ fetch ─▶ bridge (:3010)  ─▶  CoralOS + Solana
        /order · /order/:ref/paid · /autonomous/start · /autonomous/feed
```

The bridge contracts (from `bridge/server.ts`), so the code below is exact:

| Endpoint | Body | Returns |
|----------|------|---------|
| `POST /order` | `{ service }` | `{ reference, amountSol, solanaPayUrl, recipient }` |
| `POST /order/:reference/paid` | `{ sig }` | `{ status:"delivered", sig, data }` |
| `POST /autonomous/start` | — | `{ sessionId }` (400 if not configured) |
| `GET /autonomous/feed` | — | `{ running, messages:[{sender,text}] }` |

---

## Step 0 — Prerequisites
Node 20+, the bridge runnable (`docker compose up -d coral bridge`), and a Phantom wallet set to
**Devnet**. The single-file UI stays as the no-build fallback (Step 8).

## Step 1 — Scaffold
From the repo root:
```sh
cd examples/agent-economy
npm create vite@latest web -- --template react-ts
cd web
```

## Step 2 — Dependencies
```sh
npm install
npm install @solana/web3.js \
  @solana/wallet-adapter-react @solana/wallet-adapter-react-ui \
  @solana/wallet-adapter-wallets @solana/wallet-adapter-base
```

## Step 3 — Config

**`vite.config.ts`** — proxy the bridge in dev so `fetch('/order')` works with no CORS:
```ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const BRIDGE = 'http://localhost:3010'
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/order':      BRIDGE,
      '/autonomous': BRIDGE,
      '/health':     BRIDGE,
    },
  },
  build: { outDir: 'dist' },
})
```

**`package.json`** — confirm the scripts block:
```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview"
  }
}
```

## Step 4 — The API client

**`src/api.ts`** — one typed wrapper per bridge endpoint:
```ts
const json = (r: Response) =>
  r.ok ? r.json() : r.json().then(e => Promise.reject(new Error(e.error || r.statusText)))

export interface Order     { reference: string; amountSol: string; solanaPayUrl: string; recipient: string }
export interface Delivered { status: string; sig: string; data: string }
export interface FeedMsg   { sender: 'buyer-agent' | 'seller-agent'; text: string }

const POST = (url: string, body?: unknown) =>
  fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json' },
               body: body ? JSON.stringify(body) : undefined }).then(json)

export const startOrder      = (service: string): Promise<Order>     => POST('/order', { service })
export const submitPaid      = (ref: string, sig: string): Promise<Delivered> => POST(`/order/${ref}/paid`, { sig })
export const startAutonomous = (): Promise<{ sessionId: string }>    => POST('/autonomous/start')
export const getFeed         = (): Promise<{ running: boolean; messages: FeedMsg[] }> =>
  fetch('/autonomous/feed').then(json)
```

## Step 5 — Wallet providers + entry

**`src/main.tsx`**:
```tsx
import React, { useMemo } from 'react'
import ReactDOM from 'react-dom/client'
import { ConnectionProvider, WalletProvider } from '@solana/wallet-adapter-react'
import { WalletModalProvider } from '@solana/wallet-adapter-react-ui'
import { PhantomWalletAdapter } from '@solana/wallet-adapter-wallets'
import App from './App'
import '@solana/wallet-adapter-react-ui/styles.css'
import './styles.css'

const ENDPOINT = import.meta.env.VITE_RPC_URL ?? 'https://api.devnet.solana.com'

function Root() {
  const wallets = useMemo(() => [new PhantomWalletAdapter()], [])
  return (
    <ConnectionProvider endpoint={ENDPOINT}>
      <WalletProvider wallets={wallets} autoConnect>
        <WalletModalProvider>
          <App />
        </WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  )
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode><Root /></React.StrictMode>,
)
```

## Step 6 — The UI

**`src/App.tsx`** — the two-tab shell:
```tsx
import { useState } from 'react'
import { AutonomousTab } from './components/AutonomousTab'
import { CheckoutTab } from './components/CheckoutTab'

export default function App() {
  const [tab, setTab] = useState<'auto' | 'checkout'>('auto')
  return (
    <div className="app">
      <header>
        <h1>sol_coralOS</h1>
        <p className="sub">An agent economy on Solana — one seller, two front doors.</p>
      </header>
      <nav className="tabs">
        <button className={tab === 'auto' ? 'on' : ''} onClick={() => setTab('auto')}>Autonomous</button>
        <button className={tab === 'checkout' ? 'on' : ''} onClick={() => setTab('checkout')}>Checkout</button>
      </nav>
      {tab === 'auto' ? <AutonomousTab /> : <CheckoutTab />}
    </div>
  )
}
```

**`src/hooks/useFeed.ts`** — poll the autonomous conversation:
```ts
import { useEffect, useState } from 'react'
import { getFeed, type FeedMsg } from '../api'

export function useFeed(active: boolean) {
  const [messages, setMessages] = useState<FeedMsg[]>([])
  useEffect(() => {
    if (!active) return
    const id = setInterval(async () => {
      try { const f = await getFeed(); if (f.messages) setMessages(f.messages) } catch {}
    }, 2500)
    return () => clearInterval(id)
  }, [active])
  return messages
}
```

**`src/components/Feed.tsx`** — render the raw agent messages human-readably:
```tsx
import { type FeedMsg } from '../api'

function renderResult(raw: string): string {
  try {
    const d = JSON.parse(raw)
    if (d.inAmount && d.outAmount) return `${d.inAmount} → ${d.outAmount}`
    if (d.usd != null)            return `${d.usd} ${d.coin ?? ''}`
    if (d.completion)             return d.completion
    return JSON.stringify(d)
  } catch { return raw }
}

function describe(m: FeedMsg): { who: string; verb: string; detail?: string } {
  const who = m.sender === 'buyer-agent' ? 'Buyer' : 'Seller'
  const t = m.text.trim()
  if (/^request/i.test(t))        return { who, verb: 'Requests a service' }
  if (/PAYMENT_REQUIRED/.test(t)) return { who, verb: 'Asks for payment', detail: `${t.match(/amount=([\d.]+)/)?.[1] ?? ''} SOL` }
  if (/^paid/i.test(t))           return { who, verb: 'Paid on-chain', detail: `${t.match(/paid\s+([1-9A-HJ-NP-Za-km-z]{20,})/)?.[1]?.slice(0, 8) ?? ''}…` }
  if (/DELIVERED/.test(t))        return { who, verb: 'Delivered the result', detail: renderResult(t.replace(/^[\s\S]*DELIVERED\s+/, '')) }
  return { who, verb: t.slice(0, 100) }
}

export function Feed({ messages }: { messages: FeedMsg[] }) {
  if (!messages.length) return <p className="muted">No messages yet — give the agents ~20s on first run.</p>
  return (
    <ol className="feed">
      {messages.map((m, i) => {
        const d = describe(m)
        return (
          <li key={i} className={m.sender === 'buyer-agent' ? 'buyer' : 'seller'}>
            <span className="who">{d.who}</span> {d.verb}
            {d.detail && <span className="detail"> — {d.detail}</span>}
          </li>
        )
      })}
    </ol>
  )
}
```

**`src/components/AutonomousTab.tsx`**:
```tsx
import { useState } from 'react'
import { startAutonomous } from '../api'
import { useFeed } from '../hooks/useFeed'
import { Feed } from './Feed'

export function AutonomousTab() {
  const [running, setRunning] = useState(false)
  const [err, setErr] = useState('')
  const messages = useFeed(running)

  async function run() {
    try { await startAutonomous(); setRunning(true) }
    catch (e) { setErr((e as Error).message) }
  }

  return (
    <section>
      <p>An LLM buyer agent requests a service, decides it's worth the price, pays the seller on-chain,
         and uses the result — with no human in the loop.</p>
      <button className="primary" onClick={run} disabled={running}>
        {running ? 'Running…' : 'Run the agent↔agent demo'}
      </button>
      {err && <p className="error">{err}</p>}
      <Feed messages={messages} />
    </section>
  )
}
```

**`src/hooks/useCheckout.ts`** — connect Phantom, pay the reference-bound transfer, submit proof:
```ts
import { useWallet, useConnection } from '@solana/wallet-adapter-react'
import { SystemProgram, Transaction, PublicKey, LAMPORTS_PER_SOL } from '@solana/web3.js'
import { startOrder, submitPaid } from '../api'

export function useCheckout() {
  const { publicKey, sendTransaction } = useWallet()
  const { connection } = useConnection()

  return async function buy(service: string, onStep: (s: string) => void) {
    if (!publicKey) throw new Error('Connect your wallet first')

    onStep('Asking the seller for a price…')
    const order = await startOrder(service)                 // { reference, amountSol, recipient }

    // Build the transfer and write the reference key — this binds the payment to THIS order.
    const ix = SystemProgram.transfer({
      fromPubkey: publicKey,
      toPubkey: new PublicKey(order.recipient),
      lamports: Math.round(Number(order.amountSol) * LAMPORTS_PER_SOL),
    })
    ix.keys.push({ pubkey: new PublicKey(order.reference), isSigner: false, isWritable: false })

    const tx = new Transaction().add(ix)
    const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash()
    tx.recentBlockhash = blockhash
    tx.feePayer = publicKey

    onStep(`Paying ${order.amountSol} SOL — confirm in Phantom…`)
    const sig = await sendTransaction(tx, connection)
    await connection.confirmTransaction({ signature: sig, blockhash, lastValidBlockHeight }, 'confirmed')

    onStep('Payment confirmed — seller is verifying + delivering…')
    const delivered = await submitPaid(order.reference, sig) // seller checks on-chain, returns data
    return { sig, ...delivered }
  }
}
```

**`src/components/CheckoutTab.tsx`**:
```tsx
import { useState } from 'react'
import { WalletMultiButton } from '@solana/wallet-adapter-react-ui'
import { useWallet } from '@solana/wallet-adapter-react'
import { useCheckout } from '../hooks/useCheckout'

const SERVICES = [
  { id: 'jupiter',   name: 'Live SOL→USDC price', desc: 'a Jupiter swap quote' },
  { id: 'coingecko', name: 'Crypto spot price',   desc: 'a CoinGecko price' },
  { id: 'inference', name: 'AI completion',        desc: 'an LLM answer' },
]

export function CheckoutTab() {
  const { connected } = useWallet()
  const buy = useCheckout()
  const [service, setService] = useState('jupiter')
  const [steps, setSteps] = useState<string[]>([])
  const [result, setResult] = useState('')
  const [busy, setBusy] = useState(false)

  async function pay() {
    setBusy(true); setSteps([]); setResult('')
    try {
      const r = await buy(service, s => setSteps(p => [...p, s]))
      setResult(r.data)
    } catch (e) {
      setSteps(p => [...p, `Error: ${(e as Error).message}`])
    } finally { setBusy(false) }
  }

  return (
    <section>
      <p>You are the buyer. Connect Phantom (on Devnet), pick a service, and pay the same seller the
         autonomous agent uses — one click, settled on-chain.</p>

      <WalletMultiButton />

      <div className="services">
        {SERVICES.map(s => (
          <label key={s.id} className={service === s.id ? 'svc on' : 'svc'}>
            <input type="radio" name="svc" checked={service === s.id} onChange={() => setService(s.id)} />
            <span><b>{s.name}</b><br /><span className="muted">{s.desc}</span></span>
          </label>
        ))}
      </div>

      <button className="primary" onClick={pay} disabled={!connected || busy}>
        {busy ? 'Working…' : connected ? 'Buy with Phantom' : 'Connect a wallet first'}
      </button>

      {steps.length > 0 && (
        <ol className="timeline">{steps.map((s, i) => <li key={i}>{s}</li>)}</ol>
      )}
      {result && <pre className="result">{result}</pre>}
    </section>
  )
}
```

**`src/styles.css`** — minimal, themeable:
```css
:root { --bg:#0b0d12; --card:#151923; --line:#232a37; --fg:#e7ecf3; --muted:#8b97a8; --accent:#7c5cff; }
* { box-sizing: border-box }
body { margin:0; background:var(--bg); color:var(--fg); font:15px/1.5 ui-sans-serif,system-ui,sans-serif }
.app { max-width:720px; margin:0 auto; padding:32px 20px }
header h1 { margin:0; font-size:26px } .sub { color:var(--muted); margin:4px 0 20px }
.tabs { display:flex; gap:8px; margin-bottom:20px }
.tabs button { background:var(--card); color:var(--muted); border:1px solid var(--line); padding:8px 16px; border-radius:10px; cursor:pointer }
.tabs button.on { color:var(--fg); border-color:var(--accent) }
.primary { background:var(--accent); color:#fff; border:0; padding:11px 18px; border-radius:10px; font-weight:600; cursor:pointer; margin:14px 0 }
.primary:disabled { opacity:.5; cursor:not-allowed }
.services { display:grid; gap:8px; margin:14px 0 }
.svc { display:flex; gap:10px; align-items:center; background:var(--card); border:1px solid var(--line); border-radius:10px; padding:12px; cursor:pointer }
.svc.on { border-color:var(--accent) }
.feed, .timeline { list-style:none; padding:0; margin:16px 0; display:flex; flex-direction:column; gap:8px }
.feed li { background:var(--card); border:1px solid var(--line); border-left:3px solid var(--line); border-radius:8px; padding:10px 12px }
.feed li.buyer { border-left-color:#3da9fc } .feed li.seller { border-left-color:var(--accent) }
.who { font-weight:700; margin-right:6px } .detail { color:var(--muted) }
.timeline li { background:var(--card); border:1px solid var(--line); border-radius:8px; padding:8px 12px }
.result { background:var(--card); border:1px solid var(--line); border-radius:8px; padding:12px; white-space:pre-wrap; overflow:auto }
.muted { color:var(--muted) } .error { color:#ff6b6b }
```

## Step 7 — Run it in dev
Bring the stack up, then start Vite:
```sh
docker compose up -d coral bridge      # the backend
cd examples/agent-economy/web && npm run dev
# open http://localhost:5173  — proxied to the bridge on :3010
```
Test both tabs: **Autonomous** (click Run → feed populates) and **Checkout** (connect Phantom on
Devnet → Buy → timeline → result).

## Step 8 — Production: let the bridge serve the build
Build to static files and have the bridge serve them (same-origin, no proxy needed).

```sh
cd examples/agent-economy/web && npm run build      # → web/dist/
```

Add one line to **`bridge/server.ts`** (before the existing `express.static(webDir)`), so the React
build is primary and the single-file stays as a fallback at `/minimal.html`:
```ts
// serve the React build if it exists (prod), else fall back to the single-file UI
const reactDist = join(dirname(fileURLToPath(import.meta.url)), '..', 'web', 'dist')
app.use(express.static(reactDist))
```
And mount it into the bridge container in **`docker-compose.yml`**:
```yaml
    volumes:
      - ./examples/agent-economy/bridge/web:/app/web:ro
      - ./examples/agent-economy/web/dist:/app/../web/dist:ro   # the React build
```
*(Or simpler: set Vite's `build.outDir` to `../bridge/web` to overwrite the served dir directly — but
then keep a copy of the single-file first; see Step 9.)*

## Step 9 — Keep the single-file as the minimal reference
Don't delete it — it's the no-build, readable fallback and the quickstart UI. Move it:
```sh
git mv examples/agent-economy/bridge/web/index.html \
       examples/agent-economy/quickstart/minimal-ui.html
```
Note it in the quickstart README as "the zero-build version of the demo." Now you have two artifacts:
the **React app** (showcase) and the **single file** (minimal reference) — same backend.

---

## Verification checklist
- [ ] `npm run dev` → `:5173` loads, no console errors
- [ ] **Autonomous**: Run → feed shows Buyer/Seller messages, ends in "Delivered"
- [ ] **Checkout**: Phantom connects (Devnet) → Buy → real tx (check the sig on Explorer) → result shows
- [ ] `npm run build` succeeds → `dist/`
- [ ] Bridge serves `dist/` in prod (open `:3010`, no Vite running)
- [ ] Single-file preserved as the minimal reference

## Optional polish
- An **Explorer link** for each settled tx (`https://explorer.solana.com/tx/${sig}?cluster=devnet`)
- A **seller earnings** widget (add `GET /earnings` to the bridge, fetch it)
- **Toasts** for each step, a **history** of past orders, a **dark/light** toggle
- Swap `PhantomWalletAdapter` for the wallet-standard auto-discovery (drop the explicit adapter)

> Reminder: none of this touches the agents, CoralOS, or Solana settlement. You're building a nicer
> window onto the same economy — which is exactly why it's safe to go as far as you want.
