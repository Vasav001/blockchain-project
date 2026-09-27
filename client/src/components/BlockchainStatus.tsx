import type { Block } from '../types'
import { MINING_DIFFICULTY } from '../config'
import { shortHex } from '../format'

export function BlockchainStatus({
  blocks,
  valid,
  loading,
  error,
  onRefresh,
}: {
  blocks: Block[] | null
  valid: boolean | null
  loading: boolean
  error: string | null
  onRefresh: () => void
}) {
  const latest = blocks && blocks.length > 0 ? blocks[blocks.length - 1] : null

  return (
    <section className="card">
      <div className="card-header">
        <h2>Blockchain</h2>
        <button type="button" onClick={onRefresh} disabled={loading}>
          {loading ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>

      {error && <p className="error">{error}</p>}

      {!error && (
        <dl className="stat-grid">
          <dt>Validity</dt>
          <dd>
            {valid === null ? (
              '—'
            ) : valid ? (
              <span className="badge badge-ok">valid</span>
            ) : (
              <span className="badge badge-bad">invalid</span>
            )}
          </dd>

          <dt>Block count</dt>
          <dd>{blocks ? blocks.length : '—'}</dd>

          <dt>Mining difficulty</dt>
          <dd>{MINING_DIFFICULTY} leading hex zeros</dd>

          <dt>Latest block</dt>
          <dd>
            {latest ? (
              <>
                #{latest.index} <span className="mono">{shortHex(latest.hash)}</span>
              </>
            ) : (
              '—'
            )}
          </dd>
        </dl>
      )}
    </section>
  )
}
