// src/storage/record.rs
//! Low-level storage record format with magic header, record type, length, and Adler-32 checksum.
//!
//! Record layout:
//! [0..4]   Magic: b"P2RC"
//! [4..6]   Version: u16 (1)
//! [6..8]   Record Type: u16
//! [8..12]  Payload Length: u32
//! [12..]   Payload: N bytes
//! [..+4]   Adler-32 Checksum: u32

use crate::error::StorageError;
use crate::types::adler32;
use std::io::{Read, Write};

pub const RECORD_MAGIC: [u8; 4] = *b"P2RC";
pub const RECORD_VERSION: u16 = 1;

pub const TYPE_IDENTITY: u16 = 1;
pub const TYPE_CONTACT: u16 = 2;
pub const TYPE_PEER_ADDR: u16 = 3;
pub const TYPE_CHAT_MESSAGE: u16 = 4;
pub const TYPE_RELAY_ENVELOPE: u16 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRecord {
    pub record_type: u16,
    pub payload: Vec<u8>,
}

impl StorageRecord {
    pub fn new(record_type: u16, payload: Vec<u8>) -> Self {
        Self {
            record_type,
            payload,
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), StorageError> {
        let checksum = adler32(&self.payload);
        writer.write_all(&RECORD_MAGIC)?;
        writer.write_all(&RECORD_VERSION.to_be_bytes())?;
        writer.write_all(&self.record_type.to_be_bytes())?;
        writer.write_all(&(self.payload.len() as u32).to_be_bytes())?;
        writer.write_all(&self.payload)?;
        writer.write_all(&checksum.to_be_bytes())?;
        Ok(())
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Option<Self>, StorageError> {
        let mut magic = [0u8; 4];
        match reader.read_exact(&mut magic) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(StorageError::Io(e)),
        }

        if magic != RECORD_MAGIC {
            return Err(StorageError::CorruptData(format!(
                "Invalid storage magic: {magic:?}"
            )));
        }

        let mut v_buf = [0u8; 2];
        reader.read_exact(&mut v_buf)?;
        let version = u16::from_be_bytes(v_buf);
        if version != RECORD_VERSION {
            return Err(StorageError::CorruptData(format!(
                "Unsupported storage version: {version}"
            )));
        }

        let mut t_buf = [0u8; 2];
        reader.read_exact(&mut t_buf)?;
        let record_type = u16::from_be_bytes(t_buf);

        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf)?;
        let payload_len = u32::from_be_bytes(len_buf) as usize;

        // Bound payload length to 10 MB per record
        if payload_len > 10 * 1024 * 1024 {
            return Err(StorageError::CorruptData(format!(
                "Record payload too large: {payload_len} bytes"
            )));
        }

        let mut payload = vec![0u8; payload_len];
        reader.read_exact(&mut payload)?;

        let mut cs_buf = [0u8; 4];
        reader.read_exact(&mut cs_buf)?;
        let expected_cs = u32::from_be_bytes(cs_buf);
        let actual_cs = adler32(&payload);

        if expected_cs != actual_cs {
            return Err(StorageError::ChecksumMismatch {
                expected: expected_cs,
                calculated: actual_cs,
            });
        }

        Ok(Some(Self {
            record_type,
            payload,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_record_roundtrip() {
        let original = StorageRecord::new(
            TYPE_CHAT_MESSAGE,
            b"Important persisted chat message".to_vec(),
        );
        let mut buf = Vec::new();
        original.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(buf);
        let read = StorageRecord::read_from(&mut cursor)
            .unwrap()
            .expect("record found");
        assert_eq!(original, read);
    }

    #[test]
    fn test_record_checksum_tamper() {
        let original = StorageRecord::new(TYPE_CHAT_MESSAGE, b"Secret".to_vec());
        let mut buf = Vec::new();
        original.write_to(&mut buf).unwrap();

        // Tamper payload byte
        buf[12] ^= 0x01;

        let mut cursor = Cursor::new(buf);
        assert!(matches!(
            StorageRecord::read_from(&mut cursor),
            Err(StorageError::ChecksumMismatch { .. })
        ));
    }
}
