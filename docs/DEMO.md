# Demo script

A reliable, step-by-step sequence for demonstrating this project live, e.g. in a
college presentation. Every step below has been verified against the actual running
Docker deployment with real browser automation (see `docs/TESTING.md`).

## Before the demo

1. Make sure Docker is running, then from the repository root:
   ```bash
   docker compose up --build -d
   ```
   Wait for both containers to report healthy/running:
   ```bash
   docker compose ps
   ```
2. Open **http://localhost:3000** in a browser.
3. **Fund a wallet in advance.** This project has no coinbase/reward mechanism and no
   genesis allocation (deliberate scope decision - see the README), so a freshly
   generated wallet has a balance of `0` and cannot submit its first transaction. Do
   this *before* presenting, not live:
   - Generate a wallet through the UI (see step 4 below) and copy its address.
   - The dev-only funding endpoint is already enabled in `docker-compose.yml`
     (`DEV_FUNDING_ENABLED: "true"`), so just:
     ```bash
     curl -X POST http://localhost:3000/api/dev/fund \
       -H "content-type: application/json" \
       -d '{"address": "<the address you copied>", "amount": 100}'
     ```
     This is a real, documented, off-by-default-outside-this-compose-file endpoint
     (see `docs/API.md`) - not something to explain away live; it's fine to mention
     it exists specifically
     because there's no coinbase mechanism.
   - Reload the page; the same wallet won't persist across a reload (it's
     regenerated), so either keep that browser tab open until the demo, or fund the
     *next* wallet you generate right before presenting.

   Alternative, simpler for a live demo: skip pre-funding and instead demonstrate the
   *rejection* path first (step 4-6 below show a `0`-balance wallet correctly failing
   to send anything), then switch to a second, already-funded wallet/tab to show the
   full success path. Both are genuine, correct application behavior.

## Live walkthrough

1. **Open the app.** Point out the four visible sections: Blockchain status, Mempool,
   Wallet balance, Send transaction, Mining, and the Explorer at the bottom.
2. **Genesis + validity.** The Blockchain card shows block count `1`, a Latest block
   starting `#0`, and Validity `valid` - this is the chain's fixed, deterministic
   starting point.
3. **Explorer.** Click the genesis block in the Explorer list. Show its detail: index
   `0`, nonce `0`, previous hash all zeros, no transactions. Explain that this block is
   never mined - it's a fixed special case.
4. **Generate a wallet.** Click "Generate wallet" in the Send transaction card. Point
   out the displayed address and public key, and read the note: the private key never
   leaves the browser tab.
5. **Check its balance.** In the Wallet balance card, click "Use my wallet", then
   "Check balance". A fresh wallet shows `0`.
6. **(Optional) Show the unfunded rejection.** Try to send anything from this fresh
   wallet - it's rejected with `insufficient balance`. This demonstrates that
   overspending is impossible, not just discouraged.
7. **Switch to a funded wallet** (the one prepared before the demo, or fund this one
   now via the documented development mechanism if presenting in a setting where that's
   appropriate to show).
8. **Create and submit a signed transaction.** Fill in a recipient address (anything,
   e.g. `bob`) and an amount less than the funded balance. Click "Sign & submit". Point
   out the success message with the transaction's server-assigned id.
9. **Show it in the mempool.** The Mempool card now lists 1 pending transaction, with
   sender, recipient, amount, id, public key, and signature all visible.
10. **Show the balance hasn't moved yet.** Check the sender's balance again - still the
    pre-transaction amount. Explain: only *confirmed* (mined) transactions count.
11. **Mine it.** Click "Mine pending transactions" in the Mining card. Point out the
    result: the new block's index, its hash (visibly starting with several `0`s -
    Proof-of-Work satisfied), and its nonce (how many attempts it took).
12. **Show the mempool is now empty**, and the Blockchain card's block count increased.
13. **Show updated balances.** Sender's balance decreased by the amount sent;
    recipient's increased by the same amount.
14. **Explorer again.** Click the newly mined block. Show its full detail - index,
    timestamp, previous hash (linking to the prior block), hash, nonce, and the
    transaction inside it with all its fields.
15. **Chain validity, again.** Still `valid` - the new block extended the chain
    correctly.
16. **(Optional) Mine with an empty mempool.** Click "Mine pending transactions" again
    with nothing pending - shows a clear `no pending transactions to mine` error rather
    than creating an empty block.
17. **(Optional) Persistence.** To show data survives a restart:
    ```bash
    docker compose down
    docker compose up -d
    ```
    Reload the page - all blocks, transactions, and balances are exactly as before.

## If something looks wrong during the demo

- **Blank page / connection refused**: confirm both containers are up
  (`docker compose ps`) and healthy; check `docker compose logs backend`.
- **A transaction won't submit**: check the exact error message shown in the form - it
  is almost always either `insufficient balance` (the wallet needs funding first) or a
  structural/signature issue from stale form state (regenerate the wallet and retry).
- **Mining says nothing to mine**: expected if the mempool is genuinely empty - submit
  a transaction first.
