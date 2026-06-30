/**
 * Devnet guard regression (F1) — escrow is the real settlement path, so makeProgram must refuse a
 * mainnet RPC just like the legacy transfer path. The guard throws before any network call, so this
 * runs offline in CI.
 */
import { describe, it, expect } from 'vitest'
import { Keypair } from '@solana/web3.js'
import { makeProgram, escrowPda } from './escrow.js'

describe('buyer escrow devnet guard', () => {
  it('makeProgram refuses a mainnet RPC', async () => {
    await expect(
      makeProgram(Keypair.generate(), 'https://api.mainnet-beta.solana.com'),
    ).rejects.toThrow(/devnet-only/)
  })

  it('escrowPda is deterministic for (buyer, reference)', () => {
    const buyer = Keypair.generate().publicKey
    const reference = Keypair.generate().publicKey
    expect(escrowPda(buyer, reference).toBase58()).toBe(escrowPda(buyer, reference).toBase58())
  })
})
