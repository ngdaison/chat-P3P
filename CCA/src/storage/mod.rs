// src/storage/mod.rs
//! Pure Rust local persistence engine with atomic writes and checksum verification.

pub mod engine;
pub mod record;

pub use engine::StorageEngine;
pub use record::StorageRecord;
