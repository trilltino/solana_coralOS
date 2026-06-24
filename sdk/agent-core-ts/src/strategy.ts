import type { AgentState } from './types.js'

// Internal mutable view passed into strategy.run() and strategy.handleMessage()
export interface MutableAgentState {
  readonly id: string
  readonly rpcEndpoint: string
  readonly network: string
  recordAction(actionType: string, details: string, txSignature?: string, slot?: number): void
  snapshot(): AgentState
}

// Mirror of Rust Strategy trait — implement this interface to define agent behaviour.
// Extend BaseStrategy instead of implementing this directly to get the default handleMessage.
export interface Strategy {
  readonly name: string
  run(state: MutableAgentState, signal: AbortSignal): Promise<void>
  // Called when a CoralOS mention or a payment-triggered message arrives.
  // Mirrors Rust: async fn handle_message(&self, text: &str, state: Arc<Mutex<AgentState>>) -> String
  handleMessage(text: string, state: MutableAgentState): Promise<string>
}

// Base class with a default handleMessage — mirrors the Rust default impl on the Strategy trait.
// Students extend this instead of implementing Strategy directly.
export abstract class BaseStrategy implements Strategy {
  abstract readonly name: string
  abstract run(state: MutableAgentState, signal: AbortSignal): Promise<void>

  async handleMessage(text: string, _state: MutableAgentState): Promise<string> {
    return `agent received: ${text.slice(0, 120)}`
  }
}

export function untilAborted(signal: AbortSignal): Promise<void> {
  return new Promise<void>((resolve) => {
    if (signal.aborted) { resolve(); return }
    signal.addEventListener('abort', () => resolve(), { once: true })
  })
}
