// src/crypto/ed25519.rs
//! Pure Rust implementation of Ed25519 Digital Signatures (RFC 8032).
//! Zero external dependencies.

use crate::crypto::sha512::Sha512;
use crate::crypto::x25519::{
    FE_ONE, FE_ZERO, Fe, fe_add, fe_from_bytes, fe_invert, fe_mul, fe_sqr, fe_sub, fe_to_bytes,
};

pub const ED25519_PUBLIC_KEY_LEN: usize = 32;
pub const ED25519_SECRET_KEY_LEN: usize = 32;
pub const ED25519_SIGNATURE_LEN: usize = 64;

// Edwards d = -121665 / 121666 mod (2^255 - 19)
const D_BYTES: [u8; 32] = [
    0xa3, 0x78, 0x59, 0x13, 0xca, 0x4d, 0xeb, 0x75, 0xab, 0xd8, 0x41, 0x41, 0x4d, 0x0a, 0x70, 0x00,
    0x98, 0xe8, 0x79, 0x77, 0x79, 0x40, 0xc7, 0x8c, 0x73, 0xfe, 0x6f, 0x2b, 0xee, 0x6c, 0x03, 0x52,
];

// 2 * d mod (2^255 - 19)
const TWO_D_BYTES: [u8; 32] = [
    0x59, 0xf1, 0xb2, 0x26, 0x94, 0x9b, 0xd6, 0xeb, 0x56, 0xb1, 0x83, 0x82, 0x9a, 0x14, 0xe0, 0x00,
    0x30, 0xd1, 0xf3, 0xee, 0xf2, 0x80, 0x8e, 0x19, 0xe7, 0xfc, 0xdf, 0x56, 0xdc, 0xd9, 0x06, 0x24,
];

// Base point B coordinates
const BASE_X_BYTES: [u8; 32] = [
    0x1a, 0xd5, 0x25, 0x8f, 0x60, 0x2d, 0x56, 0xc9, 0xb2, 0xa7, 0x25, 0x95, 0x60, 0xc7, 0x2c, 0x69,
    0x5c, 0xdc, 0xd6, 0xfd, 0x31, 0xe2, 0xa4, 0xc0, 0xfe, 0x53, 0x6e, 0xcd, 0xd3, 0x36, 0x69, 0x21,
];

const BASE_Y_BYTES: [u8; 32] = [
    0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
];

// sqrt(-1) mod (2^255 - 19) = 2^((p-1)/4) mod p
const SQRT_M1_BYTES: [u8; 32] = [
    0xb0, 0xa0, 0x0e, 0x4a, 0x27, 0x1b, 0xee, 0xc4, 0x78, 0xe4, 0x2f, 0xad, 0x06, 0x18, 0x43, 0x2f,
    0xa7, 0xd7, 0xfb, 0x3d, 0x99, 0x00, 0x4d, 0x2b, 0x0b, 0xdf, 0xc1, 0x4f, 0x80, 0x24, 0x83, 0x2b,
];

// Group order L = 2^252 + 27742317777372353535851937790883648493
const L_WORDS: [u64; 4] = [
    0x5812631a5cf5d3ed,
    0x14def9dea2f79cd6,
    0x0000000000000000,
    0x1000000000000000,
];

#[derive(Clone, Copy)]
struct Point {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

impl Point {
    const IDENTITY: Point = Point {
        x: FE_ZERO,
        y: FE_ONE,
        z: FE_ONE,
        t: FE_ZERO,
    };

    fn base() -> Point {
        let x = fe_from_bytes(&BASE_X_BYTES);
        let y = fe_from_bytes(&BASE_Y_BYTES);
        let z = FE_ONE;
        let t = fe_mul(&x, &y);
        Point { x, y, z, t }
    }

    fn double(&self) -> Point {
        let a = fe_sqr(&self.x);
        let b = fe_sqr(&self.y);
        let fe_two: Fe = [2, 0, 0, 0, 0];
        let c = fe_mul(&fe_sqr(&self.z), &fe_two);
        let d = fe_sub(&FE_ZERO, &a);
        let xy = fe_add(&self.x, &self.y);
        let e = fe_sub(&fe_sub(&fe_sqr(&xy), &a), &b);
        let g = fe_add(&d, &b);
        let f = fe_sub(&g, &c);
        let h = fe_sub(&d, &b);
        Point {
            x: fe_mul(&e, &f),
            y: fe_mul(&g, &h),
            z: fe_mul(&f, &g),
            t: fe_mul(&e, &h),
        }
    }

