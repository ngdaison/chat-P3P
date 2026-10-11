// src/files/transfer_manager.rs
//! File Transfer Manager coordinating chunk streaming, reassembly, resume, and hash verification.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::crypto::random::random_bytes_32;
use crate::crypto::sha256::Sha256;
use crate::error::ChatError;
use crate::events::{EventBus, NodeEvent};
use crate::files::chunk::{CHUNK_SIZE, decrypt_chunk, encrypt_chunk, hash_chunk};
use crate::files::manifest::ManifestBuilder;
use crate::identity::contacts::ContactBook;
use crate::identity::keypair::KeyPair;
use crate::identity::peer_id::PeerId;
use crate::network::protocol::{
    FileChunkRequestPayload, FileChunkResponsePayload, FileManifestPayload,
};
use crate::types::to_hex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferState {
    Active,
    Completed,
    Failed(String),
    Cancelled,
}

pub struct OutboundTransfer {
    pub file_id: [u8; 32],
    pub file_path: PathBuf,
    pub recipient: PeerId,
    pub file_key: [u8; 32],
    pub manifest: FileManifestPayload,
    pub state: TransferState,
    pub chunks_requested: HashSet<u32>,
}

pub struct InboundTransfer {
    pub file_id: [u8; 32],
    pub sender: PeerId,
    pub file_name: String,
    pub file_size: u64,
    pub file_key: [u8; 32],
    pub manifest: FileManifestPayload,
    pub received_chunk_indices: HashSet<u32>,
    pub temp_path: PathBuf,
    pub destination_path: PathBuf,
    pub state: TransferState,
}

pub struct FileTransferManager {
    outbound: Mutex<HashMap<[u8; 32], OutboundTransfer>>,
    inbound: Mutex<HashMap<[u8; 32], InboundTransfer>>,
    downloads_dir: PathBuf,
}

