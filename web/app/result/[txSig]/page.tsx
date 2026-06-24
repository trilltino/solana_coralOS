'use client'

import { useEffect, useState } from 'react'
import { useParams, useSearchParams } from 'next/navigation'
import Link from 'next/link'
import { CheckCircle, ExternalLink, RefreshCw, ArrowLeft } from 'lucide-react'
import { Header } from '@/components/Header'

interface WeatherData {
  city?: string
  temperature_c?: number
  temperature_f?: number
  humidity_pct?: number
  wind_mph?: number
  condition?: string
  fetched_at?: string
  error?: string
  [key: string]: unknown
}

export default function ResultPage() {
  const params = useParams()
  const searchParams = useSearchParams()
  const txSig = params.txSig as string
  const agentId = searchParams.get('agent') ?? ''
  const prompt = decodeURIComponent(searchParams.get('prompt') ?? '')

  const [data, setData] = useState<WeatherData | null>(null)
  const [loading, setLoading] = useState(true)
  const [fetchError, setFetchError] = useState('')

  useEffect(() => {
    if (!prompt) { setLoading(false); return }

    const base = process.env.NEXT_PUBLIC_CORAL_SERVER ?? 'http://localhost:8080'

    fetch(`${base}/api/v1/weather`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ city: prompt }),
    })
      .then(r => r.ok ? r.json() : Promise.reject(new Error(`status ${r.status}`)))
      .then(j => setData(j.data))
      .catch(e => setFetchError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false))
  }, [prompt])

  return (
    <div className="min-h-screen">
      <Header />
      <main className="max-w-lg mx-auto px-4 py-12 space-y-4">
        <Link href="/" className="inline-flex items-center gap-1.5 text-gray-500 hover:text-white text-xs transition-colors">
          <ArrowLeft size={12} /> Marketplace
        </Link>

        {/* Payment confirmation */}
        <div className="card flex items-start gap-3">
          <CheckCircle className="text-solana-green shrink-0 mt-0.5" size={18} />
          <div className="min-w-0">
            <p className="font-medium text-sm text-white">Payment confirmed on Solana</p>
            <a
              href={`https://explorer.solana.com/tx/${txSig}?cluster=devnet`}
              target="_blank"
              rel="noopener noreferrer"
              className="text-[11px] text-gray-500 hover:text-brand flex items-center gap-1 mt-0.5 transition-colors font-mono"
            >
              {txSig.slice(0, 28)}… <ExternalLink size={9} />
            </a>
            {prompt && (
              <p className="text-[11px] text-gray-600 mt-1">Query: {prompt}</p>
            )}
          </div>
        </div>

        {/* Weather result */}
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <h2 className="font-semibold text-sm">Agent Response</h2>
            {loading && <RefreshCw size={13} className="animate-spin text-brand" />}
          </div>

          {loading && (
            <div className="space-y-2">
              <div className="h-8 rounded bg-[#1e1e2e] animate-pulse w-1/3" />
              <div className="h-4 rounded bg-[#1e1e2e] animate-pulse w-2/3" />
              <div className="h-4 rounded bg-[#1e1e2e] animate-pulse w-1/2" />
            </div>
          )}

          {!loading && data && !data.error && (
            <div className="space-y-4">
              <div className="flex items-end gap-3">
                <span className="text-4xl font-bold text-solana-green">
                  {data.temperature_c !== undefined ? `${data.temperature_c}°C` : '—'}
                </span>
                <div className="pb-1">
                  <p className="text-white font-medium text-sm">{data.condition}</p>
                  <p className="text-gray-500 text-xs">{data.city}</p>
                </div>
              </div>

              <div className="grid grid-cols-3 gap-3">
                {[
                  { label: 'Feels like', value: data.temperature_f !== undefined ? `${data.temperature_f}°F` : '—' },
                  { label: 'Humidity',   value: data.humidity_pct !== undefined ? `${data.humidity_pct}%` : '—' },
                  { label: 'Wind',       value: data.wind_mph !== undefined ? `${data.wind_mph} mph` : '—' },
                ].map(s => (
                  <div key={s.label} className="bg-[#0d0d15] rounded-lg p-3">
                    <p className="text-[10px] text-gray-500 mb-0.5">{s.label}</p>
                    <p className="text-sm font-semibold">{s.value}</p>
                  </div>
                ))}
              </div>

              {data.fetched_at && (
                <p className="text-[10px] text-gray-600 font-mono">
                  {new Date(data.fetched_at).toLocaleTimeString()} · open-meteo.com · no API key
                </p>
              )}
            </div>
          )}

          {!loading && (data?.error || fetchError) && (
            <div className="space-y-3">
              <p className="text-red-400 text-xs">{data?.error ?? fetchError}</p>
              <p className="text-gray-500 text-xs">
                Start the API server:{' '}
                <span className="font-mono bg-[#0d0d15] px-1.5 py-0.5 rounded text-gray-400">
                  cd api && cargo run
                </span>
              </p>
            </div>
          )}
        </div>

        <div className="flex gap-3">
          <Link href={`/pay/${agentId}`} className="btn-secondary flex-1 text-center text-xs py-2.5">
            Query again
          </Link>
          <Link href="/" className="btn-secondary flex-1 text-center text-xs py-2.5">
            Marketplace
          </Link>
        </div>
      </main>
    </div>
  )
}
