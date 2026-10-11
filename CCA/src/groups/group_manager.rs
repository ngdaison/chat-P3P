// src/groups/group_manager.rs
//! Group manager coordinating encrypted group messaging, membership, and key rotation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::crypto::aead::{AEAD_TAG_LEN, chacha20_poly1305_decrypt, chacha20_poly1305_encrypt};
use crate::crypto::ed25519::ed25519_verify;
use crate::crypto::random::{random_bytes_12, random_bytes_32};
use crate::error::ChatError;
use crate::events::{EventBus, NodeEvent};
use crate::files::manifest::ManifestBuilder;
use crate::groups::group::GroupInfo;
use crate::identity::keypair::KeyPair;
use crate::identity::peer_id::PeerId;
use crate::network::protocol::{
    GroupKeyDistributionPayload, GroupMembershipUpdatePayload, GroupMessagePayload,
};
use crate::types::current_timestamp_secs;

#[derive(Default)]
pub struct GroupManager {
    groups: Mutex<HashMap<[u8; 32], GroupInfo>>,
}

impl GroupManager {
    pub fn new() -> Self {
        Self {
            groups: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a new encrypted chat group.
    pub fn create_group(&self, name: &str, identity: &KeyPair) -> Result<GroupInfo, ChatError> {
        let group = GroupInfo::new(name.to_string(), *identity.peer_id())?;
        self.groups
            .lock()
            .unwrap()
            .insert(group.group_id, group.clone());
        Ok(group)
    }

    /// Adds a new member to an existing group and rotates the symmetric key.
    pub fn add_member(
        &self,
        group_id: &[u8; 32],
        member: PeerId,
        member_pubkeys: &[(PeerId, [u8; 32])],
        identity: &KeyPair,
    ) -> Result<
        (
            Vec<GroupKeyDistributionPayload>,
            GroupMembershipUpdatePayload,
        ),
        ChatError,
    > {
        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| ChatError::InvalidInput("Group not found".into()))?;

        if group.creator != *identity.peer_id() {
            return Err(ChatError::InvalidInput(
                "Only group creator can add members".into(),
            ));
        }

        group.members.insert(member);
        let new_key = group.rotate_key()?;

        // Encrypt new key for each member
        let mut key_dists = Vec::with_capacity(member_pubkeys.len());
        for (recip_peer, recip_pub) in member_pubkeys {
            let encrypted_key = ManifestBuilder::wrap_file_key(recip_pub, &new_key)?;
            key_dists.push(GroupKeyDistributionPayload {
                group_id: *group_id,
                epoch: group.epoch,
                group_name: group.name.clone(),
                creator: group.creator,
                recipient: *recip_peer,
                encrypted_key,
            });
        }

        // Create signed membership update
        let action = 1u8; // 1 = Add
        let mut sig_content = Vec::new();
        sig_content.extend_from_slice(group_id);
        sig_content.extend_from_slice(&group.epoch.to_be_bytes());
        sig_content.extend_from_slice(identity.peer_id().as_bytes());
        sig_content.push(action);
        sig_content.extend_from_slice(member.as_bytes());
        let signature = identity.sign(&sig_content);
        let admin_ed25519_pub = identity.ed25519_public_key();
        let identity_proof = identity.identity_proof();

        let membership_update = GroupMembershipUpdatePayload {
            group_id: *group_id,
            epoch: group.epoch,
            admin: *identity.peer_id(),
            admin_ed25519_pub,
            identity_proof,
            action,
            member,
            signature,
        };

        Ok((key_dists, membership_update))
    }

    /// Removes a member from an existing group and rotates the symmetric key.
    pub fn remove_member(
        &self,
        group_id: &[u8; 32],
        member: &PeerId,
        remaining_member_pubkeys: &[(PeerId, [u8; 32])],
        identity: &KeyPair,
    ) -> Result<
        (
            Vec<GroupKeyDistributionPayload>,
            GroupMembershipUpdatePayload,
        ),
        ChatError,
    > {
        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .get_mut(group_id)
            .ok_or_else(|| ChatError::InvalidInput("Group not found".into()))?;

        if group.creator != *identity.peer_id() {
            return Err(ChatError::InvalidInput(
                "Only group creator can remove members".into(),
            ));
        }

        group.members.remove(member);
        let new_key = group.rotate_key()?;

        // Encrypt new key for all remaining members
        let mut key_dists = Vec::with_capacity(remaining_member_pubkeys.len());
        for (recip_peer, recip_pub) in remaining_member_pubkeys {
            let encrypted_key = ManifestBuilder::wrap_file_key(recip_pub, &new_key)?;
            key_dists.push(GroupKeyDistributionPayload {
                group_id: *group_id,
                epoch: group.epoch,
                group_name: group.name.clone(),
                creator: group.creator,
                recipient: *recip_peer,
                encrypted_key,
            });
        }

        let action = 2u8; // 2 = Remove
        let mut sig_content = Vec::new();
        sig_content.extend_from_slice(group_id);
        sig_content.extend_from_slice(&group.epoch.to_be_bytes());
        sig_content.extend_from_slice(identity.peer_id().as_bytes());
        sig_content.push(action);
        sig_content.extend_from_slice(member.as_bytes());
        let signature = identity.sign(&sig_content);

        let admin_ed25519_pub = identity.ed25519_public_key();
        let identity_proof = identity.identity_proof();

        let membership_update = GroupMembershipUpdatePayload {
            group_id: *group_id,
            epoch: group.epoch,
            admin: *identity.peer_id(),
            admin_ed25519_pub,
            identity_proof,
            action,
            member: *member,
            signature,
        };

        Ok((key_dists, membership_update))
    }

    /// Handles an incoming group key distribution message for this node.
    pub fn handle_key_distribution(
        &self,
        payload: &GroupKeyDistributionPayload,
        identity: &KeyPair,
        event_bus: &Arc<EventBus>,
    ) -> Result<(), ChatError> {
        let group_key =
            ManifestBuilder::unwrap_file_key(identity.private_key(), &payload.encrypted_key)?;

        let mut groups = self.groups.lock().unwrap();
        if let Some(group) = groups.get_mut(&payload.group_id) {
            group.group_key = group_key;
            group.epoch = payload.epoch;
            group.members.insert(payload.recipient);
        } else {
            let mut members = std::collections::HashSet::new();
            members.insert(payload.creator);
            members.insert(payload.recipient);

            let group = GroupInfo {
                group_id: payload.group_id,
                name: payload.group_name.clone(),
                creator: payload.creator,
                members,
                epoch: payload.epoch,
                group_key,
                created_at: current_timestamp_secs(),
            };
            groups.insert(payload.group_id, group);
        }

        event_bus.publish(NodeEvent::GroupMembershipChanged {
            group_id: payload.group_id,
            group_name: payload.group_name.clone(),
            member: payload.recipient,
            added: true,
        });

        Ok(())
    }

    /// Handles an incoming group membership update.
    pub fn handle_membership_update(
        &self,
        payload: &GroupMembershipUpdatePayload,
        admin_x25519_pub: &[u8; 32],
        event_bus: &Arc<EventBus>,
    ) -> Result<(), ChatError> {
        if !KeyPair::verify_identity_proof(
            &payload.admin_ed25519_pub,
            admin_x25519_pub,
            &payload.identity_proof,
        ) {
            return Err(ChatError::SignatureVerificationFailed);
        }

        let mut sig_content = Vec::new();
        sig_content.extend_from_slice(&payload.group_id);
        sig_content.extend_from_slice(&payload.epoch.to_be_bytes());
        sig_content.extend_from_slice(payload.admin.as_bytes());
        sig_content.push(payload.action);
        sig_content.extend_from_slice(payload.member.as_bytes());

        if !ed25519_verify(&payload.admin_ed25519_pub, &sig_content, &payload.signature) {
            return Err(ChatError::SignatureVerificationFailed);
        }

        let mut groups = self.groups.lock().unwrap();
        if let Some(group) = groups.get_mut(&payload.group_id) {
            if payload.action == 1 {
                group.members.insert(payload.member);
            } else if payload.action == 2 {
                group.members.remove(&payload.member);
            }
            group.epoch = payload.epoch;

            event_bus.publish(NodeEvent::GroupMembershipChanged {
                group_id: payload.group_id,
                group_name: group.name.clone(),
                member: payload.member,
                added: payload.action == 1,
            });
        }

        Ok(())
    }

    /// Encrypts a message using the current group symmetric key.
    pub fn encrypt_group_message(
        &self,
        group_id: &[u8; 32],
        sender: &KeyPair,
        text: &str,
    ) -> Result<GroupMessagePayload, ChatError> {
        let groups = self.groups.lock().unwrap();
        let group = groups
            .get(group_id)
            .ok_or_else(|| ChatError::InvalidInput("Group not found".into()))?;

        if !group.is_member(sender.peer_id()) {
            return Err(ChatError::InvalidInput(
                "Sender is not a member of this group".into(),
            ));
        }

        let timestamp = current_timestamp_secs();
        let message_id = random_bytes_32().map_err(ChatError::Crypto)?;
        let nonce = random_bytes_12().map_err(ChatError::Crypto)?;

        // AAD = group_id || epoch || sender
        let mut aad = Vec::with_capacity(32 + 8 + 32);
        aad.extend_from_slice(group_id);
        aad.extend_from_slice(&group.epoch.to_be_bytes());
        aad.extend_from_slice(sender.peer_id().as_bytes());

        let (mut ciphertext, tag) =
            chacha20_poly1305_encrypt(&group.group_key, &nonce, &aad, text.as_bytes());
        ciphertext.extend_from_slice(&tag);

        Ok(GroupMessagePayload {
            group_id: *group_id,
            epoch: group.epoch,
            message_id,
            sender: *sender.peer_id(),
            timestamp,
            nonce,
            ciphertext,
        })
    }

    /// Decrypts and authenticates an incoming group message.
    pub fn decrypt_group_message(
        &self,
        payload: &GroupMessagePayload,
        event_bus: &Arc<EventBus>,
    ) -> Result<String, ChatError> {
        let groups = self.groups.lock().unwrap();
        let group = groups
            .get(&payload.group_id)
            .ok_or_else(|| ChatError::InvalidInput("Group not found".into()))?;

        if !group.is_member(&payload.sender) {
            return Err(ChatError::InvalidInput(
                "Sender is not a member of this group".into(),
            ));
        }

        if payload.ciphertext.len() < AEAD_TAG_LEN {
            return Err(ChatError::TamperedData);
        }

        let ct_len = payload.ciphertext.len() - AEAD_TAG_LEN;
        let ct = &payload.ciphertext[..ct_len];
        let tag: [u8; AEAD_TAG_LEN] = payload.ciphertext[ct_len..].try_into().unwrap();

        let mut aad = Vec::with_capacity(32 + 8 + 32);
        aad.extend_from_slice(&payload.group_id);
        aad.extend_from_slice(&payload.epoch.to_be_bytes());
        aad.extend_from_slice(payload.sender.as_bytes());

        let plaintext_bytes =
            chacha20_poly1305_decrypt(&group.group_key, &payload.nonce, &aad, ct, &tag)
                .map_err(|_| ChatError::TamperedData)?;

        let text = String::from_utf8(plaintext_bytes)
            .map_err(|_| ChatError::InvalidInput("Invalid UTF-8 in group message".into()))?;

        event_bus.publish(NodeEvent::GroupMessageReceived {
            group_id: payload.group_id,
            group_name: group.name.clone(),
            sender: payload.sender,
            text: text.clone(),
            timestamp: payload.timestamp,
        });

        Ok(text)
    }

    pub fn list_groups(&self) -> Vec<GroupInfo> {
        self.groups.lock().unwrap().values().cloned().collect()
    }

    pub fn get_group(&self, group_id: &[u8; 32]) -> Option<GroupInfo> {
        self.groups.lock().unwrap().get(group_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_chat_lifecycle() {
        let alice = KeyPair::generate().unwrap();
        let bob = KeyPair::generate().unwrap();
        let event_bus = EventBus::new(10);

        let alice_mgr = GroupManager::new();
        let bob_mgr = GroupManager::new();

        // 1. Alice creates group
        let group = alice_mgr.create_group("Project Titan", &alice).unwrap();
        assert_eq!(group.epoch, 1);
        assert_eq!(group.members.len(), 1);

        // 2. Alice adds Bob
        let (key_dists, _) = alice_mgr
            .add_member(
                &group.group_id,
                *bob.peer_id(),
                &[(*bob.peer_id(), *bob.public_key())],
                &alice,
            )
            .unwrap();

        // 3. Bob processes key distribution
        bob_mgr
            .handle_key_distribution(&key_dists[0], &bob, &event_bus)
            .unwrap();

        // 4. Bob sends message to group
        let msg_payload = bob_mgr
            .encrypt_group_message(&group.group_id, &bob, "Hello Titan team!")
            .unwrap();

        // 5. Alice decrypts Bob's message
        let decrypted = alice_mgr
            .decrypt_group_message(&msg_payload, &event_bus)
            .unwrap();
        assert_eq!(decrypted, "Hello Titan team!");

        // 6. Non-member Eve cannot decrypt
        let _eve = KeyPair::generate().unwrap();
        let eve_mgr = GroupManager::new();
        assert!(
            eve_mgr
                .decrypt_group_message(&msg_payload, &event_bus)
                .is_err()
        );
    }
}
