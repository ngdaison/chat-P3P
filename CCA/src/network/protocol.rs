// src/network/protocol.rs
//! High-level protocol message definitions, payloads, and binary serialization.

use crate::error::ProtocolError;
use crate::identity::peer_id::PeerId;
use crate::types::{PEER_ID_LEN, PROTOCOL_VERSION, PUBLIC_KEY_LEN};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Hello = 0x01,
    HelloAck = 0x02,
    PeerListRequest = 0x03,
    PeerListResponse = 0x04,
    Ping = 0x05,
    Pong = 0x06,
    Message = 0x10,
    MessageReceived = 0x11,
    RelayStore = 0x20,
    RelayStored = 0x21,
    MessageFetchRequest = 0x22,
    MessageFetchResponse = 0x23,
    DeliveryReceipt = 0x24,
    FileManifest = 0x30,
    FileChunkRequest = 0x31,
    FileChunkResponse = 0x32,
    GroupKeyDistribution = 0x40,
    GroupMessage = 0x41,
    GroupMembershipUpdate = 0x42,
    Error = 0xFE,
    Goodbye = 0xFF,
}

impl TryFrom<u8> for MessageType {
    type Error = ProtocolError;

    fn try_from(val: u8) -> Result<Self, ProtocolError> {
        match val {
            0x01 => Ok(MessageType::Hello),
            0x02 => Ok(MessageType::HelloAck),
            0x03 => Ok(MessageType::PeerListRequest),
            0x04 => Ok(MessageType::PeerListResponse),
            0x05 => Ok(MessageType::Ping),
            0x06 => Ok(MessageType::Pong),
            0x10 => Ok(MessageType::Message),
            0x11 => Ok(MessageType::MessageReceived),
            0x20 => Ok(MessageType::RelayStore),
            0x21 => Ok(MessageType::RelayStored),
            0x22 => Ok(MessageType::MessageFetchRequest),
            0x23 => Ok(MessageType::MessageFetchResponse),
            0x24 => Ok(MessageType::DeliveryReceipt),
            0x30 => Ok(MessageType::FileManifest),
            0x31 => Ok(MessageType::FileChunkRequest),
            0x32 => Ok(MessageType::FileChunkResponse),
            0x40 => Ok(MessageType::GroupKeyDistribution),
            0x41 => Ok(MessageType::GroupMessage),
            0x42 => Ok(MessageType::GroupMembershipUpdate),
            0xFE => Ok(MessageType::Error),
            0xFF => Ok(MessageType::Goodbye),
            other => Err(ProtocolError::UnknownMessageType(other)),
        }
    }
}

// Bit flags for node features in Hello handshake
pub const FEATURE_RELAY: u32 = 1 << 0;
pub const FEATURE_STORAGE: u32 = 1 << 1;
pub const FEATURE_DISCOVERY: u32 = 1 << 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelloPayload {
    pub protocol_version: u16,
    pub peer_id: PeerId,
    pub public_key: [u8; PUBLIC_KEY_LEN],
    pub listen_port: u16,
    pub features: u32,
}

impl HelloPayload {
    pub fn new(peer_id: PeerId, public_key: [u8; 32], listen_port: u16, features: u32) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            peer_id,
            public_key,
            listen_port,
            features,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(72);
        buf.extend_from_slice(&self.protocol_version.to_be_bytes());
        buf.extend_from_slice(self.peer_id.as_bytes());
        buf.extend_from_slice(&self.public_key);
        buf.extend_from_slice(&self.listen_port.to_be_bytes());
        buf.extend_from_slice(&self.features.to_be_bytes());
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 72 {
            return Err(ProtocolError::MalformedPayload(format!(
                "Hello payload too short: {}",
                payload.len()
            )));
        }
        let protocol_version = u16::from_be_bytes(payload[0..2].try_into().unwrap());
        let peer_id_bytes: [u8; PEER_ID_LEN] = payload[2..34].try_into().unwrap();
        let public_key: [u8; PUBLIC_KEY_LEN] = payload[34..66].try_into().unwrap();
        let listen_port = u16::from_be_bytes(payload[66..68].try_into().unwrap());
        let features = u32::from_be_bytes(payload[68..72].try_into().unwrap());

