// src/files/manifest.rs
//! File Manifest construction, signing, verification, and key exchange.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::crypto::aead::{chacha20_poly1305_decrypt, chacha20_poly1305_encrypt};
use crate::crypto::ed25519::ed25519_verify;
use crate::crypto::hkdf::hkdf;
use crate::crypto::random::random_bytes_32;
use crate::crypto::sha256::Sha256;
use crate::crypto::x25519::{x25519, x25519_base_point};
use crate::error::ChatError;
use crate::files::chunk::{CHUNK_SIZE, encrypt_chunk, hash_chunk};
use crate::identity::keypair::KeyPair;
use crate::identity::peer_id::PeerId;
use crate::network::protocol::FileManifestPayload;

pub struct ManifestBuilder;

impl ManifestBuilder {
    /// Wraps a 32-byte symmetric file key for a recipient's X25519 public key.
    pub fn wrap_file_key(
        recipient_x25519_pub: &[u8; 32],
        file_key: &[u8; 32],
    ) -> Result<Vec<u8>, ChatError> {
        // Ephemeral DH keypair
        let eph_priv = random_bytes_32().map_err(ChatError::Crypto)?;
        let eph_pub = x25519_base_point(&eph_priv);
        let shared = x25519(&eph_priv, recipient_x25519_pub).map_err(ChatError::Crypto)?;

        let mut wrap_key = [0u8; 32];
        hkdf(b"", &shared, b"P2P-FILE-KEY-WRAP", &mut wrap_key).map_err(ChatError::Crypto)?;
        let nonce = [0u8; 12];
        let (mut enc_key, tag) = chacha20_poly1305_encrypt(&wrap_key, &nonce, &eph_pub, file_key);
        enc_key.extend_from_slice(&tag);

        let mut out = Vec::with_capacity(32 + enc_key.len());
        out.extend_from_slice(&eph_pub);
        out.extend_from_slice(&enc_key);
        Ok(out)
    }

    /// Unwraps an encrypted file key using the recipient's X25519 private key.
    pub fn unwrap_file_key(
        recipient_x25519_priv: &[u8; 32],
        wrapped: &[u8],
    ) -> Result<[u8; 32], ChatError> {
        if wrapped.len() < 32 + 16 {
            return Err(ChatError::InvalidInput("Wrapped key too short".into()));
        }
        let eph_pub: [u8; 32] = wrapped[0..32].try_into().unwrap();
        let enc_with_tag = &wrapped[32..];
        if enc_with_tag.len() < 16 {
            return Err(ChatError::InvalidInput(
                "Wrapped key ciphertext too short".into(),
            ));
        }
        let ct_len = enc_with_tag.len() - 16;
        let ct = &enc_with_tag[..ct_len];
        let tag: [u8; 16] = enc_with_tag[ct_len..].try_into().unwrap();

        let shared = x25519(recipient_x25519_priv, &eph_pub).map_err(ChatError::Crypto)?;
        let mut wrap_key = [0u8; 32];
        hkdf(b"", &shared, b"P2P-FILE-KEY-WRAP", &mut wrap_key).map_err(ChatError::Crypto)?;
        let nonce = [0u8; 12];
        let decrypted = chacha20_poly1305_decrypt(&wrap_key, &nonce, &eph_pub, ct, &tag)
            .map_err(ChatError::Crypto)?;

        if decrypted.len() != 32 {
            return Err(ChatError::InvalidInput("Decrypted key invalid size".into()));
        }
        Ok(decrypted.try_into().unwrap())
    }

    /// Builds a FileManifestPayload from a file on disk.
    pub fn create_manifest(
        path: &Path,
        file_key: &[u8; 32],
        sender_identity: &KeyPair,
        recipient: PeerId,
        recipient_x25519_pub: &[u8; 32],
    ) -> Result<(FileManifestPayload, Vec<[u8; 32]>), ChatError> {
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file.bin")
            .to_string();

        let chunk_count = if file_size == 0 {
            1
        } else {
            file_size.div_ceil(CHUNK_SIZE as u64) as u32
        };

        let mut file_hasher = Sha256::new();
        let mut chunk_hashes = Vec::with_capacity(chunk_count as usize);
        let mut chunk_buf = vec![0u8; CHUNK_SIZE];

        for chunk_idx in 0..chunk_count {
            let bytes_read = file.read(&mut chunk_buf)?;
            let slice = &chunk_buf[..bytes_read];
            file_hasher.update(slice);

            let encrypted = encrypt_chunk(file_key, chunk_idx, slice).map_err(ChatError::Crypto)?;
            chunk_hashes.push(hash_chunk(&encrypted));
        }

        let file_hash = file_hasher.finalize();

        // Derive unique file_id
        let mut id_hasher = Sha256::new();
        id_hasher.update(sender_identity.peer_id().as_bytes());
        id_hasher.update(file_name.as_bytes());
        id_hasher.update(&file_size.to_be_bytes());
        id_hasher.update(&file_hash);
        let file_id = id_hasher.finalize();

        // Wrap file key for recipient
        let encrypted_file_key = Self::wrap_file_key(recipient_x25519_pub, file_key)?;

        // Sign the manifest content
        let mut sig_content = Vec::new();
        sig_content.extend_from_slice(&file_id);
        sig_content.extend_from_slice(sender_identity.peer_id().as_bytes());
        sig_content.extend_from_slice(&file_size.to_be_bytes());
        sig_content.extend_from_slice(&file_hash);
        for h in &chunk_hashes {
            sig_content.extend_from_slice(h);
        }
        let sender_ed25519_pub = sender_identity.ed25519_public_key();
        let identity_proof = sender_identity.identity_proof();
        let signature = sender_identity.sign(&sig_content);

        let manifest = FileManifestPayload {
            file_id,
            sender: *sender_identity.peer_id(),
            recipient,
            sender_ed25519_pub,
            identity_proof,
            file_name,
            file_size,
            chunk_size: CHUNK_SIZE as u32,
            chunk_count,
            file_hash,
            encrypted_file_key,
            chunk_hashes: chunk_hashes.clone(),
            signature,
        };

        Ok((manifest, chunk_hashes))
    }

    /// Verifies the digital signature and authenticity of a FileManifestPayload.
    pub fn verify_manifest(manifest: &FileManifestPayload, sender_ed25519_pub: &[u8; 32]) -> bool {
        let mut sig_content = Vec::new();
        sig_content.extend_from_slice(&manifest.file_id);
        sig_content.extend_from_slice(manifest.sender.as_bytes());
        sig_content.extend_from_slice(&manifest.file_size.to_be_bytes());
        sig_content.extend_from_slice(&manifest.file_hash);
        for h in &manifest.chunk_hashes {
            sig_content.extend_from_slice(h);
        }
        ed25519_verify(sender_ed25519_pub, &sig_content, &manifest.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_unwrap_file_key() {
        let alice = KeyPair::generate().unwrap();
        let file_key = [77u8; 32];

        let wrapped =
            ManifestBuilder::wrap_file_key(alice.public_key(), &file_key).expect("wrap file key");
        let unwrapped = ManifestBuilder::unwrap_file_key(alice.private_key(), &wrapped)
            .expect("unwrap file key");

        assert_eq!(file_key, unwrapped);

        // Bob cannot unwrap Alice's key
        let bob = KeyPair::generate().unwrap();
        assert!(ManifestBuilder::unwrap_file_key(bob.private_key(), &wrapped).is_err());
    }
}
