// src/ffi.rs
//! C-ABI Foreign Function Interface (FFI) for P2P Chat Core.
//! Allows native Windows (C#/C++/Win32) applications to seamlessly interact with the P2P node.

#![allow(unsafe_op_in_unsafe_fn)]

use std::collections::VecDeque;
use std::ffi::CStr;
use std::net::SocketAddr;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::NodeConfig;
use crate::events::NodeEvent;
use crate::node::runtime::Node;
use crate::types::{from_hex, to_hex};

/// Opaque handle representing an active P2P Node runtime.
pub struct P2pNodeHandle {
    pub node: Arc<Node>,
    pub events: Arc<Mutex<VecDeque<String>>>,
}

fn escape_json(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    for c in input.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

unsafe fn copy_to_c_buffer(src: &str, out_buf: *mut c_char, max_len: usize) -> c_int {
    if out_buf.is_null() || max_len == 0 {
        return -1;
    }
    let bytes = src.as_bytes();
    let copy_len = bytes.len().min(max_len - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, out_buf, copy_len);
    *out_buf.add(copy_len) = 0;
    copy_len as c_int
}

/// Initializes and starts a P2P node.
/// Returns a non-null pointer on success, or null on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_start(
    data_dir: *const c_char,
    listen_addr: *const c_char,
    is_relay: c_int,
) -> *mut P2pNodeHandle {
    if data_dir.is_null() || listen_addr.is_null() {
        return std::ptr::null_mut();
    }

    let data_dir_str = match CStr::from_ptr(data_dir).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let listen_addr_str = match CStr::from_ptr(listen_addr).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let listen_sock: SocketAddr = match listen_addr_str.parse() {
        Ok(a) => a,
        Err(_) => return std::ptr::null_mut(),
    };

    let mut config = NodeConfig::default();
    config.data_directory = PathBuf::from(data_dir_str);
    config.listen_address = listen_sock;
    config.relay_enabled = is_relay != 0;

    let node = match Node::start(config) {
        Ok(n) => n,
        Err(_) => return std::ptr::null_mut(),
    };

    let events: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    let events_clone = events.clone();

    node.event_bus.subscribe(move |ev| {
        let json = match ev {
            NodeEvent::PeerConnected { peer_id, addr } => {
                format!(
                    r#"{{"event":"peer_connected","peer_id":"{}","address":"{}"}}"#,
                    peer_id.to_hex(),
                    addr
                )
            }
            NodeEvent::PeerDisconnected { peer_id } => {
                format!(
                    r#"{{"event":"peer_disconnected","peer_id":"{}"}}"#,
                    peer_id.to_hex()
                )
            }
            NodeEvent::DirectMessageReceived { message } => {
                format!(
                    r#"{{"event":"message_received","id":"{}","sender":"{}","sender_peer_id":"{}","content":"{}","timestamp":{}}}"#,
                    message.id_hex(),
                    escape_json(&message.sender_name),
                    message.sender_peer_id.to_hex(),
                    escape_json(&message.content),
                    message.timestamp
                )
            }
            NodeEvent::DeliveryReceiptReceived { message_id, recipient, timestamp } => {
                format!(
                    r#"{{"event":"delivery_receipt","message_id":"{}","recipient":"{}","timestamp":{}}}"#,
                    to_hex(message_id),
                    recipient.to_hex(),
                    timestamp
                )
            }
            NodeEvent::FileOfferReceived { file_id, sender, file_name, file_size } => {
                format!(
                    r#"{{"event":"file_offer","file_id":"{}","sender":"{}","file_name":"{}","file_size":{}}}"#,
                    to_hex(file_id),
                    sender.to_hex(),
                    escape_json(file_name),
                    file_size
                )
            }
            NodeEvent::FileChunkProgress { file_id, chunks_completed, total_chunks, bytes_transferred } => {
                format!(
                    r#"{{"event":"file_progress","file_id":"{}","completed":{},"total":{},"bytes":{}}}"#,
                    to_hex(file_id),
                    chunks_completed,
                    total_chunks,
                    bytes_transferred
                )
            }
            NodeEvent::FileTransferCompleted { file_id, file_name, path } => {
                format!(
                    r#"{{"event":"file_completed","file_id":"{}","file_name":"{}","path":"{}"}}"#,
                    to_hex(file_id),
                    escape_json(file_name),
                    escape_json(path)
                )
            }
            NodeEvent::FileTransferFailed { file_id, reason } => {
                format!(
                    r#"{{"event":"file_failed","file_id":"{}","reason":"{}"}}"#,
                    to_hex(file_id),
                    escape_json(reason)
                )
            }
            NodeEvent::GroupMessageReceived { group_id, group_name, sender, text, timestamp } => {
                format!(
                    r#"{{"event":"group_message","group_id":"{}","group_name":"{}","sender":"{}","content":"{}","timestamp":{}}}"#,
                    to_hex(group_id),
                    escape_json(group_name),
                    sender.to_hex(),
                    escape_json(text),
                    timestamp
                )
            }
            NodeEvent::GroupMembershipChanged { group_id, group_name, member, added } => {
                format!(
                    r#"{{"event":"group_membership","group_id":"{}","group_name":"{}","member":"{}","added":{}}}"#,
                    to_hex(group_id),
                    escape_json(group_name),
                    member.to_hex(),
                    added
                )
            }
            NodeEvent::Notice(msg) => {
                format!(r#"{{"event":"notice","message":"{}"}}"#, escape_json(msg))
            }
        };

        let mut queue = events_clone.lock().unwrap();
        if queue.len() >= 1000 {
            queue.pop_front();
        }
        queue.push_back(json);
    });

    let handle = Box::new(P2pNodeHandle { node, events });
    Box::into_raw(handle)
}

/// Stops the node and frees all resources.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_stop(handle: *mut P2pNodeHandle) {
    if !handle.is_null() {
        let boxed = Box::from_raw(handle);
        boxed.node.stop();
        let _ = boxed.node.save_all();
    }
}

