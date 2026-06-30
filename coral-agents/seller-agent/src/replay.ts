/**
 * Guard against payment-signature replay.
 *
 * `verifyPayment` confirms a transaction paid the right amount to the right wallet — but that alone
 * lets a buyer reuse **one** paid transaction as proof for **many** requests (same amount + recipient
 * verifies every time). This records consumed signatures and rejects reuse, so each on-chain payment
 * settles exactly one order.
 *
 * In-memory only: a restart forgets consumed signatures. For production, back this with a durable
 * store (Redis/SQLite) so a proof can't be replayed across restarts — see
 * `docs/PRODUCTION_HARDENING.md` §1.1.
 */
export class ReplayGuard {
  private readonly seen = new Set<string>()

  /** Has this signature already been consumed as payment proof? */
  has(sig: string): boolean {
    return this.seen.has(sig)
  }

  /** Mark a signature consumed so it cannot be reused. */
  consume(sig: string): void {
    this.seen.add(sig)
  }

  /** Count of consumed signatures (for metrics/tests). */
  get size(): number {
    return this.seen.size
  }
}
