// src/network/discovery.rs
//! Peer discovery, address book, and reconnect scheduler.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct PeerStats {
    pub last_seen: Option<Instant>,
    pub attempts: u32,
    pub successes: u32,
    pub failures: u32,
    pub next_retry: Instant,
    pub backoff_secs: u64,
}

impl Default for PeerStats {
    fn default() -> Self {
        Self {
            last_seen: None,
            attempts: 0,
            successes: 0,
            failures: 0,
            next_retry: Instant::now(),
            backoff_secs: 1,
        }
    }
}

#[derive(Debug, Default)]
pub struct PeerAddressBook {
    peers: HashMap<SocketAddr, PeerStats>,
}

impl PeerAddressBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, addr: SocketAddr) -> bool {
        use std::collections::hash_map::Entry;
        match self.peers.entry(addr) {
            Entry::Vacant(e) => {
                e.insert(PeerStats::default());
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    pub fn remove(&mut self, addr: &SocketAddr) {
        self.peers.remove(addr);
    }

    pub fn get_all(&self) -> Vec<SocketAddr> {
        self.peers.keys().copied().collect()
    }

    pub fn record_success(&mut self, addr: &SocketAddr) {
        if let Some(stats) = self.peers.get_mut(addr) {
            stats.successes += 1;
            stats.attempts += 1;
            stats.last_seen = Some(Instant::now());
            stats.backoff_secs = 1; // reset backoff on success
            stats.next_retry = Instant::now() + Duration::from_secs(30);
        }
    }

    pub fn record_failure(&mut self, addr: &SocketAddr) {
        if let Some(stats) = self.peers.get_mut(addr) {
            stats.failures += 1;
            stats.attempts += 1;
            // Exponential backoff: 1s -> 2s -> 4s -> 8s -> 16s -> 60s max
            let next_backoff = (stats.backoff_secs * 2).min(60);
            stats.backoff_secs = next_backoff;
            stats.next_retry = Instant::now() + Duration::from_secs(stats.backoff_secs);
        }
    }

    /// Gets addresses that are ready to attempt connection based on backoff schedule.
    pub fn get_candidates_to_connect(&self) -> Vec<SocketAddr> {
        let now = Instant::now();
        self.peers
            .iter()
            .filter(|(_, stats)| now >= stats.next_retry)
            .map(|(addr, _)| *addr)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_book_backoff() {
        let mut book = PeerAddressBook::new();
        let addr = "127.0.0.1:7001".parse().unwrap();
        book.add(addr);

        assert_eq!(book.get_all().len(), 1);

        // Record failure increases backoff
        book.record_failure(&addr);
        let stats = book.peers.get(&addr).unwrap();
        assert_eq!(stats.failures, 1);
        assert_eq!(stats.backoff_secs, 2);

        book.record_failure(&addr);
        assert_eq!(book.peers.get(&addr).unwrap().backoff_secs, 4);

        // Record success resets backoff
        book.record_success(&addr);
        assert_eq!(book.peers.get(&addr).unwrap().backoff_secs, 1);
    }
}
