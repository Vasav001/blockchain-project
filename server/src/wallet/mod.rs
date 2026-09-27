use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

/// A local Ed25519 keypair plus its derived address.
///
/// Wallets are a client-side concept in this project: the server never
/// generates, stores, or serves one on anyone's behalf (see the API layer -
/// there is deliberately no wallet endpoint). This type exists so tests,
/// and any future client/CLI code sharing this crate, can create a keypair
/// and sign transactions with it, the way a real client would locally.
pub struct Wallet {
    signing_key: SigningKey,
}

impl Wallet {
    /// Generates a new random keypair.
    ///
    /// Not called from `main` - the server never creates a wallet on
    /// anyone's behalf (see the module doc comment above). Used by tests,
    /// and by `Transaction::signed_by`, which is itself only used by tests.
    #[allow(dead_code)]
    pub fn generate() -> Self {
        Wallet {
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn public_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// The public key, hex-encoded - safe to share, store, or log.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key().to_bytes())
    }

    /// This wallet's address, derived from its public key.
    pub fn address(&self) -> String {
        address_from_public_key(&self.public_key())
    }

    /// Signs `message` with this wallet's private key.
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
}

impl std::fmt::Debug for Wallet {
    /// Deliberately omits the private key. Only the address - a public,
    /// one-way-derived value - is safe to include in logs or debug output.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet")
            .field("address", &self.address())
            .finish()
    }
}

/// Derives an address from a public key: SHA-256 of the raw public key
/// bytes, hex-encoded.
pub fn address_from_public_key(public_key: &VerifyingKey) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key.to_bytes());
    hex::encode(hasher.finalize())
}

/// Derives an address from a hex-encoded public key, or `None` if it isn't
/// a validly-encoded Ed25519 public key.
pub fn address_from_public_key_hex(public_key_hex: &str) -> Option<String> {
    parse_public_key(public_key_hex).map(|key| address_from_public_key(&key))
}

/// Checks a signature against a hex-encoded public key and message.
///
/// Any parsing failure (malformed hex, wrong-length key or signature) is
/// treated as "not a valid signature" rather than surfaced as a distinct
/// error - from a caller's perspective, malformed input can't possibly have
/// a valid signature, so a plain `bool` is all that's needed here.
pub fn verify(public_key_hex: &str, message: &[u8], signature_hex: &str) -> bool {
    let (Some(public_key), Some(signature)) =
        (parse_public_key(public_key_hex), parse_signature(signature_hex))
    else {
        return false;
    };

    public_key.verify_strict(message, &signature).is_ok()
}

fn parse_public_key(hex_str: &str) -> Option<VerifyingKey> {
    let bytes: [u8; 32] = hex::decode(hex_str).ok()?.try_into().ok()?;
    VerifyingKey::from_bytes(&bytes).ok()
}

fn parse_signature(hex_str: &str) -> Option<Signature> {
    let bytes: [u8; 64] = hex::decode(hex_str).ok()?.try_into().ok()?;
    Some(Signature::from_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_wallet_has_a_64_character_hex_address() {
        let wallet = Wallet::generate();

        // SHA-256 hex-encoded is always 64 hex characters.
        assert_eq!(wallet.address().len(), 64);
    }

    #[test]
    fn same_public_key_always_derives_the_same_address() {
        let wallet = Wallet::generate();

        assert_eq!(wallet.address(), address_from_public_key(&wallet.public_key()));
    }

    #[test]
    fn different_wallets_get_different_addresses() {
        let a = Wallet::generate();
        let b = Wallet::generate();

        assert_ne!(a.address(), b.address());
    }

    #[test]
    fn valid_signature_verifies() {
        let wallet = Wallet::generate();
        let message = b"transfer 10 from alice to bob";
        let signature = wallet.sign(message);

        assert!(verify(
            &wallet.public_key_hex(),
            message,
            &hex::encode(signature.to_bytes())
        ));
    }

    #[test]
    fn signature_does_not_verify_with_a_different_public_key() {
        let wallet = Wallet::generate();
        let other = Wallet::generate();
        let message = b"transfer 10 from alice to bob";
        let signature = wallet.sign(message);

        assert!(!verify(
            &other.public_key_hex(),
            message,
            &hex::encode(signature.to_bytes())
        ));
    }

    #[test]
    fn signature_does_not_verify_after_message_is_tampered_with() {
        let wallet = Wallet::generate();
        let signature = wallet.sign(b"transfer 10 from alice to bob");

        assert!(!verify(
            &wallet.public_key_hex(),
            b"transfer 99 from alice to bob",
            &hex::encode(signature.to_bytes())
        ));
    }

    #[test]
    fn malformed_signature_hex_does_not_verify() {
        let wallet = Wallet::generate();

        assert!(!verify(&wallet.public_key_hex(), b"message", "not-hex"));
    }

    #[test]
    fn malformed_public_key_hex_does_not_verify() {
        let wallet = Wallet::generate();
        let message = b"message";
        let signature = wallet.sign(message);

        assert!(!verify("not-hex", message, &hex::encode(signature.to_bytes())));
    }
}
