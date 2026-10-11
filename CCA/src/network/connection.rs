// src/network/connection.rs
//! TCP Peer Connection wrapper with framing, timeouts, and state tracking.

use crate::error::NetworkError;
use crate::network::frame::Frame;
use std::io::{BufReader, BufWriter};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct PeerConnection {
    pub remote_addr: SocketAddr,
    reader: Mutex<BufReader<TcpStream>>,
    writer: Mutex<BufWriter<TcpStream>>,
    closed: Arc<AtomicBool>,
}

impl PeerConnection {
    pub fn new(
        stream: TcpStream,
        remote_addr: SocketAddr,
        read_timeout: Duration,
    ) -> Result<Self, NetworkError> {
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(read_timeout))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        stream.set_nodelay(true)?;

        let read_stream = stream.try_clone()?;
        let write_stream = stream;

        Ok(Self {
            remote_addr,
            reader: Mutex::new(BufReader::new(read_stream)),
            writer: Mutex::new(BufWriter::new(write_stream)),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Connects to a remote peer address with a timeout.
    pub fn connect(
        addr: SocketAddr,
        connect_timeout: Duration,
        read_timeout: Duration,
    ) -> Result<Self, NetworkError> {
        let stream = TcpStream::connect_timeout(&addr, connect_timeout)?;
        Self::new(stream, addr, read_timeout)
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }

    /// Sends a frame to the remote peer.
    pub fn send(&self, frame: &Frame) -> Result<(), NetworkError> {
        if self.is_closed() {
            return Err(NetworkError::ConnectionClosed);
        }
        let mut writer = self.writer.lock().unwrap();
        frame.write_to(&mut *writer).map_err(|e| {
            self.close();
            NetworkError::Protocol(e)
        })
    }

    /// Reads a single frame from the remote peer.
    pub fn receive(&self) -> Result<Frame, NetworkError> {
        if self.is_closed() {
            return Err(NetworkError::ConnectionClosed);
        }
        let mut reader = self.reader.lock().unwrap();
        Frame::read_from(&mut *reader).map_err(|e| {
            self.close();
            NetworkError::Protocol(e)
        })
    }
}
