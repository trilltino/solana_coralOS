'use client'

import { useState } from 'react'
import { useParams, useRouter } from 'next/navigation'
import { useWallet, useConnection } from '@solana/wallet-adapter-react'
import { PublicKey, SystemProgram, Transaction, LAMPORTS_PER_SOL } from '@solana/web3.js'
import Link from 'next/link'
import { ArrowLeft } from 'lucide-react'
import { Header } from '@/components/Header'
import { getClient } from '@/lib/coral'

const AGENT_META: Record<string, { label: string; priceLamports: number; placeholder: string; sellerWallet: string }> = {
  'weather-agent': {
    label: 'Live Weather',
    priceLamports: 500_000,
    placeholder: 'London, Tokyo, New York…',
    sellerWallet: process.env.NEXT_PUBLIC_SELLER_WALLET ?? '7xKFqjHEsLqQFmXSnqmWTBPEHJLCgtcf7fUJ4E4s7fQ1',
  },
}

type TxStatus = 'idle' | 'building' | 'signing' | 'broadcasting' | 'done' | 'error'

const STATUS_LABEL: Record<TxStatus, string> = {
  idle:         '',
  building:     'Building transaction…',
  signing:      'Waiting for Phantom…',
  broadcasting: 'Broadcasting…',
  done:         'Confirmed!',
  error:        'Retry',
}

export default function PayPage() {
  const params = useParams()
  const router = useRouter()
  const agentId = params.agentId as string
  const meta = AGENT_META[agentId] ?? { label: agentId, priceLamports: 1_000_000, placeholder: 'Enter request…', sellerWallet: '' }

  const { publicKey, signTransaction } = useWallet()
  const { connection } = useConnection()

  const [prompt, setPrompt] = useState('')
  const [status, setStatus] = useState<TxStatus>('idle')
  const [error, setError] = useState('')

  const priceSOL = (meta.priceLamports / LAMPORTS_PER_SOL).toFixed(4)

  async function handlePay() {
    if (!publicKey || !signTransaction || !prompt.trim()) return
    setStatus('building')
    setError('')

    try {
      const client = getClient()
      try {
        await client.setState(`request:${agentId}`, prompt.trim(), 'web-ui')
      } catch { /* api server may not be running */ }

      const tx = new Transaction().add(
        SystemProgram.transfer({
          fromPubkey: publicKey,
          toPubkey: new PublicKey(meta.sellerWallet),
          lamports: meta.priceLamports,
        })
      )
      const { blockhash } = await connection.getLatestBlockhash()
      tx.recentBlockhash = blockhash
      tx.feePayer = publicKey

      setStatus('signing')
      const signed = await signTransaction(tx)

      setStatus('broadcasting')
      const sig = await connection.sendRawTransaction(signed.serialize())
      await connection.confirmTransaction(sig, 'confirmed')

      setStatus('done')
      router.push(`/result/${sig}?agent=${agentId}&prompt=${encodeURIComponent(prompt)}`)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Transaction failed')
      setStatus('error')
    }
  }

  const busy = status !== 'idle' && status !== 'error'

  return (
    <div className="min-h-screen">
      <Header />
      <main className="max-w-lg mx-auto px-4 py-12">
        <Link href="/" className="inline-flex items-center gap-1.5 text-gray-500 hover:text-white text-xs mb-8 transition-colors">
          <ArrowLeft size={12} /> Back
        </Link>

        <div className="card space-y-6">
          <div>
            <h1 className="text-xl font-bold mb-0.5">{meta.label}</h1>
            <p className="text-gray-500 text-xs">Rust agent · Solana devnet · open-meteo.com</p>
          </div>

          <div className="flex items-center justify-between py-3 border-y border-[#1e1e2e]">
            <span className="text-sm text-gray-400">Price per query</span>
            <span className="text-solana-green font-bold text-lg">{priceSOL} SOL</span>
          </div>

          <div>
            <label className="block text-xs text-gray-500 mb-1.5">Your city</label>
            <input
              className="input-field"
              placeholder={meta.placeholder}
              value={prompt}
              onChange={e => setPrompt(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && !busy && handlePay()}
            />
          </div>

          {!publicKey ? (
            <p className="text-center text-xs text-gray-500 py-2">
              Connect your Phantom wallet (top right) to continue
            </p>
          ) : (
            <div className="space-y-2">
              <button
                className="btn-primary w-full text-sm py-3"
                onClick={handlePay}
                disabled={!prompt.trim() || busy}
              >
                {busy
                  ? STATUS_LABEL[status]
                  : status === 'error'
                  ? 'Retry'
                  : `Pay ${priceSOL} SOL → Get weather`}
              </button>

              {error && (
                <p className="text-red-400 text-xs text-center">{error}</p>
              )}

              <p className="text-center text-[10px] text-gray-600">
                From {publicKey.toBase58().slice(0, 6)}…{publicKey.toBase58().slice(-4)}
              </p>
            </div>
          )}
        </div>
      </main>
    </div>
  )
}
