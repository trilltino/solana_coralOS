import { BaseStrategy, MutableAgentState, untilAborted } from '../strategy.js'
import { PublicKey } from '@solana/web3.js'
import { encodeURL } from '@solana/pay'
import BigNumber from 'bignumber.js'

export interface TransferConfig {
  recipient: string
  amountSol: number
  label?: string
  message?: string
}

export class TransferStrategy extends BaseStrategy {
  readonly name = 'solana-pay-transfer'
  private config: TransferConfig

  constructor(config: TransferConfig) {
    this.config = config
  }

  // Mirrors Rust TransferStrategy::handle_message — returns a solana: URL.
  // Input: {"recipient":"...","amount":0.01,"label":"..."} or plain address
  async handleMessage(text: string, state: MutableAgentState): Promise<string> {
    try {
      const req = JSON.parse(text) as { recipient?: string; amount?: number; label?: string }
      const recipient = req.recipient ?? text.trim()
      const amount = req.amount ?? this.config.amountSol
      const label = req.label ?? this.config.label
      const url = encodeURL({
        recipient: new PublicKey(recipient),
        amount: new BigNumber(amount),
        label,
        message: this.config.message,
      })
      const urlStr = url.toString()
      state.recordAction('coral-url-generated', urlStr)
      return urlStr
    } catch (e) {
      return `error: ${String(e)}`
    }
  }

  async run(state: MutableAgentState, signal: AbortSignal): Promise<void> {
    try {
      const url = encodeURL({
        recipient: new PublicKey(this.config.recipient),
        amount: new BigNumber(this.config.amountSol),
        label: this.config.label,
        message: this.config.message,
      })
      state.recordAction('url-generated', url.toString())
    } catch (e) {
      state.recordAction('url-error', String(e))
    }
    await untilAborted(signal)
  }
}