    fn add(&self, other: &Point) -> Point {
        let a = fe_mul(&fe_sub(&self.y, &self.x), &fe_sub(&other.y, &other.x));
        let b = fe_mul(&fe_add(&self.y, &self.x), &fe_add(&other.y, &other.x));
        let two_d = fe_from_bytes(&TWO_D_BYTES);
        let fe_two: Fe = [2, 0, 0, 0, 0];
        let c = fe_mul(&fe_mul(&self.t, &other.t), &two_d);
        let d = fe_mul(&fe_mul(&self.z, &other.z), &fe_two);
        let e = fe_sub(&b, &a);
        let f = fe_sub(&d, &c);
        let g = fe_add(&d, &c);
        let h = fe_add(&b, &a);
        Point {
            x: fe_mul(&e, &f),
            y: fe_mul(&g, &h),
            z: fe_mul(&f, &g),
            t: fe_mul(&e, &h),
        }
    }

    fn scalar_mul(&self, scalar: &[u8; 32]) -> Point {
        let mut r = Point::IDENTITY;
        for i in (0..256).rev() {
            r = r.double();
            let bit = (scalar[i / 8] >> (i % 8)) & 1;
            if bit == 1 {
                r = r.add(self);
            }
        }
        r
    }

    fn to_bytes(self) -> [u8; 32] {
        let z_inv = fe_invert(&self.z);
        let x = fe_mul(&self.x, &z_inv);
        let y = fe_mul(&self.y, &z_inv);
        let mut bytes = fe_to_bytes(&y);
        let x_bytes = fe_to_bytes(&x);
        bytes[31] |= (x_bytes[0] & 1) << 7;
        bytes
    }