/// Retrieves local Peer ID and Public Key in hex format.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_get_identity(
    handle: *mut P2pNodeHandle,
    out_peer_id: *mut c_char,
    max_id_len: usize,
    out_pubkey: *mut c_char,
    max_key_len: usize,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let h = &*handle;
    let peer_id_hex = h.node.identity.peer_id().to_hex();
    let pubkey_hex = to_hex(h.node.identity.public_key());

    let r1 = copy_to_c_buffer(&peer_id_hex, out_peer_id, max_id_len);
    let r2 = copy_to_c_buffer(&pubkey_hex, out_pubkey, max_key_len);
    if r1 >= 0 && r2 >= 0 { 0 } else { -1 }
}

/// Connects to a remote peer address (e.g. "127.0.0.1:7002").
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_connect_peer(
    handle: *mut P2pNodeHandle,
    addr_str: *const c_char,
) -> c_int {
    if handle.is_null() || addr_str.is_null() {
        return -1;
    }
    let h = &*handle;
    let addr_s = match CStr::from_ptr(addr_str).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let sock: SocketAddr = match addr_s.parse() {
        Ok(a) => a,
        Err(_) => return -2,
    };

    match h.node.connect_peer(sock) {
        Ok(_) => 0,
        Err(_) => -3,
    }
}

/// Adds a contact to the local contact book.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_add_contact(
    handle: *mut P2pNodeHandle,
    name: *const c_char,
    pubkey_hex: *const c_char,
) -> c_int {
    if handle.is_null() || name.is_null() || pubkey_hex.is_null() {
        return -1;
    }
    let h = &*handle;
    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let key_str = match CStr::from_ptr(pubkey_hex).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    match h.node.add_contact(name_str, key_str) {
        Ok(_) => {
            let _ = h.node.save_all();
            0
        }
        Err(_) => -2,
    }
}

/// Lists all contacts as a JSON array string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_list_contacts_json(
    handle: *mut P2pNodeHandle,
    out_buf: *mut c_char,
    max_len: usize,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let h = &*handle;
    let contacts = h.node.list_contacts();
    let mut items = Vec::new();
    for c in contacts {
        items.push(format!(
            r#"{{"name":"{}","peer_id":"{}","public_key":"{}","fingerprint":"{}"}}"#,
            escape_json(&c.name),
            c.peer_id.to_hex(),
            to_hex(&c.public_key),
            c.fingerprint()
        ));
    }
    let json = format!("[{}]", items.join(","));
    copy_to_c_buffer(&json, out_buf, max_len)
}

/// Sends a direct E2EE chat message to a named contact.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_send_chat(
    handle: *mut P2pNodeHandle,
    contact_name: *const c_char,
    text: *const c_char,
) -> c_int {
    if handle.is_null() || contact_name.is_null() || text.is_null() {
        return -1;
    }
    let h = &*handle;
    let name_str = match CStr::from_ptr(contact_name).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    match h.node.send_chat(name_str, text_str) {
        Ok(_) => {
            let _ = h.node.save_all();
            0
        }
        Err(_) => -2,
    }
}

/// Retrieves all messages (inbox & outbox) as a combined JSON array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_get_messages_json(
    handle: *mut P2pNodeHandle,
    out_buf: *mut c_char,
    max_len: usize,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let h = &*handle;
    let mut all_msgs = h.node.get_inbox();
    all_msgs.extend(h.node.get_outbox());
    all_msgs.sort_by_key(|m| m.timestamp);

    let mut items = Vec::new();
    for m in all_msgs {
        items.push(format!(
            r#"{{"id":"{}","sender":"{}","recipient":"{}","content":"{}","timestamp":{},"is_outgoing":{},"status":"{}"}}"#,
            m.id_hex(),
            escape_json(&m.sender_name),
            escape_json(&m.recipient_name),
            escape_json(&m.content),
            m.timestamp,
            m.is_outgoing,
            escape_json(&m.status.to_string())
        ));
    }
    let json = format!("[{}]", items.join(","));
    copy_to_c_buffer(&json, out_buf, max_len)
}

