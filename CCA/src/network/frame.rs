// src/network/frame.rs
//! Low-level binary framing for P2P network transport.
//!
//! Frame layout:
//! [0..4]   Magic: b"P2CH"
//! [4..6]   Version: u16 (big-endian)
//! [6]      Message Type: u8
//! [7]      Flags: u8
//! [8..12]  Payload Length: u32 (big-endian)
//! [12..]   Payload: N bytes

use crate::error::ProtocolError;
use crate::types::{MAX_FRAME_SIZE, PROTOCOL_MAGIC, PROTOCOL_VERSION};
use std::io::{Read, Write};

pub const HEADER_LEN: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub version: u16,
    pub message_type: u8,
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(message_type: u8, flags: u8, payload: Vec<u8>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            message_type,
            flags,
            payload,
        }
    }

    /// Encodes this frame into a byte vector.
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        if self.payload.len() > MAX_FRAME_SIZE {
            return Err(ProtocolError::FrameTooLarge {
                size: self.payload.len(),
                max: MAX_FRAME_SIZE,
            });
        }

        let mut buf = Vec::with_capacity(HEADER_LEN + self.payload.len());
        buf.extend_from_slice(&PROTOCOL_MAGIC);
        buf.extend_from_slice(&self.version.to_be_bytes());
        buf.push(self.message_type);
        buf.push(self.flags);
        buf.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        buf.extend_from_slice(&self.payload);
        Ok(buf)
    }

    /// Writes this encoded frame directly to a writer.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<(), ProtocolError> {
        let bytes = self.encode()?;
        writer.write_all(&bytes)?;
        writer.flush()?;
        Ok(())
    }

    /// Reads and decodes a single frame from a reader.
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, ProtocolError> {
        let mut header = [0u8; HEADER_LEN];
        reader.read_exact(&mut header).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                ProtocolError::UnexpectedEof
            } else {
                ProtocolError::Io(e)
            }
        })?;

        // 1. Verify Magic
        let magic: [u8; 4] = header[0..4].try_into().unwrap();
        if magic != PROTOCOL_MAGIC {
            return Err(ProtocolError::InvalidMagic(magic));
        }

        // 2. Verify Version
        let version = u16::from_be_bytes(header[4..6].try_into().unwrap());
        if version != PROTOCOL_VERSION {
            return Err(ProtocolError::UnsupportedVersion(version));
        }

        let message_type = header[6];
        let flags = header[7];
        let payload_len = u32::from_be_bytes(header[8..12].try_into().unwrap()) as usize;

        // 3. Verify maximum frame size before allocation
        if payload_len > MAX_FRAME_SIZE {
            return Err(ProtocolError::FrameTooLarge {
                size: payload_len,
                max: MAX_FRAME_SIZE,
            });
        }

        // 4. Read payload
        let mut payload = vec![0u8; payload_len];
        reader.read_exact(&mut payload).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                ProtocolError::UnexpectedEof
            } else {
                ProtocolError::Io(e)
            }
        })?;

        Ok(Self {
            version,
            message_type,
            flags,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_frame_encode_decode() {
        let original = Frame::new(0x10, 0x00, b"Hello P2P Frame!".to_vec());
        let mut encoded = Vec::new();
        original.write_to(&mut encoded).expect("write frame");

        let mut cursor = Cursor::new(encoded);
        let decoded = Frame::read_from(&mut cursor).expect("read frame");

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_invalid_magic() {
        let mut bad_data = vec![b'B', b'A', b'D', b'!'];
        bad_data.extend_from_slice(&[0; 8]);
        let mut cursor = Cursor::new(bad_data);
        assert!(matches!(
            Frame::read_from(&mut cursor),
            Err(ProtocolError::InvalidMagic(_))
        ));
    }

    #[test]
    fn test_oversized_frame_rejected() {
        let mut bad_header = Vec::new();
        bad_header.extend_from_slice(&PROTOCOL_MAGIC);
        bad_header.extend_from_slice(&1u16.to_be_bytes());
        bad_header.push(0x10);
        bad_header.push(0x00);
        bad_header.extend_from_slice(&(600_000u32).to_be_bytes()); // > 512 KiB
        let mut cursor = Cursor::new(bad_header);
        assert!(matches!(
            Frame::read_from(&mut cursor),
            Err(ProtocolError::FrameTooLarge { .. })
        ));
    }
}
