# blockchain-project

A lightweight blockchain built from scratch in Rust, for learning core blockchain
concepts (blocks, hashing, chain validation, transactions, wallets, proof-of-work,
persistence) without relying on an existing blockchain framework.

## Structure

- `server/` — Rust backend (Axum + Tokio + SQLx/SQLite). Owns all blockchain logic,
  persistence, and the REST API.
- `client/` — React + Vite + TypeScript dashboard/explorer for the chain.

The server is split into:
- `blockchain/` — core domain logic (blocks, chain, transactions, mining). No HTTP or
  SQL dependencies, so it's testable on its own and reusable if a P2P layer is added later.
- `api/` — Axum routes/handlers, a thin adapter over `blockchain/`.
- `db/` — SQLx pool setup and repository functions, translating domain structs to/from
  SQLite rows.
- `wallet/` — keypair generation, address derivation, transaction signing (Ed25519).

## Development

Requires: Rust (stable, via `cargo`), Node.js + npm.

Run the backend and frontend in separate terminals:

```bash
# Terminal 1 — backend, listens on http://127.0.0.1:8080
cd server
cargo run

# Terminal 2 — frontend, listens on http://127.0.0.1:5173
cd client
npm run dev
```

The Vite dev server proxies `/api/*` requests to the backend (see `client/vite.config.ts`),
so the browser only ever talks to `http://127.0.0.1:5173`.

## Data

SQLite data lives at `server/data/blockchain.db` (created on first run, gitignored).
The database location can be overridden with the `DATABASE_URL` environment variable.

## Status

Project scaffolding only — no blockchain logic implemented yet. See the implementation
plan for the phased build-out (genesis block → chain validation → persistence → REST API
→ transactions → wallets/signatures → proof-of-work → balances → explorer UI).
