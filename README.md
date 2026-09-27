# blockchain-project

A lightweight blockchain built from scratch in Rust, for learning core blockchain
concepts (blocks, hashing, chain validation, transactions, wallets, Proof-of-Work,
persistence) without relying on an existing blockchain framework. A React/TypeScript
dashboard runs on top of it, signing transactions in the browser.

## 1. Overview

This is a single-node, educational blockchain: a Rust/Axum backend that implements
blocks, SHA-256 hashing, chain validation, Ed25519-signed transactions, a mempool,
Proof-of-Work mining, and account-based wallet balances, persisted to SQLite - plus a
React frontend that talks to it as a real client would, generating its own keypairs and
signing transactions locally.

## 2. Features

- Blocks linked by SHA-256 hash, with tamper-detecting chain validation
- Proof-of-Work mining with a configurable leading-zero-hex difficulty
- Ed25519-signed, account-based transactions and an in-memory mempool
- Wallet balances derived by replaying confirmed transactions (no UTXO set, no ledger table)
- SQLite persistence with hand-rolled, numbered migrations
- A REST API over all of the above
- A React + TypeScript dashboard/explorer, signing transactions with a real
  client-side Ed25519 wallet (`@noble/ed25519`) - private keys never leave the browser
- Docker images and a Compose file for reproducible deployment

