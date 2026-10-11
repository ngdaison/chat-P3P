// src/messaging/mod.rs
//! End-to-end encrypted messaging, envelopes, message router, and deduplication.

pub mod dedup;
pub mod envelope;
pub mod message;
pub mod router;

pub use dedup::SeenMessageCache;
pub use envelope::{DEFAULT_HOPS, MessageEnvelope};
pub use message::{ChatMessage, DeliveryStatus};
pub use router::MessageRouter;
