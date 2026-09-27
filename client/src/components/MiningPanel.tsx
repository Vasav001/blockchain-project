import { useState } from 'react'
import { ApiRequestError, mine } from '../api'
import type { Block } from '../types'
import { shortHex } from '../format'

export function MiningPanel({ onMined }: { onMined: () => void }) {
  const [mining, setMining] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<Block | null>(null)

  async function handleMine() {
    setMining(true)
    setError(null)
    setResult(null)
    try {
      const minedBlock = await mine()
      setResult(minedBlock)
      onMined()
    } catch (err) {
      setError(err instanceof ApiRequestError ? err.message : 'Mining failed.')
    } finally {
      setMining(false)
    }
  }

  return (
    <section className="card">
      <div className="card-header">
        <h2>Mining</h2>
      </div>

      <button type="button" onClick={() => void handleMine()} disabled={mining}>
        {mining ? 'Mining…' : 'Mine pending transactions'}
      </button>

      {error && <p className="error">{error}</p>}

      {result && (
        <dl className="stat-grid">
          <dt>Mined block</dt>
          <dd>#{result.index}</dd>
          <dt>Hash</dt>
          <dd className="mono">{shortHex(result.hash, 16, 16)}</dd>
          <dt>Nonce</dt>
          <dd>{result.nonce}</dd>
          <dt>Transactions included</dt>
          <dd>{result.transactions.length}</dd>
        </dl>
      )}
    </section>
  )
}
