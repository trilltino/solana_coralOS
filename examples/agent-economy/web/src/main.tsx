import { Buffer } from 'buffer'
// @solana/web3.js v1 uses Node's Buffer; provide it in the browser before anything imports web3.
;(globalThis as unknown as { Buffer: typeof Buffer }).Buffer = Buffer

import React, { useMemo } from 'react'
import ReactDOM from 'react-dom/client'
import { ConnectionProvider, WalletProvider } from '@solana/wallet-adapter-react'
import { WalletModalProvider } from '@solana/wallet-adapter-react-ui'
import { PhantomWalletAdapter, SolflareWalletAdapter } from '@solana/wallet-adapter-wallets'
import App from './App'
import '@solana/wallet-adapter-react-ui/styles.css'
import './styles.css'

const ENDPOINT = import.meta.env.VITE_RPC_URL ?? 'https://api.devnet.solana.com'

function Root() {
  const wallets = useMemo(() => [new PhantomWalletAdapter(), new SolflareWalletAdapter()], [])
  return (
    <ConnectionProvider endpoint={ENDPOINT}>
      <WalletProvider wallets={wallets} autoConnect>
        <WalletModalProvider>
          <App />
        </WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  )
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
)