**Deliberately not implemented** (see [§18](#18-known-limitations)): P2P networking,
mining rewards/coinbase transactions, difficulty adjustment, authentication, WebSockets.

## 3. Architecture

```
                     ┌─────────────────────────┐
   Browser  ───────► │  nginx (frontend)       │
   (signs            │  - serves static React  │
   transactions      │  - reverse-proxies      │──────►  Rust backend (Axum)
   client-side)      │    /api/* to backend    │         - REST API
                     └─────────────────────────┘         - blockchain domain logic
                                                         - SQLite persistence
```

The backend is layered:
- `blockchain/` — core domain logic (blocks, chain, transactions, mempool, mining,
  balances). No HTTP or SQL dependencies; testable and reusable in isolation.
- `wallet/` — Ed25519 keypair generation, address derivation, signing/verification.
  Also has no HTTP/SQL dependency.
- `api/` — Axum routes/handlers, a thin adapter over `blockchain/` and `db/`.
- `db/` — SQLx pool setup, hand-rolled migrations, and repository functions
  translating domain structs to/from SQLite rows.

The frontend mirrors this separation: `client/src/wallet.ts` is the only file that
touches cryptography; `client/src/api.ts` is a typed REST client; React components only
ever see already-signed payloads or already-fetched data.

## 4. Technology stack

| Layer | Technology |
|---|---|
| Backend | Rust, Axum, Tokio, SQLx (SQLite), Serde, `sha2`, `ed25519-dalek`, `thiserror`, `tracing` |
| Frontend | React 19, TypeScript, Vite, `@noble/ed25519` |
| Persistence | SQLite (file-based, no separate DB server) |
| Deployment | Docker (multi-stage builds), Docker Compose, nginx (frontend static + reverse proxy) |

## 5. Repository structure

```
blockchain-project/
├── docker-compose.yml
├── server/                      Rust backend
│   ├── Dockerfile
│   ├── src/
│   │   ├── main.rs              startup: config, DB init, genesis seed, router, serve
│   │   ├── config.rs            HOST / PORT / MINING_DIFFICULTY
│   │   ├── error.rs             AppError -> HTTP response mapping
│   │   ├── blockchain/          Block, Chain, Transaction, Mempool (pure domain logic)
│   │   ├── wallet/               Ed25519 keypair/address/sign/verify
│   │   ├── db/                  SQLx pool, migrations, block/transaction repository
│   │   └── api/                 Axum routes and handlers
│   └── migrations/              numbered .sql files, embedded into the binary
└── client/                      React + Vite + TypeScript frontend
    ├── Dockerfile
    ├── nginx.conf
    └── src/
        ├── wallet.ts             isolated Ed25519 signing module
        ├── api.ts                typed REST client
        ├── types.ts              types mirroring the backend's JSON
        └── components/           dashboard/explorer UI
```

## 6. Running the project (Docker - recommended)

This is the primary, recommended way to run the project. **You do not need Rust,
Node.js, npm, SQLite, or any other development dependency installed on your machine -
only Docker.** Two services (`backend`, `frontend`) and one named volume for SQLite
persistence. The frontend's nginx reverse-proxies `/api/*` to the backend container
over the Compose network - the browser only ever talks to the frontend's origin, never
to a Docker-internal hostname.

```bash
docker compose up --build
```

Then open **http://localhost:3000**. Stop with `Ctrl+C`, or `docker compose down` (the
named volume, and therefore the blockchain data, survives `down`/`up` - only
`docker compose down -v` removes it and starts genesis-only again).

## 7. Development (running backend/frontend separately)

Useful while editing the code; Docker remains the easiest/recommended way to just run
the finished application. Requires Rust (stable, via `cargo`) and Node.js + npm.

```bash
# Terminal 1 - backend, listens on http://127.0.0.1:8080 by default
cd server
cargo run

# Terminal 2 - frontend, listens on http://127.0.0.1:5173
cd client
npm install
npm run dev
```

The Vite dev server proxies `/api/*` to the backend (see `client/vite.config.ts`), so
the browser only ever talks to `http://127.0.0.1:5173`.

## 8. Exact Docker commands

```bash
docker compose up --build      # build and start both services in the foreground
docker compose up --build -d   # same, detached
docker compose down            # stop and remove containers - data survives
docker compose down -v         # stop and remove containers AND the data volume
```

The dev-only funding endpoint (`POST /api/dev/fund` - see §11) is already enabled in
`docker-compose.yml` (`DEV_FUNDING_ENABLED: "true"` on the `backend` service) - remove
that line for a deployment where it shouldn't be available. For local (non-Docker)
development:
```bash
DEV_FUNDING_ENABLED=true cargo run    # from server/
```
Example request:
```bash
curl -X POST http://localhost:3000/api/dev/fund \
  -H "content-type: application/json" \
  -d '{"address": "<wallet-address>", "amount": 100}'
```

## 9. Frontend URL

**http://localhost:3000**

## 10. Backend health URL

Directly: **http://localhost:8080/health** is not published to the host by default
(see `docker-compose.yml` - the backend only `expose`s its port to other containers,
matching "backend exposes only what is actually necessary"). Through the frontend's
proxy: **http://localhost:3000/health**. In local (non-Docker) development the backend
does publish directly: `http://127.0.0.1:8080/health`.

## 11. API endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/health` | Liveness check, returns `200 OK` |
| GET | `/api/blocks` | All blocks, in chain order, with their transactions |
| GET | `/api/blocks/{index}` | A single block by index, or `404` |
| GET | `/api/chain/valid` | `{"valid": bool}` - hash/link/PoW validity of the persisted chain |
| POST | `/api/transactions` | Submit an already-signed transaction; `400` on invalid/unfunded/duplicate |
| GET | `/api/transactions/pending` | Every transaction currently in the mempool |
| POST | `/api/mine` | Mine all pending transactions into a new block; `400` if the mempool is empty |
| GET | `/api/wallets/{address}/balance` | `{"address": ..., "balance": ...}`, confirmed balance only |
| POST | `/api/dev/fund` | **Dev/demo only**, requires `DEV_FUNDING_ENABLED=true` or `404`. Mines a real starting balance to an address. See `docs/API.md`. |

## 12. Blockchain flow

1. **Genesis**: a fixed, deterministic block (`index 0`, `nonce 0`, no transactions) is
   seeded into SQLite on first startup only.
2. **Transactions** are created and signed client-side, then submitted via
   `POST /api/transactions`. The mempool admits them only if they're structurally
   valid, not a duplicate, and affordable (see [§14](#14-balance-calculation)).
3. **Mining** (`POST /api/mine`) takes every pending transaction, searches for a nonce
   satisfying the configured Proof-of-Work difficulty, and persists the resulting block
   atomically (block + its transactions, in one SQL transaction) - only after
   persistence succeeds are those transactions cleared from the mempool.
4. **Validation** (`GET /api/chain/valid`) replays the whole persisted chain, checking
   every block's hash, its link to the previous block, and its Proof-of-Work.

## 13. Transaction / signature flow

A transaction moves `amount` from `sender` to `recipient`, authorized by `sender`'s
Ed25519 signature. The signed bytes are:

```
sender (UTF-8) ++ recipient (UTF-8) ++ amount (i64, big-endian) ++ sender_public_key (hex string, UTF-8)
```

computed identically on both sides (`server/src/blockchain/transaction.rs`'s
`compute_signing_bytes`, `client/src/wallet.ts`'s `signTransaction`). A wallet's
**address** is `SHA-256(raw public key bytes)`, hex-encoded (`server/src/wallet/mod.rs`,
`client/src/wallet.ts`). The transaction **id** is a SHA-256 over the fully-signed
transaction (including the signature itself), computed and assigned only by the
server - a client can never supply its own id.

The frontend's wallet module (`client/src/wallet.ts`) is the sole place private keys
exist: generated with `@noble/ed25519`, held only in React state (in-memory,
per-browser-tab), never written to `localStorage`/`sessionStorage`, never logged, and
never sent in any request body. The server never generates, stores, receives, or
returns a private key - there is no such endpoint.

## 14. Mining / Proof-of-Work

A block is mined by repeatedly incrementing a `nonce` and recomputing the block's
SHA-256 hash (which covers the block's index, timestamp, data, previous hash, each
included transaction's id, and the nonce itself) until the hash has at least
`MINING_DIFFICULTY` leading hex-zero characters (default `4`; `server/src/config.rs`).
This is a synchronous, single-threaded search - no worker pool, no async mining tasks.
Genesis is exempt from Proof-of-Work (it's a fixed, deterministic special case, never
mined). Difficulty is a fixed constant; there is no dynamic adjustment.

## 15. Balance calculation

Wallet balances are **not** stored anywhere - they're computed on demand by replaying
every confirmed transaction in every block, in chain order (`Chain::balance_of`): each
transaction decreases the sender's running total and increases the recipient's. A
wallet that has never appeared in a confirmed transaction has balance `0`. There is no
mining reward and no genesis allocation, so **every wallet starts at zero and can only
ever spend what it has actually received** - see [§18](#18-known-limitations) for what
this means for testing.

Before a transaction is admitted into the mempool, its sender's *spendable* balance is
checked as `confirmed_balance - already_pending_from_this_sender` - this prevents two
pending transactions from together overspending a balance that either one alone could
afford (`Mempool::try_add_transaction`). A pending-but-unmined transaction never affects
`GET /api/wallets/{address}/balance`, which only ever reads confirmed (persisted) state.

## 16. Persistence behavior

SQLite is the only datastore - no Postgres, no Redis, no separate database service.
Data lives at whatever `DATABASE_URL` resolves to (default: `data/blockchain.db`,
relative to the backend process's working directory). In Docker, this file lives on a
named volume (`blockchain-data`, mounted at `/app/data`) that survives container
recreation - `docker compose down && docker compose up` keeps all blocks, transactions,
and balances; only `docker compose down -v` discards it. Migrations are numbered `.sql`
files embedded into the compiled binary and applied automatically, in order, on every
startup (tracked in a `schema_migrations` table, so already-applied migrations are
skipped).

## 17. Testing

```bash
# Backend - full test suite (unit + integration, in-process, isolated in-memory DBs)
cd server
cargo test

# Frontend - typecheck + production build
cd client
npm install
npm run build
```

Beyond the automated suite, this project has been verified with real browser
automation against the actual Docker deployment: a generated in-browser wallet
signing a real transaction, submission, mining, balance updates, the explorer, error
handling (insufficient balance, empty-mempool mining), a mobile viewport, and a full
container-removal-and-recreation persistence check - all through the running UI, not
simulated. See [`docs/TESTING.md`](docs/TESTING.md) for the full breakdown of what was
automated-tested vs. manually/browser-verified.

## 18. Known limitations

- **Single-node only.** There is no P2P networking, no peer discovery, no consensus
  between nodes - this is one server with one chain. P2P was intentionally scoped out.
- **No mining rewards / coinbase transactions.** Mining persists a block but does not
  credit the miner anything.
- **No genesis allocation.** Combined with the above, no wallet ever has a starting
  balance through normal use - a freshly generated wallet cannot submit its first
  transaction, because it has nothing to spend. For local testing/demos, an optional
  **development-only** endpoint, `POST /api/dev/fund`, mints a starting balance by
  mining a real funding block (see §11's API table and `docs/API.md`). It is not part
  of the normal application surface: it only exists when the server is started with
  `DEV_FUNDING_ENABLED=true`, and is otherwise not registered at all (a plain `404`,
  same as any unknown path) - a normal deployment never has it. It is not a faucet in
  the "public tap" sense; it's a demo/testing convenience, off by default.
- **No difficulty adjustment.** `MINING_DIFFICULTY` is a fixed constant.
- **No authentication, no WebSockets.** The API is unauthenticated (anyone who can
  reach it can submit transactions or trigger mining), and all data transfer is
  request/response - the frontend does one-time loads plus refreshes after mutations,
  not live push updates.
- **Not production-grade decentralization or security.** This project demonstrates
  blockchain mechanics for learning purposes; it is not a hardened, decentralized,
  economically-sound cryptocurrency implementation.
