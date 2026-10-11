// src/network/discovery.rs
//! Peer discovery, address book, reconnect scheduler, and UDP LAN Beacon discovery.

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::identity::peer_id::PeerId;
use crate::types::{from_hex, to_hex, PUBLIC_KEY_LEN};

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

#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub peer_id: PeerId,
    pub public_key: [u8; PUBLIC_KEY_LEN],
    pub addr: SocketAddr,
    pub device_name: String,
    pub last_seen: Instant,
}

pub struct UdpBeaconDiscovery {
    socket: Arc<UdpSocket>,
    my_peer_id: PeerId,
    my_pubkey: [u8; PUBLIC_KEY_LEN],
    my_listen_port: u16,
    my_device_name: String,
    discovered: Arc<RwLock<HashMap<[u8; PUBLIC_KEY_LEN], DiscoveredPeer>>>,
    running: Arc<AtomicBool>,
    worker_threads: RwLock<Vec<JoinHandle<()>>>,
}

impl UdpBeaconDiscovery {
    pub fn new(
        my_peer_id: PeerId,
        my_pubkey: [u8; PUBLIC_KEY_LEN],
        my_listen_port: u16,
    ) -> Result<Arc<Self>, std::io::Error> {
        let socket = match UdpSocket::bind(format!("0.0.0.0:{}", my_listen_port)) {
            Ok(s) => s,
            Err(_) => UdpSocket::bind("0.0.0.0:0")?,
        };
        socket.set_broadcast(true)?;
        socket.set_read_timeout(Some(Duration::from_millis(500)))?;

        let hostname = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| format!("Peer-{}", my_listen_port));

        let disc = Arc::new(Self {
            socket: Arc::new(socket),
            my_peer_id,
            my_pubkey,
            my_listen_port,
            my_device_name: hostname,
            discovered: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(true)),
            worker_threads: RwLock::new(Vec::new()),
        });

        disc.start_background_workers();
        Ok(disc)
    }

    pub fn broadcast_beacon(&self) {
        let msg = format!(
            "P2P_BEACON:v1:{}:{}:{}:{}",
            self.my_peer_id.to_hex(),
            to_hex(&self.my_pubkey),
            self.my_listen_port,
            self.my_device_name
        );
        let bytes = msg.as_bytes();

        // Broadcast to local subnet and localhost on ports 7001..=7020
        for port in 7001..=7020 {
            if let Ok(addr) = format!("255.255.255.255:{}", port).parse::<SocketAddr>() {
                let _ = self.socket.send_to(bytes, addr);
            }
            if let Ok(addr) = format!("127.0.0.1:{}", port).parse::<SocketAddr>() {
                let _ = self.socket.send_to(bytes, addr);
            }
        }
    }

    fn start_background_workers(self: &Arc<Self>) {
        let self_recv = self.clone();
        let recv_handle = thread::spawn(move || {
            let mut buf = [0u8; 1024];
            while self_recv.running.load(Ordering::Relaxed) {
                match self_recv.socket.recv_from(&mut buf) {
                    Ok((n, src_addr)) => {
                        if let Ok(text) = std::str::from_utf8(&buf[..n]) {
                            self_recv.handle_packet(text, src_addr);
                        }
                    }
                    Err(_) => {}
                }
            }
        });

        let self_bc = self.clone();
        let bc_handle = thread::spawn(move || {
            while self_bc.running.load(Ordering::Relaxed) {
                self_bc.broadcast_beacon();
                for _ in 0..25 {
                    if !self_bc.running.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });

        let mut workers = self.worker_threads.write().unwrap();
        workers.push(recv_handle);
        workers.push(bc_handle);
    }

    fn handle_packet(&self, text: &str, src_addr: SocketAddr) {
        let parts: Vec<&str> = text.splitn(6, ':').collect();
        if parts.len() < 5 || parts[0] != "P2P_BEACON" || parts[1] != "v1" {
            return;
        }

        let peer_id_hex = parts[2];
        let pubkey_hex = parts[3];
        let listen_port: u16 = match parts[4].parse() {
            Ok(p) => p,
            Err(_) => return,
        };
        let device_name = if parts.len() >= 6 { parts[5] } else { "P2P Node" };

        let peer_id = match PeerId::from_hex(peer_id_hex) {
            Ok(id) => id,
            Err(_) => return,
        };

        if peer_id == self.my_peer_id {
            return;
        }

        let pubkey_bytes: [u8; PUBLIC_KEY_LEN] = match from_hex(pubkey_hex) {
            Ok(b) if b.len() == PUBLIC_KEY_LEN => {
                let mut arr = [0u8; PUBLIC_KEY_LEN];
                arr.copy_from_slice(&b);
                arr
            }
            _ => return,
        };

        let target_addr = SocketAddr::new(src_addr.ip(), listen_port);

        let peer = DiscoveredPeer {
            peer_id,
            public_key: pubkey_bytes,
            addr: target_addr,
            device_name: device_name.to_string(),
            last_seen: Instant::now(),
        };

        let mut map = self.discovered.write().unwrap();
        map.insert(pubkey_bytes, peer);
    }

    pub fn get_discovered(&self) -> Vec<DiscoveredPeer> {
        let now = Instant::now();
        let map = self.discovered.read().unwrap();
        map.values()
            .filter(|p| now.duration_since(p.last_seen) < Duration::from_secs(60))
            .cloned()
            .collect()
    }

    pub fn find_addr_by_pubkey(&self, pubkey: &[u8; PUBLIC_KEY_LEN]) -> Option<SocketAddr> {
        let now = Instant::now();
        let map = self.discovered.read().unwrap();
        map.get(pubkey).and_then(|p| {
            if now.duration_since(p.last_seen) < Duration::from_secs(60) {
                Some(p.addr)
            } else {
                None
            }
        })
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
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
