// src/node/runtime.rs
//! Core P2P Node runtime: coordinates networking, identity, storage, messaging, and relay.

use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config::NodeConfig;
use crate::error::ChatError;
use crate::events::EventBus;
use crate::files::FileTransferManager;
use crate::groups::{GroupInfo, GroupManager};
use crate::identity::contacts::{Contact, ContactBook};
use crate::identity::keypair::KeyPair;
use crate::identity::peer_id::PeerId;
use crate::messaging::envelope::MessageEnvelope;
use crate::messaging::message::ChatMessage;
use crate::messaging::router::MessageRouter;
use crate::network::connection::PeerConnection;
use crate::network::discovery::PeerAddressBook;
use crate::network::frame::Frame;
use crate::network::peer_manager::{PeerManager, PeerSession};
use crate::network::protocol::{
    DeliveryReceiptPayload, FEATURE_DISCOVERY, FEATURE_RELAY, FEATURE_STORAGE,
    FileChunkRequestPayload, FileChunkResponsePayload, FileManifestPayload,
    GroupKeyDistributionPayload, GroupMembershipUpdatePayload, GroupMessagePayload,
    MessageFetchRequestPayload, MessageType, PeerEntry, PeerListResponsePayload, PingPayload,
    RelayStoredPayload,
};
use crate::network::rate_limiter::RateLimiter;
use crate::relay::store::RelayStore;
use crate::storage::engine::StorageEngine;

pub struct Node {
    pub config: NodeConfig,
    pub identity: KeyPair,
    pub storage: StorageEngine,
    pub peer_manager: Arc<PeerManager>,
    pub address_book: Arc<Mutex<PeerAddressBook>>,
    pub contacts: Arc<Mutex<ContactBook>>,
    pub router: Arc<MessageRouter>,
    pub relay_store: Arc<Mutex<RelayStore>>,
    pub event_bus: Arc<EventBus>,
    pub rate_limiter: Arc<RateLimiter>,
    pub file_manager: Arc<FileTransferManager>,
    pub group_manager: Arc<GroupManager>,
    pub debug_mode: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    worker_threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Node {
    /// Initializes and starts a new Node instance.
    pub fn start(config: NodeConfig) -> Result<Arc<Self>, ChatError> {
        config.validate().map_err(ChatError::InvalidInput)?;

        // 1. Storage engine
        let storage = StorageEngine::new(&config.data_directory)?;

        // 2. Load or generate Identity
        let identity = match storage.load_identity()? {
            Some(kp) => kp,
            None => {
                let kp = KeyPair::generate()?;
                storage.save_identity(&kp)?;
                kp
            }
        };

        // 3. Load Contacts
        let mut contact_book = ContactBook::new();
        for c in storage.load_contacts()? {
            contact_book.add(c);
        }
        let contacts = Arc::new(Mutex::new(contact_book));

        // 4. Load Address Book
        let mut address_book = PeerAddressBook::new();
        for addr in storage.load_peers()? {
            address_book.add(addr);
        }
        let address_book = Arc::new(Mutex::new(address_book));

        // 5. Features bitmask
        let mut features = FEATURE_DISCOVERY;
        if config.relay_enabled {
            features |= FEATURE_RELAY | FEATURE_STORAGE;
        }

        // 6. Peer Manager
        let peer_manager = Arc::new(PeerManager::new(
            identity.clone(),
            config.listen_address.port(),
            features,
            config.max_peers,
        ));

        // 7. Message Router
        let router = Arc::new(MessageRouter::new(
            identity.clone(),
            peer_manager.clone(),
            contacts.clone(),
            config.message_ttl_seconds,
        ));

        // Load historical messages
        for msg in storage.load_messages()? {
            if msg.is_outgoing {
                router.add_loaded_outbox_message(msg);
            } else {
                router.add_loaded_inbox_message(msg);
            }
        }

        // 8. Relay Store
        let mut relay_store =
            RelayStore::new(config.relay_enabled, config.relay_storage_limit_bytes, 1000);
        for env in storage.load_relay()? {
            let _ = relay_store.store(env);
        }
        let relay_store = Arc::new(Mutex::new(relay_store));

        let running = Arc::new(AtomicBool::new(true));

        let event_bus = EventBus::new(500);
        let rate_limiter = Arc::new(RateLimiter::new(100.0, 50.0));
        let downloads_dir = config.data_directory.join("downloads");
        let file_manager = Arc::new(FileTransferManager::new(&downloads_dir)?);
        let group_manager = Arc::new(GroupManager::new());
        let debug_mode = Arc::new(AtomicBool::new(false));

        let node = Arc::new(Self {
            config,
            identity,
            storage,
            peer_manager,
            address_book,
            contacts,
            router,
            relay_store,
            event_bus,
            rate_limiter,
            file_manager,
            group_manager,
            debug_mode,
            running,
            worker_threads: Mutex::new(Vec::new()),
        });

        // 9. Start TCP listener thread
        node.spawn_listener_thread()?;

        // 10. Start background maintenance thread (heartbeats, reconnects, cleanup)
        node.spawn_maintenance_thread();

        Ok(node)
    }

    /// Spawns the TCP server listener thread.
    fn spawn_listener_thread(self: &Arc<Self>) -> Result<(), ChatError> {
        let listener = TcpListener::bind(self.config.listen_address)?;
        listener.set_nonblocking(true)?;

        let node_weak = Arc::downgrade(self);
        let running = self.running.clone();

        let handle = thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, remote_addr)) => {
                        if let Some(node) = node_weak.upgrade() {
                            let peer_mgr = node.peer_manager.clone();
                            let read_timeout = node.config.peer_timeout;
                            let node_clone = node.clone();

                            thread::spawn(move || {
                                if let Ok(conn) =
                                    PeerConnection::new(stream, remote_addr, read_timeout)
                                {
                                    let conn_arc = Arc::new(conn);
                                    if let Ok(session) = peer_mgr.handshake_inbound(conn_arc) {
                                        node_clone
                                            .address_book
                                            .lock()
                                            .unwrap()
                                            .record_success(&remote_addr);
                                        node_clone.run_session_loop(session);
                                    }
                                }
                            });
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => {
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        });

        self.worker_threads.lock().unwrap().push(handle);
        Ok(())
    }