    fn from_bytes(bytes: &[u8; 32]) -> Option<Point> {
        let x_sign = (bytes[31] >> 7) & 1;
        let mut y_bytes = *bytes;
        y_bytes[31] &= 0x7f;
        let y = fe_from_bytes(&y_bytes);
        let y_sqr = fe_sqr(&y);
        let u = fe_sub(&y_sqr, &FE_ONE);
        let d = fe_from_bytes(&D_BYTES);
        let v = fe_add(&fe_mul(&d, &y_sqr), &FE_ONE);

        let v_inv = fe_invert(&v);
        let u_div_v = fe_mul(&u, &v_inv);
        let u_div_v_bytes = fe_to_bytes(&u_div_v);
        if u_div_v_bytes == [0u8; 32] {
            if x_sign != 0 {
                return None;
            }
            return Some(Point {
                x: FE_ZERO,
                y,
                z: FE_ONE,
                t: FE_ZERO,
            });
        }

        // x = (u/v)^((p+3)/8)
        let mut x = fe_pow_p38(&u_div_v);
        let mut check = fe_mul(&fe_sqr(&x), &v);
        if fe_to_bytes(&check) != fe_to_bytes(&u) {
            let sqrt_m1 = fe_from_bytes(&SQRT_M1_BYTES);
            x = fe_mul(&x, &sqrt_m1);
            check = fe_mul(&fe_sqr(&x), &v);
            if fe_to_bytes(&check) != fe_to_bytes(&u) {
                return None;
            }
        }

        let x_bytes = fe_to_bytes(&x);
        if (x_bytes[0] & 1) != x_sign {
            x = fe_sub(&FE_ZERO, &x);
        }

        let t = fe_mul(&x, &y);
        Some(Point { x, y, z: FE_ONE, t })
    }
}

// Computes z^((p+3)/8) mod (2^255 - 19)
fn fe_pow_p38(z: &Fe) -> Fe {
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
    let z31 = fe_mul(&z22, &z9);
    let z2_10 = fe_mul(&sqr_n(z31, 5), &z31);
    let z2_20 = fe_mul(&sqr_n(z2_10, 10), &z2_10);
    let z2_40 = fe_mul(&sqr_n(z2_20, 20), &z2_20);
    let z2_50 = fe_mul(&sqr_n(z2_40, 10), &z2_10);
    let z2_100 = fe_mul(&sqr_n(z2_50, 50), &z2_50);
    let z2_200 = fe_mul(&sqr_n(z2_100, 100), &z2_100);
    let z2_250 = fe_mul(&sqr_n(z2_200, 50), &z2_50);
    let z2_251 = fe_mul(&fe_sqr(&z2_250), z);
    fe_sqr(&z2_251)
}

/// Reduces a 512-bit integer (little endian 64 bytes) modulo group order L.
fn reduce_mod_l(input: &[u8; 64]) -> [u8; 32] {
    let mut rem = [0u64; 5]; // 320 bits accumulator

    for bit_idx in (0..512).rev() {
        // Shift rem left by 1 bit
        let mut carry = 0u64;
        for word in rem.iter_mut() {
            let next_carry = *word >> 63;
            *word = (*word << 1) | carry;
            carry = next_carry;
        }

        let bit = ((input[bit_idx / 8] >> (bit_idx % 8)) & 1) as u64;
        rem[0] |= bit;

        // If rem >= L, rem -= L
        if rem_gte_l(&rem) {
            rem_sub_l(&mut rem);
        }
    }

    let mut out = [0u8; 32];
    out[0..8].copy_from_slice(&rem[0].to_le_bytes());
    out[8..16].copy_from_slice(&rem[1].to_le_bytes());
    out[16..24].copy_from_slice(&rem[2].to_le_bytes());
    out[24..32].copy_from_slice(&rem[3].to_le_bytes());
    out
}

fn rem_gte_l(rem: &[u64; 5]) -> bool {
    if rem[4] > 0 {
        return true;
    }
    for i in (0..4).rev() {
        if rem[i] > L_WORDS[i] {
            return true;
        }
        if rem[i] < L_WORDS[i] {
            return false;
        }
    }
    true
}

fn rem_sub_l(rem: &mut [u64; 5]) {
    let mut borrow = 0u64;
    for i in 0..4 {
        let diff = (rem[i] as u128)
            .wrapping_sub(L_WORDS[i] as u128)
            .wrapping_sub(borrow as u128);
        rem[i] = diff as u64;
        borrow = if diff > u64::MAX as u128 { 1 } else { 0 };
    }
    if rem[4] >= borrow {
        rem[4] -= borrow;
    } else {
        rem[4] = 0;
    }
}

/// Computes (a + b * c) mod L for 32-byte little endian integers.
fn mul_add_mod_l(a: &[u8; 32], b: &[u8; 32], c: &[u8; 32]) -> [u8; 32] {
    // 256-bit * 256-bit = 512-bit product
    let mut prod = [0u64; 8];
    for i in 0..4 {
        let bi = u64::from_le_bytes(b[i * 8..(i + 1) * 8].try_into().unwrap()) as u128;
        let mut carry = 0u128;
        for j in 0..4 {
            let cj = u64::from_le_bytes(c[j * 8..(j + 1) * 8].try_into().unwrap()) as u128;
            let sum = prod[i + j] as u128 + bi * cj + carry;
            prod[i + j] = sum as u64;
            carry = sum >> 64;
        }
        prod[i + 4] += carry as u64;
    }

    // Add a (256 bits) to product (512 bits)
    let mut carry = 0u128;
    for i in 0..4 {
        let ai = u64::from_le_bytes(a[i * 8..(i + 1) * 8].try_into().unwrap()) as u128;
        let sum = prod[i] as u128 + ai + carry;
        prod[i] = sum as u64;
        carry = sum >> 64;
    }
    let mut idx = 4;
    while carry > 0 && idx < 8 {
        let sum = prod[idx] as u128 + carry;
        prod[idx] = sum as u64;
        carry = sum >> 64;
        idx += 1;
    }

    // Convert prod to 64 bytes and reduce mod L
    let mut bytes = [0u8; 64];
    for (i, word) in prod.iter().enumerate() {
        bytes[i * 8..(i + 1) * 8].copy_from_slice(&word.to_le_bytes());
    }
    reduce_mod_l(&bytes)
}

/// Derives the 32-byte Ed25519 public key from a 32-byte secret seed.
pub fn ed25519_public_key(secret_key: &[u8; 32]) -> [u8; 32] {
    let h = Sha512::digest(secret_key);
    let mut a = [0u8; 32];
    a.copy_from_slice(&h[0..32]);
    a[0] &= 248;
    a[31] &= 127;
    a[31] |= 64;

    let a_point = Point::base().scalar_mul(&a);
    a_point.to_bytes()
}

/// Signs a message using Ed25519 according to RFC 8032.
pub fn ed25519_sign(secret_key: &[u8; 32], message: &[u8]) -> [u8; 64] {
    let h = Sha512::digest(secret_key);
    let mut a = [0u8; 32];
    a.copy_from_slice(&h[0..32]);
    a[0] &= 248;
    a[31] &= 127;
    a[31] |= 64;

    let pubkey = Point::base().scalar_mul(&a).to_bytes();

    // r = SHA-512(h[32..64] || message) mod L
    let mut hasher = Sha512::new();
    hasher.update(&h[32..64]);
    hasher.update(message);
    let r_digest = hasher.finalize();
    let r = reduce_mod_l(&r_digest);

    // R = r * B
    let r_point = Point::base().scalar_mul(&r);
    let r_bytes = r_point.to_bytes();

    // k = SHA-512(R || pubkey || message) mod L
    let mut hasher = Sha512::new();
    hasher.update(&r_bytes);
    hasher.update(&pubkey);
    hasher.update(message);
    let k_digest = hasher.finalize();
    let k = reduce_mod_l(&k_digest);

    // S = (r + k * a) mod L
    let s_bytes = mul_add_mod_l(&r, &k, &a);

    let mut sig = [0u8; 64];
    sig[0..32].copy_from_slice(&r_bytes);
    sig[32..64].copy_from_slice(&s_bytes);
    sig
}

/// Verifies an Ed25519 signature according to RFC 8032.
pub fn ed25519_verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let r_bytes: [u8; 32] = signature[0..32].try_into().unwrap();
    let s_bytes: [u8; 32] = signature[32..64].try_into().unwrap();

