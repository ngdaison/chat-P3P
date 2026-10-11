// src/identity/contacts.rs
//! Contact book for managing known friends and their verified public keys.

use crate::identity::peer_id::PeerId;
use crate::types::{PUBLIC_KEY_LEN, current_timestamp_secs, from_hex, to_hex};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    pub name: String,
    pub public_key: [u8; PUBLIC_KEY_LEN],
    pub peer_id: PeerId,
    pub verified: bool,
    pub created_at: u64,
}

impl Contact {
    pub fn new(name: String, public_key: [u8; PUBLIC_KEY_LEN]) -> Self {
        let peer_id = PeerId::from_public_key(&public_key);
        Self {
            name,
            public_key,
            peer_id,
            verified: false,
            created_at: current_timestamp_secs(),
        }
    }

    pub fn public_key_hex(&self) -> String {
        to_hex(&self.public_key)
    }

    /// Short human-readable cryptographic fingerprint for out-of-band comparison.
    pub fn fingerprint(&self) -> String {
        let hex = self.public_key_hex();
        format!("{}-{}", &hex[..8], &hex[hex.len() - 8..])
    }
}

#[derive(Debug, Default, Clone)]
pub struct ContactBook {
    contacts_by_name: HashMap<String, Contact>,
    contacts_by_peer: HashMap<PeerId, String>,
}

impl ContactBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, contact: Contact) {
        let name_lower = contact.name.to_lowercase();
        self.contacts_by_peer
            .insert(contact.peer_id, name_lower.clone());
        self.contacts_by_name.insert(name_lower, contact);
    }

    pub fn add_from_hex(&mut self, name: &str, pubkey_hex: &str) -> Result<Contact, String> {
        let raw = from_hex(pubkey_hex)?;
        if raw.len() != PUBLIC_KEY_LEN {
            return Err(format!("Expected 32-byte public key, got {}", raw.len()));
        }
        let mut key = [0u8; PUBLIC_KEY_LEN];
        key.copy_from_slice(&raw);
        let contact = Contact::new(name.to_string(), key);
        self.add(contact.clone());
        Ok(contact)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&Contact> {
        self.contacts_by_name.get(&name.to_lowercase())
    }

    pub fn get_by_peer_id(&self, peer_id: &PeerId) -> Option<&Contact> {
        let name = self.contacts_by_peer.get(peer_id)?;
        self.contacts_by_name.get(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<Contact> {
        let name_lower = name.to_lowercase();
        if let Some(c) = self.contacts_by_name.remove(&name_lower) {
            self.contacts_by_peer.remove(&c.peer_id);
            Some(c)
        } else {
            None
        }
    }

    pub fn list(&self) -> Vec<&Contact> {
        self.contacts_by_name.values().collect()
    }

    pub fn verify(&mut self, name: &str) -> bool {
        if let Some(c) = self.contacts_by_name.get_mut(&name.to_lowercase()) {
            c.verified = true;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contact_book() {
        let mut book = ContactBook::new();
        let pubkey = [0x55u8; 32];
        let contact = Contact::new("Bob".to_string(), pubkey);
        let peer_id = contact.peer_id;

        book.add(contact);

        assert!(book.get_by_name("bob").is_some());
        assert!(book.get_by_name("Bob").is_some());
        assert_eq!(book.get_by_peer_id(&peer_id).unwrap().name, "Bob");

        assert!(book.verify("BOB"));
        assert!(book.get_by_name("bob").unwrap().verified);

        book.remove("bob");
        assert!(book.get_by_name("bob").is_none());
        assert!(book.get_by_peer_id(&peer_id).is_none());
    }
}
