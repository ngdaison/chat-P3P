// src/crypto/x25519.rs
//! Pure Rust implementation of Curve25519 / X25519 key agreement (RFC 7748).

use crate::crypto::constant_time::{cswap, ct_eq};
use crate::error::CryptoError;

const MASK51: u64 = (1u64 << 51) - 1;

pub(crate) type Fe = [u64; 5];

pub(crate) const FE_ZERO: Fe = [0, 0, 0, 0, 0];
pub(crate) const FE_ONE: Fe = [1, 0, 0, 0, 0];
const A24: u64 = 121665; // (486662 - 2) / 4

pub(crate) fn fe_from_bytes(b: &[u8; 32]) -> Fe {
    let f0 = u64::from_le_bytes(b[0..8].try_into().unwrap()) & MASK51;
    let f1 = (u64::from_le_bytes(b[6..14].try_into().unwrap()) >> 3) & MASK51;
    let f2 = (u64::from_le_bytes(b[12..20].try_into().unwrap()) >> 6) & MASK51;
    let f3 = (u64::from_le_bytes(b[19..27].try_into().unwrap()) >> 1) & MASK51;
    let f4 = (u64::from_le_bytes(b[24..32].try_into().unwrap()) >> 12) & ((1u64 << 51) - 1);
    [f0, f1, f2, f3, f4]
}

pub(crate) fn fe_to_bytes(h_in: &Fe) -> [u8; 32] {
    let mut h = *h_in;

    // Strict reduction mod 2^255 - 19
    let mut c = h[0] >> 51;
    h[0] &= MASK51;
    h[1] += c;
    c = h[1] >> 51;
    h[1] &= MASK51;
    h[2] += c;
    c = h[2] >> 51;
    h[2] &= MASK51;
    h[3] += c;
    c = h[3] >> 51;
    h[3] &= MASK51;
    h[4] += c;
    c = h[4] >> 51;
    h[4] &= MASK51;
    h[0] += c * 19;
    c = h[0] >> 51;
    h[0] &= MASK51;
    h[1] += c;

    // Subtract p = 2^255 - 19
    let mut q = [h[0].wrapping_add(19), h[1], h[2], h[3], h[4]];
    let mut carry = q[0] >> 51;
    q[0] &= MASK51;
    q[1] += carry;
    carry = q[1] >> 51;
    q[1] &= MASK51;
    q[2] += carry;
    carry = q[2] >> 51;
    q[2] &= MASK51;
    q[3] += carry;
    carry = q[3] >> 51;
    q[3] &= MASK51;
    q[4] += carry;
    carry = q[4] >> 51;
    q[4] &= MASK51;

    let mask = 0u64.wrapping_sub(carry);
    for i in 0..5 {
        h[i] = (h[i] & !mask) | (q[i] & mask);
    }

    let w0 = h[0] | (h[1] << 51);
    let w1 = (h[1] >> 13) | (h[2] << 38);
    let w2 = (h[2] >> 26) | (h[3] << 25);
    let w3 = (h[3] >> 39) | (h[4] << 12);

    let mut out = [0u8; 32];
    out[0..8].copy_from_slice(&w0.to_le_bytes());
    out[8..16].copy_from_slice(&w1.to_le_bytes());
    out[16..24].copy_from_slice(&w2.to_le_bytes());
    out[24..32].copy_from_slice(&w3.to_le_bytes());
    out
}

pub(crate) fn fe_add(a: &Fe, b: &Fe) -> Fe {
    [
        a[0] + b[0],
        a[1] + b[1],
        a[2] + b[2],
        a[3] + b[3],
        a[4] + b[4],
    ]
}

pub(crate) fn fe_sub(a: &Fe, b: &Fe) -> Fe {
    // 2 * p in limbs to avoid underflow
    const P2_0: u64 = 2 * MASK51 - 36;
    const P2_1: u64 = 2 * MASK51;
    [
        (a[0] + P2_0) - b[0],
        (a[1] + P2_1) - b[1],
        (a[2] + P2_1) - b[2],
        (a[3] + P2_1) - b[3],
        (a[4] + P2_1) - b[4],
    ]
}

