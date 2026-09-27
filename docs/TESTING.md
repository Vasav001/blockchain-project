# Testing

This document distinguishes what kind of verification was actually performed on this
project, and how. All of the below reflects real runs, not aspirational coverage.

## 1. Automated backend tests

```bash
cd server && cargo test
```

**100 tests**, all passing. In-process (no external processes spawned), each test using
an isolated in-memory SQLite database. Covers: block hashing and genesis determinism,
chain validation and tamper detection, Proof-of-Work (nonce search, difficulty
satisfaction, PoW validation), transaction signing/verification and all tamper variants
(amount/sender/public-key/signature changed independently, and a self-consistent
identity-substitution attack), mempool admission (validity, duplicates, balance/
overspend prevention), balance replay, database round-trips for every entity, migration
upgrade paths (including simulated pre-existing databases from earlier schema
versions), and the REST API layer (via Axum's in-process router testing, not real HTTP
sockets).

## 2. Frontend typecheck/build

```bash
cd client && npm install && npm run build
```

`tsc -b && vite build` - full TypeScript typecheck plus a production build. No
type errors.

## 3. API/integration verification (real HTTP, real process, no browser)

At various points during development, the actual compiled backend binary was started
(via `cargo run`, and separately inside its real Docker container) and exercised with
genuine HTTP requests - `curl` for spot checks, and Node scripts that import and run the
project's own `client/src/wallet.ts` / `client/src/api.ts` unmodified (neither touches
the DOM) against the live server. This is real signing (`@noble/ed25519`, not
simulated) and real network calls, confirming the exact byte-for-byte signing format
matches between the TypeScript and Rust implementations, and that every endpoint
behaves as documented - but it does not exercise React rendering, DOM events, or actual
browser behavior.

## 4. Frontend/manual (real browser) verification

A real headless Chromium instance (Playwright) was used to drive the actual running
frontend at `http://localhost:3000` as a user would: clicking buttons, filling forms,
reading rendered text, and screenshotting. This is genuine browser testing, not a
simulation or an API-only substitute. Verified end-to-end through the UI:

- Initial load: Blockchain/Mempool/Wallet balance/Send transaction/Mining/Explorer
  sections all render with correct initial state (genesis block, `valid` chain, `0`
  pending transactions)
- Wallet generation via the "Generate wallet" button, address and public key displayed
- Balance lookup via both manual address entry and the "Use my wallet" shortcut
- Funding a wallet (development-only mechanism - see §6 below) and observing the
  balance update in the UI
- Filling in and submitting a real signed transaction through the actual form
- The submitted transaction appearing in the Mempool card with all its fields
- Confirming the sender's balance is unchanged while the transaction is only pending
- Mining via the "Mine pending transactions" button, and the result (block index, hash,
  nonce, transaction count) displayed correctly
- The mempool becoming empty and the Blockchain card's block count updating after
  mining
- Post-mining balances updating correctly for both sender and recipient
- The Explorer listing the new block and, on click, showing correct detail (index,
  timestamp, previous hash, hash, nonce, transaction count) and the transaction's full
  detail inside it
- Chain validity remaining `true` after mining
- Error handling through the actual UI: an overspend attempt shows `insufficient
  balance` in the form; mining with an empty mempool shows `no pending transactions to
  mine` in the mining panel
- A 375px-wide (mobile) viewport: no horizontal overflow (`document.documentElement`'s
  `scrollWidth` equals `clientWidth`), all cards readable and usable
- Browser console/page errors and failed/5xx network requests were captured throughout:
  zero JavaScript errors, zero unhandled exceptions, zero failed or 5xx requests. The
  only console entries logged were the browser's own network log for the two
  *deliberately* triggered `400` responses (the overspend and empty-mempool error
  tests) - not application bugs.

**Environment note:** the sandbox this was verified in has no browser installed by
default and no root access to install one via the system package manager. Real
Chromium was still obtained without root: Playwright's browser binary was downloaded
(no privileges needed for that), and its handful of missing shared library
dependencies were fetched with `apt-get download` (downloading, not installing - no
root needed) and used via `LD_LIBRARY_PATH` rather than being installed system-wide.
This is noted because it's the specific mechanism that made real browser testing
possible here, not just API-level testing.

## 5. Docker/deployment verification

```bash
docker compose down -v      # completely fresh start, no leftover volume
docker compose up --build -d
```

Verified: both images build successfully from a clean cache; both containers reach a
running/healthy state (the backend has a Compose healthcheck against `/health`; the
frontend depends on it being healthy before starting); backend logs show clean
startup (`database ready`, genesis seeded exactly once, `listening on 0.0.0.0:8080`);
frontend logs show nginx starting cleanly; the backend's port is confirmed *not*
reachable directly from the host (only the frontend's port is published); `/api/*` and
`/health` both correctly reach the backend through nginx's reverse proxy; static
assets (JS/CSS) are served with correct content types; unknown routes fall back to
`index.html` (SPA-safe) with `200`.

## 6. Development-only funding mechanism

There is no coinbase/reward mechanism and no genesis allocation, so no wallet can ever
acquire its first balance through the public API - this is a deliberate scope decision,
not a bug (see the README's "Known limitations"). To set up a funded wallet for manual
testing/demo purposes, a block crediting the target address is inserted directly
through the backend's repository layer (`db::insert_block`, which performs no
validation by design - see its own doc comment), bypassing the mempool's balance check
entirely. In this sandbox that meant briefly stopping the running backend container,
running that insertion via a throwaway container built from the same Dockerfile's
builder stage against the same data volume, then restarting the real backend - avoiding
two processes writing to the SQLite file at once. This mechanism is not exposed through
any API endpoint and is not part of the application; it exists only to make manual
verification and demos possible without an economic bootstrapping problem.

## 7. Persistence verification

Performed twice, independently, with real state actually inspected before and after
(not just "containers restart cleanly"):

**Via HTTP** (`curl`, described above): funded a wallet, submitted a signed
transaction, mined it, recorded the resulting block count, block hashes, and balances;
ran `docker compose down` (containers fully stopped and *removed*, not just paused) and
`docker compose up -d`; confirmed via fresh requests that the block count, every block's
hash (byte-for-byte), the transaction's presence, and both balances were unchanged, and
that the backend's startup log did *not* show a second genesis insertion.

**Via the real browser** (Playwright, described above): repeated the same
down/up cycle, then reloaded `http://localhost:3000` in the browser and re-checked,
through the UI, the block count, the Explorer's block list and the mined block's full
detail, and both wallets' balances - all identical to before the restart, with zero
console/page errors on reload.

## What was *not* tested

- **Multiple concurrent real users** clicking through the UI simultaneously (concurrent
  API requests *are* covered by dedicated `cargo test` cases for both transaction
  submission and mining).
- **Long-term data volume** (thousands of blocks/transactions) - this project's
  in-memory Proof-of-Work search and N+1-query block loading are appropriate for an
  educational/demo scale, not benchmarked at production scale.
- **Non-Chromium browsers.** Verification used Chromium (via Playwright). The frontend
  uses only widely-supported web platform features (fetch, `crypto.subtle.digest` for
  SHA-256, standard React/Vite output), so no Chromium-specific behavior is relied on,
  but Firefox/Safari were not independently driven.