        let peer_id = PeerId::from_bytes(peer_id_bytes);

        // Verify Peer ID matches Public Key
        let expected_peer_id = PeerId::from_public_key(&public_key);
        if peer_id != expected_peer_id {
            return Err(ProtocolError::MalformedPayload(
                "Handshake peer_id does not match public_key".to_string(),
            ));
        }

        Ok(Self {
            protocol_version,
            peer_id,
            public_key,
            listen_port,
            features,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelloAckPayload {
    pub status: u8, // 0 = Accepted, 1 = Incompatible / Rejected
    pub peer_id: PeerId,
    pub public_key: [u8; PUBLIC_KEY_LEN],
}

impl HelloAckPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(65);
        buf.push(self.status);
        buf.extend_from_slice(self.peer_id.as_bytes());
        buf.extend_from_slice(&self.public_key);
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 65 {
            return Err(ProtocolError::MalformedPayload(
                "HelloAck payload too short".to_string(),
            ));
        }
        let status = payload[0];
        let peer_id_bytes: [u8; PEER_ID_LEN] = payload[1..33].try_into().unwrap();
        let public_key: [u8; PUBLIC_KEY_LEN] = payload[33..65].try_into().unwrap();

        let peer_id = PeerId::from_bytes(peer_id_bytes);
        let expected_peer_id = PeerId::from_public_key(&public_key);
        if peer_id != expected_peer_id {
            return Err(ProtocolError::MalformedPayload(
                "HelloAck peer_id does not match public_key".to_string(),
            ));
        }

        Ok(Self {
            status,
            peer_id,
            public_key,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerEntry {
    pub peer_id: PeerId,
    pub addr: SocketAddr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerListResponsePayload {
    pub peers: Vec<PeerEntry>,
}

impl PeerListResponsePayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(self.peers.len() as u16).to_be_bytes());
        for p in &self.peers {
            buf.extend_from_slice(p.peer_id.as_bytes());
            match p.addr.ip() {
                IpAddr::V4(ipv4) => {
                    buf.push(4);
                    buf.extend_from_slice(&ipv4.octets());
                }
                IpAddr::V6(ipv6) => {
                    buf.push(6);
                    buf.extend_from_slice(&ipv6.octets());
                }
            }
            buf.extend_from_slice(&p.addr.port().to_be_bytes());
        }
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 2 {
            return Err(ProtocolError::MalformedPayload(
                "PeerList too short".to_string(),
            ));
        }
        let count = u16::from_be_bytes(payload[0..2].try_into().unwrap()) as usize;
        let mut peers = Vec::with_capacity(count);
        let mut offset = 2;

        for _ in 0..count {
            if offset + 32 + 1 > payload.len() {
                return Err(ProtocolError::MalformedPayload(
                    "Truncated peer list".to_string(),
                ));
            }
            let peer_id = PeerId::from_bytes(payload[offset..offset + 32].try_into().unwrap());
            offset += 32;
            let ip_type = payload[offset];
            offset += 1;

            let ip: IpAddr = match ip_type {
                4 => {
                    if offset + 4 > payload.len() {
                        return Err(ProtocolError::MalformedPayload(
                            "Truncated IPv4".to_string(),
                        ));
                    }
                    let octets: [u8; 4] = payload[offset..offset + 4].try_into().unwrap();
                    offset += 4;
                    IpAddr::V4(Ipv4Addr::from(octets))
                }
                6 => {
                    if offset + 16 > payload.len() {
                        return Err(ProtocolError::MalformedPayload(
                            "Truncated IPv6".to_string(),
                        ));
                    }
                    let octets: [u8; 16] = payload[offset..offset + 16].try_into().unwrap();
                    offset += 16;
                    IpAddr::V6(Ipv6Addr::from(octets))
                }
                _ => {
                    return Err(ProtocolError::MalformedPayload(
                        "Unknown IP type".to_string(),
                    ));
                }
            };

            if offset + 2 > payload.len() {
                return Err(ProtocolError::MalformedPayload(
                    "Truncated port".to_string(),
                ));
            }
            let port = u16::from_be_bytes(payload[offset..offset + 2].try_into().unwrap());
            offset += 2;

            peers.push(PeerEntry {
                peer_id,
                addr: SocketAddr::new(ip, port),
            });
        }

        Ok(Self { peers })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PingPayload {
    pub nonce: u64,
}

impl PingPayload {
    pub fn encode(&self) -> Vec<u8> {
        self.nonce.to_be_bytes().to_vec()
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 8 {
            return Err(ProtocolError::MalformedPayload(
                "Ping too short".to_string(),
            ));
        }
        let nonce = u64::from_be_bytes(payload[0..8].try_into().unwrap());
        Ok(Self { nonce })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageFetchRequestPayload {
    pub recipient_peer_id: PeerId,
    pub since_timestamp: u64,
}

impl MessageFetchRequestPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(40);
        buf.extend_from_slice(self.recipient_peer_id.as_bytes());
        buf.extend_from_slice(&self.since_timestamp.to_be_bytes());
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 40 {
            return Err(ProtocolError::MalformedPayload(
                "FetchRequest too short".to_string(),
            ));
        }
        let peer_id_bytes: [u8; 32] = payload[0..32].try_into().unwrap();
        let since_timestamp = u64::from_be_bytes(payload[32..40].try_into().unwrap());
        Ok(Self {
            recipient_peer_id: PeerId::from_bytes(peer_id_bytes),
            since_timestamp,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayStoredPayload {
    pub message_id: [u8; 32],
    pub status: u8, // 0 = Stored, 1 = QuotaExceeded, 2 = Rejected
}

impl RelayStoredPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(33);
        buf.extend_from_slice(&self.message_id);
        buf.push(self.status);
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 33 {
            return Err(ProtocolError::MalformedPayload(
                "RelayStored too short".to_string(),
            ));
        }
        let message_id: [u8; 32] = payload[0..32].try_into().unwrap();
        let status = payload[32];
        Ok(Self { message_id, status })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryReceiptPayload {
    pub message_id: [u8; 32],
    pub recipient_peer_id: PeerId,
    pub timestamp: u64,
}

impl DeliveryReceiptPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(72);
        buf.extend_from_slice(&self.message_id);
        buf.extend_from_slice(self.recipient_peer_id.as_bytes());
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 72 {
            return Err(ProtocolError::MalformedPayload(
                "DeliveryReceipt too short".to_string(),
            ));
        }
        let message_id: [u8; 32] = payload[0..32].try_into().unwrap();
        let recipient_bytes: [u8; 32] = payload[32..64].try_into().unwrap();
        let timestamp = u64::from_be_bytes(payload[64..72].try_into().unwrap());
        Ok(Self {
            message_id,
            recipient_peer_id: PeerId::from_bytes(recipient_bytes),
            timestamp,
        })
    }
}

// --- FILE TRANSFER PROTOCOL PAYLOADS ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileManifestPayload {
    pub file_id: [u8; 32],
    pub sender: PeerId,
    pub recipient: PeerId,
    pub sender_ed25519_pub: [u8; 32],
    pub identity_proof: [u8; 64],
    pub file_name: String,
    pub file_size: u64,
    pub chunk_size: u32,
    pub chunk_count: u32,
    pub file_hash: [u8; 32],
    pub encrypted_file_key: Vec<u8>,
    pub chunk_hashes: Vec<[u8; 32]>,
    pub signature: [u8; 64],
}

impl FileManifestPayload {
    pub fn encode(&self) -> Vec<u8> {
        let name_bytes = self.file_name.as_bytes();
        let mut buf = Vec::with_capacity(350 + self.chunk_hashes.len() * 32);
        buf.extend_from_slice(&self.file_id);
        buf.extend_from_slice(self.sender.as_bytes());
        buf.extend_from_slice(self.recipient.as_bytes());
        buf.extend_from_slice(&self.sender_ed25519_pub);
        buf.extend_from_slice(&self.identity_proof);
        buf.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        buf.extend_from_slice(name_bytes);
        buf.extend_from_slice(&self.file_size.to_be_bytes());
        buf.extend_from_slice(&self.chunk_size.to_be_bytes());
        buf.extend_from_slice(&self.chunk_count.to_be_bytes());
        buf.extend_from_slice(&self.file_hash);
        buf.extend_from_slice(&(self.encrypted_file_key.len() as u16).to_be_bytes());
        buf.extend_from_slice(&self.encrypted_file_key);
        buf.extend_from_slice(&(self.chunk_hashes.len() as u32).to_be_bytes());
        for h in &self.chunk_hashes {
            buf.extend_from_slice(h);
        }
        buf.extend_from_slice(&self.signature);
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 32 + 32 + 32 + 32 + 64 + 2 {
            return Err(ProtocolError::MalformedPayload(
                "FileManifest too short".into(),
            ));
        }
        let mut offset = 0;
        let file_id: [u8; 32] = payload[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let sender = PeerId::from_bytes(payload[offset..offset + 32].try_into().unwrap());
        offset += 32;
        let recipient = PeerId::from_bytes(payload[offset..offset + 32].try_into().unwrap());
        offset += 32;
        let sender_ed25519_pub: [u8; 32] = payload[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let identity_proof: [u8; 64] = payload[offset..offset + 64].try_into().unwrap();
        offset += 64;

        let name_len = u16::from_be_bytes(payload[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;
        if offset + name_len > payload.len() {
            return Err(ProtocolError::MalformedPayload(
                "FileManifest invalid name length".into(),
            ));
        }
        let file_name = String::from_utf8(payload[offset..offset + name_len].to_vec())
            .map_err(|_| ProtocolError::MalformedPayload("Invalid UTF-8 in file name".into()))?;
        offset += name_len;

        if offset + 8 + 4 + 4 + 32 + 2 > payload.len() {
            return Err(ProtocolError::MalformedPayload(
                "FileManifest truncated header".into(),
            ));
        }
        let file_size = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let chunk_size = u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let chunk_count = u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let file_hash: [u8; 32] = payload[offset..offset + 32].try_into().unwrap();
        offset += 32;

        let key_len = u16::from_be_bytes(payload[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;
        if offset + key_len + 4 > payload.len() {
            return Err(ProtocolError::MalformedPayload(
                "FileManifest truncated key".into(),
            ));
        }
        let encrypted_file_key = payload[offset..offset + key_len].to_vec();
        offset += key_len;

        let hash_count =
            u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        if offset + hash_count * 32 + 64 > payload.len() {
            return Err(ProtocolError::MalformedPayload(
                "FileManifest truncated hashes/sig".into(),
            ));
        }

        let mut chunk_hashes = Vec::with_capacity(hash_count);
        for _ in 0..hash_count {
            let h: [u8; 32] = payload[offset..offset + 32].try_into().unwrap();
            chunk_hashes.push(h);
            offset += 32;
        }

        let signature: [u8; 64] = payload[offset..offset + 64].try_into().unwrap();

        Ok(Self {
            file_id,
            sender,
            recipient,
            sender_ed25519_pub,
            identity_proof,
            file_name,
            file_size,
            chunk_size,
            chunk_count,
            file_hash,
            encrypted_file_key,
            chunk_hashes,
            signature,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChunkRequestPayload {
    pub file_id: [u8; 32],
    pub chunk_index: u32,
    pub requester: PeerId,
}

impl FileChunkRequestPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(68);
        buf.extend_from_slice(&self.file_id);
        buf.extend_from_slice(&self.chunk_index.to_be_bytes());
        buf.extend_from_slice(self.requester.as_bytes());
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 68 {
            return Err(ProtocolError::MalformedPayload(
                "FileChunkRequest too short".into(),
            ));
        }
        let file_id: [u8; 32] = payload[0..32].try_into().unwrap();
        let chunk_index = u32::from_be_bytes(payload[32..36].try_into().unwrap());
        let requester = PeerId::from_bytes(payload[36..68].try_into().unwrap());
        Ok(Self {
            file_id,
            chunk_index,
            requester,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChunkResponsePayload {
    pub file_id: [u8; 32],
    pub chunk_index: u32,
    pub chunk_hash: [u8; 32],
    pub data: Vec<u8>,
}

impl FileChunkResponsePayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(72 + self.data.len());
        buf.extend_from_slice(&self.file_id);
        buf.extend_from_slice(&self.chunk_index.to_be_bytes());
        buf.extend_from_slice(&self.chunk_hash);
        buf.extend_from_slice(&(self.data.len() as u32).to_be_bytes());
        buf.extend_from_slice(&self.data);
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 72 {
            return Err(ProtocolError::MalformedPayload(
                "FileChunkResponse too short".into(),
            ));
        }
        let file_id: [u8; 32] = payload[0..32].try_into().unwrap();
        let chunk_index = u32::from_be_bytes(payload[32..36].try_into().unwrap());
        let chunk_hash: [u8; 32] = payload[36..68].try_into().unwrap();
        let data_len = u32::from_be_bytes(payload[68..72].try_into().unwrap()) as usize;
        if payload.len() < 72 + data_len {
            return Err(ProtocolError::MalformedPayload(
                "FileChunkResponse truncated data".into(),
            ));
        }
        let data = payload[72..72 + data_len].to_vec();
        Ok(Self {
            file_id,
            chunk_index,
            chunk_hash,
            data,
        })
    }
}

// --- GROUP CHAT PROTOCOL PAYLOADS ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupKeyDistributionPayload {
    pub group_id: [u8; 32],
    pub epoch: u64,
    pub group_name: String,
    pub creator: PeerId,
    pub recipient: PeerId,
    pub encrypted_key: Vec<u8>,
}

impl GroupKeyDistributionPayload {
    pub fn encode(&self) -> Vec<u8> {
        let name_bytes = self.group_name.as_bytes();
        let mut buf = Vec::with_capacity(128 + self.encrypted_key.len());
        buf.extend_from_slice(&self.group_id);
        buf.extend_from_slice(&self.epoch.to_be_bytes());
        buf.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        buf.extend_from_slice(name_bytes);
        buf.extend_from_slice(self.creator.as_bytes());
        buf.extend_from_slice(self.recipient.as_bytes());
        buf.extend_from_slice(&(self.encrypted_key.len() as u16).to_be_bytes());
        buf.extend_from_slice(&self.encrypted_key);
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 32 + 8 + 2 {
            return Err(ProtocolError::MalformedPayload(
                "GroupKeyDistribution too short".into(),
            ));
        }
        let mut offset = 0;
        let group_id: [u8; 32] = payload[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let epoch = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let name_len = u16::from_be_bytes(payload[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;

        if offset + name_len + 32 + 32 + 2 > payload.len() {
            return Err(ProtocolError::MalformedPayload(
                "GroupKeyDistribution truncated".into(),
            ));
        }
        let group_name = String::from_utf8(payload[offset..offset + name_len].to_vec())
            .map_err(|_| ProtocolError::MalformedPayload("Invalid group name UTF-8".into()))?;
        offset += name_len;

        let creator = PeerId::from_bytes(payload[offset..offset + 32].try_into().unwrap());
        offset += 32;
        let recipient = PeerId::from_bytes(payload[offset..offset + 32].try_into().unwrap());
        offset += 32;

        let key_len = u16::from_be_bytes(payload[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;
        if offset + key_len > payload.len() {
            return Err(ProtocolError::MalformedPayload(
                "GroupKeyDistribution truncated key".into(),
            ));
        }
        let encrypted_key = payload[offset..offset + key_len].to_vec();

        Ok(Self {
            group_id,
            epoch,
            group_name,
            creator,
            recipient,
            encrypted_key,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMessagePayload {
    pub group_id: [u8; 32],
    pub epoch: u64,
    pub message_id: [u8; 32],
    pub sender: PeerId,
    pub timestamp: u64,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

impl GroupMessagePayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128 + self.ciphertext.len());
        buf.extend_from_slice(&self.group_id);
        buf.extend_from_slice(&self.epoch.to_be_bytes());
        buf.extend_from_slice(&self.message_id);
        buf.extend_from_slice(self.sender.as_bytes());
        buf.extend_from_slice(&self.timestamp.to_be_bytes());
        buf.extend_from_slice(&self.nonce);
        buf.extend_from_slice(&(self.ciphertext.len() as u32).to_be_bytes());
        buf.extend_from_slice(&self.ciphertext);
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 32 + 8 + 32 + 32 + 8 + 12 + 4 {
            return Err(ProtocolError::MalformedPayload(
                "GroupMessage too short".into(),
            ));
        }
        let mut offset = 0;
        let group_id: [u8; 32] = payload[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let epoch = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let message_id: [u8; 32] = payload[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let sender = PeerId::from_bytes(payload[offset..offset + 32].try_into().unwrap());
        offset += 32;
        let timestamp = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let nonce: [u8; 12] = payload[offset..offset + 12].try_into().unwrap();
        offset += 12;

        let cipher_len =
            u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;
        if offset + cipher_len > payload.len() {
            return Err(ProtocolError::MalformedPayload(
                "GroupMessage truncated ciphertext".into(),
            ));
        }
        let ciphertext = payload[offset..offset + cipher_len].to_vec();

        Ok(Self {
            group_id,
            epoch,
            message_id,
            sender,
            timestamp,
            nonce,
            ciphertext,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMembershipUpdatePayload {
    pub group_id: [u8; 32],
    pub epoch: u64,
    pub admin: PeerId,
    pub admin_ed25519_pub: [u8; 32],
    pub identity_proof: [u8; 64],
    pub action: u8, // 1 = Add, 2 = Remove
    pub member: PeerId,
    pub signature: [u8; 64],
}

impl GroupMembershipUpdatePayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(266);
        buf.extend_from_slice(&self.group_id);
        buf.extend_from_slice(&self.epoch.to_be_bytes());
        buf.extend_from_slice(self.admin.as_bytes());
        buf.extend_from_slice(&self.admin_ed25519_pub);
        buf.extend_from_slice(&self.identity_proof);
        buf.push(self.action);
        buf.extend_from_slice(self.member.as_bytes());
        buf.extend_from_slice(&self.signature);
        buf
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        if payload.len() < 32 + 8 + 32 + 32 + 64 + 1 + 32 + 64 {
            return Err(ProtocolError::MalformedPayload(
                "GroupMembershipUpdate too short".into(),
            ));
        }
        let mut offset = 0;
        let group_id: [u8; 32] = payload[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let epoch = u64::from_be_bytes(payload[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let admin = PeerId::from_bytes(payload[offset..offset + 32].try_into().unwrap());
        offset += 32;
        let admin_ed25519_pub: [u8; 32] = payload[offset..offset + 32].try_into().unwrap();
        offset += 32;
        let identity_proof: [u8; 64] = payload[offset..offset + 64].try_into().unwrap();
        offset += 64;
        let action = payload[offset];
        offset += 1;
        let member = PeerId::from_bytes(payload[offset..offset + 32].try_into().unwrap());
        offset += 32;
        let signature: [u8; 64] = payload[offset..offset + 64].try_into().unwrap();

        Ok(Self {
            group_id,
            epoch,
            admin,
            admin_ed25519_pub,
            identity_proof,
            action,
            member,
            signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::keypair::KeyPair;

    #[test]
    fn test_hello_encode_decode() {
        let kp = KeyPair::generate().unwrap();
        let hello = HelloPayload::new(
            *kp.peer_id(),
            *kp.public_key(),
            7001,
            FEATURE_RELAY | FEATURE_DISCOVERY,
        );
        let encoded = hello.encode();
        let decoded = HelloPayload::decode(&encoded).expect("decode hello");
        assert_eq!(hello, decoded);
    }

    #[test]
    fn test_hello_tamper_rejected() {
        let kp = KeyPair::generate().unwrap();
        let hello = HelloPayload::new(*kp.peer_id(), *kp.public_key(), 7001, 0);
        let mut encoded = hello.encode();
        // Tamper public key bytes
        encoded[34] ^= 0xff;
        assert!(HelloPayload::decode(&encoded).is_err());
    }

    #[test]
    fn test_peer_list_encode_decode() {
        let p1 = PeerEntry {
            peer_id: PeerId::from_bytes([1; 32]),
            addr: "127.0.0.1:7001".parse().unwrap(),
        };
        let p2 = PeerEntry {
            peer_id: PeerId::from_bytes([2; 32]),
            addr: "[::1]:7002".parse().unwrap(),
        };
        let list = PeerListResponsePayload {
            peers: vec![p1, p2],
        };
        let encoded = list.encode();
        let decoded = PeerListResponsePayload::decode(&encoded).expect("decode peer list");
        assert_eq!(list, decoded);
    }

    #[test]
    fn test_file_payloads_roundtrip() {
        let manifest = FileManifestPayload {
            file_id: [42; 32],
            sender: PeerId::from_bytes([1; 32]),
            recipient: PeerId::from_bytes([2; 32]),
            sender_ed25519_pub: [3; 32],
            identity_proof: [4; 64],
            file_name: "test_doc.pdf".to_string(),
            file_size: 500000,
            chunk_size: 262144,
            chunk_count: 2,
            file_hash: [88; 32],
            encrypted_file_key: vec![1, 2, 3, 4],
            chunk_hashes: vec![[10; 32], [20; 32]],
            signature: [77; 64],
        };
        let encoded = manifest.encode();
        let decoded = FileManifestPayload::decode(&encoded).expect("decode manifest");
        assert_eq!(manifest, decoded);

        let req = FileChunkRequestPayload {
            file_id: [42; 32],
            chunk_index: 1,
            requester: PeerId::from_bytes([2; 32]),
        };
        assert_eq!(req, FileChunkRequestPayload::decode(&req.encode()).unwrap());

        let resp = FileChunkResponsePayload {
            file_id: [42; 32],
            chunk_index: 1,
            chunk_hash: [20; 32],
            data: vec![9, 8, 7, 6, 5],
        };
        assert_eq!(
            resp,
            FileChunkResponsePayload::decode(&resp.encode()).unwrap()
        );
    }

    #[test]
    fn test_group_payloads_roundtrip() {
        let key_dist = GroupKeyDistributionPayload {
            group_id: [99; 32],
            epoch: 1,
            group_name: "Secret Society".into(),
            creator: PeerId::from_bytes([1; 32]),
            recipient: PeerId::from_bytes([2; 32]),
            encrypted_key: vec![100; 32],
        };
        assert_eq!(
            key_dist,
            GroupKeyDistributionPayload::decode(&key_dist.encode()).unwrap()
        );

        let grp_msg = GroupMessagePayload {
            group_id: [99; 32],
            epoch: 1,
            message_id: [123; 32],
            sender: PeerId::from_bytes([1; 32]),
            timestamp: 1690000000,
            nonce: [7; 12],
            ciphertext: vec![1, 2, 3, 4, 5, 6],
        };
        assert_eq!(
            grp_msg,
            GroupMessagePayload::decode(&grp_msg.encode()).unwrap()
        );

        let mem_upd = GroupMembershipUpdatePayload {
            group_id: [99; 32],
            epoch: 2,
            admin: PeerId::from_bytes([1; 32]),
            admin_ed25519_pub: [22; 32],
            identity_proof: [33; 64],
            action: 1,
            member: PeerId::from_bytes([3; 32]),
            signature: [55; 64],
        };
        assert_eq!(
            mem_upd,
            GroupMembershipUpdatePayload::decode(&mem_upd.encode()).unwrap()
        );
    }
}
