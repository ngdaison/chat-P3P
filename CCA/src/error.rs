// src/error.rs
//! Unified error types for P2P Chat Core.
//!
//! Designed without external crates, using standard Rust `Display` and `std::error::Error`.

use std::fmt;

#[derive(Debug)]
pub enum CryptoError {
    RandomGenerationFailed(String),
    InvalidKeyLength { expected: usize, actual: usize },
    InvalidNonceLength { expected: usize, actual: usize },
    InvalidTagLength { expected: usize, actual: usize },
    AuthenticationFailed,
    WeakSharedSecret,
    HkdfExpandTooLong,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CryptoError::RandomGenerationFailed(msg) => {
                write!(f, "Random generation failed: {msg}")
            }
            CryptoError::InvalidKeyLength { expected, actual } => {
                write!(f, "Invalid key length: expected {expected}, got {actual}")
            }
            CryptoError::InvalidNonceLength { expected, actual } => {
                write!(f, "Invalid nonce length: expected {expected}, got {actual}")
            }
            CryptoError::InvalidTagLength { expected, actual } => {
                write!(f, "Invalid tag length: expected {expected}, got {actual}")
            }
            CryptoError::AuthenticationFailed => {
                write!(f, "AEAD authentication tag verification failed")
            }
            CryptoError::WeakSharedSecret => write!(
                f,
                "X25519 shared secret is all zeros (weak/invalid peer key)"
            ),
            CryptoError::HkdfExpandTooLong => {
                write!(f, "HKDF requested expansion length is too long")
            }
        }
    }
}

impl std::error::Error for CryptoError {}

#[derive(Debug)]
pub enum ProtocolError {
    InvalidMagic([u8; 4]),
    UnsupportedVersion(u16),
    UnknownMessageType(u8),
    FrameTooLarge { size: usize, max: usize },
    MalformedPayload(String),
    UnexpectedEof,
    Io(std::io::Error),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::InvalidMagic(m) => write!(f, "Invalid frame magic bytes: {m:?}"),
            ProtocolError::UnsupportedVersion(v) => write!(f, "Unsupported protocol version: {v}"),
            ProtocolError::UnknownMessageType(t) => {
                write!(f, "Unknown protocol message type: 0x{t:02X}")
            }
            ProtocolError::FrameTooLarge { size, max } => {
                write!(f, "Frame size {size} exceeds maximum permitted {max}")
            }
            ProtocolError::MalformedPayload(msg) => write!(f, "Malformed protocol payload: {msg}"),
            ProtocolError::UnexpectedEof => {
                write!(f, "Unexpected end of stream while reading frame")
            }
            ProtocolError::Io(e) => write!(f, "Protocol I/O error: {e}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

impl From<std::io::Error> for ProtocolError {
    fn from(e: std::io::Error) -> Self {
        ProtocolError::Io(e)
    }
}

#[derive(Debug)]
pub enum NetworkError {
    Io(std::io::Error),
    Protocol(ProtocolError),
    ConnectionClosed,
    ConnectionTimeout,
    MaxPeersReached(usize),
    DuplicateConnection(String),
    HandshakeFailed(String),
    PeerNotFound(String),
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::Io(e) => write!(f, "Network I/O error: {e}"),
            NetworkError::Protocol(e) => write!(f, "Network protocol error: {e}"),
            NetworkError::ConnectionClosed => write!(f, "Connection closed by remote peer"),
            NetworkError::ConnectionTimeout => write!(f, "Connection timed out"),
            NetworkError::MaxPeersReached(max) => {
                write!(f, "Maximum peer connection limit reached ({max})")
            }
            NetworkError::DuplicateConnection(id) => write!(f, "Already connected to peer: {id}"),
            NetworkError::HandshakeFailed(msg) => write!(f, "Handshake failed: {msg}"),
            NetworkError::PeerNotFound(id) => write!(f, "Peer not found: {id}"),
        }
    }
}

impl std::error::Error for NetworkError {}

impl From<std::io::Error> for NetworkError {
    fn from(e: std::io::Error) -> Self {
        NetworkError::Io(e)
    }
}

impl From<ProtocolError> for NetworkError {
    fn from(e: ProtocolError) -> Self {
        NetworkError::Protocol(e)
    }
}

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    CorruptData(String),
    QuotaExceeded { used: usize, limit: usize },
    NotFound(String),
    ChecksumMismatch { expected: u32, calculated: u32 },
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Io(e) => write!(f, "Storage I/O error: {e}"),
            StorageError::CorruptData(msg) => write!(f, "Storage corruption: {msg}"),
            StorageError::QuotaExceeded { used, limit } => {
                write!(f, "Storage quota exceeded: {used}/{limit} bytes")
            }
            StorageError::NotFound(item) => write!(f, "Item not found in storage: {item}"),
            StorageError::ChecksumMismatch {
                expected,
                calculated,
            } => {
                write!(
                    f,
                    "Storage record checksum mismatch: expected {expected:#x}, got {calculated:#x}"
                )
            }
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        StorageError::Io(e)
    }
}

#[derive(Debug)]
pub enum ChatError {
    Crypto(CryptoError),
    Protocol(ProtocolError),
    Network(NetworkError),
    Storage(StorageError),
    InvalidInput(String),
    ContactNotFound(String),
    MessageExpired,
    DuplicateMessage(String),
    SignatureVerificationFailed,
    TamperedData,
    Other(String),
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::Crypto(e) => write!(f, "Crypto error: {e}"),
            ChatError::Protocol(e) => write!(f, "Protocol error: {e}"),
            ChatError::Network(e) => write!(f, "Network error: {e}"),
            ChatError::Storage(e) => write!(f, "Storage error: {e}"),
            ChatError::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
            ChatError::ContactNotFound(name) => write!(f, "Contact not found: {name}"),
            ChatError::MessageExpired => write!(f, "Message has expired"),
            ChatError::DuplicateMessage(id) => write!(f, "Duplicate message ID: {id}"),
            ChatError::SignatureVerificationFailed => {
                write!(f, "Digital signature verification failed")
            }
            ChatError::TamperedData => write!(f, "Tampered or corrupted data detected"),
            ChatError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ChatError {}

impl From<CryptoError> for ChatError {
    fn from(e: CryptoError) -> Self {
        ChatError::Crypto(e)
    }
}

impl From<ProtocolError> for ChatError {
    fn from(e: ProtocolError) -> Self {
        ChatError::Protocol(e)
    }
}

impl From<NetworkError> for ChatError {
    fn from(e: NetworkError) -> Self {
        ChatError::Network(e)
    }
}

impl From<StorageError> for ChatError {
    fn from(e: StorageError) -> Self {
        ChatError::Storage(e)
    }
}

impl From<std::io::Error> for ChatError {
    fn from(e: std::io::Error) -> Self {
        ChatError::Storage(StorageError::Io(e))
    }
}
