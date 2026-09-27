use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::wallet::{self, Wallet};

/// A single account-based transfer: `amount` moves from `sender` to
/// `recipient`, authorized by `sender`'s Ed25519 signature over the
/// transaction's economic fields.
///
/// `sender_public_key` and `signature` are both hex-encoded - the same
/// convention `Block`/`Transaction` already use for `hash`/`id`. Balance
/// checking (does `sender` actually have `amount` to spend) is not this
/// type's concern - see `Mempool::try_add_transaction` and
/// `Chain::balance_of`, which apply it at admission time using this type's
/// fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Transaction {
    pub sender: String,
    pub recipient: String,
    pub amount: i64,
    pub sender_public_key: String,
    pub signature: String,
    pub id: String,
}

impl Transaction {
    /// Builds a transaction from already-signed components - as received
    /// from an API client, or reconstructed from storage - and computes its
    /// id. This always succeeds structurally, mirroring `Block::new`; call
    /// `validation_error()`/`is_valid()` separately to check the business
    /// rules, including whether the signature actually verifies.
    pub fn new(
        sender: String,
        recipient: String,
        amount: i64,
        sender_public_key: String,
        signature: String,
    ) -> Self {
        let id = Self::calculate_id(&sender, &recipient, amount, &sender_public_key, &signature);
        Transaction {
            sender,
            recipient,
            amount,
            sender_public_key,
            signature,
            id,
        }
    }

    /// Convenience constructor: builds a transaction and signs it with
    /// `wallet`, deriving `sender`/`sender_public_key` from the wallet
    /// itself.
    ///
    /// The server never signs on a caller's behalf - a real client signs
    /// locally with its own wallet and submits the already-signed result to
    /// `POST /api/transactions`. This exists for tests, and any future
    /// in-process signing (e.g. a CLI), where a `Wallet` is at hand.
    #[allow(dead_code)]
    pub fn signed_by(wallet: &Wallet, recipient: String, amount: i64) -> Self {
        let sender = wallet.address();
        let sender_public_key = wallet.public_key_hex();
        let signing_bytes = Self::compute_signing_bytes(&sender, &recipient, amount, &sender_public_key);
        let signature = hex::encode(wallet.sign(&signing_bytes).to_bytes());

        Transaction::new(sender, recipient, amount, sender_public_key, signature)
    }

    /// Structural/business-rule validation: positive amount, non-empty
    /// sender and recipient, sender != recipient, the sender address
    /// actually matches the supplied public key, the signature verifies
    /// against the sender's public key, and the id matches the content.
    /// Does not check balances - that needs wallet *state* from a later
    /// phase.
    #[allow(dead_code)]
    pub fn is_valid(&self) -> bool {
        self.validation_error().is_none()
    }

