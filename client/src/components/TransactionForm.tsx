import { useState, type FormEvent } from 'react'
import { ApiRequestError, submitTransaction } from '../api'
import { signTransaction } from '../wallet'
import type { WalletKeys } from '../wallet'
import type { Transaction } from '../types'

export function TransactionForm({
  wallet,
  generatingWallet,
  onGenerateWallet,
  onSubmitted,
}: {
  wallet: WalletKeys | null
  generatingWallet: boolean
  onGenerateWallet: () => void
  onSubmitted: () => void
}) {
  const [recipient, setRecipient] = useState('')
  const [amount, setAmount] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [lastSubmitted, setLastSubmitted] = useState<Transaction | null>(null)

  async function handleSubmit(event: FormEvent) {
    event.preventDefault()
    if (!wallet) return

    const parsedAmount = Number(amount)
    if (!Number.isInteger(parsedAmount) || parsedAmount <= 0) {
      setError('Amount must be a positive whole number.')
      return
    }

    setSubmitting(true)
    setError(null)
    setLastSubmitted(null)
    try {
      const payload = await signTransaction(wallet, recipient, parsedAmount)
      const accepted = await submitTransaction(payload)
      setLastSubmitted(accepted)
      setRecipient('')
      setAmount('')
      onSubmitted()
    } catch (err) {
      setError(err instanceof ApiRequestError ? err.message : 'Failed to submit transaction.')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <section className="card">
      <div className="card-header">
        <h2>Send transaction</h2>
        <button type="button" onClick={onGenerateWallet} disabled={generatingWallet}>
          {generatingWallet ? 'Generating…' : wallet ? 'New wallet' : 'Generate wallet'}
        </button>
      </div>

      {wallet ? (
        <dl className="stat-grid">
          <dt>Your address</dt>
          <dd className="mono">{wallet.address}</dd>
          <dt>Public key</dt>
          <dd className="mono">{wallet.publicKeyHex}</dd>
        </dl>
      ) : (
        <p className="empty-state">
          Generate a wallet to sign and send transactions. The private key stays in this
          browser tab's memory only - it is never sent to the server or saved anywhere.
        </p>
      )}

      <form className="stacked-form" onSubmit={handleSubmit}>
        <label>
          Recipient address
          <input
            className="mono"
            type="text"
            value={recipient}
            onChange={(event) => setRecipient(event.target.value)}
            required
            disabled={!wallet}
          />
        </label>
        <label>
          Amount
          <input
            type="number"
            min={1}
            step={1}
            value={amount}
            onChange={(event) => setAmount(event.target.value)}
            required
            disabled={!wallet}
          />
        </label>
        <button type="submit" disabled={!wallet || submitting}>
          {submitting ? 'Signing & submitting…' : 'Sign & submit'}
        </button>
      </form>

      {error && <p className="error">{error}</p>}
      {lastSubmitted && (
        <p className="success">
          Submitted transaction <span className="mono">{lastSubmitted.id}</span> - now pending.
        </p>
      )}
    </section>
  )
}
