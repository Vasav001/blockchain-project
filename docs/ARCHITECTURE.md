# Architecture

## System overview

```
                     ┌─────────────────────────┐
   Browser  ───────► │  nginx (frontend)       │
   (signs            │  - serves static React  │
   transactions      │  - reverse-proxies      │──────►  Rust backend (Axum)
   client-side)      │    /api/* to backend    │         - REST API
                     └─────────────────────────┘         - blockchain domain logic
                                                         - SQLite persistence
```

Two Docker services (`backend`, `frontend`), one named volume for SQLite. The browser
only ever talks to the frontend's origin - nginx forwards `/api/*` and `/health`
server-side to the backend over the Compose network. The backend's port is not
published to the host (`expose`, not `ports`) - only the frontend needs to reach it.

## Backend layering (`server/src/`)

```
main.rs        startup: read config, init DB, seed genesis if needed, build router, serve
config.rs      HOST / PORT / MINING_DIFFICULTY
error.rs       AppError -> HTTP status/JSON mapping
blockchain/    Block, Chain, Transaction, Mempool - pure domain logic
wallet/        Ed25519 keypair generation, address derivation, sign/verify
db/            SQLx pool, hand-rolled migrations, block/transaction repository
api/           Axum routes and handlers
```

- **`blockchain/`** has no Axum or SQLx dependency. It's the core: `Block` (hashing,
  Proof-of-Work), `Chain` (validation, balance replay), `Transaction` (signing bytes,
  id, structural validation), `Mempool` (admission: validity + balance + duplicate
  checks). Testable and reusable without a database or HTTP server.
- **`wallet/`** also has no HTTP/SQL dependency - just Ed25519 keys and SHA-256 address
  derivation, shared by both the domain layer (`Transaction` calls into it to verify
  signatures) and, conceptually, by any client (the frontend's `wallet.ts` is an
  independent TypeScript implementation of the same rules).
- **`db/`** is the only layer that knows about SQLx/SQLite. It translates domain
  structs (`Block`, `Transaction`) to and from row types (`BlockRow`,
  `TransactionRow`) - the domain types never appear in a SQL query directly.
- **`api/`** is a thin adapter: handlers parse requests, call into `blockchain/`/`db/`,
  and translate the result (or an error) into JSON. No business logic lives here.

This split means the blockchain and wallet logic could be reused by a completely
different frontend (a CLI, a different UI) without change, and could be unit-tested
without ever starting an HTTP server or touching a real database (most tests use an
isolated in-memory SQLite database per test).

## Frontend layering (`client/src/`)

```
wallet.ts       the only file that touches cryptography (Ed25519 keygen/sign, SHA-256 address)
api.ts          typed REST client (fetch wrappers + error type)
types.ts        TypeScript interfaces mirroring the backend's JSON responses
components/     React UI - only ever handles already-signed payloads or already-fetched data
App.tsx         top-level state (blocks, mempool, wallet) and data-fetching orchestration
```

`wallet.ts` uses `@noble/ed25519` (a small, audited, pure-TypeScript implementation)
rather than the browser's native Web Crypto Ed25519 support, which is still
inconsistent across browsers. Native `crypto.subtle.digest('SHA-256', ...)` is used for
address derivation, since that part of Web Crypto is universally reliable. A private
key exists only as an in-memory JavaScript value (React state) for the lifetime of the
browser tab - never written to storage, never logged, never included in a request body.

## Data flow: a transaction, start to finish

1. User fills in recipient/amount in the frontend; clicking submit calls
   `wallet.ts`'s `signTransaction`, which builds the exact byte sequence the backend
   will later verify and signs it with the in-memory private key.
2. The signed payload (`sender`, `recipient`, `amount`, `sender_public_key`,
   `signature` - never a private key) is POSTed to `/api/transactions`.
3. The backend reconstructs a `Transaction`, computing its own `id` server-side
   (a client can never supply one), and checks it structurally
   (`Transaction::validation_error`), then admits it into the mempool only if it's not
   a duplicate and the sender can afford it (`Mempool::try_add_transaction`, checking
   confirmed balance minus already-pending amounts from that sender).
4. `POST /api/mine` takes every pending transaction, searches for a `nonce` producing a
   block hash with enough leading zero hex characters (`Block::mine`), and persists the
   block and its transactions together in one SQL transaction
   (`db::insert_block`) - only then are those transactions removed from the mempool.
5. `GET /api/wallets/{address}/balance` and `GET /api/chain/valid` both work by loading
   the full persisted chain and replaying/re-checking it on demand - there is no
   separately-maintained balance table or cached validity flag.

## Persistence

SQLite only - no Postgres, Redis, or other service. Migrations are numbered `.sql`
files, embedded into the compiled binary at build time (`include_str!`) and applied in
order on every startup, tracked in a `schema_migrations` table so re-running is a
no-op. In Docker, the database file lives on a named volume mounted at `/app/data`,
independent of container lifecycle.

## Why no P2P / coinbase / difficulty adjustment

These were deliberately out of scope for this project (see the README's
"Known limitations" and `docs/TESTING.md`) - the goal was to build and demonstrate a
single-node blockchain's core mechanics clearly, not a production-grade decentralized
network.
