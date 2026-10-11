// src/crypto/random.rs
//! Cryptographically secure random number generation using OS CSPRNG.
//!
//! Windows: Uses `RtlGenRandom` (`SystemFunction036` in Advapi32).
//! Unix / Linux / macOS: Uses `/dev/urandom`.

use crate::error::CryptoError;

#[cfg(windows)]
#[link(name = "advapi32")]
unsafe extern "system" {
    fn SystemFunction036(random_buffer: *mut u8, random_buffer_length: u32) -> u8;
}

/// Fills the given destination slice with cryptographically secure random bytes from the OS.
pub fn fill_bytes(dest: &mut [u8]) -> Result<(), CryptoError> {
    if dest.is_empty() {
        return Ok(());
    }

    #[cfg(windows)]
    {
        // Safe FFI call to Windows RtlGenRandom
        let len = dest.len() as u32;
        let ret = unsafe { SystemFunction036(dest.as_mut_ptr(), len) };
        if ret == 0 {
            return Err(CryptoError::RandomGenerationFailed(
                "Windows SystemFunction036 (RtlGenRandom) returned failure".to_string(),
            ));
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open("/dev/urandom").map_err(|e| {
            CryptoError::RandomGenerationFailed(format!("Failed to open /dev/urandom: {e}"))
        })?;
        file.read_exact(dest).map_err(|e| {
            CryptoError::RandomGenerationFailed(format!("Failed to read from /dev/urandom: {e}"))
        })?;
        Ok(())
    }
}

/// Generates a secure random 32-byte array (used for keys, seeds, message IDs).
pub fn random_bytes_32() -> Result<[u8; 32], CryptoError> {
    let mut buf = [0u8; 32];
    fill_bytes(&mut buf)?;
    Ok(buf)
}

/// Generates a secure random 16-byte array (used for salts).
pub fn random_bytes_16() -> Result<[u8; 16], CryptoError> {
    let mut buf = [0u8; 16];
    fill_bytes(&mut buf)?;
    Ok(buf)
}

/// Generates a secure random 12-byte array (used for ChaCha20 nonces).
pub fn random_bytes_12() -> Result<[u8; 12], CryptoError> {
    let mut buf = [0u8; 12];
    fill_bytes(&mut buf)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_bytes() {
        let r1 = random_bytes_32().expect("random generation should succeed");
        let r2 = random_bytes_32().expect("random generation should succeed");
        assert_ne!(r1, r2, "Two 32-byte random arrays should not be identical");
        assert_ne!(r1, [0u8; 32], "Random bytes should not be all zeros");
    }
}
