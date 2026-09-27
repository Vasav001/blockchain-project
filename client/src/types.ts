/**
 * Types mirroring the server's JSON responses (server/src/blockchain and
 * server/src/api/handlers.rs). `u64`/`i64` fields come across as plain JSON
 * numbers - fine for this project's scale, but note JS numbers only carry
 * full precision up to 2^53.
 */

export interface Transaction {
  sender: string
  recipient: string
  amount: number
  sender_public_key: string
  signature: string
  id: string
}

export interface Block {
  index: number
  timestamp: number
  data: string
  previous_hash: string
  transactions: Transaction[]
  nonce: number
  hash: string
}

export interface ValidationResponse {
  valid: boolean
}

export interface BalanceResponse {
  address: string
  balance: number
}
