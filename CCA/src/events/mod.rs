// src/events/mod.rs
//! Internal Event Bus and notification system for decoupled node event handling.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use crate::identity::peer_id::PeerId;
use crate::messaging::message::ChatMessage;

#[derive(Debug, Clone)]
pub enum NodeEvent {
    PeerConnected {
        peer_id: PeerId,
        addr: SocketAddr,
    },
    PeerDisconnected {
        peer_id: PeerId,
    },
    DirectMessageReceived {
        message: ChatMessage,
    },
    DeliveryReceiptReceived {
        message_id: [u8; 32],
        recipient: PeerId,
        timestamp: u64,
    },
    FileOfferReceived {
        file_id: [u8; 32],
        sender: PeerId,
        file_name: String,
        file_size: u64,
    },
    FileChunkProgress {
        file_id: [u8; 32],
        chunks_completed: u32,
        total_chunks: u32,
        bytes_transferred: u64,
    },
    FileTransferCompleted {
        file_id: [u8; 32],
        file_name: String,
        path: String,
    },
    FileTransferFailed {
        file_id: [u8; 32],
        reason: String,
    },
    GroupMessageReceived {
        group_id: [u8; 32],
        group_name: String,
        sender: PeerId,
        text: String,
        timestamp: u64,
    },
    GroupMembershipChanged {
        group_id: [u8; 32],
        group_name: String,
        member: PeerId,
        added: bool,
    },
    PeerDiscovered {
        peer_id: PeerId,
        public_key: [u8; 32],
        addr: SocketAddr,
        device_name: String,
    },
    Notice(String),
}

type EventHandler = Box<dyn Fn(&NodeEvent) + Send + Sync>;

#[derive(Default)]
pub struct EventBus {
    subscribers: Mutex<Vec<EventHandler>>,
    history: Mutex<Vec<NodeEvent>>,
    max_history: usize,
}

impl EventBus {
    pub fn new(max_history: usize) -> Arc<Self> {
        Arc::new(Self {
            subscribers: Mutex::new(Vec::new()),
            history: Mutex::new(Vec::new()),
            max_history,
        })
    }

    /// Registers a new event listener callback.
    pub fn subscribe<F>(&self, handler: F)
    where
        F: Fn(&NodeEvent) + Send + Sync + 'static,
    {
        let mut subs = self.subscribers.lock().unwrap();
        subs.push(Box::new(handler));
    }

    /// Dispatches an event to all registered listeners.
    pub fn publish(&self, event: NodeEvent) {
        // Record in history
        {
            let mut hist = self.history.lock().unwrap();
            if hist.len() >= self.max_history && self.max_history > 0 {
                hist.remove(0);
            }
            hist.push(event.clone());
        }

        // Notify subscribers
        let subs = self.subscribers.lock().unwrap();
        for sub in subs.iter() {
            sub(&event);
        }
    }

    /// Retrieves a snapshot of the recent event history.
    pub fn get_history(&self) -> Vec<NodeEvent> {
        self.history.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new(10);
        let counter = Arc::new(AtomicUsize::new(0));

        let c_clone = counter.clone();
        bus.subscribe(move |event| {
            if let NodeEvent::Notice(_) = event {
                c_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        bus.publish(NodeEvent::Notice("Test notice 1".into()));
        bus.publish(NodeEvent::Notice("Test notice 2".into()));

        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert_eq!(bus.get_history().len(), 2);
    }
}