pub(crate) fn fe_mul(a: &Fe, b: &Fe) -> Fe {
    let a0 = a[0] as u128;
    let a1 = a[1] as u128;
    let a2 = a[2] as u128;
    let a3 = a[3] as u128;
    let a4 = a[4] as u128;

    let b0 = b[0] as u128;
    let b1 = b[1] as u128;
    let b2 = b[2] as u128;
    let b3 = b[3] as u128;
    let b4 = b[4] as u128;

    let a1_19 = a1 * 19;
    let a2_19 = a2 * 19;
    let a3_19 = a3 * 19;
    let a4_19 = a4 * 19;

    let r0 = a0 * b0 + a1_19 * b4 + a2_19 * b3 + a3_19 * b2 + a4_19 * b1;
    let mut r1 = a0 * b1 + a1 * b0 + a2_19 * b4 + a3_19 * b3 + a4_19 * b2;
    let mut r2 = a0 * b2 + a1 * b1 + a2 * b0 + a3_19 * b4 + a4_19 * b3;
    let mut r3 = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0 + a4_19 * b4;
    let mut r4 = a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;

    let mask = MASK51 as u128;
    let mut c: u64;
    let mut out = [0u64; 5];

    c = (r0 >> 51) as u64;
    out[0] = (r0 & mask) as u64;
    r1 += c as u128;

    c = (r1 >> 51) as u64;
    out[1] = (r1 & mask) as u64;
    r2 += c as u128;

    c = (r2 >> 51) as u64;
    out[2] = (r2 & mask) as u64;
    r3 += c as u128;

    c = (r3 >> 51) as u64;
    out[3] = (r3 & mask) as u64;
    r4 += c as u128;

    c = (r4 >> 51) as u64;
    out[4] = (r4 & mask) as u64;

    out[0] += c * 19;
    c = out[0] >> 51;
    out[0] &= MASK51;
    out[1] += c;

    out
}

pub(crate) fn fe_sqr(a: &Fe) -> Fe {
    fe_mul(a, a)
}

pub(crate) fn fe_mul_u32(a: &Fe, b: u64) -> Fe {
    let a0 = a[0] as u128 * b as u128;
    let a1 = a[1] as u128 * b as u128;
    let a2 = a[2] as u128 * b as u128;
    let a3 = a[3] as u128 * b as u128;
    let a4 = a[4] as u128 * b as u128;

    let mask = MASK51 as u128;
    let mut c = (a0 >> 51) as u64;
    let mut out = [0u64; 5];
    out[0] = (a0 & mask) as u64;

    let r1 = a1 + c as u128;
    c = (r1 >> 51) as u64;
    out[1] = (r1 & mask) as u64;

    let r2 = a2 + c as u128;
    c = (r2 >> 51) as u64;
    out[2] = (r2 & mask) as u64;

    let r3 = a3 + c as u128;
    c = (r3 >> 51) as u64;
    out[3] = (r3 & mask) as u64;

    let r4 = a4 + c as u128;
    c = (r4 >> 51) as u64;
    out[4] = (r4 & mask) as u64;

    out[0] += c * 19;
    c = out[0] >> 51;
    out[0] &= MASK51;
    out[1] += c;

    out
}

pub(crate) fn fe_invert(z: &Fe) -> Fe {
    fn sqr_n(mut a: Fe, n: usize) -> Fe {
        for _ in 0..n {
            a = fe_sqr(&a);
        }
        a
    }

    let z2 = fe_sqr(z);
    let z8 = sqr_n(z2, 2);
    let z9 = fe_mul(&z8, z);
    let z11 = fe_mul(&z9, &z2);
    let z22 = fe_sqr(&z11);
    let z31 = fe_mul(&z22, &z9); // 2^5 - 1
    let z2_10 = fe_mul(&sqr_n(z31, 5), &z31); // 2^10 - 1
    let z2_20 = fe_mul(&sqr_n(z2_10, 10), &z2_10); // 2^20 - 1
    let z2_40 = fe_mul(&sqr_n(z2_20, 20), &z2_20); // 2^40 - 1
    let z2_50 = fe_mul(&sqr_n(z2_40, 10), &z2_10); // 2^50 - 1
    let z2_100 = fe_mul(&sqr_n(z2_50, 50), &z2_50); // 2^100 - 1
    let z2_200 = fe_mul(&sqr_n(z2_100, 100), &z2_100); // 2^200 - 1
    let z2_250 = fe_mul(&sqr_n(z2_200, 50), &z2_50); // 2^250 - 1
    fe_mul(&sqr_n(z2_250, 5), &z11) // 2^255 - 21
}

