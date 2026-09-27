/**
 * Client-side Ed25519 wallet: keypair generation, address derivation, and
 * transaction signing. Kept isolated from React components on purpose -
 * this is the one place in the frontend that touches cryptography.
 *
 * Uses `@noble/ed25519`, a small, audited, pure-TS Ed25519 implementation
 * (the JS-side equivalent of the server's `ed25519-dalek`), rather than
 * the browser's native Web Crypto Ed25519 support - that support is still
 * inconsistent across browsers (solid on recent Chrome/Safari, historically
 * unreliable on Firefox), which would make this demo flaky depending on
 * which browser it's run in. We do use native Web Crypto for SHA-256
 * (`crypto.subtle.digest`), which is universally well-supported - only the
 * signature scheme itself needed a portable alternative.
 *
 * The secret key never leaves this module except as a return value held in
 * memory (React state). It is never written to localStorage, never logged,
 * and never sent to the server - only the derived public key, address, and
 * signatures are.
 */
import * as ed from '@noble/ed25519'

export interface WalletKeys {
  /** 32-byte Ed25519 secret key. In-memory only - see module doc comment. */
  secretKey: Uint8Array
  /** Hex-encoded 32-byte Ed25519 public key. Safe to share. */
  publicKeyHex: string
  /** SHA-256 of the raw public key bytes, hex-encoded - this wallet's address. */
  address: string
}

export interface SignedTransactionPayload {
  sender: string
  recipient: string
  amount: number
  sender_public_key: string
  signature: string
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  // Re-wrapped in a plain ArrayBuffer-backed Uint8Array: @noble's return
  // types are generic over ArrayBufferLike (which includes
  // SharedArrayBuffer), while SubtleCrypto's types require a concrete
  // ArrayBuffer - this satisfies that without changing any bytes.
  const buffer: ArrayBuffer = new Uint8Array(bytes).buffer
  const digest = await crypto.subtle.digest('SHA-256', buffer)
  return ed.etc.bytesToHex(new Uint8Array(digest))
}

/**
 * Big-endian 8-byte representation of an integer amount, matching the
 * server's `amount.to_be_bytes()` for an `i64`
 * (server/src/blockchain/transaction.rs).
 */
function amountToBeBytes(amount: number): Uint8Array {
  const view = new DataView(new ArrayBuffer(8))
  view.setBigInt64(0, BigInt(amount), false)
  return new Uint8Array(view.buffer)
}

/**
 * Generates a fresh Ed25519 keypair and derives this project's wallet
 * address from it - exactly `wallet::address_from_public_key` on the
 * server (server/src/wallet/mod.rs): SHA-256 of the raw public key bytes,
 * hex-encoded.
 */
export async function generateWallet(): Promise<WalletKeys> {
  const { secretKey, publicKey } = await ed.keygenAsync()
  const publicKeyHex = ed.etc.bytesToHex(publicKey)
  const address = await sha256Hex(publicKey)
  return { secretKey, publicKeyHex, address }
}

/**
 * Signs a transfer with `wallet` and returns the exact JSON body
 * `POST /api/transactions` expects.
 *
 * The signed bytes must match the server's
 * `Transaction::signing_bytes`/`compute_signing_bytes` byte-for-byte:
 * sender ++ recipient ++ amount.to_be_bytes() ++ sender_public_key, where
 * the string fields are their raw UTF-8 bytes (note: `sender_public_key`
 * is the *hex string's* UTF-8 bytes, not the raw 32 public key bytes) - or
 * the server's signature check will simply fail.
 */
export async function signTransaction(
  wallet: WalletKeys,
  recipient: string,
  amount: number,
): Promise<SignedTransactionPayload> {
  const encoder = new TextEncoder()
  const message = ed.etc.concatBytes(
    encoder.encode(wallet.address),
    encoder.encode(recipient),
    amountToBeBytes(amount),
    encoder.encode(wallet.publicKeyHex),
  )

  const signature = await ed.signAsync(message, wallet.secretKey)

  return {
    sender: wallet.address,
    recipient,
    amount,
    sender_public_key: wallet.publicKeyHex,
    signature: ed.etc.bytesToHex(signature),
  }
}
