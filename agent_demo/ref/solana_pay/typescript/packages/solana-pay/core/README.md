# Solana Pay

`@solana/pay` is a JavaScript library for facilitating commerce on Solana by using a token transfer URL scheme. The URL scheme ensures that no matter the wallet or service used, the payment request must be created and interpreted in one standard way.

> **v1.0** — This version is built on [`@solana/kit`](https://github.com/anza-xyz/kit) v6. If you're migrating from v0.2 (which used `@solana/web3.js`), see the [Migration Guide](#migration-guide) below.

## Requirements

- Node.js >= 20 (required for Ed25519 crypto.subtle support)

## Installation

```bash
npm install @solana/pay @solana/kit
```

### Peer dependencies

| Package | Version |
|---------|---------|
| `@solana/kit` | `^6.9.0` |

### Optional dependencies (for `createTransfer` / `validateTransfer`)

| Package | Version |
|---------|---------|
| `@solana-program/system` | `^0.12.0` |
| `@solana-program/token` | `^0.11.0` |
| `@solana-program/token-2022` | `^0.9.0` |
| `@solana-program/memo` | `^0.11.0` |

## Quick Start

### Merchant client

For the receiving side — encode payment URLs, generate QR codes, find and validate payments. No signer needed.

```ts
import { address } from '@solana/kit';
import { createMerchantClient } from '@solana/pay';

const merchant = createMerchantClient({
  rpcUrl: 'https://api.devnet.solana.com',
});

const recipient = address('MERCHANT_WALLET_ADDRESS');
const reference = address('UNIQUE_REFERENCE_ADDRESS');

// Encode a payment URL and show as QR code
const url = merchant.pay.encodeURL({ recipient, amount: 1, reference });
const qr = merchant.pay.createQR(url);

// After customer pays, find and validate the transaction
const found = await merchant.pay.findReference(reference);
await merchant.pay.validateTransfer(found.signature, { recipient, amount: 1, reference });
```

### Wallet client

For the paying side — parse payment URLs, create transfers, and send transactions. Includes a transaction planner/executor so you don't need to manually compose transactions.

```ts
import { createWalletClient } from '@solana/pay';

const wallet = createWalletClient({
  rpcUrl: 'https://api.devnet.solana.com',
  payer: myWalletSigner,
});

// Parse a Solana Pay URL and send the payment
const parsed = wallet.pay.parseURL(url);
const instructions = await wallet.pay.createTransfer({
  recipient: parsed.recipient,
  amount: parsed.amount,
});
await wallet.sendTransaction(instructions);
```

### Combined client

Use `createSolanaPayClient` when you need both merchant and wallet methods on one client:

```ts
import { createSolanaPayClient } from '@solana/pay';

const client = createSolanaPayClient({
  rpcUrl: 'https://api.devnet.solana.com',
  payer: myWalletSigner,
});

const url = client.pay.encodeURL({ recipient, amount: 1 });
const instructions = await client.pay.createTransfer({ recipient, amount: 1 });
await client.sendTransaction(instructions);
```

### Standalone usage

Use the functions directly without a client:

```ts
import { address, createSolanaRpc } from '@solana/kit';
import { encodeURL, createTransfer, findReference, validateTransfer, createQR } from '@solana/pay';

const rpc = createSolanaRpc('https://api.devnet.solana.com');
const recipient = address('MERCHANT_WALLET_ADDRESS');

const url = encodeURL({ recipient, amount: 1 });
const qr = createQR(url);
const instructions = await createTransfer(rpc, sender, { recipient, amount: 1 });

const signatureInfo = await findReference(rpc, reference);
await validateTransfer(rpc, signatureInfo.signature, { recipient, amount: 1 });
```

### Kit Plugin usage

Compose with other kit plugins for full control:

```ts
import { createClient } from '@solana/kit';
import { solanaRpcConnection } from '@solana/kit-plugin-rpc';
import { payerFromFile } from '@solana/kit-plugin-signer';
import { solanaPayMerchant, solanaPayWallet } from '@solana/pay';

const client = await createClient()
  .use(solanaRpcConnection({ rpcUrl: 'https://api.devnet.solana.com' }))
  .use(payerFromFile('~/.config/solana/id.json'))
  .use(solanaPayMerchant())
  .use(solanaPayWallet());

const url = client.pay.encodeURL({ recipient, amount: 1 });
const instructions = await client.pay.createTransfer({ recipient, amount: 1 });
```

## API

### URL Encoding & Parsing

- **`encodeURL(fields)`** — Encode a transfer or transaction request into a `solana:` URL.
- **`parseURL(url)`** — Parse a `solana:` URL into its fields.

### QR Codes

- **`createQR(url, size?, background?, color?)`** — Create a QR code from a Solana Pay URL.

### Transfers

- **`createTransfer(rpc, sender, fields)`** — Create transfer `Instruction[]` for a payment.
- **`findReference(rpc, reference, options?)`** — Find a transaction signature by reference address.
- **`validateTransfer(rpc, signature, fields, options?)`** — Validate that a confirmed transaction matches the expected payment.

### Transaction Requests

- **`fetchTransaction(rpc, account, link, options?)`** — Fetch a transaction from a transaction request endpoint.

### Clients

- **`createMerchantClient({ rpcUrl })`** — Merchant client with RPC and merchant plugin. No signer needed.
- **`createWalletClient({ rpcUrl, payer })`** — Wallet client with RPC, payer, transaction planner/executor, and wallet plugin.
- **`createSolanaPayClient({ rpcUrl, payer })`** — Combined client with all merchant + wallet methods.

### Plugins

- **`solanaPayMerchant()`** — Merchant plugin: `encodeURL`, `createQR`, `createQROptions`, `findReference`, `validateTransfer`.
- **`solanaPayWallet()`** — Wallet plugin: `parseURL`, `createTransfer`, `fetchTransaction`.

## How it works

### Web app to mobile wallet

Payment requests can be encoded as a URL according to the scheme, scanned using a QR code, sent and confirmed by the wallet, and discovered by the app.

### Web app to browser wallet

With a Solana Pay button, you could integrate an embeddable payment button that can be added to your existing app.

### Mobile app to mobile wallet

Payment requests could be encoded as a deep link. The app prepares a payment request, and passes control to the wallet. The wallet signs, sends, and confirms it, or cancels the request and passes control back to the app.

## Transaction Requests

A Solana Pay transaction request URL describes an interactive request for any Solana transaction. The parameters in the URL are used by a wallet to make an HTTP request to compose any transaction.

## Transfer Requests

A Solana Pay transfer request URL describes a non-interactive request for a SOL or SPL Token transfer. The parameters in the URL are used by a wallet to directly compose the transaction.

## Migration Guide

### v0.2 → v1.0

**Breaking changes:**

| v0.2 (`@solana/web3.js`) | v1.0 (`@solana/kit`) |
|--------------------------|----------------------|
| `PublicKey` | `Address` (branded string — use `address()` to create, `===` to compare) |
| `Connection` | `Rpc` from `@solana/kit` |
| `BigNumber` (from `bignumber.js`) | `number` — plain JS number for human-readable amounts |
| `createTransfer()` returns `Transaction` | Returns `Instruction[]` — compose with kit's `pipe()` + `createTransactionMessage()` |
| `sender: PublicKey` | `sender: TransactionSigner` |
| `@solana/spl-token` | `@solana-program/token` |
| `Buffer` | `Uint8Array` / `TextEncoder` |

**Typical migration:**

```diff
- import { Connection, PublicKey, Transaction } from '@solana/web3.js';
+ import { address, createSolanaRpc, pipe, createTransactionMessage,
+          setTransactionMessageFeePayer, appendTransactionMessageInstructions,
+          setTransactionMessageLifetimeUsingBlockhash, compileTransaction,
+          signTransaction, getBase64EncodedWireTransaction } from '@solana/kit';

- const connection = new Connection('https://api.devnet.solana.com');
+ const rpc = createSolanaRpc('https://api.devnet.solana.com');

- const recipient = new PublicKey('...');
+ const recipient = address('...');

- const transaction = await createTransfer(connection, sender.publicKey, { recipient, amount });
- transaction.feePayer = sender.publicKey;
- transaction.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
+ const instructions = await createTransfer(rpc, sender, { recipient, amount });
+ // Compose transaction using kit pipe() pattern
```

## Architecture

### URL Encoding & Parsing

`encodeURL(fields)` constructs a `solana:` URL from `TransferFields` or `TransactionRequestFields`. `parseURL(url)` does the reverse. Both are pure functions — no RPC calls, no side effects.

### QR Code Generation

`createQR(url)` uses `@solana/qr-code-styling` to generate a styled QR code. The QR encodes the `solana:` URL so any Solana wallet can scan and pay.

### Transfer Validation

`findReference(rpc, reference)` polls the RPC for a transaction that includes the reference address. `validateTransfer(rpc, signature, fields)` checks that the transaction:
- Matches the expected recipient
- Matches the expected amount (within tolerance)
- Includes the reference address
- Is confirmed on the expected cluster

This is the **merchant-side** flow: generate URL → show QR → wait for payment → validate.

### Transaction Requests

`fetchTransaction(rpc, account, link)` fetches a partially-signed transaction from a merchant server. This is the **interactive** flow: the merchant composes the transaction dynamically (e.g., "buy this NFT for 1.5 SOL") and the wallet signs and submits.

### Spec Compliance

This library is the reference implementation of the [Solana Pay specification](../spec/SPEC.md). The spec defines:
- URL scheme format
- Required and optional fields
- How wallets should parse and display payment requests
- How merchants should generate and validate them

## For Contributors

### Kit v6 Architecture

`@solana/pay` v1.0 is built on `@solana/kit` v6, which uses a **plugin-based client** instead of the monolithic `Connection` from `@solana/web3.js`:

```
createClient()
  .use(solanaRpcConnection({ rpcUrl }))
  .use(payerFromFile('~/.config/solana/id.json'))
  .use(solanaPayMerchant())   // adds encodeURL, createQR, findReference
  .use(solanaPayWallet())     // adds parseURL, createTransfer
```

Plugins add methods to the client object. This lets consumers compose exactly the capabilities they need.

### Platform Artifact Distribution

The `supportedPlatforms` field in `package.json` maps platform triples to downloadable artifacts. When `npx pay` runs:

1. Detect platform (e.g., `x86_64-pc-windows-msvc`)
2. Check cache for existing artifact
3. Download matching `.tar.gz` or `.zip` if missing
4. Extract and delegate to native `pay` binary

Adding a new platform means adding an entry to `supportedPlatforms` and publishing the corresponding artifact in CI.

### Build

```sh
# Development build with watch
pnpm build
pnpm watch

# Tests
pnpm test

# Typecheck
pnpm typecheck

# Lint + format
pnpm lint
pnpm fmt
```

## License

The Solana Pay JavaScript SDK is open source and available under the MIT License. See the [LICENSE](./LICENSE) file for more info.

Subject to the foregoing, the Terms of Service available at solana.com/tos
