// src/groups/group.rs
//! Group state, membership representation, and epoch key definitions.

use std::collections::HashSet;

use crate::crypto::random::random_bytes_32;
use crate::crypto::sha256::Sha256;
use crate::error::ChatError;
use crate::identity::peer_id::PeerId;
use crate::types::current_timestamp_secs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupInfo {
    pub group_id: [u8; 32],
    pub name: String,
    pub creator: PeerId,
    pub members: HashSet<PeerId>,
    pub epoch: u64,
    pub group_key: [u8; 32],
    pub created_at: u64,
}

impl GroupInfo {
    /// Creates a new group initialized with epoch 1 and a freshly generated symmetric key.
    pub fn new(name: String, creator: PeerId) -> Result<Self, ChatError> {
        let created_at = current_timestamp_secs();
        let mut hasher = Sha256::new();
        hasher.update(creator.as_bytes());
        hasher.update(name.as_bytes());
        hasher.update(&created_at.to_be_bytes());
        let group_id = hasher.finalize();

        let group_key = random_bytes_32().map_err(ChatError::Crypto)?;
        let mut members = HashSet::new();
        members.insert(creator);

        Ok(Self {
            group_id,
            name,
            creator,
            members,
            epoch: 1,
            group_key,
            created_at,
        })
    }

    /// Rotates the group symmetric key to a new epoch.
    pub fn rotate_key(&mut self) -> Result<[u8; 32], ChatError> {
        let new_key = random_bytes_32().map_err(ChatError::Crypto)?;
        self.group_key = new_key;
        self.epoch += 1;
        Ok(new_key)
    }

    pub fn is_member(&self, peer_id: &PeerId) -> bool {
        self.members.contains(peer_id)
    }
}
