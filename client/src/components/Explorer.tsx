import { useState } from 'react'
import type { Block } from '../types'
import { formatTimestamp, shortHex } from '../format'
import { TransactionList } from './TransactionList'

export function Explorer({
  blocks,
  loading,
  error,
}: {
  blocks: Block[] | null
  loading: boolean
  error: string | null
}) {
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null)

  const selectedBlock = blocks?.find((block) => block.index === selectedIndex) ?? null

  return (
    <section className="card card-wide">
      <div className="card-header">
        <h2>Explorer</h2>
      </div>

      {error && <p className="error">{error}</p>}
      {loading && <p className="empty-state">Loading blocks…</p>}

      {!error && !loading && blocks && blocks.length === 0 && (
        <p className="empty-state">No blocks yet.</p>
      )}

      {!error && blocks && blocks.length > 0 && (
        <div className="explorer-layout">
          <ul className="block-list">
            {[...blocks].reverse().map((block) => (
              <li key={block.index}>
                <button
                  type="button"
                  className={`block-list-item${block.index === selectedIndex ? ' selected' : ''}`}
                  onClick={() => setSelectedIndex(block.index)}
                >
                  <span className="block-index">#{block.index}</span>
                  <span className="mono">{shortHex(block.hash)}</span>
                  <span className="tx-count">{block.transactions.length} tx</span>
                </button>
              </li>
            ))}
          </ul>

          <div className="block-detail">
            {selectedBlock ? (
              <>
                <dl className="stat-grid">
                  <dt>Index</dt>
                  <dd>{selectedBlock.index}</dd>
                  <dt>Timestamp</dt>
                  <dd>{formatTimestamp(selectedBlock.timestamp)}</dd>
                  <dt>Previous hash</dt>
                  <dd className="mono">{selectedBlock.previous_hash}</dd>
                  <dt>Hash</dt>
                  <dd className="mono">{selectedBlock.hash}</dd>
                  <dt>Nonce</dt>
                  <dd>{selectedBlock.nonce}</dd>
                  <dt>Transaction count</dt>
                  <dd>{selectedBlock.transactions.length}</dd>
                </dl>
                <h3>Transactions</h3>
                <TransactionList transactions={selectedBlock.transactions} />
              </>
            ) : (
              <p className="empty-state">Select a block to inspect its transactions.</p>
            )}
          </div>
        </div>
      )}
    </section>
  )
}
