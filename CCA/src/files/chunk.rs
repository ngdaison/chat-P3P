// src/files/chunk.rs
//! Chunk splitting, SHA-256 chunk hashing, and per-chunk ChaCha20-Poly1305 encryption.

use crate::crypto::aead::{AEAD_TAG_LEN, chacha20_poly1305_decrypt, chacha20_poly1305_encrypt};
use crate::crypto::sha256::sha256;
use crate::error::CryptoError;

pub const CHUNK_SIZE: usize = 262144; // 256 KiB

/// Derives a deterministic, unique 12-byte AEAD nonce for a chunk index.
pub fn derive_chunk_nonce(chunk_index: u32) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[0..4].copy_from_slice(b"CHUN");
    nonce[4..8].copy_from_slice(&chunk_index.to_be_bytes());
    nonce[8..12].copy_from_slice(&(chunk_index ^ 0xA5A5A5A5).to_be_bytes());
    nonce
}

/// Encrypts a chunk payload using ChaCha20-Poly1305 with unique per-chunk nonce.
/// Appends the 16-byte Poly1305 authentication tag to the returned ciphertext.
pub fn encrypt_chunk(
    file_key: &[u8; 32],
    chunk_index: u32,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let nonce = derive_chunk_nonce(chunk_index);
    let aad = chunk_index.to_be_bytes();
    let (mut ct, tag) = chacha20_poly1305_encrypt(file_key, &nonce, &aad, plaintext);
    ct.extend_from_slice(&tag);
    Ok(ct)
}

/// Decrypts and authenticates a chunk payload using ChaCha20-Poly1305.
pub fn decrypt_chunk(
    file_key: &[u8; 32],
    chunk_index: u32,
    ciphertext_with_tag: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if ciphertext_with_tag.len() < AEAD_TAG_LEN {
        return Err(CryptoError::AuthenticationFailed);
    }
    let ct_len = ciphertext_with_tag.len() - AEAD_TAG_LEN;
    let ct = &ciphertext_with_tag[..ct_len];
    let tag: [u8; AEAD_TAG_LEN] = ciphertext_with_tag[ct_len..].try_into().unwrap();
    let nonce = derive_chunk_nonce(chunk_index);
    let aad = chunk_index.to_be_bytes();
    chacha20_poly1305_decrypt(file_key, &nonce, &aad, ct, &tag)
}

/// Computes the SHA-256 hash of a chunk's encrypted data.
pub fn hash_chunk(data: &[u8]) -> [u8; 32] {
    sha256(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_encryption_roundtrip() {
        let key = [42u8; 32];
        let chunk_index = 3;
        let data = b"Hello, encrypted chunk transfer test data!";

        let encrypted = encrypt_chunk(&key, chunk_index, data).expect("encrypt chunk");
        assert_ne!(&encrypted[..data.len()], data);

        let decrypted = decrypt_chunk(&key, chunk_index, &encrypted).expect("decrypt chunk");
        assert_eq!(decrypted, data);

        // Wrong chunk index must fail authentication
        assert!(decrypt_chunk(&key, chunk_index + 1, &encrypted).is_err());

        // Tampered ciphertext must fail authentication
        let mut tampered = encrypted.clone();
        tampered[0] ^= 1;
        assert!(decrypt_chunk(&key, chunk_index, &tampered).is_err());
    }
}
