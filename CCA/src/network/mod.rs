// src/network/mod.rs
//! P2P network transport, binary framing, peer manager, and discovery.

pub mod connection;
pub mod discovery;
pub mod frame;
pub mod peer_manager;
pub mod protocol;
pub mod rate_limiter;

pub use connection::PeerConnection;
pub use discovery::{PeerAddressBook, PeerStats};
pub use frame::{Frame, HEADER_LEN};
pub use peer_manager::{PeerManager, PeerSession};
pub use protocol::{
    DeliveryReceiptPayload, FEATURE_DISCOVERY, FEATURE_RELAY, FEATURE_STORAGE,
    FileChunkRequestPayload, FileChunkResponsePayload, FileManifestPayload,
    GroupKeyDistributionPayload, GroupMembershipUpdatePayload, GroupMessagePayload,
    HelloAckPayload, HelloPayload, MessageFetchRequestPayload, MessageType, PeerEntry,
    PeerListResponsePayload, PingPayload, RelayStoredPayload,
};
pub use rate_limiter::RateLimiter;
