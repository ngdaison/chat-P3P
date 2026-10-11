// src/messaging/dedup.rs
//! Message deduplication and replay protection cache.

use crate::types::MESSAGE_ID_LEN;
use std::collections::HashMap;
use std::time::Instant;

pub struct SeenMessageCache {
    seen: HashMap<[u8; MESSAGE_ID_LEN], Instant>,
    max_capacity: usize,
}

impl SeenMessageCache {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            seen: HashMap::with_capacity(max_capacity.min(1000)),
            max_capacity,
        }
    }

    /// Checks if message ID has been seen. If not seen, records it and returns true.
    /// If already seen, returns false.
    pub fn check_and_record(&mut self, message_id: &[u8; MESSAGE_ID_LEN]) -> bool {
        if self.seen.contains_key(message_id) {
            false
        } else {
            // Enforce capacity limit: prune oldest if full
            if self.seen.len() >= self.max_capacity {
                self.prune_oldest();
            }
            self.seen.insert(*message_id, Instant::now());
            true
        }
    }

    pub fn is_seen(&self, message_id: &[u8; MESSAGE_ID_LEN]) -> bool {
        self.seen.contains_key(message_id)
    }

    fn prune_oldest(&mut self) {
        // Remove 10% of entries that were recorded earliest
        let to_remove = (self.max_capacity / 10).max(1);
        let mut entries: Vec<_> = self.seen.iter().map(|(k, v)| (*k, *v)).collect();
        entries.sort_by_key(|(_, t)| *t);
        for (k, _) in entries.into_iter().take(to_remove) {
            self.seen.remove(&k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_cache() {
        let mut cache = SeenMessageCache::new(3);
        let id1 = [1u8; 32];
        let id2 = [2u8; 32];

        assert!(cache.check_and_record(&id1));
        assert!(!cache.check_and_record(&id1)); // second check returns false

        assert!(cache.check_and_record(&id2));
        assert!(cache.is_seen(&id1));
        assert!(cache.is_seen(&id2));
    }
}
