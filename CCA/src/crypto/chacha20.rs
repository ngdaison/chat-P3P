// src/crypto/chacha20.rs
//! ChaCha20 stream cipher implementation according to RFC 8439.

const CONSTANTS: [u32; 4] = [
    0x61707865, // "expa"
    0x3320646e, // "nd 3"
    0x79622d32, // "2-by"
    0x6b206574, // "te k"
];

pub struct ChaCha20 {
    state: [u32; 16],
}

impl ChaCha20 {
    pub fn new(key: &[u8; 32], nonce: &[u8; 12], counter: u32) -> Self {
        let mut state = [0u32; 16];
        state[0..4].copy_from_slice(&CONSTANTS);

        for i in 0..8 {
            state[4 + i] =
                u32::from_le_bytes([key[i * 4], key[i * 4 + 1], key[i * 4 + 2], key[i * 4 + 3]]);
        }

        state[12] = counter;

        for i in 0..3 {
            state[13 + i] = u32::from_le_bytes([
                nonce[i * 4],
                nonce[i * 4 + 1],
                nonce[i * 4 + 2],
                nonce[i * 4 + 3],
            ]);
        }

        Self { state }
    }

    #[inline]
    fn quarter_round(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
        s[a] = s[a].wrapping_add(s[b]);
        s[d] ^= s[a];
        s[d] = s[d].rotate_left(16);

        s[c] = s[c].wrapping_add(s[d]);
        s[b] ^= s[c];
        s[b] = s[b].rotate_left(12);

        s[a] = s[a].wrapping_add(s[b]);
        s[d] ^= s[a];
        s[d] = s[d].rotate_left(8);

        s[c] = s[c].wrapping_add(s[d]);
        s[b] ^= s[c];
        s[b] = s[b].rotate_left(7);
    }

    /// Generates one 64-byte keystream block for the current state counter.
    pub fn block(&self) -> [u8; 64] {
        let mut working = self.state;

        for _ in 0..10 {
            // Column round
            Self::quarter_round(&mut working, 0, 4, 8, 12);
            Self::quarter_round(&mut working, 1, 5, 9, 13);
            Self::quarter_round(&mut working, 2, 6, 10, 14);
            Self::quarter_round(&mut working, 3, 7, 11, 15);

            // Diagonal round
            Self::quarter_round(&mut working, 0, 5, 10, 15);
            Self::quarter_round(&mut working, 1, 6, 11, 12);
            Self::quarter_round(&mut working, 2, 7, 8, 13);
            Self::quarter_round(&mut working, 3, 4, 9, 14);
        }

        let mut block = [0u8; 64];
        for i in 0..16 {
            let word = working[i].wrapping_add(self.state[i]);
            block[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
        }
        block
    }

    /// Encrypts or decrypts in-place with ChaCha20 stream cipher.
    pub fn apply_keystream(&mut self, data: &mut [u8]) {
        for chunk in data.chunks_mut(64) {
            let key_block = self.block();
            for (d, k) in chunk.iter_mut().zip(key_block.iter()) {
                *d ^= *k;
            }
            self.state[12] = self.state[12].wrapping_add(1);
        }
    }
}

/// Convenience function to encrypt/decrypt a buffer.
pub fn chacha20_xor(key: &[u8; 32], nonce: &[u8; 12], counter: u32, data: &mut [u8]) {
    let mut cipher = ChaCha20::new(key, nonce, counter);
    cipher.apply_keystream(data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{from_hex, to_hex};

    // RFC 8439 Section 2.4.2 Test Vector
    #[test]
    fn test_rfc8439_section2_4_2() {
        let key_bytes =
            from_hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f").unwrap();
        let key: &[u8; 32] = key_bytes.as_slice().try_into().unwrap();
        let nonce_bytes = from_hex("000000000000004a00000000").unwrap();
        let nonce: &[u8; 12] = nonce_bytes.as_slice().try_into().unwrap();
        let counter = 1;

        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let mut buffer = plaintext.to_vec();

        chacha20_xor(key, nonce, counter, &mut buffer);

        let expected_ciphertext = "6e2e359a2568f98041ba0728dd0d6981e97e7aec1d4360c20a27afccfd9fae0bf91b65c5524733ab8f593dabcd62b3571639d624e65152ab8f530c359f0861d807ca0dbf500d6a6156a38e088a22b65e52bc514d16ccf806818ce91ab77937365af90bbf74a35be6b40b8eedf2785e42874d";
        assert_eq!(to_hex(&buffer), expected_ciphertext);

        // Decrypt back to verify symmetry
        chacha20_xor(key, nonce, counter, &mut buffer);
        assert_eq!(buffer, plaintext);
    }
}
