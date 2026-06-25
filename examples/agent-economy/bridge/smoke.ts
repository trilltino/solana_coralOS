/**
 * Bridge smoke test — verifies the human path end-to-end WITHOUT a browser.
 *
 * The real flow signs the payment with Phantom; here we substitute a scripted transfer from the
 * funded devnet keypair (same tx shape) so the round-trip is testable headless:
 *
 *   POST /order → seller PAYMENT_REQUIRED → pay from keypair on devnet → POST /order/:memo/paid
 *   → assert the seller DELIVERED real data.
 *
 * Preconditions: coral-server up, agent images built, the bridge running on BRIDGE_URL, and a
 * funded BUYER_KEYPAIR_B58 in the repo-root .env (the "human's" wallet stand-in).
 */
import { Connection, Keypair, PublicKey, SystemProgram, Transaction, LAMPORTS_PER_SOL } from '@solana/web3.js'
import bs58 from 'bs58'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const BRIDGE = process.env.BRIDGE_URL ?? 'http://localhost:3010'

function env(key: string): string | undefined {
  if (process.env[key]) return process.env[key]
  const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..')
  try {
    for (const line of readFileSync(join(root, '.env'), 'utf8').split('\n')) {
      const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*)\s*$/)
      if (m && m[1] === key) return m[2].replace(/^["']|["']$/g, '')
    }
  } catch { /* none */ }
  return undefined
}

async function main() {
  const b58 = env('BUYER_KEYPAIR_B58')
  if (!b58) throw new Error('BUYER_KEYPAIR_B58 not in .env — the human-payment stand-in')
  const payer = Keypair.fromSecretKey(bs58.decode(b58))
  const conn = new Connection(env('SOLANA_RPC_URL') ?? 'https://api.devnet.solana.com', 'confirmed')
  console.error(`[smoke] paying wallet: ${payer.publicKey.toBase58()}`)

  // 1. Start an order.
  const order = await fetch(`${BRIDGE}/order`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ service: 'jupiter' }),
  }).then(r => r.json()) as { memo?: string; amountSol?: string; recipient?: string; error?: string }
  if (order.error || !order.memo || !order.recipient) throw new Error(`order failed: ${JSON.stringify(order)}`)
  console.error(`[smoke] order memo=${order.memo} amount=${order.amountSol} → ${order.recipient}`)

  // 2. Pay it from the keypair (stands in for the Phantom click).
  const { blockhash } = await conn.getLatestBlockhash()
  const tx = new Transaction({ feePayer: payer.publicKey, recentBlockhash: blockhash }).add(
    SystemProgram.transfer({
      fromPubkey: payer.publicKey,
      toPubkey: new PublicKey(order.recipient),
      lamports: Math.round(Number(order.amountSol) * LAMPORTS_PER_SOL),
    }),
  )
  tx.sign(payer)
  const sig = await conn.sendRawTransaction(tx.serialize())
  await conn.confirmTransaction(sig, 'confirmed')
  console.error(`[smoke] paid sig=${sig}`)

  // 3. Submit proof, expect delivery.
  const done = await fetch(`${BRIDGE}/order/${order.memo}/paid`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ sig }),
  }).then(r => r.json()) as { status?: string; data?: string; error?: string }
  if (done.error || done.status !== 'delivered') throw new Error(`delivery failed: ${JSON.stringify(done)}`)

  console.error(`[smoke] PASS — DELIVERED: ${done.data?.slice(0, 120)}`)
}

main().catch((e) => { console.error(`[smoke] FAIL — ${e}`); process.exitCode = 1 })
