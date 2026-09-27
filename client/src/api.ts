/**
 * Typed fetch wrappers for the blockchain REST API. Every function returns
 * a parsed, typed response or throws `ApiRequestError` with the server's
 * own error message (see server/src/error.rs's `AppError`).
 */
import type { Block, BalanceResponse, Transaction, ValidationResponse } from './types'
import type { SignedTransactionPayload } from './wallet'

const BASE = '/api'

export class ApiRequestError extends Error {
  status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = 'ApiRequestError'
    this.status = status
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${BASE}${path}`, {
    headers: { 'content-type': 'application/json' },
    ...init,
  })

  const body: unknown = await response.json().catch(() => null)

  if (!response.ok) {
    const message =
      body && typeof body === 'object' && 'error' in body && typeof body.error === 'string'
        ? body.error
        : `request failed with status ${response.status}`
    throw new ApiRequestError(message, response.status)
  }

  return body as T
}

export function getBlocks(): Promise<Block[]> {
  return request<Block[]>('/blocks')
}

export function getBlock(index: number): Promise<Block> {
  return request<Block>(`/blocks/${index}`)
}

export function getChainValidity(): Promise<ValidationResponse> {
  return request<ValidationResponse>('/chain/valid')
}

export function getPendingTransactions(): Promise<Transaction[]> {
  return request<Transaction[]>('/transactions/pending')
}

export function submitTransaction(payload: SignedTransactionPayload): Promise<Transaction> {
  return request<Transaction>('/transactions', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

export function mine(): Promise<Block> {
  return request<Block>('/mine', { method: 'POST' })
}

export function getBalance(address: string): Promise<BalanceResponse> {
  return request<BalanceResponse>(`/wallets/${encodeURIComponent(address)}/balance`)
}
