// src/config.rs
//! Node configuration management.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct NodeConfig {
    pub listen_address: SocketAddr,
    pub data_directory: PathBuf,
    pub max_peers: usize,
    pub connect_timeout: Duration,
    pub peer_timeout: Duration,
    pub ping_interval: Duration,
    pub relay_enabled: bool,
    pub relay_storage_limit_bytes: usize,
    pub message_ttl_seconds: u64,
    pub max_message_size: usize,
    pub peer_discovery_enabled: bool,
    pub reconnect_enabled: bool,
    pub debug_logging: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            listen_address: "127.0.0.1:7001".parse().unwrap(),
            data_directory: PathBuf::from("./p2p_data"),
            max_peers: 64,
            connect_timeout: Duration::from_secs(5),
            peer_timeout: Duration::from_secs(90),
            ping_interval: Duration::from_secs(30),
            relay_enabled: false,
            relay_storage_limit_bytes: 50 * 1024 * 1024, // 50 MB
            message_ttl_seconds: 48 * 3600,              // 48 hours
            max_message_size: 64 * 1024,                 // 64 KiB
            peer_discovery_enabled: true,
            reconnect_enabled: true,
            debug_logging: false,
        }
    }
}

impl NodeConfig {
    pub fn new(listen_address: SocketAddr, data_directory: impl AsRef<Path>) -> Self {
        Self {
            listen_address,
            data_directory: data_directory.as_ref().to_path_buf(),
            ..Default::default()
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.max_peers == 0 {
            return Err("max_peers must be greater than 0".to_string());
        }
        if self.max_message_size == 0 || self.max_message_size > 10 * 1024 * 1024 {
            return Err("max_message_size out of bounds (1..10MB)".to_string());
        }
        if self.relay_storage_limit_bytes == 0 {
            return Err("relay_storage_limit_bytes must be greater than 0".to_string());
        }
        Ok(())
    }
}
