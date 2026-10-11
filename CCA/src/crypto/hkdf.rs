// src/crypto/hkdf.rs
//! HKDF-SHA256 key derivation according to RFC 5869.

use crate::crypto::hmac::{HmacSha256, hmac_sha256};
use crate::error::CryptoError;

/// Step 1: HKDF-Extract(salt, ikm) -> PRK
pub fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    if salt.is_empty() {
        let zero_salt = [0u8; 32];
        hmac_sha256(&zero_salt, ikm)
    } else {
        hmac_sha256(salt, ikm)
    }
}

/// Step 2: HKDF-Expand(prk, info, okm)
pub fn hkdf_expand(prk: &[u8], info: &[u8], okm: &mut [u8]) -> Result<(), CryptoError> {
    let out_len = okm.len();
    if out_len == 0 {
        return Ok(());
    }

    let n = out_len.div_ceil(32);
    if n > 255 {
        return Err(CryptoError::HkdfExpandTooLong);
    }

    let mut t = [0u8; 32];
    let mut written = 0;

    for i in 1..=n as u8 {
        let mut hmac = HmacSha256::new(prk);
        if i > 1 {
            hmac.update(&t);
        }
        hmac.update(info);
        hmac.update(&[i]);
        t = hmac.finalize();

        let to_copy = std::cmp::min(32, out_len - written);
        okm[written..written + to_copy].copy_from_slice(&t[..to_copy]);
        written += to_copy;
    }

    Ok(())
}

/// Combined HKDF-Extract-and-Expand.
pub fn hkdf(salt: &[u8], ikm: &[u8], info: &[u8], okm: &mut [u8]) -> Result<(), CryptoError> {
    let prk = hkdf_extract(salt, ikm);
    hkdf_expand(&prk, info, okm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{from_hex, to_hex};

    // RFC 5869 Test Case 1
    #[test]
    fn test_rfc5869_case1() {
        let ikm = from_hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b").unwrap();
        let salt = from_hex("000102030405060708090a0b0c").unwrap();
        let info = from_hex("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let expected_prk = "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5";
        let expected_okm =
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865";

        let prk = hkdf_extract(&salt, &ikm);
        assert_eq!(to_hex(&prk), expected_prk);

        let mut okm = vec![0u8; 42];
        hkdf_expand(&prk, &info, &mut okm).unwrap();
        assert_eq!(to_hex(&okm), expected_okm);
    }

    // RFC 5869 Test Case 2
    #[test]
    fn test_rfc5869_case2() {
        let ikm = from_hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f").unwrap();
        let salt = from_hex("606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeaf").unwrap();
        let info = from_hex("b0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff").unwrap();
        let expected_prk = "06a6b88c5853361a06104c9ceb35b45cef760014904671014a193f40c15fc244";
        let expected_okm = "b11e398dc80327a1c8e7f78c596a49344f012eda2d4efad8a050cc4c19afa97c59045a99cac7827271cb41c65e590e09da3275600c2f09b8367793a9aca3db71cc30c58179ec3e87c14c01d5c1f3434f1d87";

        let prk = hkdf_extract(&salt, &ikm);
        assert_eq!(to_hex(&prk), expected_prk);

        let mut okm = vec![0u8; 82];
        hkdf_expand(&prk, &info, &mut okm).unwrap();
        assert_eq!(to_hex(&okm), expected_okm);
    }
}
