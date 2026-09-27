import { useCallback, useEffect, useState } from 'react'
import './App.css'
import { ApiRequestError, getBlocks, getChainValidity, getPendingTransactions } from './api'
import { generateWallet } from './wallet'
import type { WalletKeys } from './wallet'
import type { Block, Transaction } from './types'
import { BlockchainStatus } from './components/BlockchainStatus'
import { MempoolPanel } from './components/MempoolPanel'
import { BalanceLookup } from './components/BalanceLookup'
import { TransactionForm } from './components/TransactionForm'
import { MiningPanel } from './components/MiningPanel'
import { Explorer } from './components/Explorer'

function App() {
  const [blocks, setBlocks] = useState<Block[] | null>(null)
  const [chainValid, setChainValid] = useState<boolean | null>(null)
  const [chainLoading, setChainLoading] = useState(false)
  const [chainError, setChainError] = useState<string | null>(null)

  const [pending, setPending] = useState<Transaction[] | null>(null)
  const [pendingLoading, setPendingLoading] = useState(false)
  const [pendingError, setPendingError] = useState<string | null>(null)

  const [wallet, setWallet] = useState<WalletKeys | null>(null)
  const [generatingWallet, setGeneratingWallet] = useState(false)

  const refreshChain = useCallback(async () => {
    setChainLoading(true)
    setChainError(null)
    try {
      const [blockList, validity] = await Promise.all([getBlocks(), getChainValidity()])
      setBlocks(blockList)
      setChainValid(validity.valid)
    } catch (err) {
      setChainError(err instanceof ApiRequestError ? err.message : 'Failed to load the chain.')
    } finally {
      setChainLoading(false)
    }
  }, [])

  const refreshPending = useCallback(async () => {
    setPendingLoading(true)
    setPendingError(null)
    try {
      setPending(await getPendingTransactions())
    } catch (err) {
      setPendingError(
        err instanceof ApiRequestError ? err.message : 'Failed to load pending transactions.',
      )
    } finally {
      setPendingLoading(false)
    }
  }, [])

  // One-time load on mount - no polling, per this phase's scope. Everything
  // after this refreshes only in response to an explicit user action
  // (Refresh buttons) or a successful mutation (submit/mine).
  useEffect(() => {
    void refreshChain()
    void refreshPending()
  }, [refreshChain, refreshPending])

  async function handleGenerateWallet() {
    setGeneratingWallet(true)
    try {
      setWallet(await generateWallet())
    } finally {
      setGeneratingWallet(false)
    }
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>Blockchain Explorer</h1>
        <p>A lightweight, from-scratch blockchain - Rust backend, signed by this browser.</p>
      </header>

      <main className="dashboard">
        <BlockchainStatus
          blocks={blocks}
          valid={chainValid}
          loading={chainLoading}
          error={chainError}
          onRefresh={() => void refreshChain()}
        />

        <MempoolPanel
          pending={pending}
          loading={pendingLoading}
          error={pendingError}
          onRefresh={() => void refreshPending()}
        />

        <BalanceLookup prefillAddress={wallet?.address} />

        <TransactionForm
          wallet={wallet}
          generatingWallet={generatingWallet}
          onGenerateWallet={() => void handleGenerateWallet()}
          onSubmitted={() => void refreshPending()}
        />

        <MiningPanel
          onMined={() => {
            void refreshChain()
            void refreshPending()
          }}
        />

        <Explorer blocks={blocks} loading={chainLoading} error={chainError} />
      </main>
    </div>
  )
}

export default App
