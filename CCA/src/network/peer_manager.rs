// src/network/peer_manager.rs
//! Peer connection lifecycle management, handshakes, and active session registry.

use crate::error::NetworkError;
use crate::identity::keypair::KeyPair;
use crate::identity::peer_id::PeerId;
use crate::network::connection::PeerConnection;
use crate::network::frame::Frame;
use crate::network::protocol::{HelloAckPayload, HelloPayload, MessageType};
use crate::types::PUBLIC_KEY_LEN;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Instant;

pub struct PeerSession {
    pub peer_id: PeerId,
    pub public_key: [u8; PUBLIC_KEY_LEN],
    pub connection: Arc<PeerConnection>,
    pub features: u32,
    pub listen_port: u16,
    pub last_seen: RwLock<Instant>,
}

pub struct PeerManager {
    identity: KeyPair,
    listen_port: u16,
    features: u32,
    max_peers: usize,
    sessions: RwLock<HashMap<PeerId, Arc<PeerSession>>>,
}

impl PeerManager {
    pub fn new(identity: KeyPair, listen_port: u16, features: u32, max_peers: usize) -> Self {
        Self {
            identity,
            listen_port,
            features,
            max_peers,
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Performs the initiator (outbound) handshake.
    pub fn handshake_outbound(
        &self,
        conn: Arc<PeerConnection>,
    ) -> Result<Arc<PeerSession>, NetworkError> {
        // 1. Send HELLO
        let hello = HelloPayload::new(
            *self.identity.peer_id(),
            *self.identity.public_key(),
            self.listen_port,
            self.features,
        );
        let frame = Frame::new(MessageType::Hello as u8, 0, hello.encode());
        conn.send(&frame)?;

        // 2. Read HELLO_ACK
        let ack_frame = conn.receive()?;
        if ack_frame.message_type != MessageType::HelloAck as u8 {
            return Err(NetworkError::HandshakeFailed(format!(
                "Expected HELLO_ACK (0x02), got 0x{:02X}",
                ack_frame.message_type
            )));
        }

        let ack = HelloAckPayload::decode(&ack_frame.payload)?;
        if ack.status != 0 {
            return Err(NetworkError::HandshakeFailed(
                "Remote peer rejected handshake".to_string(),
            ));
        }

        if ack.peer_id == *self.identity.peer_id() {
            return Err(NetworkError::HandshakeFailed(
                "Cannot connect to self".to_string(),
            ));
        }

        let session = Arc::new(PeerSession {
            peer_id: ack.peer_id,
            public_key: ack.public_key,
            connection: conn,
            features: 0,
            listen_port: 0,
            last_seen: RwLock::new(Instant::now()),
        });

        self.register_session(session.clone())?;
        Ok(session)
    }

    /// Performs the responder (inbound) handshake.
    pub fn handshake_inbound(
        &self,
        conn: Arc<PeerConnection>,
    ) -> Result<Arc<PeerSession>, NetworkError> {
        // 1. Read HELLO
        let hello_frame = conn.receive()?;
        if hello_frame.message_type != MessageType::Hello as u8 {
            return Err(NetworkError::HandshakeFailed(format!(
                "Expected HELLO (0x01), got 0x{:02X}",
                hello_frame.message_type
            )));
        }

        let hello = HelloPayload::decode(&hello_frame.payload)?;
        if hello.peer_id == *self.identity.peer_id() {
            return Err(NetworkError::HandshakeFailed(
                "Self connection rejected".to_string(),
            ));
        }

        // 2. Send HELLO_ACK
        let ack = HelloAckPayload {
            status: 0,
            peer_id: *self.identity.peer_id(),
            public_key: *self.identity.public_key(),
        };
        let ack_frame = Frame::new(MessageType::HelloAck as u8, 0, ack.encode());
        conn.send(&ack_frame)?;

        let session = Arc::new(PeerSession {
            peer_id: hello.peer_id,
            public_key: hello.public_key,
            connection: conn,
            features: hello.features,
            listen_port: hello.listen_port,
            last_seen: RwLock::new(Instant::now()),
        });

        self.register_session(session.clone())?;
        Ok(session)
    }

    fn register_session(&self, session: Arc<PeerSession>) -> Result<(), NetworkError> {
        let mut map = self.sessions.write().unwrap();
        if map.len() >= self.max_peers {
            return Err(NetworkError::MaxPeersReached(self.max_peers));
        }
        if map.contains_key(&session.peer_id) {
            return Err(NetworkError::DuplicateConnection(
                session.peer_id.to_string(),
            ));
        }
        map.insert(session.peer_id, session);
        Ok(())
    }

    pub fn disconnect_peer(&self, peer_id: &PeerId) {
        let mut map = self.sessions.write().unwrap();
        if let Some(session) = map.remove(peer_id) {
            session.connection.close();
        }
    }

    pub fn get_session(&self, peer_id: &PeerId) -> Option<Arc<PeerSession>> {
        self.sessions.read().unwrap().get(peer_id).cloned()
    }

    pub fn get_connected_peers(&self) -> Vec<PeerId> {
        self.sessions.read().unwrap().keys().copied().collect()
    }

    pub fn get_all_sessions(&self) -> Vec<Arc<PeerSession>> {
        self.sessions.read().unwrap().values().cloned().collect()
    }

    pub fn send_to_peer(&self, peer_id: &PeerId, frame: &Frame) -> Result<(), NetworkError> {
        let session = self
            .get_session(peer_id)
            .ok_or_else(|| NetworkError::PeerNotFound(peer_id.to_string()))?;
        session.connection.send(frame)
    }

    pub fn broadcast(&self, frame: &Frame, except: Option<&PeerId>) {
        let sessions = self.get_all_sessions();
        for session in sessions {
            if except.is_some_and(|exc| &session.peer_id == exc) {
                continue;
            }
            let _ = session.connection.send(frame);
        }
    }

    pub fn cleanup_closed_connections(&self) {
        let mut map = self.sessions.write().unwrap();
        map.retain(|_, session| !session.connection.is_closed());
    }

    pub fn get_peer_addresses(&self) -> Vec<(PeerId, SocketAddr)> {
        let map = self.sessions.read().unwrap();
        map.values()
            .map(|s| (s.peer_id, s.connection.remote_addr))
            .collect()
    }
}
