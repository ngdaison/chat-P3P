// src/crypto/constant_time.rs
//! Constant-time operations to avoid timing side-channels.

/// Compares two byte slices in constant time.
/// Returns true if and only if both slices have identical length and contents.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Constant-time conditional swap.
/// If `swap == 1`, swaps values of `a` and `b`.
/// If `swap == 0`, leaves `a` and `b` unchanged.
#[inline]
pub fn cswap(swap: u8, a: &mut [u64; 5], b: &mut [u64; 5]) {
    let mask = 0u64.wrapping_sub(swap as u64);
    for i in 0..5 {
        let t = mask & (a[i] ^ b[i]);
        a[i] ^= t;
        b[i] ^= t;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ct_eq() {
        assert!(ct_eq(b"hello", b"hello"));
        assert!(!ct_eq(b"hello", b"world"));
        assert!(!ct_eq(b"hello", b"hell"));
        assert!(!ct_eq(b"", b"a"));
        assert!(ct_eq(b"", b""));
    }
}