    /// Spawns the periodic maintenance thread.
    fn spawn_maintenance_thread(self: &Arc<Self>) {
        let node_weak = Arc::downgrade(self);
        let running = self.running.clone();

        let handle = thread::spawn(move || {
            let mut last_ping = Instant::now();
            let mut last_cleanup = Instant::now();
            let mut last_save = Instant::now();
            let mut last_reconnect = Instant::now();

            while running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(500));
                let Some(node) = node_weak.upgrade() else {
                    break;
                };

                let now = Instant::now();

                // 1. Heartbeat Ping / Timeout check (every ping_interval)
                if now.duration_since(last_ping) >= node.config.ping_interval {
                    last_ping = now;
                    let ping = PingPayload {
                        nonce: now.elapsed().as_nanos() as u64,
                    };
                    let frame = Frame::new(MessageType::Ping as u8, 0, ping.encode());
                    node.peer_manager.broadcast(&frame, None);
                    node.peer_manager.cleanup_closed_connections();
                }

                // 2. Reconnect candidates (every 5 seconds)
                if node.config.reconnect_enabled
                    && now.duration_since(last_reconnect) >= Duration::from_secs(5)
                {
                    last_reconnect = now;
                    let candidates = node
                        .address_book
                        .lock()
                        .unwrap()
                        .get_candidates_to_connect();
                    for addr in candidates {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }
                        let _ = node.connect_peer(addr);
                    }
                }

                // 3. Relay store cleanup (every 30 seconds)
                if now.duration_since(last_cleanup) >= Duration::from_secs(30) {
                    last_cleanup = now;
                    node.relay_store.lock().unwrap().cleanup_expired();
                }

                // 4. Auto-save storage (every 60 seconds)
                if now.duration_since(last_save) >= Duration::from_secs(60) {
                    last_save = now;
                    let _ = node.save_all();
                }
            }
        });

        self.worker_threads.lock().unwrap().push(handle);
    }

    /// Connects to a remote peer address.
    pub fn connect_peer(self: &Arc<Self>, addr: SocketAddr) -> Result<(), ChatError> {
        if addr == self.config.listen_address {
            return Ok(()); // Do not connect to self
        }

        let conn =
            PeerConnection::connect(addr, self.config.connect_timeout, self.config.peer_timeout)
                .map_err(|e| {
                    self.address_book.lock().unwrap().record_failure(&addr);
                    ChatError::Network(e)
                })?;

        let conn_arc = Arc::new(conn);
        let session = self
            .peer_manager
            .handshake_outbound(conn_arc)
            .map_err(|e| {
                self.address_book.lock().unwrap().record_failure(&addr);
                ChatError::Network(e)
            })?;

        self.address_book.lock().unwrap().record_success(&addr);
        self.address_book.lock().unwrap().add(addr);

        // Request their peer list for discovery
        let req_frame = Frame::new(MessageType::PeerListRequest as u8, 0, Vec::new());
        let _ = session.connection.send(&req_frame);

        // Fetch stored offline messages from this peer
        self.fetch_messages_from_peer(&session);

        // Start session reader loop in separate thread
        let node_clone = self.clone();
        thread::spawn(move || {
            node_clone.run_session_loop(session);
        });

        Ok(())
    }

    /// Reader loop for an established peer session.
    fn run_session_loop(&self, session: Arc<PeerSession>) {
        while self.running.load(Ordering::Relaxed) && !session.connection.is_closed() {
            match session.connection.receive() {
                Ok(frame) => {
                    *session.last_seen.write().unwrap() = Instant::now();
                    self.handle_incoming_frame(&session, frame);
                }
                Err(_) => {
                    session.connection.close();
                    break;
                }
            }
        }
        self.peer_manager.disconnect_peer(&session.peer_id);
    }

    /// Dispatches and processes an incoming frame from a peer session.
    fn handle_incoming_frame(&self, session: &Arc<PeerSession>, frame: Frame) {
        if !self
            .rate_limiter
            .check_allowed(&session.connection.remote_addr, 1.0)
        {
            return;
        }

        let msg_type = match MessageType::try_from(frame.message_type) {
            Ok(t) => t,
            Err(_) => return, // Unknown frame ignored safely
        };

        match msg_type {
            MessageType::Ping => {
                if let Ok(ping) = PingPayload::decode(&frame.payload) {
                    let pong = Frame::new(MessageType::Pong as u8, 0, ping.encode());
                    let _ = session.connection.send(&pong);
                }
            }
            MessageType::Pong => {
                // Heartbeat confirmed
            }
            MessageType::PeerListRequest => {
                let addrs = self.peer_manager.get_peer_addresses();
                let peers = addrs
                    .into_iter()
                    .map(|(id, addr)| PeerEntry { peer_id: id, addr })
                    .collect();
                let resp = PeerListResponsePayload { peers };
                let resp_frame = Frame::new(MessageType::PeerListResponse as u8, 0, resp.encode());
                let _ = session.connection.send(&resp_frame);
            }
            MessageType::PeerListResponse => {
                if let Ok(resp) = PeerListResponsePayload::decode(&frame.payload) {
                    let mut book = self.address_book.lock().unwrap();
                    for entry in resp.peers {
                        if entry.peer_id != *self.identity.peer_id() {
                            book.add(entry.addr);
                        }
                    }
                }
            }
            MessageType::Message => {
                if let Ok(env) = MessageEnvelope::decode(&frame.payload) {
                    let _ = self.router.process_incoming_envelope(env);
                }
            }
            MessageType::RelayStore => {
                if let Ok(env) = MessageEnvelope::decode(&frame.payload) {
                    let mut store = self.relay_store.lock().unwrap();
                    let status = if store.is_enabled() {
                        match store.store(env.clone()) {
                            Ok(()) => 0, // Stored
                            Err(_) => 1, // Quota exceeded
                        }
                    } else {
                        2 // Rejected
                    };

                    let ack = RelayStoredPayload {
                        message_id: env.message_id,
                        status,
                    };
                    let ack_frame = Frame::new(MessageType::RelayStored as u8, 0, ack.encode());
                    let _ = session.connection.send(&ack_frame);
                }
            }
            MessageType::RelayStored => {
                // Relay storage acknowledgment received
            }
            MessageType::MessageFetchRequest => {
                if let Ok(req) = MessageFetchRequestPayload::decode(&frame.payload) {
                    let store = self.relay_store.lock().unwrap();
                    if store.is_enabled() {
                        let envs =
                            store.fetch_for_recipient(&req.recipient_peer_id, req.since_timestamp);
                        for env in envs {
                            let resp_frame = Frame::new(
                                MessageType::MessageFetchResponse as u8,
                                0,
                                env.encode(),
                            );
                            let _ = session.connection.send(&resp_frame);
                        }
                    }
                }
            }
            MessageType::MessageFetchResponse => {
                if let Ok(env) = MessageEnvelope::decode(&frame.payload) {
                    let _ = self.router.process_incoming_envelope(env);
                }
            }
            MessageType::DeliveryReceipt => {
                if let Ok(receipt) = DeliveryReceiptPayload::decode(&frame.payload) {
                    self.router.handle_delivery_receipt(receipt);
                }
            }
            MessageType::FileManifest => {
                if let Ok(manifest) = FileManifestPayload::decode(&frame.payload) {
                    let contacts = self.contacts.lock().unwrap();
                    if self
                        .file_manager
                        .handle_inbound_manifest(
                            manifest.clone(),
                            &self.identity,
                            &contacts,
                            &self.event_bus,
                        )
                        .is_ok()
                        && let Some(next_chunk) =
                            self.file_manager.get_next_missing_chunk(&manifest.file_id)
                    {
                        let req = FileChunkRequestPayload {
                            file_id: manifest.file_id,
                            chunk_index: next_chunk,
                            requester: *self.identity.peer_id(),
                        };
                        let req_frame =
                            Frame::new(MessageType::FileChunkRequest as u8, 0, req.encode());
                        let _ = session.connection.send(&req_frame);
                    }
                }
            }
            MessageType::FileChunkRequest => {
                if let Ok(req) = FileChunkRequestPayload::decode(&frame.payload)
                    && let Ok(Some(resp)) = self.file_manager.handle_chunk_request(&req)
                {
                    let resp_frame =
                        Frame::new(MessageType::FileChunkResponse as u8, 0, resp.encode());
                    let _ = session.connection.send(&resp_frame);
                }
            }
            MessageType::FileChunkResponse => {
                if let Ok(resp) = FileChunkResponsePayload::decode(&frame.payload)
                    && let Ok(completed) = self
                        .file_manager
                        .handle_chunk_response(&resp, &self.event_bus)
                    && !completed
                    && let Some(next_chunk) =
                        self.file_manager.get_next_missing_chunk(&resp.file_id)
                {
                    let req = FileChunkRequestPayload {
                        file_id: resp.file_id,
                        chunk_index: next_chunk,
                        requester: *self.identity.peer_id(),
                    };
                    let req_frame =
                        Frame::new(MessageType::FileChunkRequest as u8, 0, req.encode());
                    let _ = session.connection.send(&req_frame);
                }
            }
            MessageType::GroupKeyDistribution => {
                if let Ok(dist) = GroupKeyDistributionPayload::decode(&frame.payload) {
                    let _ = self.group_manager.handle_key_distribution(
                        &dist,
                        &self.identity,
                        &self.event_bus,
                    );
                }
            }
            MessageType::GroupMessage => {
                if let Ok(msg) = GroupMessagePayload::decode(&frame.payload)
                    && self
                        .group_manager
                        .decrypt_group_message(&msg, &self.event_bus)
                        .is_ok()
                {
                    self.peer_manager.broadcast(&frame, Some(&session.peer_id));
                }
            }
            MessageType::GroupMembershipUpdate => {
                if let Ok(upd) = GroupMembershipUpdatePayload::decode(&frame.payload) {
                    let contacts = self.contacts.lock().unwrap();
                    if let Some(admin_contact) = contacts.get_by_peer_id(&upd.admin) {
                        let _ = self.group_manager.handle_membership_update(
                            &upd,
                            &admin_contact.public_key,
                            &self.event_bus,
                        );
                    }
                }
            }
            MessageType::Goodbye => {
                session.connection.close();
            }
            _ => {}
        }
    }

    /// Requests any stored offline messages for this node from a peer.
    pub fn fetch_messages_from_peer(&self, session: &Arc<PeerSession>) {
        let req = MessageFetchRequestPayload {
            recipient_peer_id: *self.identity.peer_id(),
            since_timestamp: 0,
        };
        let frame = Frame::new(MessageType::MessageFetchRequest as u8, 0, req.encode());
        let _ = session.connection.send(&frame);
    }

    /// Fetches offline messages from all connected peers.
    pub fn fetch_all_relay_messages(&self) {
        let sessions = self.peer_manager.get_all_sessions();
        for s in sessions {
            self.fetch_messages_from_peer(&s);
        }
    }

    // --- High Level User Actions ---

    pub fn send_chat(&self, contact_name: &str, text: &str) -> Result<ChatMessage, ChatError> {
        self.router.send_message(contact_name, text)
    }

    pub fn add_contact(&self, name: &str, pubkey_hex: &str) -> Result<Contact, String> {
        let mut book = self.contacts.lock().unwrap();
        book.add_from_hex(name, pubkey_hex)
    }

    pub fn list_contacts(&self) -> Vec<Contact> {
        let book = self.contacts.lock().unwrap();
        book.list().into_iter().cloned().collect()
    }

    pub fn get_inbox(&self) -> Vec<ChatMessage> {
        self.router.get_inbox()
    }

    pub fn get_outbox(&self) -> Vec<ChatMessage> {
        self.router.get_outbox()
    }

    pub fn get_connected_peers(&self) -> Vec<PeerId> {
        self.peer_manager.get_connected_peers()
    }

    pub fn get_known_peers(&self) -> Vec<SocketAddr> {
        self.address_book.lock().unwrap().get_all()
    }

    pub fn set_relay(&self, enabled: bool) {
        self.relay_store.lock().unwrap().set_enabled(enabled);
    }

    pub fn is_relay_enabled(&self) -> bool {
        self.relay_store.lock().unwrap().is_enabled()
    }

    pub fn get_relay_stats(&self) -> (usize, usize) {
        self.relay_store.lock().unwrap().get_usage()
    }

    // --- File Transfer Actions ---

    pub fn send_file(&self, contact_name: &str, file_path: &Path) -> Result<[u8; 32], ChatError> {
        let contact = {
            let book = self.contacts.lock().unwrap();
            book.get_by_name(contact_name)
                .cloned()
                .ok_or_else(|| ChatError::ContactNotFound(contact_name.to_string()))?
        };

        let manifest = self.file_manager.prepare_outbound(
            file_path,
            contact.peer_id,
            &contact.public_key,
            &self.identity,
        )?;

        let frame = Frame::new(MessageType::FileManifest as u8, 0, manifest.encode());
        if let Some(session) = self.peer_manager.get_session(&contact.peer_id) {
            session
                .connection
                .send(&frame)
                .map_err(ChatError::Network)?;
        } else {
            self.peer_manager.broadcast(&frame, None);
        }

        Ok(manifest.file_id)
    }

    pub fn get_file_transfers(&self) -> Vec<String> {
        self.file_manager.get_status_summary()
    }

    pub fn cancel_file_transfer(&self, file_id: &[u8; 32]) -> bool {
        self.file_manager.cancel_transfer(file_id)
    }

    // --- Group Chat Actions ---

    pub fn create_group(&self, name: &str) -> Result<GroupInfo, ChatError> {
        self.group_manager.create_group(name, &self.identity)
    }

    pub fn add_group_member(
        &self,
        group_id: &[u8; 32],
        contact_name: &str,
    ) -> Result<(), ChatError> {
        let contact = {
            let book = self.contacts.lock().unwrap();
            book.get_by_name(contact_name)
                .cloned()
                .ok_or_else(|| ChatError::ContactNotFound(contact_name.to_string()))?
        };

        let member_keys = {
            let book = self.contacts.lock().unwrap();
            let mut keys = Vec::new();
            keys.push((contact.peer_id, contact.public_key));
            if let Some(grp) = self.group_manager.get_group(group_id) {
                for m in &grp.members {
                    if *m != *self.identity.peer_id()
                        && *m != contact.peer_id
                        && let Some(c) = book.get_by_peer_id(m)
                    {
                        keys.push((*m, c.public_key));
                    }
                }
            }
            keys
        };

        let (key_dists, membership_upd) = self.group_manager.add_member(
            group_id,
            contact.peer_id,
            &member_keys,
            &self.identity,
        )?;

        for dist in key_dists {
            let key_frame = Frame::new(MessageType::GroupKeyDistribution as u8, 0, dist.encode());
            if let Some(session) = self.peer_manager.get_session(&dist.recipient) {
                let _ = session.connection.send(&key_frame);
            }
        }

        let upd_frame = Frame::new(
            MessageType::GroupMembershipUpdate as u8,
            0,
            membership_upd.encode(),
        );
        self.peer_manager.broadcast(&upd_frame, None);

        Ok(())
    }

    pub fn remove_group_member(
        &self,
        group_id: &[u8; 32],
        contact_name: &str,
    ) -> Result<(), ChatError> {
        let contact = {
            let book = self.contacts.lock().unwrap();
            book.get_by_name(contact_name)
                .cloned()
                .ok_or_else(|| ChatError::ContactNotFound(contact_name.to_string()))?
        };

        let member_keys = {
            let book = self.contacts.lock().unwrap();
            let mut keys = Vec::new();
            if let Some(grp) = self.group_manager.get_group(group_id) {
                for m in &grp.members {
                    if *m != *self.identity.peer_id()
                        && *m != contact.peer_id
                        && let Some(c) = book.get_by_peer_id(m)
                    {
                        keys.push((*m, c.public_key));
                    }
                }
            }
            keys
        };

        let (key_dists, upd) = self.group_manager.remove_member(
            group_id,
            &contact.peer_id,
            &member_keys,
            &self.identity,
        )?;

        for dist in key_dists {
            let key_frame = Frame::new(MessageType::GroupKeyDistribution as u8, 0, dist.encode());
            if let Some(session) = self.peer_manager.get_session(&dist.recipient) {
                let _ = session.connection.send(&key_frame);
            }
        }

        let upd_frame = Frame::new(MessageType::GroupMembershipUpdate as u8, 0, upd.encode());
        self.peer_manager.broadcast(&upd_frame, None);

        Ok(())
    }

    pub fn send_group_chat(&self, group_id: &[u8; 32], text: &str) -> Result<(), ChatError> {
        let payload = self
            .group_manager
            .encrypt_group_message(group_id, &self.identity, text)?;

        let frame = Frame::new(MessageType::GroupMessage as u8, 0, payload.encode());
        self.peer_manager.broadcast(&frame, None);
        Ok(())
    }

    pub fn list_groups(&self) -> Vec<GroupInfo> {
        self.group_manager.list_groups()
    }

    // --- Diagnostics & Network Stats ---

    pub fn set_debug(&self, on: bool) {
        self.debug_mode.store(on, Ordering::SeqCst);
    }

    pub fn is_debug(&self) -> bool {
        self.debug_mode.load(Ordering::Relaxed)
    }

    pub fn get_network_stats(&self) -> (usize, usize, usize) {
        let connected = self.peer_manager.get_connected_peers().len();
        let known = self.address_book.lock().unwrap().get_all().len();
        let (_, relay_count) = self.get_relay_stats();
        (connected, known, relay_count)
    }

    /// Saves all runtime state to persistent local storage.
    pub fn save_all(&self) -> Result<(), ChatError> {
        // Contacts
        let contacts: Vec<Contact> = self
            .contacts
            .lock()
            .unwrap()
            .list()
            .into_iter()
            .cloned()
            .collect();
        self.storage.save_contacts(&contacts)?;

        // Peers
        let peers = self.address_book.lock().unwrap().get_all();
        self.storage.save_peers(&peers)?;

        // Messages
        let mut all_msgs = self.router.get_inbox();
        all_msgs.extend(self.router.get_outbox());
        self.storage.save_messages(&all_msgs)?;

        // Relay envelopes
        let relay_envs = self.relay_store.lock().unwrap().get_all_envelopes();
        self.storage.save_relay(&relay_envs)?;

        Ok(())
    }

    /// Graceful shutdown: notifies peers, saves data, closes sockets.
    pub fn stop(&self) {
        if !self.running.swap(false, Ordering::SeqCst) {
            return; // Already stopped
        }

        // Send GOODBYE to all connected peers
        let goodbye_frame = Frame::new(MessageType::Goodbye as u8, 0, vec![0]);
        self.peer_manager.broadcast(&goodbye_frame, None);

        // Disconnect all sessions
        for peer in self.peer_manager.get_connected_peers() {
            self.peer_manager.disconnect_peer(&peer);
        }

        // Persist all state
        let _ = self.save_all();
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.stop();
    }
}
