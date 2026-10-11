// src/messaging/message.rs
//! Local representation of chat messages and delivery status.

use crate::identity::peer_id::PeerId;
use crate::types::{MESSAGE_ID_LEN, current_timestamp_secs, to_hex};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    Created,
    Queued,
    SentToPeer,
    StoredByRelay { count: usize },
    Delivered,
    Failed,
    Expired,
}

impl fmt::Display for DeliveryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeliveryStatus::Created => write!(f, "Created"),
            DeliveryStatus::Queued => write!(f, "Queued"),
            DeliveryStatus::SentToPeer => write!(f, "Sent to peer"),
            DeliveryStatus::StoredByRelay { count } => write!(f, "Stored by {count} relay node(s)"),
            DeliveryStatus::Delivered => write!(f, "Delivered"),
            DeliveryStatus::Failed => write!(f, "Failed"),
            DeliveryStatus::Expired => write!(f, "Expired"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: [u8; MESSAGE_ID_LEN],
    pub sender_name: String,
    pub sender_peer_id: PeerId,
    pub recipient_name: String,
    pub recipient_peer_id: PeerId,
    pub content: String,
    pub timestamp: u64,
    pub is_outgoing: bool,
    pub status: DeliveryStatus,
}

impl ChatMessage {
    pub fn new_outgoing(
        id: [u8; MESSAGE_ID_LEN],
        sender_name: String,
        sender_peer_id: PeerId,
        recipient_name: String,
        recipient_peer_id: PeerId,
        content: String,
    ) -> Self {
        Self {
            id,
            sender_name,
            sender_peer_id,
            recipient_name,
            recipient_peer_id,
            content,
            timestamp: current_timestamp_secs(),
            is_outgoing: true,
            status: DeliveryStatus::Created,
        }
    }

    pub fn new_incoming(
        id: [u8; MESSAGE_ID_LEN],
        sender_name: String,
        sender_peer_id: PeerId,
        recipient_name: String,
        recipient_peer_id: PeerId,
        content: String,
        timestamp: u64,
    ) -> Self {
        Self {
            id,
            sender_name,
            sender_peer_id,
            recipient_name,
            recipient_peer_id,
            content,
            timestamp,
            is_outgoing: false,
            status: DeliveryStatus::Delivered,
        }
    }

    pub fn id_hex(&self) -> String {
        to_hex(&self.id)
    }

    pub fn short_id(&self) -> String {
        to_hex(&self.id)[..8].to_string()
    }
}
