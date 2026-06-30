/**
 * Devnet guard regression (F1) — the seller's read-only makeProgram must also refuse a mainnet RPC.
 * The guard throws before any network call, so this runs offline in CI.
 */
import { describe, it, expect } from 'vitest'
import { makeProgram } from './escrow.js'

describe('seller escrow devnet guard', () => {
  it('makeProgram refuses a mainnet RPC', async () => {
    await expect(makeProgram('https://api.mainnet-beta.solana.com')).rejects.toThrow(/devnet-only/)
  })
})
