import { useState } from 'react'
import { ApiRequestError, getBalance } from '../api'

export function BalanceLookup({ prefillAddress }: { prefillAddress?: string }) {
  const [address, setAddress] = useState('')
  const [balance, setBalance] = useState<number | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function lookup(addressToLookup: string) {
    if (!addressToLookup) return
    setLoading(true)
    setError(null)
    try {
      const result = await getBalance(addressToLookup)
      setBalance(result.balance)
    } catch (err) {
      setBalance(null)
      setError(err instanceof ApiRequestError ? err.message : 'Failed to look up balance.')
    } finally {
      setLoading(false)
    }
  }

  return (
    <section className="card">
      <div className="card-header">
        <h2>Wallet balance</h2>
      </div>

      <form
        className="inline-form"
        onSubmit={(event) => {
          event.preventDefault()
          void lookup(address)
        }}
      >
        <input
          className="mono"
          type="text"
          placeholder="wallet address"
          value={address}
          onChange={(event) => setAddress(event.target.value)}
        />
        <button type="submit" disabled={loading || !address}>
          {loading ? 'Checking…' : 'Check balance'}
        </button>
        {prefillAddress && (
          <button
            type="button"
            className="secondary"
            onClick={() => {
              setAddress(prefillAddress)
              void lookup(prefillAddress)
            }}
          >
            Use my wallet
          </button>
        )}
      </form>

      {error && <p className="error">{error}</p>}
      {!error && balance !== null && (
        <p className="stat-line">
          Confirmed balance: <strong>{balance}</strong>
        </p>
      )}
    </section>
  )
}
