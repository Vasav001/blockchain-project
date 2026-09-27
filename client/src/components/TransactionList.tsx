import type { Transaction } from '../types'
import { shortHex } from '../format'

/** A list of transactions with all fields visible - shared by the mempool
 * panel and the block explorer's transaction detail view. */
export function TransactionList({ transactions }: { transactions: Transaction[] }) {
  if (transactions.length === 0) {
    return <p className="empty-state">No transactions.</p>
  }

  return (
    <ul className="transaction-list">
      {transactions.map((tx) => (
        <li key={tx.id} className="transaction-row">
          <div className="transaction-row-main">
            <span className="mono">{shortHex(tx.sender)}</span>
            <span aria-hidden="true">&rarr;</span>
            <span className="mono">{shortHex(tx.recipient)}</span>
            <span className="amount">{tx.amount}</span>
          </div>
          <dl className="transaction-row-details">
            <dt>id</dt>
            <dd className="mono">{tx.id}</dd>
            <dt>public key</dt>
            <dd className="mono">{tx.sender_public_key}</dd>
            <dt>signature</dt>
            <dd className="mono">{shortHex(tx.signature, 16, 16)}</dd>
          </dl>
        </li>
      ))}
    </ul>
  )
}
