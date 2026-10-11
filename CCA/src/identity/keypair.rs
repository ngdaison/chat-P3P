// src/identity/keypair.rs
//! Identity keypair management.

use crate::crypto::random::random_bytes_32;
use crate::crypto::x25519::x25519_base_point;
use crate::error::CryptoError;
use crate::identity::peer_id::PeerId;
use crate::types::{PRIVATE_KEY_LEN, PUBLIC_KEY_LEN, to_hex};
use std::fmt;

#[derive(Clone)]
pub struct KeyPair {
    private_key: [u8; PRIVATE_KEY_LEN],
    public_key: [u8; PUBLIC_KEY_LEN],
    peer_id: PeerId,
}

impl KeyPair {
    /// Generates a new cryptographic identity using OS CSPRNG.
    pub fn generate() -> Result<Self, CryptoError> {
        let private_key = random_bytes_32()?;
        Ok(Self::from_private_key(private_key))
    }

    /// Derives identity from an existing 32-byte private key.
    pub fn from_private_key(private_key: [u8; PRIVATE_KEY_LEN]) -> Self {
        let public_key = x25519_base_point(&private_key);
        let peer_id = PeerId::from_public_key(&public_key);
        Self {
            private_key,
            public_key,
            peer_id,
        }
    }

    pub fn private_key(&self) -> &[u8; PRIVATE_KEY_LEN] {
        &self.private_key
    }

    pub fn public_key(&self) -> &[u8; PUBLIC_KEY_LEN] {
        &self.public_key
    }

    pub fn public_key_hex(&self) -> String {
        to_hex(&self.public_key)
    }

    pub fn ed25519_public_key(&self) -> [u8; 32] {
        crate::crypto::ed25519::ed25519_public_key(&self.private_key)
    }

    /// Signs an arbitrary message using this node's Ed25519 identity key.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        crate::crypto::ed25519::ed25519_sign(&self.private_key, message)
    }

    /// Generates cryptographic proof that binds this node's X25519 encryption key
    /// to its Ed25519 identity key.
    pub fn identity_proof(&self) -> [u8; 64] {
        self.sign(&self.public_key)
    }

    /// Verifies that an X25519 public key is genuinely bound to an Ed25519 identity key.
    pub fn verify_identity_proof(
        ed25519_pub: &[u8; 32],
        x25519_pub: &[u8; 32],
        proof: &[u8; 64],
    ) -> bool {
        crate::crypto::ed25519::ed25519_verify(ed25519_pub, x25519_pub, proof)
    }

    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }
}

// Ensure private key is NOT exposed in Debug output
impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyPair")
            .field("peer_id", &self.peer_id)
            .field("public_key", &self.public_key_hex())
            .field("private_key", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let kp1 = KeyPair::generate().expect("keypair gen");
        let kp2 = KeyPair::generate().expect("keypair gen");
        assert_ne!(kp1.public_key(), kp2.public_key());
        assert_ne!(kp1.peer_id(), kp2.peer_id());
        assert_eq!(kp1.peer_id(), &PeerId::from_public_key(kp1.public_key()));

        // Check debug output redacts private key
        let debug_str = format!("{kp1:?}");
        assert!(debug_str.contains("[REDACTED]"));

        // Check identity binding proof
        let proof = kp1.identity_proof();
        assert!(KeyPair::verify_identity_proof(
            &kp1.ed25519_public_key(),
            kp1.public_key(),
            &proof
        ));
        // Tampered public key should fail verification
        let mut tampered_pub = *kp1.public_key();
        tampered_pub[0] ^= 1;
        assert!(!KeyPair::verify_identity_proof(
            &kp1.ed25519_public_key(),
            &tampered_pub,
            &proof
        ));
    }
}
