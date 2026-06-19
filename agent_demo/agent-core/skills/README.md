# Pay Skills

MCP skill definitions and provider registry documentation for Pay. This directory contains the instructions that AI assistants read when using Pay via MCP, plus reference guides for contributors.

## What Are Pay Skills?

When Claude, Cursor, or Codex connects to Pay via MCP, they don't just get tools — they get **context**. The `SKILL.md` file tells the agent:

- What Pay can do (search web, scrape, blockchain analytics, image generation, etc.)
- When to use Pay vs. other tools
- How to select providers safely
- How to plan paid calls and avoid wasted spend
- Safety rules (keys stay local, every payment requires approval)

## Directory Layout

```
skills/
└── pay/
    ├── SKILL.md                 # Main skill definition — what agents see
    └── references/
        ├── monetize-api.md      # How to monetize an API with Pay
        ├── provider-selection.md # How agents should choose providers
        ├── security.md          # Safety model and external content handling
        └── setup-cli.md         # How to install, configure, and use the CLI
```

## SKILL.md Structure

The skill file has three parts:

1. **Frontmatter** — Triggers, services, and routing rules for the agent
2. **MCP Tools** — What tools are available and what they do
3. **Core Workflow** — Step-by-step instructions for the agent

### Triggers

Agents activate the Pay skill when the user says things like:
- "can I use pay to X"
- "pay for X"
- "use pay to buy/get X"
- "x402", "MPP", "HTTP 402"

### Services

The skill lists the service families Pay covers: web search, live research, blockchain analytics, image generation, translation, maps, email, BigQuery, and many more. This prevents agents from saying "I can't do that" when Pay actually has a provider.

### Progressive Disclosure

Agents don't read everything upfront. They read specific reference files as needed:

- **`provider-selection.md`** — when choosing between providers, planning paid calls, estimating cost
- **`security.md`** — when explaining Pay's safety model to the user
- **`monetize-api.md`** — when a developer wants to put their API behind Pay
- **`setup-cli.md`** — when the user asks how to install or configure Pay

## Provider Registry (External)

The actual provider listings live in the [`pay-skills`](https://github.com/solana-foundation/pay-skills) repository, not this repo. Each provider is a markdown file:

```
providers/<operator>/<name>.md
```

Example: `providers/solana-foundation/bigquery.md`

The registry is:
- **Curated** — entries are validated before publication
- **Probe-tested** — CI hits each endpoint to verify it returns a valid 402 challenge
- **Agent-optimized** — descriptions and usage notes are written for AI consumption, not human marketing

## Contributing to Skills

### Agent instructions (this repo)

Edit `skills/pay/SKILL.md` or `skills/pay/references/*.md` to improve how agents use Pay. Keep instructions concrete and actionable.

### Provider listings (external repo)

Add a new paid API to the registry:

1. Write a provider markdown file with frontmatter (name, title, description, use_case, category, service_url, endpoints, pricing).
2. Open a PR against `solana-foundation/pay-skills`.
3. CI runs `pay skills validate` to check static structure and probe endpoints.

See `references/monetize-api.md` for the full authoring guide.
