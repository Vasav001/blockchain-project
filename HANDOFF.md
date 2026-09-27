# Project Handoff

## Status

The blockchain application is **complete and ready for demonstration/submission**. It
has passed final acceptance testing (automated tests, real browser verification, and
Docker deployment/persistence verification).

## Stack

Rust, Axum, Tokio, SQLite/SQLx, SHA-256, Ed25519, React, Vite, TypeScript, nginx,
Docker Compose.

## Architecture

Browser → nginx (frontend container: serves the built React app, reverse-proxies
`/api/*` and `/health` to the backend) → Rust/Axum backend → SQLite (on a named Docker
volume). The backend's `blockchain/` (blocks, chain, transactions, mempool, mining,
balances) and `wallet/` (Ed25519 keys/signing) modules have no Axum or SQLx dependency -
they're pure domain logic, testable in isolation; `api/` and `db/` are thin adapters
around them. The frontend mirrors this: `client/src/wallet.ts` is the only file that
touches cryptography (real client-side Ed25519 signing via `@noble/ed25519`); private
keys never leave the browser tab.

## Implemented Features

- Blocks linked by SHA-256 hash, with chain validation and tamper detection
- Proof-of-Work mining with a configurable leading-zero-hex difficulty
- Ed25519-signed, account-based transactions and an in-memory mempool (with
  duplicate/overspend prevention)
- Wallet balances computed by replaying confirmed transactions (no stored ledger)
- SQLite persistence with hand-rolled, numbered migrations
- A REST API over all of the above
- A React + TypeScript dashboard/explorer, signing transactions client-side
- Docker images (multi-stage) and Docker Compose for reproducible deployment

## Deliberate Scope / Non-Features

- No P2P networking (single-node only)
- No authentication
- No mining rewards / coinbase transactions
- No difficulty adjustment (fixed constant)
- No WebSockets (request/response only, manual refresh after mutations)
- A development-only funding mechanism (direct backend-side block insertion) is used
  for test/demo setup, since there's no coinbase/faucet - documented, not an API
  endpoint

## Verification

- 100/100 backend tests passed
- Real Playwright/Chromium browser testing passed
- All 8 API endpoints tested through the live deployment
- Ed25519 signing passed
- PoW mining passed
- Balances passed
- Persistence passed across container recreation and browser reload
- Docker clean deployment passed
- Zero frontend console/page/runtime errors
- No application bugs were found during final acceptance testing

## Documentation

- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/API.md`
- `docs/DEMO.md`
- `docs/TESTING.md`

## Running

Primary command:
```bash
docker compose up --build
```

Frontend: `http://localhost:3000`

## Future Session Rules

- Treat the current implementation as the verified baseline.
- Do not add features or refactor unless explicitly requested.
- Read README.md and docs/ before making changes.
- Preserve existing architecture and behavior.
- If asked to change something, test the affected behavior before and after the change.