    /// `None` if valid, otherwise a message naming the first violated rule
    /// (checked in a fixed order). Used by the API to explain a rejection.
    pub fn validation_error(&self) -> Option<&'static str> {
        if self.amount <= 0 {
            return Some("amount must be greater than zero");
        }
        if self.sender.is_empty() {
            return Some("sender must not be empty");
        }
        if self.recipient.is_empty() {
            return Some("recipient must not be empty");
        }
        if self.sender == self.recipient {
            return Some("sender and recipient must not be identical");
        }
        match wallet::address_from_public_key_hex(&self.sender_public_key) {
            Some(address) if address == self.sender => {}
            _ => return Some("sender does not match the supplied public key"),
        }
        if !wallet::verify(&self.sender_public_key, &self.signing_bytes(), &self.signature) {
            return Some("signature is invalid");
        }
        if self.id != Self::calculate_id(&self.sender, &self.recipient, self.amount, &self.sender_public_key, &self.signature) {
            return Some("transaction id does not match its contents");
        }
        None
    }

    /// The deterministic bytes a wallet signs (and a verifier re-derives to
    /// check a signature against): the transaction's economic fields and
    /// sender identity. Deliberately excludes `id` (the id is derived from
    /// the signed transaction, not the other way around) and `signature`
    /// itself (a signature can't cover its own bytes).
    pub fn signing_bytes(&self) -> Vec<u8> {
        Self::compute_signing_bytes(&self.sender, &self.recipient, self.amount, &self.sender_public_key)
    }

    fn compute_signing_bytes(sender: &str, recipient: &str, amount: i64, sender_public_key: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(sender.as_bytes());
        bytes.extend_from_slice(recipient.as_bytes());
        bytes.extend_from_slice(&amount.to_be_bytes());
        bytes.extend_from_slice(sender_public_key.as_bytes());
        bytes
    }

    /// SHA-256 hash of the transaction's identifying fields (including the
    /// signature), hex-encoded. Because it covers the signature too, the id
    /// represents the fully-formed *signed* transaction: changing anything
    /// about a signed transaction - even just re-signing the same economic
    /// data - changes its id.
    fn calculate_id(
        sender: &str,
        recipient: &str,
        amount: i64,
        sender_public_key: &str,
        signature: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(sender.as_bytes());
        hasher.update(recipient.as_bytes());
        hasher.update(amount.to_be_bytes());
        hasher.update(sender_public_key.as_bytes());
        hasher.update(signature.as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validly_signed_transaction_passes_validation() {
        let wallet = Wallet::generate();
        let tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        assert!(tx.is_valid());
    }

    #[test]
    fn zero_amount_is_rejected() {
        let tx = Transaction::new("alice".to_string(), "bob".to_string(), 0, String::new(), String::new());

        assert!(!tx.is_valid());
    }

    #[test]
    fn negative_amount_is_rejected() {
        let tx = Transaction::new("alice".to_string(), "bob".to_string(), -5, String::new(), String::new());

        assert!(!tx.is_valid());
    }

    #[test]
    fn empty_sender_is_rejected() {
        let tx = Transaction::new(String::new(), "bob".to_string(), 10, String::new(), String::new());

        assert!(!tx.is_valid());
    }

    #[test]
    fn empty_recipient_is_rejected() {
        let tx = Transaction::new("alice".to_string(), String::new(), 10, String::new(), String::new());

        assert!(!tx.is_valid());
    }

    #[test]
    fn identical_sender_and_recipient_is_rejected() {
        let tx = Transaction::new("alice".to_string(), "alice".to_string(), 10, String::new(), String::new());

        assert!(!tx.is_valid());
    }

    #[test]
    fn sender_not_matching_public_key_is_rejected() {
        let wallet = Wallet::generate();
        let mut tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        tx.sender = "someone-else".to_string();

        assert_eq!(
            tx.validation_error(),
            Some("sender does not match the supplied public key")
        );
    }

    #[test]
    fn substituting_a_different_but_self_consistent_public_key_is_rejected() {
        let wallet = Wallet::generate();
        let attacker = Wallet::generate();
        let mut tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        // Swap in a different wallet's public key *and* its matching
        // address, so the address-matches-public-key check alone can't
        // catch this - only the signature check can, since the original
        // signature was produced by `wallet`'s private key, not
        // `attacker`'s.
        tx.sender = attacker.address();
        tx.sender_public_key = attacker.public_key_hex();

        assert_eq!(tx.validation_error(), Some("signature is invalid"));
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let wallet = Wallet::generate();
        let mut tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        tx.signature = "00".repeat(64);

        assert_eq!(tx.validation_error(), Some("signature is invalid"));
    }

    #[test]
    fn tampering_with_amount_after_signing_is_rejected() {
        let wallet = Wallet::generate();
        let mut tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        // The signature was computed over amount = 10; changing the field
        // afterwards, without re-signing, must invalidate the transaction.
        tx.amount = 999;

        assert!(!tx.is_valid());
    }

    #[test]
    fn stale_id_after_tampering_is_rejected() {
        let wallet = Wallet::generate();
        let mut tx = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        // Re-sign a tampered amount so the signature check passes, but
        // leave the old id in place - the id check must still catch it.
        let new_signing_bytes = Transaction::compute_signing_bytes(&tx.sender, &tx.recipient, 999, &tx.sender_public_key);
        tx.signature = hex::encode(wallet.sign(&new_signing_bytes).to_bytes());
        tx.amount = 999;

        assert_eq!(tx.validation_error(), Some("transaction id does not match its contents"));
    }

    #[test]
    fn identical_signed_transactions_produce_the_same_id() {
        let wallet = Wallet::generate();
        let a = Transaction::signed_by(&wallet, "bob".to_string(), 10);
        let b = Transaction::signed_by(&wallet, "bob".to_string(), 10);

        // Ed25519 signing is deterministic (RFC 8032): the same key and
        // message always produce the same signature, so two transactions
        // built from identical inputs get identical ids.
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn changing_transaction_data_changes_the_id() {
        let wallet = Wallet::generate();
        let a = Transaction::signed_by(&wallet, "bob".to_string(), 10);
        let b = Transaction::signed_by(&wallet, "bob".to_string(), 20);

        assert_ne!(a.id, b.id);
    }

    #[test]
    fn different_wallets_produce_different_ids_for_the_same_recipient_and_amount() {
        let a = Transaction::signed_by(&Wallet::generate(), "bob".to_string(), 10);
        let b = Transaction::signed_by(&Wallet::generate(), "bob".to_string(), 10);

        assert_ne!(a.id, b.id);
    }
}
