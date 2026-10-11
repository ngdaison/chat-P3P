// src/storage/engine.rs
//! Storage engine providing atomic file writes and crash-resilient persistence.

use crate::error::StorageError;
use crate::identity::contacts::Contact;
use crate::identity::keypair::KeyPair;
use crate::identity::peer_id::PeerId;
use crate::messaging::envelope::MessageEnvelope;
use crate::messaging::message::{ChatMessage, DeliveryStatus};
use crate::storage::record::{
    StorageRecord, TYPE_CHAT_MESSAGE, TYPE_CONTACT, TYPE_IDENTITY, TYPE_PEER_ADDR,
    TYPE_RELAY_ENVELOPE,
};
use crate::types::PUBLIC_KEY_LEN;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};

pub struct StorageEngine {
    data_dir: PathBuf,
}

impl StorageEngine {
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = data_dir.as_ref().to_path_buf();
        fs::create_dir_all(&path)?;
        Ok(Self { data_dir: path })
    }

    fn write_atomic<F>(&self, file_name: &str, write_fn: F) -> Result<(), StorageError>
    where
        F: FnOnce(&mut BufWriter<File>) -> Result<(), StorageError>,
    {
        let target_path = self.data_dir.join(file_name);
        let tmp_path = self.data_dir.join(format!("{file_name}.tmp"));

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;

        let mut writer = BufWriter::new(file);
        write_fn(&mut writer)?;

        // Ensure flush and sync to disk
        use std::io::Write;
        writer.flush()?;
        writer.get_ref().sync_all()?;
        drop(writer);

        // Atomic replace
        fs::rename(tmp_path, target_path)?;
        Ok(())
    }

    // --- Identity ---

    pub fn save_identity(&self, keypair: &KeyPair) -> Result<(), StorageError> {
        self.write_atomic("identity.dat", |writer| {
            let record = StorageRecord::new(TYPE_IDENTITY, keypair.private_key().to_vec());
            record.write_to(writer)?;
            Ok(())
        })
    }

    pub fn load_identity(&self) -> Result<Option<KeyPair>, StorageError> {
        let path = self.data_dir.join("identity.dat");
        if !path.exists() {
            return Ok(None);
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        if let Some(record) = StorageRecord::read_from(&mut reader)? {
            if record.record_type != TYPE_IDENTITY || record.payload.len() != 32 {
                return Err(StorageError::CorruptData(
                    "Invalid identity record".to_string(),
                ));
            }
            let priv_key: [u8; 32] = record.payload.try_into().unwrap();
            Ok(Some(KeyPair::from_private_key(priv_key)))
        } else {
            Ok(None)
        }
    }

    // --- Contacts ---

    pub fn save_contacts(&self, contacts: &[Contact]) -> Result<(), StorageError> {
        self.write_atomic("contacts.dat", |writer| {
            for c in contacts {
                let mut payload = Vec::new();
                let name_bytes = c.name.as_bytes();
                payload.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
                payload.extend_from_slice(name_bytes);
                payload.extend_from_slice(&c.public_key);
                payload.push(if c.verified { 1 } else { 0 });
                payload.extend_from_slice(&c.created_at.to_be_bytes());

                let record = StorageRecord::new(TYPE_CONTACT, payload);
                record.write_to(writer)?;
            }
            Ok(())
        })
    }

    pub fn load_contacts(&self) -> Result<Vec<Contact>, StorageError> {
        let path = self.data_dir.join("contacts.dat");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut contacts = Vec::new();

        while let Some(record) = StorageRecord::read_from(&mut reader)? {
            if record.record_type != TYPE_CONTACT || record.payload.len() < 2 + 32 + 1 + 8 {
                continue; // skip corrupt entry safely
            }

            let p = &record.payload;
            let name_len = u16::from_be_bytes(p[0..2].try_into().unwrap()) as usize;
            if 2 + name_len + 32 + 1 + 8 > p.len() {
                continue;
            }

            let name = match std::str::from_utf8(&p[2..2 + name_len]) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            let mut offset = 2 + name_len;
            let public_key: [u8; PUBLIC_KEY_LEN] = p[offset..offset + 32].try_into().unwrap();
            offset += 32;

            let verified = p[offset] == 1;
            offset += 1;

            let created_at = u64::from_be_bytes(p[offset..offset + 8].try_into().unwrap());

            let peer_id = PeerId::from_public_key(&public_key);
            contacts.push(Contact {
                name,
                public_key,
                peer_id,
                verified,
                created_at,
            });
        }

        Ok(contacts)
    }

    // --- Peer Addresses ---

    pub fn save_peers(&self, peers: &[SocketAddr]) -> Result<(), StorageError> {
        self.write_atomic("peers.dat", |writer| {
            for addr in peers {
                let mut payload = Vec::new();
                match addr.ip() {
                    IpAddr::V4(ipv4) => {
                        payload.push(4);
                        payload.extend_from_slice(&ipv4.octets());
                    }
                    IpAddr::V6(ipv6) => {
                        payload.push(6);
                        payload.extend_from_slice(&ipv6.octets());
                    }
                }
                payload.extend_from_slice(&addr.port().to_be_bytes());

                let record = StorageRecord::new(TYPE_PEER_ADDR, payload);
                record.write_to(writer)?;
            }
            Ok(())
        })
    }

    pub fn load_peers(&self) -> Result<Vec<SocketAddr>, StorageError> {
        let path = self.data_dir.join("peers.dat");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut peers = Vec::new();

        while let Some(record) = StorageRecord::read_from(&mut reader)? {
            if record.record_type != TYPE_PEER_ADDR || record.payload.is_empty() {
                continue;
            }

            let p = &record.payload;
            let ip_type = p[0];
            let addr = match ip_type {
                4 => {
                    if p.len() < 1 + 4 + 2 {
                        continue;
                    }
                    let octets: [u8; 4] = p[1..5].try_into().unwrap();
                    let port = u16::from_be_bytes(p[5..7].try_into().unwrap());
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::from(octets)), port)
                }
                6 => {
                    if p.len() < 1 + 16 + 2 {
                        continue;
                    }
                    let octets: [u8; 16] = p[1..17].try_into().unwrap();
                    let port = u16::from_be_bytes(p[17..19].try_into().unwrap());
                    SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port)
                }
                _ => continue,
            };

            peers.push(addr);
        }

        Ok(peers)
    }

    // --- Messages (Inbox & Outbox) ---

    pub fn save_messages(&self, messages: &[ChatMessage]) -> Result<(), StorageError> {
        self.write_atomic("messages.dat", |writer| {
            for m in messages {
                let mut payload = Vec::new();
                payload.extend_from_slice(&m.id);
                payload.push(if m.is_outgoing { 1 } else { 0 });

                let status_byte = match m.status {
                    DeliveryStatus::Created => 0,
                    DeliveryStatus::Queued => 1,
                    DeliveryStatus::SentToPeer => 2,
                    DeliveryStatus::StoredByRelay { .. } => 3,
                    DeliveryStatus::Delivered => 4,
                    DeliveryStatus::Failed => 5,
                    DeliveryStatus::Expired => 6,
                };
                payload.push(status_byte);
                payload.extend_from_slice(&m.timestamp.to_be_bytes());

                let s_name = m.sender_name.as_bytes();
                payload.extend_from_slice(&(s_name.len() as u16).to_be_bytes());
                payload.extend_from_slice(s_name);
                payload.extend_from_slice(m.sender_peer_id.as_bytes());

                let r_name = m.recipient_name.as_bytes();
                payload.extend_from_slice(&(r_name.len() as u16).to_be_bytes());
                payload.extend_from_slice(r_name);
                payload.extend_from_slice(m.recipient_peer_id.as_bytes());

                let content = m.content.as_bytes();
                payload.extend_from_slice(&(content.len() as u32).to_be_bytes());
                payload.extend_from_slice(content);

                let record = StorageRecord::new(TYPE_CHAT_MESSAGE, payload);
                record.write_to(writer)?;
            }
            Ok(())
        })
    }

    pub fn load_messages(&self) -> Result<Vec<ChatMessage>, StorageError> {
        let path = self.data_dir.join("messages.dat");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut messages = Vec::new();

        while let Some(record) = StorageRecord::read_from(&mut reader)? {
            if record.record_type != TYPE_CHAT_MESSAGE
                || record.payload.len() < 32 + 1 + 1 + 8 + 2 + 32 + 2 + 32 + 4
            {
                continue;
            }

            let p = &record.payload;
            let mut offset = 0;

            let id: [u8; 32] = p[offset..offset + 32].try_into().unwrap();
            offset += 32;

            let is_outgoing = p[offset] == 1;
            offset += 1;

            let status = match p[offset] {
                0 => DeliveryStatus::Created,
                1 => DeliveryStatus::Queued,
                2 => DeliveryStatus::SentToPeer,
                3 => DeliveryStatus::StoredByRelay { count: 1 },
                4 => DeliveryStatus::Delivered,
                5 => DeliveryStatus::Failed,
                _ => DeliveryStatus::Expired,
            };
            offset += 1;

            let timestamp = u64::from_be_bytes(p[offset..offset + 8].try_into().unwrap());
            offset += 8;

            let s_len = u16::from_be_bytes(p[offset..offset + 2].try_into().unwrap()) as usize;
            offset += 2;
            let sender_name = match std::str::from_utf8(&p[offset..offset + s_len]) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };
            offset += s_len;

            let sender_peer = PeerId::from_bytes(p[offset..offset + 32].try_into().unwrap());
            offset += 32;

            let r_len = u16::from_be_bytes(p[offset..offset + 2].try_into().unwrap()) as usize;
            offset += 2;
            let recipient_name = match std::str::from_utf8(&p[offset..offset + r_len]) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };
            offset += r_len;

            let recipient_peer = PeerId::from_bytes(p[offset..offset + 32].try_into().unwrap());
            offset += 32;

            let c_len = u32::from_be_bytes(p[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;
            let content = match std::str::from_utf8(&p[offset..offset + c_len]) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            messages.push(ChatMessage {
                id,
                sender_name,
                sender_peer_id: sender_peer,
                recipient_name,
                recipient_peer_id: recipient_peer,
                content,
                timestamp,
                is_outgoing,
                status,
            });
        }

        Ok(messages)
    }

    // --- Relay Storage Envelopes ---

    pub fn save_relay(&self, envelopes: &[MessageEnvelope]) -> Result<(), StorageError> {
        self.write_atomic("relay.dat", |writer| {
            for env in envelopes {
                let encoded = env.encode();
                let record = StorageRecord::new(TYPE_RELAY_ENVELOPE, encoded);
                record.write_to(writer)?;
            }
            Ok(())
        })
    }

    pub fn load_relay(&self) -> Result<Vec<MessageEnvelope>, StorageError> {
        let path = self.data_dir.join("relay.dat");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut envelopes = Vec::new();

        while let Some(record) = StorageRecord::read_from(&mut reader)? {
            if record.record_type != TYPE_RELAY_ENVELOPE {
                continue;
            }
            if let Some(env) = MessageEnvelope::decode(&record.payload)
                .ok()
                .filter(|e| !e.is_expired())
            {
                envelopes.push(env);
            }
        }

        Ok(envelopes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_engine_identity_and_contacts() {
        let temp_dir = std::env::temp_dir().join(format!(
            "p2pchat_test_{}",
            std::time::SystemTime::now().elapsed().unwrap().as_nanos()
        ));
        let engine = StorageEngine::new(&temp_dir).unwrap();

        // 1. Identity
        let keypair = KeyPair::generate().unwrap();
        engine.save_identity(&keypair).unwrap();
        let loaded_keypair = engine.load_identity().unwrap().expect("identity found");
        assert_eq!(keypair.public_key(), loaded_keypair.public_key());
        assert_eq!(keypair.peer_id(), loaded_keypair.peer_id());

        // 2. Contacts
        let contact = Contact::new("Bob".to_string(), [0x77u8; 32]);
        engine
            .save_contacts(std::slice::from_ref(&contact))
            .unwrap();
        let loaded_contacts = engine.load_contacts().unwrap();
        assert_eq!(loaded_contacts.len(), 1);
        assert_eq!(loaded_contacts[0].name, "Bob");
        assert_eq!(loaded_contacts[0].public_key, contact.public_key);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
