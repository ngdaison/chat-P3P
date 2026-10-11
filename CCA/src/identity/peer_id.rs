// src/identity/peer_id.rs
//! Cryptographic Peer ID derived deterministically from Public Key: PeerID = SHA-256(PublicKey).

use crate::crypto::sha256::sha256;
use crate::types::{PEER_ID_LEN, from_hex, to_hex};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId(pub [u8; PEER_ID_LEN]);

impl PeerId {
    /// Computes deterministic Peer ID from a 32-byte public key.
    pub fn from_public_key(public_key: &[u8; 32]) -> Self {
        Self(sha256(public_key))
    }

    /// Creates a PeerId from raw 32 bytes.
    pub fn from_bytes(bytes: [u8; PEER_ID_LEN]) -> Self {
        Self(bytes)
    }

    /// Returns a slice of raw bytes.
    pub fn as_bytes(&self) -> &[u8; PEER_ID_LEN] {
        &self.0
    }

    /// Converts PeerId to a 64-character lowercase hex string.
    pub fn to_hex(&self) -> String {
        to_hex(&self.0)
    }

    /// Parses PeerId from a hex string.
    pub fn from_hex(s: &str) -> Result<Self, String> {
        let bytes = from_hex(s)?;
        if bytes.len() != PEER_ID_LEN {
            return Err(format!(
                "Expected {} bytes for PeerId, got {}",
                PEER_ID_LEN,
                bytes.len()
            ));
        }
        let mut arr = [0u8; PEER_ID_LEN];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Returns a short 8-character hex abbreviation for CLI logs.
    pub fn short_hex(&self) -> String {
        let full = self.to_hex();
        full[..8].to_string()
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Debug for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PeerId({})", self.short_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id_derivation() {
        let pubkey = [0x42u8; 32];
        let id1 = PeerId::from_public_key(&pubkey);
        let id2 = PeerId::from_public_key(&pubkey);
        assert_eq!(id1, id2);
        assert_eq!(id1.to_hex().len(), 64);
        assert_eq!(PeerId::from_hex(&id1.to_hex()).unwrap(), id1);
    }
}
