// src/messaging/envelope.rs
//! Encrypted message envelope that travels across the P2P wire and relay nodes.
//!
//! Relays only see and forward this envelope; they cannot decrypt `ciphertext`.
//! All critical metadata except `hops_remaining` is authenticated in AEAD AAD.

use crate::crypto::aead::{AEAD_TAG_LEN, chacha20_poly1305_decrypt, chacha20_poly1305_encrypt};
use crate::crypto::hkdf::hkdf;
use crate::crypto::random::{random_bytes_12, random_bytes_16, random_bytes_32};
use crate::crypto::x25519::x25519;
use crate::error::{ChatError, ProtocolError};
use crate::identity::keypair::KeyPair;
use crate::identity::peer_id::PeerId;
use crate::types::{
    MESSAGE_ID_LEN, NONCE_LEN, PROTOCOL_VERSION, PUBLIC_KEY_LEN, SALT_LEN, current_timestamp_secs,
    to_hex,
};

pub const DEFAULT_HOPS: u8 = 6;
const HKDF_INFO_MESSAGE: &[u8] = b"p2pchat-mvp-v1/message";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEnvelope {
    pub message_id: [u8; MESSAGE_ID_LEN],
    pub sender_peer_id: PeerId,
    pub sender_public_key: [u8; PUBLIC_KEY_LEN],
    pub recipient_peer_id: PeerId,
    pub created_at: u64,
    pub expires_at: u64,
    pub hops_remaining: u8,
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
    pub auth_tag: [u8; AEAD_TAG_LEN],
    pub ciphertext: Vec<u8>,
}

impl MessageEnvelope {
    /// Computes the Authenticated Associated Data (AAD) for AEAD encryption.
    /// Notice: `hops_remaining` is excluded so relays can decrement it without invalidating the tag.
    pub fn compute_aad(
        message_id: &[u8; MESSAGE_ID_LEN],
        sender_peer_id: &PeerId,
        recipient_peer_id: &PeerId,
        created_at: u64,
        expires_at: u64,
    ) -> Vec<u8> {
        let mut aad = Vec::with_capacity(2 + 32 + 32 + 32 + 8 + 8);
        aad.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
        aad.extend_from_slice(message_id);
        aad.extend_from_slice(sender_peer_id.as_bytes());
        aad.extend_from_slice(recipient_peer_id.as_bytes());
        aad.extend_from_slice(&created_at.to_be_bytes());
        aad.extend_from_slice(&expires_at.to_be_bytes());
        aad
    }

    /// Encrypts plaintext message into an envelope.
    pub fn encrypt(
        sender_keypair: &KeyPair,
        recipient_public_key: &[u8; PUBLIC_KEY_LEN],
        recipient_peer_id: PeerId,
        plaintext: &[u8],
        ttl_seconds: u64,
    ) -> Result<Self, ChatError> {
        // 1. Calculate X25519 shared secret
        let shared_secret = x25519(sender_keypair.private_key(), recipient_public_key)?;

        // 2. Derive unique per-message key using HKDF-SHA256
        let salt = random_bytes_16()?;
        let mut message_key = [0u8; 32];
        hkdf(&salt, &shared_secret, HKDF_INFO_MESSAGE, &mut message_key)?;

        // 3. Generate random message ID and nonce
        let message_id = random_bytes_32()?;
        let nonce = random_bytes_12()?;
        let now = current_timestamp_secs();
        let expires_at = now + ttl_seconds;

        // 4. Construct AAD and encrypt with ChaCha20-Poly1305
        let aad = Self::compute_aad(
            &message_id,
            sender_keypair.peer_id(),
            &recipient_peer_id,
            now,
            expires_at,
        );

        let (ciphertext, auth_tag) =
            chacha20_poly1305_encrypt(&message_key, &nonce, &aad, plaintext);

        Ok(Self {
            message_id,
            sender_peer_id: *sender_keypair.peer_id(),
            sender_public_key: *sender_keypair.public_key(),
            recipient_peer_id,
            created_at: now,
            expires_at,
            hops_remaining: DEFAULT_HOPS,
            salt,
            nonce,
            auth_tag,
            ciphertext,
        })
    }

    /// Decrypts ciphertext in the envelope using recipient's private key and sender's public key.
    pub fn decrypt(&self, recipient_keypair: &KeyPair) -> Result<Vec<u8>, ChatError> {
        // 1. Calculate X25519 shared secret
        let shared_secret = x25519(recipient_keypair.private_key(), &self.sender_public_key)?;

        // 2. Derive per-message key using same salt and HKDF info
        let mut message_key = [0u8; 32];
        hkdf(
            &self.salt,
            &shared_secret,
            HKDF_INFO_MESSAGE,
            &mut message_key,
        )?;

        // 3. Reconstruct AAD and authenticate/decrypt
        let aad = Self::compute_aad(
            &self.message_id,
            &self.sender_peer_id,
            &self.recipient_peer_id,
            self.created_at,
            self.expires_at,
        );

        let plaintext = chacha20_poly1305_decrypt(
            &message_key,
            &self.nonce,
            &aad,
            &self.ciphertext,
            &self.auth_tag,
        )?;

        Ok(plaintext)
    }