/// Curve25519 scalar multiplication according to RFC 7748.
pub fn x25519(scalar: &[u8; 32], point_u: &[u8; 32]) -> Result<[u8; 32], CryptoError> {
    // 1. Clamp scalar
    let mut k = *scalar;
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;

    // 2. Clear highest bit of u-coordinate
    let mut u_bytes = *point_u;
    u_bytes[31] &= 0x7f;
    let x1 = fe_from_bytes(&u_bytes);

    // 3. Montgomery ladder
    let mut x2 = FE_ONE;
    let mut z2 = FE_ZERO;
    let mut x3 = x1;
    let mut z3 = FE_ONE;
    let mut swap = 0u8;

    for i in (0..255).rev() {
        let bit = (k[i / 8] >> (i % 8)) & 1;
        swap ^= bit;
        cswap(swap, &mut x2, &mut x3);
        cswap(swap, &mut z2, &mut z3);
        swap = bit;

        let a = fe_add(&x2, &z2);
        let aa = fe_sqr(&a);
        let b = fe_sub(&x2, &z2);
        let bb = fe_sqr(&b);
        let e = fe_sub(&aa, &bb);
        let c = fe_add(&x3, &z3);
        let d = fe_sub(&x3, &z3);
        let da = fe_mul(&d, &a);
        let cb = fe_mul(&c, &b);

        x3 = fe_sqr(&fe_add(&da, &cb));
        z3 = fe_mul(&x1, &fe_sqr(&fe_sub(&da, &cb)));
        x2 = fe_mul(&aa, &bb);
        z2 = fe_mul(&e, &fe_add(&aa, &fe_mul_u32(&e, A24)));
    }

    cswap(swap, &mut x2, &mut x3);
    cswap(swap, &mut z2, &mut z3);

    let z2_inv = fe_invert(&z2);
    let result = fe_mul(&x2, &z2_inv);
    let out = fe_to_bytes(&result);

    // Reject weak/invalid all-zero shared secrets
    if ct_eq(&out, &[0u8; 32]) {
        return Err(CryptoError::WeakSharedSecret);
    }

    Ok(out)
}

/// Computes the X25519 public key corresponding to a private key scalar.
pub fn x25519_base_point(private_key: &[u8; 32]) -> [u8; 32] {
    let mut base_point = [0u8; 32];
    base_point[0] = 9; // u = 9
    x25519(private_key, &base_point)
        .expect("Base point scalar multiplication should never produce zero")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{from_hex, to_hex};

    // RFC 7748 Section 5.2 Test Vector
    #[test]
    fn test_rfc7748_section5_2() {
        let alice_priv_bytes =
            from_hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a").unwrap();
        let alice_priv: &[u8; 32] = alice_priv_bytes.as_slice().try_into().unwrap();
        let expected_alice_pub = "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a";

        let alice_pub = x25519_base_point(alice_priv);
        assert_eq!(to_hex(&alice_pub), expected_alice_pub);

        let bob_priv_bytes =
            from_hex("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb").unwrap();
        let bob_priv: &[u8; 32] = bob_priv_bytes.as_slice().try_into().unwrap();
        let expected_bob_pub = "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f";

        let bob_pub = x25519_base_point(bob_priv);
        assert_eq!(to_hex(&bob_pub), expected_bob_pub);

        let expected_shared = "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742";

        // Alice computes shared secret using Bob's public key
        let shared_alice = x25519(alice_priv, &bob_pub).unwrap();
        assert_eq!(to_hex(&shared_alice), expected_shared);

        // Bob computes shared secret using Alice's public key
        let shared_bob = x25519(bob_priv, &alice_pub).unwrap();
        assert_eq!(to_hex(&shared_bob), expected_shared);
    }
}
