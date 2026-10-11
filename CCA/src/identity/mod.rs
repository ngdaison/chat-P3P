// src/identity/mod.rs
//! Identity management module: keypairs, Peer IDs, and verified contacts.

pub mod contacts;
pub mod keypair;
pub mod peer_id;

pub use contacts::{Contact, ContactBook};
pub use keypair::KeyPair;
pub use peer_id::PeerId;
