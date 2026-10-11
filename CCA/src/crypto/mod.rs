// src/crypto/mod.rs
//! Standard-compliant cryptographic primitives implemented in 100% pure Rust.
//!
//! Includes SHA-256, HMAC-SHA256, HKDF-SHA256, ChaCha20, Poly1305, ChaCha20-Poly1305 AEAD,
//! and Curve25519/X25519, verified with official RFC test vectors.

pub mod aead;
pub mod chacha20;
pub mod constant_time;
pub mod ed25519;
pub mod hkdf;
pub mod hmac;
pub mod poly1305;
pub mod random;
pub mod sha256;
pub mod sha512;
pub mod x25519;

pub use aead::{AEAD_TAG_LEN, chacha20_poly1305_decrypt, chacha20_poly1305_encrypt};
pub use chacha20::{ChaCha20, chacha20_xor};
pub use constant_time::{cswap, ct_eq};
pub use ed25519::{
    ED25519_PUBLIC_KEY_LEN, ED25519_SECRET_KEY_LEN, ED25519_SIGNATURE_LEN, ed25519_public_key,
    ed25519_sign, ed25519_verify,
};
pub use hkdf::{hkdf, hkdf_expand, hkdf_extract};
pub use hmac::{HmacSha256, hmac_sha256};
pub use poly1305::{Poly1305, poly1305_mac};
pub use random::{fill_bytes, random_bytes_12, random_bytes_16, random_bytes_32};
pub use sha256::{Sha256, sha256};
pub use sha512::Sha512;
pub use x25519::{x25519, x25519_base_point};
