// src/messaging/router.rs
//! Message router: handles message creation, direct delivery, relay forwarding, and receipts.

use crate::error::ChatError;
use crate::identity::contacts::ContactBook;
use crate::identity::keypair::KeyPair;
use crate::messaging::dedup::SeenMessageCache;
use crate::messaging::envelope::MessageEnvelope;
use crate::messaging::message::{ChatMessage, DeliveryStatus};
use crate::network::frame::Frame;
use crate::network::peer_manager::PeerManager;
use crate::network::protocol::{DeliveryReceiptPayload, MessageType};
use crate::types::current_timestamp_secs;
use std::sync::{Arc, Mutex};

pub struct MessageRouter {
    identity: KeyPair,
    peer_manager: Arc<PeerManager>,
    contacts: Arc<Mutex<ContactBook>>,
    dedup: Mutex<SeenMessageCache>,
    inbox: Mutex<Vec<ChatMessage>>,
    outbox: Mutex<Vec<ChatMessage>>,
    ttl_seconds: u64,
}

impl MessageRouter {
    pub fn new(
        identity: KeyPair,
        peer_manager: Arc<PeerManager>,
        contacts: Arc<Mutex<ContactBook>>,
        ttl_seconds: u64,
    ) -> Self {
        Self {
            identity,
            peer_manager,
            contacts,
            dedup: Mutex::new(SeenMessageCache::new(10000)),
            inbox: Mutex::new(Vec::new()),
            outbox: Mutex::new(Vec::new()),
            ttl_seconds,
        }
    }

    /// Prepares and sends an encrypted message to a named contact.
    pub fn send_message(
        &self,
        recipient_name: &str,
        content: &str,
    ) -> Result<ChatMessage, ChatError> {
        let contact = {
            let book = self.contacts.lock().unwrap();
            book.get_by_name(recipient_name)
                .cloned()
                .ok_or_else(|| ChatError::ContactNotFound(recipient_name.to_string()))?
        };

        // 1. Encrypt message
        let envelope = MessageEnvelope::encrypt(
            &self.identity,
            &contact.public_key,
            contact.peer_id,
            content.as_bytes(),
            self.ttl_seconds,
        )?;

        let mut chat_msg = ChatMessage::new_outgoing(
            envelope.message_id,
            "Me".to_string(),
            *self.identity.peer_id(),
            contact.name.clone(),
            contact.peer_id,
            content.to_string(),
        );

        // 2. Attempt direct transmission if peer is currently connected
        if let Some(session) = self.peer_manager.get_session(&contact.peer_id) {
            let frame = Frame::new(MessageType::Message as u8, 0, envelope.encode());
            if session.connection.send(&frame).is_ok() {
                chat_msg.status = DeliveryStatus::SentToPeer;
                self.outbox.lock().unwrap().push(chat_msg.clone());
                return Ok(chat_msg);
            }
        }

        // 3. Fallback: Distribute to connected relay / storage peers
        let relay_frame = Frame::new(MessageType::RelayStore as u8, 0, envelope.encode());
        let all_sessions = self.peer_manager.get_all_sessions();
        let mut stored_count = 0;

        for session in all_sessions {
            if session.peer_id == *self.identity.peer_id() {
                continue;
            }
            if session.connection.send(&relay_frame).is_ok() {
                stored_count += 1;
            }
        }

        if stored_count > 0 {
            chat_msg.status = DeliveryStatus::StoredByRelay {
                count: stored_count,
            };
        } else {
            chat_msg.status = DeliveryStatus::Queued;
        }

        self.outbox.lock().unwrap().push(chat_msg.clone());
        Ok(chat_msg)
    }

    /// Processes an incoming raw envelope received from the network.
    pub fn process_incoming_envelope(
        &self,
        envelope: MessageEnvelope,
    ) -> Result<Option<ChatMessage>, ChatError> {
        // 1. Deduplication check
        {
            let mut cache = self.dedup.lock().unwrap();
            if !cache.check_and_record(&envelope.message_id) {
                return Ok(None); // Already processed
            }
        }

        // 2. Check if message is for this node
        if envelope.recipient_peer_id == *self.identity.peer_id() {
            // Decrypt message
            let plaintext_bytes = envelope.decrypt(&self.identity)?;
            let content = String::from_utf8(plaintext_bytes).map_err(|_| {
                ChatError::InvalidInput("Decrypted content is not valid UTF-8".to_string())
            })?;

            // Resolve contact name if known
            let sender_name = {
                let book = self.contacts.lock().unwrap();
                book.get_by_peer_id(&envelope.sender_peer_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| envelope.sender_peer_id.short_hex())
            };

            let chat_msg = ChatMessage::new_incoming(
                envelope.message_id,
                sender_name,
                envelope.sender_peer_id,
                "Me".to_string(),
                *self.identity.peer_id(),
                content,
                envelope.created_at,
            );

            self.inbox.lock().unwrap().push(chat_msg.clone());

            // Send delivery receipt back if directly connected to sender
            let receipt = DeliveryReceiptPayload {
                message_id: envelope.message_id,
                recipient_peer_id: *self.identity.peer_id(),
                timestamp: current_timestamp_secs(),
            };
            let receipt_frame = Frame::new(MessageType::DeliveryReceipt as u8, 0, receipt.encode());
            let _ = self
                .peer_manager
                .send_to_peer(&envelope.sender_peer_id, &receipt_frame);

            return Ok(Some(chat_msg));
        }

        // 3. Message is for another node: forward if hops remaining
        if envelope.hops_remaining > 0 {
            let mut forwarded = envelope.clone();
            forwarded.hops_remaining -= 1;
            let frame = Frame::new(MessageType::Message as u8, 0, forwarded.encode());

            // If directly connected to recipient, send straight to them
            if let Some(session) = self.peer_manager.get_session(&forwarded.recipient_peer_id) {
                let _ = session.connection.send(&frame);
            } else {
                // Broadcast to other peers
                self.peer_manager
                    .broadcast(&frame, Some(&forwarded.sender_peer_id));
            }
        }

        Ok(None)
    }

    /// Handles incoming delivery receipt from recipient.
    pub fn handle_delivery_receipt(&self, receipt: DeliveryReceiptPayload) {
        let mut outbox = self.outbox.lock().unwrap();
        for msg in outbox.iter_mut() {
            if msg.id == receipt.message_id {
                msg.status = DeliveryStatus::Delivered;
            }
        }
    }

    pub fn get_inbox(&self) -> Vec<ChatMessage> {
        self.inbox.lock().unwrap().clone()
    }

    pub fn get_outbox(&self) -> Vec<ChatMessage> {
        self.outbox.lock().unwrap().clone()
    }

    pub fn add_loaded_inbox_message(&self, msg: ChatMessage) {
        self.inbox.lock().unwrap().push(msg);
    }

    pub fn add_loaded_outbox_message(&self, msg: ChatMessage) {
        self.outbox.lock().unwrap().push(msg);
    }
}
