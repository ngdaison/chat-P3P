// src/files/mod.rs
//! File and multimedia transfer module (Chunking, AEAD encryption, Manifest, Resume).

pub mod chunk;
pub mod manifest;
pub mod transfer_manager;

pub use chunk::{CHUNK_SIZE, decrypt_chunk, encrypt_chunk, hash_chunk};
pub use manifest::ManifestBuilder;
pub use transfer_manager::{FileTransferManager, InboundTransfer, OutboundTransfer, TransferState};
