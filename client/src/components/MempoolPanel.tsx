import type { Transaction } from '../types'
import { TransactionList } from './TransactionList'

export function MempoolPanel({
  pending,
  loading,
  error,
  onRefresh,
}: {
  pending: Transaction[] | null
  loading: boolean
  error: string | null
  onRefresh: () => void
}) {
  return (
    <section className="card">
      <div className="card-header">
        <h2>Mempool</h2>
        <button type="button" onClick={onRefresh} disabled={loading}>
          {loading ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>

      {error && <p className="error">{error}</p>}

      {!error && (
        <>
          <p className="stat-line">
            <strong>{pending ? pending.length : '—'}</strong> pending transaction
            {pending?.length === 1 ? '' : 's'}
          </p>
          {pending && <TransactionList transactions={pending} />}
        </>
      )}
    </section>
  )
}
