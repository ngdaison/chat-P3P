// src/crypto/aead.rs
//! ChaCha20-Poly1305 Authenticated Encryption with Associated Data (AEAD) according to RFC 8439.

use crate::crypto::chacha20::ChaCha20;
use crate::crypto::constant_time::ct_eq;
use crate::crypto::poly1305::Poly1305;
use crate::error::CryptoError;

pub const AEAD_TAG_LEN: usize = 16;

/// Encrypts plaintext and generates a 16-byte authentication tag over AAD and ciphertext.
pub fn chacha20_poly1305_encrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; AEAD_TAG_LEN]) {
    // 1. Generate one-time Poly1305 key using counter 0
    let otk_cipher = ChaCha20::new(key, nonce, 0);
    let otk_block = otk_cipher.block();
    let poly_key: &[u8; 32] = otk_block[..32].try_into().unwrap();

    // 2. Encrypt plaintext using counter 1
    let mut ciphertext = plaintext.to_vec();
    let mut cipher = ChaCha20::new(key, nonce, 1);
    cipher.apply_keystream(&mut ciphertext);

    // 3. Compute Poly1305 tag
    let mut poly = Poly1305::new(poly_key);

    // Add AAD and padding
    poly.update(aad);
    let aad_pad = (16 - (aad.len() % 16)) % 16;
    if aad_pad > 0 {
        poly.update(&vec![0u8; aad_pad]);
    }

    // Add ciphertext and padding
    poly.update(&ciphertext);
    let ct_pad = (16 - (ciphertext.len() % 16)) % 16;
    if ct_pad > 0 {
        poly.update(&vec![0u8; ct_pad]);
    }

    // Add 8-byte little-endian lengths
    poly.update(&(aad.len() as u64).to_le_bytes());
    poly.update(&(ciphertext.len() as u64).to_le_bytes());

    let tag = poly.finalize();
    (ciphertext, tag)
}

/// Verifies authentication tag in constant time and decrypts ciphertext if valid.
pub fn chacha20_poly1305_decrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; AEAD_TAG_LEN],
) -> Result<Vec<u8>, CryptoError> {
    // 1. Generate one-time Poly1305 key
    let otk_cipher = ChaCha20::new(key, nonce, 0);
    let otk_block = otk_cipher.block();
    let poly_key: &[u8; 32] = otk_block[..32].try_into().unwrap();

    // 2. Compute expected tag
    let mut poly = Poly1305::new(poly_key);

    poly.update(aad);
    let aad_pad = (16 - (aad.len() % 16)) % 16;
    if aad_pad > 0 {
        poly.update(&vec![0u8; aad_pad]);
    }

    poly.update(ciphertext);
    let ct_pad = (16 - (ciphertext.len() % 16)) % 16;
    if ct_pad > 0 {
        poly.update(&vec![0u8; ct_pad]);
    }

    poly.update(&(aad.len() as u64).to_le_bytes());
    poly.update(&(ciphertext.len() as u64).to_le_bytes());

    let computed_tag = poly.finalize();

    // 3. Constant-time comparison
    if !ct_eq(&computed_tag, tag) {
        return Err(CryptoError::AuthenticationFailed);
    }

    // 4. Decrypt ciphertext
    let mut plaintext = ciphertext.to_vec();
    let mut cipher = ChaCha20::new(key, nonce, 1);
    cipher.apply_keystream(&mut plaintext);

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{from_hex, to_hex};

    // RFC 8439 Section 2.8.2 Test Vector
    #[test]
    fn test_rfc8439_section2_8_2() {
        let key_bytes =
            from_hex("1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0").unwrap();
        let key: &[u8; 32] = key_bytes.as_slice().try_into().unwrap();

        let nonce_bytes = from_hex("000000000102030405060708").unwrap();
        let nonce: &[u8; 12] = nonce_bytes.as_slice().try_into().unwrap();

        let aad = from_hex("f33388860000000000004e91").unwrap();

        let plaintext = b"Internet-Drafts are working documents of the Internet Engineering Task Force (IETF), its areas, and its working groups.  Note that other groups may also distribute working documents as Internet-Drafts.";

        let (ciphertext, tag) = chacha20_poly1305_encrypt(key, nonce, &aad, plaintext);

        let expected_ciphertext = "64a0861575861af460f062c79be643bd5e805cfd2741e084ec466445c07682a24f7de14c701c40e1e4cae7446a4429e4b5a4a1302b4e294edcff75693952ddcc3b61a3077140d6e6939aa64ee403bfa1609e4e26419022aa6cb882b1ee288516939de86ef2f6bc2ed49a2f49272f047c7116862e757b44f5ab981e60559d7ee9b6163f396e9c0855eb85000dd1fab2e7dd68c8a40c2c7158ac87b8b3a6714b2eb3502592af24bc5e471d4571a1bd437a120c2004ed325df85d7f3f27f149690d1af9106057f2618f17";
        let expected_tag = "1a1feec92d305ae528cdc23870a65ce1";

        assert_eq!(to_hex(&ciphertext), expected_ciphertext);
        assert_eq!(to_hex(&tag), expected_tag);

        // Decrypt and verify
        let decrypted = chacha20_poly1305_decrypt(key, nonce, &aad, &ciphertext, &tag).unwrap();
        assert_eq!(decrypted, plaintext);

        // Tamper test 1: altered ciphertext must fail
        let mut tampered_ct = ciphertext.clone();
        tampered_ct[0] ^= 0x01;
        assert!(chacha20_poly1305_decrypt(key, nonce, &aad, &tampered_ct, &tag).is_err());

        // Tamper test 2: altered tag must fail
        let mut tampered_tag = tag;
        tampered_tag[0] ^= 0x01;
        assert!(chacha20_poly1305_decrypt(key, nonce, &aad, &ciphertext, &tampered_tag).is_err());

        // Tamper test 3: altered AAD must fail
        let mut tampered_aad = aad.clone();
        tampered_aad[0] ^= 0x01;
        assert!(chacha20_poly1305_decrypt(key, nonce, &tampered_aad, &ciphertext, &tag).is_err());
    }
}