    // Check S < L
    let s_words = [
        u64::from_le_bytes(s_bytes[0..8].try_into().unwrap()),
        u64::from_le_bytes(s_bytes[8..16].try_into().unwrap()),
        u64::from_le_bytes(s_bytes[16..24].try_into().unwrap()),
        u64::from_le_bytes(s_bytes[24..32].try_into().unwrap()),
        0u64,
    ];
    if rem_gte_l(&s_words) {
        return false;
    }

    let a_point = match Point::from_bytes(public_key) {
        Some(p) => p,
        None => return false,
    };

    let r_point = match Point::from_bytes(&r_bytes) {
        Some(p) => p,
        None => return false,
    };

    // k = SHA-512(R || public_key || message) mod L
    let mut hasher = Sha512::new();
    hasher.update(&r_bytes);
    hasher.update(public_key);
    hasher.update(message);
    let k = reduce_mod_l(&hasher.finalize());

    // Check: S * B == R + k * A
    let sb = Point::base().scalar_mul(&s_bytes);
    let ka = a_point.scalar_mul(&k);
    let r_ka = r_point.add(&ka);

    sb.to_bytes() == r_ka.to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{from_hex, to_hex};

    // RFC 8032 Section 7.1 Test Vector 1
    #[test]
    fn test_rfc8032_vector_1() {
        let sk_hex = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
        let pk_hex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        let sig_hex = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

        let sk: [u8; 32] = from_hex(sk_hex).unwrap().try_into().unwrap();
        let expected_pk: [u8; 32] = from_hex(pk_hex).unwrap().try_into().unwrap();
        let expected_sig: [u8; 64] = from_hex(sig_hex).unwrap().try_into().unwrap();

        let pk = ed25519_public_key(&sk);
        assert_eq!(to_hex(&pk), pk_hex);
        assert_eq!(pk, expected_pk);

        let sig = ed25519_sign(&sk, b"");
        assert_eq!(to_hex(&sig), sig_hex);
        assert_eq!(sig, expected_sig);

        assert!(ed25519_verify(&pk, b"", &sig));
        assert!(!ed25519_verify(&pk, b"tampered", &sig));
    }

    // RFC 8032 Section 7.1 Test Vector 2
    #[test]
    fn test_rfc8032_vector_2() {
        let sk_hex = "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb";
        let pk_hex = "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c";
        let msg = [0x72];
        let sig_hex = "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00";

        let sk: [u8; 32] = from_hex(sk_hex).unwrap().try_into().unwrap();
        let expected_pk: [u8; 32] = from_hex(pk_hex).unwrap().try_into().unwrap();
        let expected_sig: [u8; 64] = from_hex(sig_hex).unwrap().try_into().unwrap();

        let pk = ed25519_public_key(&sk);
        assert_eq!(pk, expected_pk);

        let sig = ed25519_sign(&sk, &msg);
        assert_eq!(to_hex(&sig), sig_hex);
        assert_eq!(sig, expected_sig);

        assert!(ed25519_verify(&pk, &msg, &sig));
        assert!(!ed25519_verify(&pk, &[0x73], &sig));
    }
}