/// Sends a file chunked to a contact.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_send_file(
    handle: *mut P2pNodeHandle,
    contact_name: *const c_char,
    file_path: *const c_char,
) -> c_int {
    if handle.is_null() || contact_name.is_null() || file_path.is_null() {
        return -1;
    }
    let h = &*handle;
    let name_str = match CStr::from_ptr(contact_name).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let path_str = match CStr::from_ptr(file_path).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    match h.node.send_file(name_str, std::path::Path::new(path_str)) {
        Ok(_) => 0,
        Err(_) => -2,
    }
}

/// Gets status summary of all active file transfers as JSON array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_get_file_transfers_json(
    handle: *mut P2pNodeHandle,
    out_buf: *mut c_char,
    max_len: usize,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let h = &*handle;
    let transfers = h.node.get_file_transfers();
    let mut items = Vec::new();
    for t in transfers {
        items.push(format!(r#""{}""#, escape_json(&t)));
    }
    let json = format!("[{}]", items.join(","));
    copy_to_c_buffer(&json, out_buf, max_len)
}

/// Creates a new group.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_create_group(
    handle: *mut P2pNodeHandle,
    group_name: *const c_char,
    out_group_id_hex: *mut c_char,
    max_id_len: usize,
) -> c_int {
    if handle.is_null() || group_name.is_null() {
        return -1;
    }
    let h = &*handle;
    let name_str = match CStr::from_ptr(group_name).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    match h.node.create_group(name_str) {
        Ok(grp) => {
            let gid_hex = to_hex(&grp.group_id);
            copy_to_c_buffer(&gid_hex, out_group_id_hex, max_id_len);
            0
        }
        Err(_) => -2,
    }
}

/// Adds a member to an existing group.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_add_group_member(
    handle: *mut P2pNodeHandle,
    group_id_hex: *const c_char,
    contact_name: *const c_char,
) -> c_int {
    if handle.is_null() || group_id_hex.is_null() || contact_name.is_null() {
        return -1;
    }
    let h = &*handle;
    let gid_str = match CStr::from_ptr(group_id_hex).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let name_str = match CStr::from_ptr(contact_name).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let gid_bytes = match from_hex(gid_str) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => return -2,
    };

    match h.node.add_group_member(&gid_bytes, name_str) {
        Ok(_) => 0,
        Err(_) => -3,
    }
}

/// Sends an encrypted group message.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_send_group_chat(
    handle: *mut P2pNodeHandle,
    group_id_hex: *const c_char,
    text: *const c_char,
) -> c_int {
    if handle.is_null() || group_id_hex.is_null() || text.is_null() {
        return -1;
    }
    let h = &*handle;
    let gid_str = match CStr::from_ptr(group_id_hex).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let gid_bytes = match from_hex(gid_str) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => return -2,
    };

    match h.node.send_group_chat(&gid_bytes, text_str) {
        Ok(_) => 0,
        Err(_) => -3,
    }
}

/// Lists all groups as a JSON array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_list_groups_json(
    handle: *mut P2pNodeHandle,
    out_buf: *mut c_char,
    max_len: usize,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let h = &*handle;
    let groups = h.node.list_groups();
    let mut items = Vec::new();
    for g in groups {
        let members: Vec<String> = g.members.iter().map(|m| format!(r#""{}""#, m.to_hex())).collect();
        items.push(format!(
            r#"{{"group_id":"{}","name":"{}","creator":"{}","epoch":{},"members":[{}],"created_at":{}}}"#,
            to_hex(&g.group_id),
            escape_json(&g.name),
            g.creator.to_hex(),
            g.epoch,
            members.join(","),
            g.created_at
        ));
    }
    let json = format!("[{}]", items.join(","));
    copy_to_c_buffer(&json, out_buf, max_len)
}

/// Retrieves connected peers and network stats as a JSON object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_get_network_stats_json(
    handle: *mut P2pNodeHandle,
    out_buf: *mut c_char,
    max_len: usize,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let h = &*handle;
    let connected = h.node.get_connected_peers();
    let known = h.node.get_known_peers();
    let (relay_bytes, relay_count) = h.node.get_relay_stats();
    let relay_on = h.node.is_relay_enabled();

    let mut peer_items = Vec::new();
    for p in connected {
        peer_items.push(format!(r#""{}""#, p.to_hex()));
    }

    let mut known_items = Vec::new();
    for k in known {
        known_items.push(format!(r#""{}""#, k));
    }

    let json = format!(
        r#"{{"connected_peers":[{}],"known_addresses":[{}],"relay_enabled":{},"relay_stored_bytes":{},"relay_stored_count":{}}}"#,
        peer_items.join(","),
        known_items.join(","),
        relay_on,
        relay_bytes,
        relay_count
    );

    copy_to_c_buffer(&json, out_buf, max_len)
}

/// Polls the next event from the event queue.
/// Returns 1 if an event was copied, 0 if queue is empty, or negative on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn p2p_node_poll_event(
    handle: *mut P2pNodeHandle,
    out_buf: *mut c_char,
    max_len: usize,
) -> c_int {
    if handle.is_null() {
        return -1;
    }
    let h = &*handle;
    let mut q = h.events.lock().unwrap();
    if let Some(ev) = q.pop_front() {
        copy_to_c_buffer(&ev, out_buf, max_len)
    } else {
        0
    }
}
