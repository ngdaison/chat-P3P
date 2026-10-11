// src/types.rs
//! Common domain types, aliases, and pure-Rust helper utilities.

use std::time::{SystemTime, UNIX_EPOCH};

pub const PROTOCOL_MAGIC: [u8; 4] = *b"P2CH";
pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_FRAME_SIZE: usize = 512 * 1024; // 512 KiB
pub const PUBLIC_KEY_LEN: usize = 32;
pub const PRIVATE_KEY_LEN: usize = 32;
pub const SHARED_SECRET_LEN: usize = 32;
pub const PEER_ID_LEN: usize = 32;
pub const MESSAGE_ID_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
pub const SALT_LEN: usize = 16;
pub const TAG_LEN: usize = 16;

/// Gets current Unix epoch timestamp in seconds.
pub fn current_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Converts a byte slice to a lowercase hexadecimal string.
pub fn to_hex(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX_CHARS[(b >> 4) as usize] as char);
        s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    s
}

/// Decodes a hexadecimal string into bytes.
pub fn from_hex(hex_str: &str) -> Result<Vec<u8>, String> {
    let hex_str = hex_str.trim();
    if !hex_str.len().is_multiple_of(2) {
        return Err("Hex string must have an even length".to_string());
    }
    let mut bytes = Vec::with_capacity(hex_str.len() / 2);
    let chars = hex_str.as_bytes();
    for i in (0..chars.len()).step_by(2) {
        let hi = hex_char_to_val(chars[i])
            .ok_or_else(|| format!("Invalid hex character '{}'", chars[i] as char))?;
        let lo = hex_char_to_val(chars[i + 1])
            .ok_or_else(|| format!("Invalid hex character '{}'", chars[i + 1] as char))?;
        bytes.push((hi << 4) | lo);
    }
    Ok(bytes)
}

fn hex_char_to_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Adler-32 checksum implementation in pure standard Rust.
/// Used for integrity verification of binary storage records.
pub fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    const MOD_ADLER: u32 = 65521;

    for &byte in data {
        a = (a + byte as u32) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }

    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_roundtrip() {
        let original = b"Hello, P2P Chat!";
        let encoded = to_hex(original);
        let decoded = from_hex(&encoded).expect("valid hex");
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_invalid_hex() {
        assert!(from_hex("abc").is_err()); // odd length
        assert!(from_hex("zz").is_err()); // invalid chars
    }

    #[test]
    fn test_adler32() {
        let data = b"Wikipedia";
        // Known Adler-32 of "Wikipedia" is 0x11E60398
        assert_eq!(adler32(data), 0x11E60398);
    }
}