impl FileTransferManager {
    pub fn new<P: AsRef<Path>>(downloads_dir: P) -> Result<Self, ChatError> {
        let dir = downloads_dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            outbound: Mutex::new(HashMap::new()),
            inbound: Mutex::new(HashMap::new()),
            downloads_dir: dir,
        })
    }

    /// Prepares a new file for sending: reads file, computes hashes, creates manifest.
    pub fn prepare_outbound(
        &self,
        file_path: &Path,
        recipient: PeerId,
        recipient_x25519_pub: &[u8; 32],
        identity: &KeyPair,
    ) -> Result<FileManifestPayload, ChatError> {
        let file_key = random_bytes_32().map_err(ChatError::Crypto)?;
        let (manifest, _) = ManifestBuilder::create_manifest(
            file_path,
            &file_key,
            identity,
            recipient,
            recipient_x25519_pub,
        )?;

        let transfer = OutboundTransfer {
            file_id: manifest.file_id,
            file_path: file_path.to_path_buf(),
            recipient,
            file_key,
            manifest: manifest.clone(),
            state: TransferState::Active,
            chunks_requested: HashSet::new(),
        };

        self.outbound
            .lock()
            .unwrap()
            .insert(manifest.file_id, transfer);
        Ok(manifest)
    }

    /// Serves a requested chunk to a remote peer.
    pub fn handle_chunk_request(
        &self,
        req: &FileChunkRequestPayload,
    ) -> Result<Option<FileChunkResponsePayload>, ChatError> {
        let mut outbound = self.outbound.lock().unwrap();
        let transfer = match outbound.get_mut(&req.file_id) {
            Some(t) => t,
            None => return Ok(None),
        };

        if req.chunk_index >= transfer.manifest.chunk_count {
            return Ok(None);
        }

        // Read chunk from disk
        let mut file = File::open(&transfer.file_path)?;
        let offset = req.chunk_index as u64 * CHUNK_SIZE as u64;
        file.seek(SeekFrom::Start(offset))?;

        let mut buf = vec![0u8; CHUNK_SIZE];
        let bytes_read = file.read(&mut buf)?;
        let slice = &buf[..bytes_read];

        let encrypted =
            encrypt_chunk(&transfer.file_key, req.chunk_index, slice).map_err(ChatError::Crypto)?;
        let chunk_hash = hash_chunk(&encrypted);

        transfer.chunks_requested.insert(req.chunk_index);

        Ok(Some(FileChunkResponsePayload {
            file_id: req.file_id,
            chunk_index: req.chunk_index,
            chunk_hash,
            data: encrypted,
        }))
    }

    /// Registers a newly offered file from a remote peer after verifying its manifest.
    pub fn handle_inbound_manifest(
        &self,
        manifest: FileManifestPayload,
        identity: &KeyPair,
        contact_book: &ContactBook,
        event_bus: &Arc<EventBus>,
    ) -> Result<(), ChatError> {
        // 1. Verify sender identity
        let sender_contact = contact_book
            .get_by_peer_id(&manifest.sender)
            .ok_or_else(|| ChatError::ContactNotFound(manifest.sender.to_hex()))?;

        // Verify cryptographic identity proof binding X25519 DH key to Ed25519 signing key
        if !KeyPair::verify_identity_proof(
            &manifest.sender_ed25519_pub,
            &sender_contact.public_key,
            &manifest.identity_proof,
        ) {
            return Err(ChatError::SignatureVerificationFailed);
        }

        // Verify digital signature on the manifest
        if !ManifestBuilder::verify_manifest(&manifest, &manifest.sender_ed25519_pub) {
            return Err(ChatError::SignatureVerificationFailed);
        }

        // 2. Unwrap symmetric file key
        let file_key =
            ManifestBuilder::unwrap_file_key(identity.private_key(), &manifest.encrypted_file_key)?;

        // 3. Prepare part file path and final destination
        let part_filename = format!("{}.part", to_hex(&manifest.file_id));
        let temp_path = self.downloads_dir.join(part_filename);
        let destination_path = self.downloads_dir.join(&manifest.file_name);

        // Resume support: check if part file already exists
        let mut received = HashSet::new();
        if let Ok(metadata) = temp_path.metadata() {
            let existing_chunks = (metadata.len() / CHUNK_SIZE as u64) as u32;
            for idx in 0..existing_chunks {
                if idx < manifest.chunk_count {
                    received.insert(idx);
                }
            }
        }

        let inbound = InboundTransfer {
            file_id: manifest.file_id,
            sender: manifest.sender,
            file_name: manifest.file_name.clone(),
            file_size: manifest.file_size,
            file_key,
            manifest: manifest.clone(),
            received_chunk_indices: received,
            temp_path,
            destination_path,
            state: TransferState::Active,
        };

        self.inbound
            .lock()
            .unwrap()
            .insert(manifest.file_id, inbound);

        event_bus.publish(NodeEvent::FileOfferReceived {
            file_id: manifest.file_id,
            sender: manifest.sender,
            file_name: manifest.file_name,
            file_size: manifest.file_size,
        });

        Ok(())
    }

    /// Returns the index of the next chunk needed for an active inbound transfer.
    pub fn get_next_missing_chunk(&self, file_id: &[u8; 32]) -> Option<u32> {
        let inbound = self.inbound.lock().unwrap();
        let transfer = inbound.get(file_id)?;
        if transfer.state != TransferState::Active {
            return None;
        }

        (0..transfer.manifest.chunk_count)
            .find(|idx| !transfer.received_chunk_indices.contains(idx))
    }

    /// Processes an incoming chunk response, verifies integrity, writes to disk.
    /// Returns Ok(true) if the entire file is fully downloaded and verified.
    pub fn handle_chunk_response(
        &self,
        resp: &FileChunkResponsePayload,
        event_bus: &Arc<EventBus>,
    ) -> Result<bool, ChatError> {
        let mut inbound = self.inbound.lock().unwrap();
        let transfer = match inbound.get_mut(&resp.file_id) {
            Some(t) => t,
            None => return Ok(false),
        };

        if transfer.state != TransferState::Active {
            return Ok(false);
        }

        let idx = resp.chunk_index as usize;
        if idx >= transfer.manifest.chunk_hashes.len() {
            return Err(ChatError::InvalidInput("Chunk index out of bounds".into()));
        }

        // 1. Verify chunk ciphertext hash against manifest
        let expected_hash = transfer.manifest.chunk_hashes[idx];
        let actual_hash = hash_chunk(&resp.data);
        if actual_hash != expected_hash || actual_hash != resp.chunk_hash {
            return Err(ChatError::TamperedData);
        }

        // 2. Decrypt chunk plaintext
        let plaintext = decrypt_chunk(&transfer.file_key, resp.chunk_index, &resp.data)
            .map_err(ChatError::Crypto)?;

        // 3. Write chunk into temporary .part file
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .read(true)
            .open(&transfer.temp_path)?;

        let offset = resp.chunk_index as u64 * CHUNK_SIZE as u64;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(&plaintext)?;
        file.flush()?;

        transfer.received_chunk_indices.insert(resp.chunk_index);

        let chunks_completed = transfer.received_chunk_indices.len() as u32;
        let total_chunks = transfer.manifest.chunk_count;

        event_bus.publish(NodeEvent::FileChunkProgress {
            file_id: resp.file_id,
            chunks_completed,
            total_chunks,
            bytes_transferred: chunks_completed as u64 * CHUNK_SIZE as u64,
        });

        // 4. Check if all chunks received
        if chunks_completed >= total_chunks {
            // Verify entire file hash
            file.seek(SeekFrom::Start(0))?;
            let mut full_hasher = Sha256::new();
            let mut read_buf = vec![0u8; CHUNK_SIZE];
            let mut total_read = 0u64;

            while total_read < transfer.file_size {
                let to_read = (transfer.file_size - total_read).min(CHUNK_SIZE as u64) as usize;
                let n = file.read(&mut read_buf[..to_read])?;
                if n == 0 {
                    break;
                }
                full_hasher.update(&read_buf[..n]);
                total_read += n as u64;
            }

            let full_hash = full_hasher.finalize();
            if full_hash != transfer.manifest.file_hash {
                transfer.state = TransferState::Failed("Full file hash mismatch".into());
                let _ = fs::remove_file(&transfer.temp_path);
                event_bus.publish(NodeEvent::FileTransferFailed {
                    file_id: resp.file_id,
                    reason: "Full file hash verification failed".into(),
                });
                return Err(ChatError::TamperedData);
            }

            // Truncate exact file size (in case last chunk was padded)
            file.set_len(transfer.file_size)?;
            drop(file);

            // Atomic rename from .part to final destination
            fs::rename(&transfer.temp_path, &transfer.destination_path)?;
            transfer.state = TransferState::Completed;

            event_bus.publish(NodeEvent::FileTransferCompleted {
                file_id: resp.file_id,
                file_name: transfer.file_name.clone(),
                path: transfer.destination_path.to_string_lossy().to_string(),
            });

            return Ok(true);
        }

        Ok(false)
    }

    /// Cancels an in-progress transfer.
    pub fn cancel_transfer(&self, file_id: &[u8; 32]) -> bool {
        let mut cancelled = false;
        if let Some(t) = self.outbound.lock().unwrap().get_mut(file_id) {
            t.state = TransferState::Cancelled;
            cancelled = true;
        }
        if let Some(t) = self.inbound.lock().unwrap().get_mut(file_id) {
            t.state = TransferState::Cancelled;
            let _ = fs::remove_file(&t.temp_path);
            cancelled = true;
        }
        cancelled
    }

    /// Summarizes all file transfers for CLI display.
    pub fn get_status_summary(&self) -> Vec<String> {
        let mut lines = Vec::new();
        {
            let outbound = self.outbound.lock().unwrap();
            for (id, t) in outbound.iter() {
                lines.push(format!(
                    "[OUT] ID: {} | File: {:?} | Recipient: {} | State: {:?}",
                    to_hex(id),
                    t.file_path.file_name().unwrap_or_default(),
                    t.recipient,
                    t.state
                ));
            }
        }
        {
            let inbound = self.inbound.lock().unwrap();
            for (id, t) in inbound.iter() {
                lines.push(format!(
                    "[IN]  ID: {} | File: {} ({} bytes) | Progress: {}/{} chunks | State: {:?}",
                    to_hex(id),
                    t.file_name,
                    t.file_size,
                    t.received_chunk_indices.len(),
                    t.manifest.chunk_count,
                    t.state
                ));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_transfer_end_to_end_local() {
        let test_dir = std::env::temp_dir().join(format!("p2p_file_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();

        let sender_dir = test_dir.join("sender");
        let receiver_dir = test_dir.join("receiver");
        fs::create_dir_all(&sender_dir).unwrap();
        fs::create_dir_all(&receiver_dir).unwrap();

        // 1. Create a sample 600 KiB file (spans 3 chunks: 256 + 256 + 88 KiB)
        let sample_file = sender_dir.join("sample.dat");
        let sample_data = vec![0xABu8; 600 * 1024];
        fs::write(&sample_file, &sample_data).unwrap();

        let sender = KeyPair::generate().unwrap();
        let receiver = KeyPair::generate().unwrap();

        let mut receiver_contacts = ContactBook::new();
        receiver_contacts
            .add_from_hex("Sender", &sender.public_key_hex())
            .unwrap();

        let event_bus = EventBus::new(10);
        let sender_mgr = FileTransferManager::new(&sender_dir).unwrap();
        let receiver_mgr = FileTransferManager::new(&receiver_dir).unwrap();

        // 2. Sender prepares outbound transfer
        let manifest = sender_mgr
            .prepare_outbound(
                &sample_file,
                *receiver.peer_id(),
                receiver.public_key(),
                &sender,
            )
            .unwrap();

        assert_eq!(manifest.chunk_count, 3);

        // 3. Receiver receives manifest
        receiver_mgr
            .handle_inbound_manifest(manifest.clone(), &receiver, &receiver_contacts, &event_bus)
            .unwrap();

        // 4. Chunk by chunk transfer
        for chunk_idx in 0..manifest.chunk_count {
            let req = FileChunkRequestPayload {
                file_id: manifest.file_id,
                chunk_index: chunk_idx,
                requester: *receiver.peer_id(),
            };

            let resp = sender_mgr.handle_chunk_request(&req).unwrap().unwrap();
            let is_completed = receiver_mgr
                .handle_chunk_response(&resp, &event_bus)
                .unwrap();

            if chunk_idx == manifest.chunk_count - 1 {
                assert!(is_completed);
            } else {
                assert!(!is_completed);
            }
        }

        // 5. Verify the received file on disk
        let downloaded_file = receiver_dir.join(&manifest.file_name);
        assert!(downloaded_file.exists());
        let downloaded_data = fs::read(&downloaded_file).unwrap();
        assert_eq!(downloaded_data, sample_data);

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }
}
