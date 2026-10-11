// src/crypto/hmac.rs
//! HMAC-SHA256 implementation according to RFC 2104 and RFC 4231.

use crate::crypto::sha256::{Sha256, sha256};

const BLOCK_SIZE: usize = 64;
const IPAD: u8 = 0x36;
const OPAD: u8 = 0x5c;

#[derive(Clone)]
pub struct HmacSha256 {
    inner_hasher: Sha256,
    outer_key: [u8; BLOCK_SIZE],
}

impl HmacSha256 {
    pub fn new(key: &[u8]) -> Self {
        let mut key_block = [0u8; BLOCK_SIZE];
        if key.len() > BLOCK_SIZE {
            let hashed_key = sha256(key);
            key_block[..32].copy_from_slice(&hashed_key);
        } else {
            key_block[..key.len()].copy_from_slice(key);
        }

        let mut inner_key = [0u8; BLOCK_SIZE];
        let mut outer_key = [0u8; BLOCK_SIZE];
        for i in 0..BLOCK_SIZE {
            inner_key[i] = key_block[i] ^ IPAD;
            outer_key[i] = key_block[i] ^ OPAD;
        }

        let mut inner_hasher = Sha256::new();
        inner_hasher.update(&inner_key);

        Self {
            inner_hasher,
            outer_key,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.inner_hasher.update(data);
    }

    pub fn finalize(self) -> [u8; 32] {
        let inner_hash = self.inner_hasher.finalize();
        let mut outer_hasher = Sha256::new();
        outer_hasher.update(&self.outer_key);
        outer_hasher.update(&inner_hash);
        outer_hasher.finalize()
    }
}

/// Convenience one-shot HMAC-SHA256 function.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut hmac = HmacSha256::new(key);
    hmac.update(data);
    hmac.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::to_hex;

    // RFC 4231 Test Case 1: Key = 20 bytes of 0x0b, Data = "Hi There"
    #[test]
    fn test_rfc4231_case1() {
        let key = [0x0bu8; 20];
        let data = b"Hi There";
        let result = hmac_sha256(&key, data);
        assert_eq!(
            to_hex(&result),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    // RFC 4231 Test Case 2: Key = "Jefe", Data = "what do ya want for nothing?"
    #[test]
    fn test_rfc4231_case2() {
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let result = hmac_sha256(key, data);
        assert_eq!(
            to_hex(&result),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    // RFC 4231 Test Case 3: Key = 20 bytes of 0xaa, Data = 50 bytes of 0xdd
    #[test]
    fn test_rfc4231_case3() {
        let key = [0xaau8; 20];
        let data = [0xddu8; 50];
        let result = hmac_sha256(&key, &data);
        assert_eq!(
            to_hex(&result),
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
        );
    }
}
