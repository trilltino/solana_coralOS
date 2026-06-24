// Strategy registry — maps strategy name → factory function.
//
// Students add their strategy here, then create an agent with:
//   POST /api/v1/agents   body: { "id": "my-agent", "strategy": "my-strategy" }
//
// The agent immediately appears in GET /api/v1/agents and accepts
// POST /api/v1/agents/:id/handle  body: { "text": "..." }

import { AgentManager } from '../../sdk/agent-core-ts/src/manager.js'
import type { Strategy } from '../../sdk/agent-core-ts/src/strategy.js'
import { IdleStrategy } from '../../sdk/agent-core-ts/src/strategies/idle.js'
import { RpcPollStrategy } from '../../sdk/agent-core-ts/src/strategies/rpc_poll.js'
import { WeatherStrategy } from '../../sdk/agent-core-ts/src/strategies/weather.js'

// Solana-dependent strategies require `npm install` in typescript_sdk/agent-core-ts first:
//   import { TransferStrategy } from '../../sdk/agent-core-ts/src/strategies/transfer.js'
//   import { HeliusMonitorStrategy } from '../../sdk/agent-core-ts/src/strategies/helius_monitor.js'

// ── Strategy registry ──────────────────────────────────────────────────────────
// Map a string name to a factory that accepts an optional config object.
// Add your own strategy here, then create an agent with:
//   POST /api/v1/agents   body: { "id": "my-agent", "strategy": "my-strategy" }

type Factory = (config?: unknown) => Strategy

export const REGISTRY: Record<string, Factory> = {
  'idle':     ()         => new IdleStrategy(),
  'rpc-poll': (c: unknown) => new RpcPollStrategy((c as { intervalMs?: number } | undefined)?.intervalMs),
  'weather':  ()         => new WeatherStrategy(),
  // 'transfer':       (c) => new TransferStrategy(c),       // needs @solana/web3.js
  // 'helius-monitor': (c) => new HeliusMonitorStrategy(c),  // needs @solana/web3.js
}

export function makeStrategy(name: string, config?: unknown): Strategy {
  const factory = REGISTRY[name]
  if (!factory) throw new Error(`unknown strategy: "${name}" — registered: ${Object.keys(REGISTRY).join(', ')}`)
  return factory(config)
}

// ── Singleton manager ─────────────────────────────────────────────────────────
// One AgentManager per process — all routes share it.

export const manager = new AgentManager()

// Pre-register the default weather agent so the web marketplace works
// without any setup step.  Students can add more via POST /api/v1/agents.
manager.createAgent('weather-agent', new WeatherStrategy())
