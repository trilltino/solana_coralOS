import { describe, it, expect, vi, beforeEach } from 'vitest'

// verifyPayment now delegates the recipient/amount/reference checks to Solana Pay's validateTransfer.
// Mock it so we can test our wiring: correct params in, fail-closed on any validation error.
// `vi.hoisted` so the mock fn exists before vi.mock (which vitest hoists to the top of the file).
const { validateTransfer } = vi.hoisted(() => ({ validateTransfer: vi.fn() }))
vi.mock('@solana/pay', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@solana/pay')>()
  return { ...actual, validateTransfer }
})

import { verifyPayment } from './payment.js'

const SELLER = '7jwB6M2DtuDuXJvFT9RiEwDQUX6Q3DhwtDwg3v8DpjZw'
const REFERENCE = '47DpazckSKKXtyU4hnoJizJjMXmfodgm9pNBXTBN4L4Y'

beforeEach(() => {
  process.env.SELLER_WALLET = SELLER
  process.env.PRICE_SOL = '0.0001'
  validateTransfer.mockReset()
})

describe('verifyPayment (reference-bound)', () => {
  it('accepts when validateTransfer confirms the transfer', async () => {
    validateTransfer.mockResolvedValue({}) // no throw = valid
    expect(await verifyPayment('sig', REFERENCE)).toBe(true)
  })

  it('rejects when validateTransfer throws (mismatch / not found / RPC error) — fails closed', async () => {
    validateTransfer.mockRejectedValue(new Error('amount/recipient/reference mismatch'))
    expect(await verifyPayment('sig', REFERENCE)).toBe(false)
  })

  it('binds the check to the seller wallet, price, and request reference', async () => {
    validateTransfer.mockResolvedValue({})
    await verifyPayment('sig-123', REFERENCE)

    expect(validateTransfer).toHaveBeenCalledTimes(1)
    const [, sig, fields] = validateTransfer.mock.calls[0]
    expect(sig).toBe('sig-123')
    expect(fields.recipient.toBase58()).toBe(SELLER)
    expect(fields.reference.toBase58()).toBe(REFERENCE)
    expect(fields.amount.toNumber()).toBe(0.0001)
  })
})