    /// Checks if message has expired.
    pub fn is_expired(&self) -> bool {
        current_timestamp_secs() >= self.expires_at
    }

    pub fn message_id_hex(&self) -> String {
        to_hex(&self.message_id)
    }

    /// Serializes envelope into a binary buffer.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(
            32 + 32 + 32 + 32 + 8 + 8 + 1 + 16 + 12 + 16 + 4 + self.ciphertext.len(),
        );
        buf.extend_from_slice(&self.message_id);
        buf.extend_from_slice(self.sender_peer_id.as_bytes());
        buf.extend_from_slice(&self.sender_public_key);
        buf.extend_from_slice(self.recipient_peer_id.as_bytes());
        buf.extend_from_slice(&self.created_at.to_be_bytes());
        buf.extend_from_slice(&self.expires_at.to_be_bytes());
        buf.push(self.hops_remaining);
        buf.extend_from_slice(&self.salt);
        buf.extend_from_slice(&self.nonce);
        buf.extend_from_slice(&self.auth_tag);
        buf.extend_from_slice(&(self.ciphertext.len() as u32).to_be_bytes());
        buf.extend_from_slice(&self.ciphertext);
        buf
    }

    /// Deserializes envelope from a binary buffer.
    pub fn decode(data: &[u8]) -> Result<Self, ProtocolError> {
        const MIN_LEN: usize = 32 + 32 + 32 + 32 + 8 + 8 + 1 + 16 + 12 + 16 + 4;
        if data.len() < MIN_LEN {
            return Err(ProtocolError::MalformedPayload(
                "Envelope data too short".to_string(),
            ));
        }

        let mut offset = 0;
        let message_id: [u8; 32] = data[offset..offset + 32].try_into().unwrap();
        offset += 32;

        let sender_peer_bytes: [u8; 32] = data[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let sender_peer_id = PeerId::from_bytes(sender_peer_bytes);

        let sender_public_key: [u8; 32] = data[offset..offset + 32].try_into().unwrap();
        offset += 32;

        let recipient_peer_bytes: [u8; 32] = data[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let recipient_peer_id = PeerId::from_bytes(recipient_peer_bytes);

        let created_at = u64::from_be_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        let expires_at = u64::from_be_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        let hops_remaining = data[offset];
        offset += 1;

        let salt: [u8; 16] = data[offset..offset + 16].try_into().unwrap();
        offset += 16;

        let nonce: [u8; 12] = data[offset..offset + 12].try_into().unwrap();
        offset += 12;

        let auth_tag: [u8; 16] = data[offset..offset + 16].try_into().unwrap();
        offset += 16;

        let ct_len = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        if offset + ct_len > data.len() {
            return Err(ProtocolError::MalformedPayload(
                "Envelope ciphertext truncated".to_string(),
            ));
        }

        let ciphertext = data[offset..offset + ct_len].to_vec();

        Ok(Self {
            message_id,
            sender_peer_id,
            sender_public_key,
            recipient_peer_id,
            created_at,
            expires_at,
            hops_remaining,
            salt,
            nonce,
            auth_tag,
            ciphertext,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CryptoError;

    #[test]
    fn test_envelope_encryption_roundtrip() {
        let alice = KeyPair::generate().unwrap();
        let bob = KeyPair::generate().unwrap();

        let plaintext = b"Decentralized P2P message between Alice and Bob!";
        let envelope =
            MessageEnvelope::encrypt(&alice, bob.public_key(), *bob.peer_id(), plaintext, 3600)
                .expect("encryption succeeds");

        // Verify that ciphertext is not plaintext
        assert_ne!(envelope.ciphertext, plaintext);

        // Serialize and deserialize
        let bytes = envelope.encode();
        let decoded_envelope = MessageEnvelope::decode(&bytes).expect("decode envelope");
        assert_eq!(envelope, decoded_envelope);

        // Bob decrypts
        let decrypted = decoded_envelope.decrypt(&bob).expect("decryption succeeds");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_envelope_tamper_fails() {
        let alice = KeyPair::generate().unwrap();
        let bob = KeyPair::generate().unwrap();

        let mut envelope = MessageEnvelope::encrypt(
            &alice,
            bob.public_key(),
            *bob.peer_id(),
            b"Secret data",
            3600,
        )
        .unwrap();

        // Tamper ciphertext
        envelope.ciphertext[0] ^= 0x01;
        assert!(matches!(
            envelope.decrypt(&bob),
            Err(ChatError::Crypto(CryptoError::AuthenticationFailed))
        ));
    }
}
