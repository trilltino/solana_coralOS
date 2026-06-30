import { describe, it, expect } from 'vitest'
import { ReplayGuard } from './replay.js'

describe('ReplayGuard', () => {
  it('does not flag an unseen signature', () => {
    const g = new ReplayGuard()
    expect(g.has('sig-A')).toBe(false)
  })

  it('flags a signature once consumed — blocking replay', () => {
    const g = new ReplayGuard()
    expect(g.has('sig-A')).toBe(false) // first use: allowed
    g.consume('sig-A')
    expect(g.has('sig-A')).toBe(true) // second use: rejected
  })

  it('tracks signatures independently', () => {
    const g = new ReplayGuard()
    g.consume('sig-A')
    expect(g.has('sig-A')).toBe(true)
    expect(g.has('sig-B')).toBe(false)
    expect(g.size).toBe(1)
  })
})
