# API Reference

Base URL: `http://localhost:3000/api` (through the Docker frontend's nginx proxy) or
`http://127.0.0.1:8080/api` (backend directly, local development only). All request and
response bodies are JSON. Errors are always `{"error": "<message>"}` with an
appropriate non-2xx status code.

## `GET /health`

Not under `/api` - a plain liveness check.

**Response:** `200 OK`, body `OK` (plain text, not JSON).

## `GET /api/blocks`

All blocks, in chain order (index ascending), each with its full transaction list.

**Response:** `200 OK`
```json
[
  {
    "index": 0,
    "timestamp": 0,
    "data": "genesis block",
    "previous_hash": "000...000",
    "transactions": [],
    "nonce": 0,
    "hash": "3d2d62cc..."
  }
]
```

## `GET /api/blocks/{index}`

A single block by index.

**Response:** `200 OK` (same shape as one array entry above), or `404` with
`{"error": "block <index> not found"}`.

## `GET /api/chain/valid`

Reloads the full chain from storage and validates every block's hash, its link to the
previous block, and its Proof-of-Work (genesis is exempt from PoW). Does **not**
re-verify transaction signatures - that's checked once, at admission time.

**Response:** `200 OK`
```json
{ "valid": true }
```

## `POST /api/transactions`

Submits an already-signed transaction. The server never signs anything - a real client
signs locally and sends the result here.

**Request body:**
```json
{
  "sender": "<hex address>",
  "recipient": "<address string>",
  "amount": 10,
  "sender_public_key": "<hex Ed25519 public key>",
  "signature": "<hex Ed25519 signature>"
}
```
(No `id` field - the server always computes it.)

**Response:** `200 OK` with the accepted transaction, including its server-assigned
`id`. On failure, `400` with one of:
- `"invalid transaction: <reason>"` - a structural rule failed (non-positive amount,
  empty sender/recipient, sender == recipient, sender/public-key mismatch, or the
  signature doesn't verify)
- `"insufficient balance"` - `amount` exceeds the sender's confirmed balance minus
  whatever else that sender already has pending
- `"duplicate transaction"` - a transaction with the same id is already pending

## `GET /api/transactions/pending`

Every transaction currently in the mempool, in the order they were accepted.

**Response:** `200 OK`, an array of transaction objects (same shape as in
`POST /api/transactions`'s response).

## `POST /api/mine`

Mines every currently pending transaction into a new block, persists it, and clears
exactly those transactions from the mempool. Takes no request body - no private key, no
transaction payload; it only ever acts on what's already in the mempool.

**Response:** `200 OK` with the newly mined block (same shape as a `GET /api/blocks`
entry). If the mempool is empty: `400` with `{"error": "no pending transactions to mine"}`
rather than mining an empty block.

## `GET /api/wallets/{address}/balance`

`address`'s confirmed balance, replayed from the persisted chain only - a
pending-but-unmined transaction never affects this.

**Response:** `200 OK`
```json
{ "address": "...", "balance": 100 }
```
Any string is accepted as `address`; one that has never appeared in a confirmed
transaction simply returns `"balance": 0` (never a `404`).

## `POST /api/dev/fund` (development/demo only)

**Not part of the normal application.** Only exists when the server is started with
`DEV_FUNDING_ENABLED=true` - otherwise this route isn't registered at all, so a request
to it gets a plain `404` (identical to any other unknown path, not a special "disabled"
response). Exists because there's no coinbase/reward mechanism, so no wallet can
otherwise ever acquire a starting balance (see the README's "Known limitations").

Mints `amount` to `address` from a one-off keypair generated on the spot, using the
same building blocks as the real transaction/mining paths: `Transaction::signed_by`
(real signature, real structural validation via `Transaction::validation_error` - the
same checks `POST /api/transactions` runs), `Block::mine` (the resulting block
satisfies the same Proof-of-Work every other block does), and `db::insert_block` (the
same atomic block+transactions persistence `POST /api/mine` uses).

**Request body:**
```json
{ "address": "<address>", "amount": 100 }
```

**Response:** `200 OK` with the newly mined funding block (same shape as a
`GET /api/blocks` entry). `400` with `{"error": "invalid transaction: <reason>"}` for a
non-positive amount or empty address (the same structural rules as
`POST /api/transactions`). `404` if the endpoint isn't enabled at all.

## Concurrency notes

`POST /api/transactions` and `POST /api/mine` are each internally serialized against
concurrent requests of their own kind (a dedicated lock per operation) - two
simultaneous submissions from the same sender can't both pass a balance check computed
against the same stale state, and two simultaneous mining requests can't both mine the
same pending transactions into two different blocks. Reads and the other operation kind
are not blocked while one is in progress.
