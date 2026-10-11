// src/relay/store.rs
//! Relay offline message storage: holds encrypted envelopes for offline peers.
//!
//! Enforces storage quotas, message counts, TTL expiration, and never decrypts ciphertext.

use crate::error::StorageError;
use crate::identity::peer_id::PeerId;
use crate::messaging::envelope::MessageEnvelope;
use crate::types::{MESSAGE_ID_LEN, current_timestamp_secs};
use std::collections::HashMap;

#[derive(Debug)]
pub struct RelayStore {
    enabled: bool,
    max_storage_bytes: usize,
    max_messages: usize,
    current_storage_bytes: usize,
    // Stored messages mapped by message_id
    messages: HashMap<[u8; MESSAGE_ID_LEN], MessageEnvelope>,
    // Recipient index: PeerId -> List of message IDs
    recipient_index: HashMap<PeerId, Vec<[u8; MESSAGE_ID_LEN]>>,
}

impl RelayStore {
    pub fn new(enabled: bool, max_storage_bytes: usize, max_messages: usize) -> Self {
        Self {
            enabled,
            max_storage_bytes,
            max_messages,
            current_storage_bytes: 0,
            messages: HashMap::new(),
            recipient_index: HashMap::new(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Stores an encrypted envelope if relay is enabled and quota is not exceeded.
    pub fn store(&mut self, envelope: MessageEnvelope) -> Result<(), StorageError> {
        if !self.enabled {
            return Err(StorageError::NotFound(
                "Relay storage is disabled on this node".to_string(),
            ));
        }

        if envelope.is_expired() {
            return Err(StorageError::CorruptData(
                "Message envelope already expired".to_string(),
            ));
        }

        let envelope_bytes_len = envelope.ciphertext.len() + 160;

        // Check if message ID already exists
        if self.messages.contains_key(&envelope.message_id) {
            return Ok(());
        }

        // Check quota limits
        if self.current_storage_bytes + envelope_bytes_len > self.max_storage_bytes
            || self.messages.len() >= self.max_messages
        {
            // Try cleanup expired first
            self.cleanup_expired();
            if self.current_storage_bytes + envelope_bytes_len > self.max_storage_bytes
                || self.messages.len() >= self.max_messages
            {
                return Err(StorageError::QuotaExceeded {
                    used: self.current_storage_bytes,
                    limit: self.max_storage_bytes,
                });
            }
        }

        let message_id = envelope.message_id;
        let recipient = envelope.recipient_peer_id;

        self.current_storage_bytes += envelope_bytes_len;
        self.messages.insert(message_id, envelope);
        self.recipient_index
            .entry(recipient)
            .or_default()
            .push(message_id);

        Ok(())
    }

    /// Fetches all stored envelopes addressed to the recipient created after `since_timestamp`.
    pub fn fetch_for_recipient(
        &self,
        recipient: &PeerId,
        since_timestamp: u64,
    ) -> Vec<MessageEnvelope> {
        let mut results = Vec::new();
        if let Some(msg_ids) = self.recipient_index.get(recipient) {
            for id in msg_ids {
                if let Some(env) = self
                    .messages
                    .get(id)
                    .filter(|e| e.created_at >= since_timestamp && !e.is_expired())
                {
                    results.push(env.clone());
                }
            }
        }
        results
    }

    /// Prunes expired messages from storage.
    pub fn cleanup_expired(&mut self) -> usize {
        let now = current_timestamp_secs();
        let mut expired_ids = Vec::new();

        for (id, env) in &self.messages {
            if now >= env.expires_at {
                expired_ids.push(*id);
            }
        }

        let count = expired_ids.len();
        for id in expired_ids {
            if let Some(env) = self.messages.remove(&id) {
                let size = env.ciphertext.len() + 160;
                self.current_storage_bytes = self.current_storage_bytes.saturating_sub(size);

                if let Some(list) = self.recipient_index.get_mut(&env.recipient_peer_id) {
                    list.retain(|item| item != &id);
                }
            }
        }

        count
    }

    pub fn get_usage(&self) -> (usize, usize) {
        (self.current_storage_bytes, self.messages.len())
    }

    pub fn get_all_envelopes(&self) -> Vec<MessageEnvelope> {
        self.messages.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::keypair::KeyPair;

    #[test]
    fn test_relay_store_and_fetch() {
        let alice = KeyPair::generate().unwrap();
        let bob = KeyPair::generate().unwrap();

        let mut store = RelayStore::new(true, 1024 * 1024, 100);
        let envelope = MessageEnvelope::encrypt(
            &alice,
            bob.public_key(),
            *bob.peer_id(),
            b"Hello offline Bob!",
            3600,
        )
        .unwrap();

        assert!(store.store(envelope.clone()).is_ok());
        let (used_bytes, count) = store.get_usage();
        assert!(used_bytes > 0);
        assert_eq!(count, 1);

        // Fetch for Bob
        let fetched = store.fetch_for_recipient(bob.peer_id(), 0);
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].message_id, envelope.message_id);

        // Bob decrypts
        let decrypted = fetched[0].decrypt(&bob).unwrap();
        assert_eq!(decrypted, b"Hello offline Bob!");
    }

    #[test]
    fn test_relay_quota_exceeded() {
        let alice = KeyPair::generate().unwrap();
        let bob = KeyPair::generate().unwrap();

        // 200 bytes max storage
        let mut store = RelayStore::new(true, 200, 100);
        let env1 =
            MessageEnvelope::encrypt(&alice, bob.public_key(), *bob.peer_id(), b"Msg 1", 3600)
                .unwrap();
        let env2 =
            MessageEnvelope::encrypt(&alice, bob.public_key(), *bob.peer_id(), b"Msg 2", 3600)
                .unwrap();

        assert!(store.store(env1).is_ok());
        assert!(matches!(
            store.store(env2),
            Err(StorageError::QuotaExceeded { .. })
        ));
    }
}
