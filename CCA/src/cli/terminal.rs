// src/cli/terminal.rs
//! Interactive REPL terminal user interface for operating the P2P chat node.

use crate::events::NodeEvent;
use crate::node::Node;
use crate::types::{from_hex, to_hex};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Arc;

pub fn run_interactive(node: Arc<Node>) {
    println!("============================================================");
    println!("       P2P CHAT CORE - Decentralized Terminal MVP           ");
    println!("============================================================");
    println!("Peer ID:     {}", node.identity.peer_id());
    println!("Public Key:  {}", node.identity.public_key_hex());
    println!("Listening:   {}", node.config.listen_address);
    println!(
        "Relay Mode:  {}",
        if node.is_relay_enabled() {
            "ENABLED"
        } else {
            "DISABLED"
        }
    );
    println!("Type /help for available commands.");
    println!("------------------------------------------------------------");

    // Subscribe to internal Event Bus for real-time notifications
    node.event_bus.subscribe(|event| match event {
        NodeEvent::DirectMessageReceived { message } => {
            println!(
                "\n[Incoming DM] From {}: {}",
                message.sender_name, message.content
            );
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::DeliveryReceiptReceived {
            message_id,
            recipient,
            ..
        } => {
            let id_hex = to_hex(message_id);
            println!(
                "\n[Delivery Receipt] Message {} received by {}",
                &id_hex[..8],
                recipient
            );
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::FileOfferReceived {
            file_id,
            sender,
            file_name,
            file_size,
        } => {
            let id_hex = to_hex(file_id);
            println!(
                "\n[File Offer] From {}: '{}' ({} bytes, ID: {})",
                sender, file_name, file_size, id_hex
            );
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::FileChunkProgress {
            file_id,
            chunks_completed,
            total_chunks,
            bytes_transferred,
        } => {
            let id_hex = to_hex(file_id);
            println!(
                "\n[File Progress] ID {}: {}/{} chunks ({} bytes)",
                &id_hex[..8],
                chunks_completed,
                total_chunks,
                bytes_transferred
            );
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::FileTransferCompleted {
            file_id,
            file_name,
            path,
        } => {
            let id_hex = to_hex(file_id);
            println!(
                "\n[File Completed] '{}' (ID: {}) saved to {}",
                file_name,
                &id_hex[..8],
                path
            );
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::FileTransferFailed { file_id, reason } => {
            let id_hex = to_hex(file_id);
            println!("\n[File Failed] ID {}: {}", &id_hex[..8], reason);
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::GroupMessageReceived {
            group_name,
            sender,
            text,
            ..
        } => {
            println!("\n[Group: {}] <{}> {}", group_name, sender, text);
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::GroupMembershipChanged {
            group_name,
            member,
            added,
            ..
        } => {
            let action = if *added { "joined" } else { "left" };
            println!("\n[Group: {}] Member {} {}", group_name, member, action);
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::PeerConnected { peer_id, addr } => {
            println!("\n[Network] Peer connected: {} ({})", peer_id, addr);
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::PeerDisconnected { peer_id } => {
            println!("\n[Network] Peer disconnected: {}", peer_id);
            print!("> ");
            let _ = io::stdout().flush();
        }
        NodeEvent::Notice(msg) => {
            println!("\n[Notice] {}", msg);
            print!("> ");
            let _ = io::stdout().flush();
        }
    });

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    print!("> ");
    let _ = stdout.flush();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l.trim().to_string(),
            Err(_) => break,
        };

        if line.is_empty() {
            print!("> ");
            let _ = stdout.flush();
            continue;
        }

        if line.starts_with('/') {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let cmd = parts[0];

            match cmd {
                "/help" => print_help(),
                "/id" => {
                    println!("Your Decentralized Identity:");
                    println!("  Peer ID:     {}", node.identity.peer_id());
                    println!("  Public Key:  {}", node.identity.public_key_hex());
                }
                "/status" => {
                    let peers = node.get_connected_peers();
                    let (relay_bytes, relay_count) = node.get_relay_stats();
                    println!("Node Status:");
                    println!("  Listen Address:   {}", node.config.listen_address);
                    println!("  Connected Peers:  {}", peers.len());
                    println!(
                        "  Relay Mode:       {}",
                        if node.is_relay_enabled() {
                            "ACTIVE"
                        } else {
                            "INACTIVE"
                        }
                    );
                    println!("  Relay Storage:    {relay_count} messages ({relay_bytes} bytes)");
                }
                "/peers" => {
                    let peers = node.get_connected_peers();
                    if peers.is_empty() {
                        println!("No peers currently connected.");
                    } else {
                        println!("Connected Peers ({}):", peers.len());
                        for (idx, p) in peers.iter().enumerate() {
                            println!("  [{}] Peer ID: {}", idx + 1, p);
                        }
                    }
                }
                "/connect" => {
                    if parts.len() < 2 {
                        println!("Usage: /connect <IP:PORT>");
                    } else {
                        match parts[1].parse() {
                            Ok(addr) => match node.connect_peer(addr) {
                                Ok(()) => println!("Successfully connected to {addr}"),
                                Err(e) => println!("Connection failed: {e}"),
                            },
                            Err(e) => println!("Invalid address format: {e}"),
                        }
                    }
                }
                "/routes" => {
                    let connected = node.get_connected_peers();
                    let known = node.get_known_peers();
                    println!("Routing & Peer Table:");
                    println!("  Active Connections ({}):", connected.len());
                    for p in &connected {
                        println!("    * {p}");
                    }
                    println!("  Known Addresses ({}):", known.len());
                    for addr in &known {
                        println!("    * {addr}");
                    }
                }
                "/network-stats" => {
                    let (connected, known, relay_stored) = node.get_network_stats();
                    let (relay_bytes, _) = node.get_relay_stats();
                    println!("Network Statistics:");
                    println!("  Connected Peers:  {}", connected);
                    println!("  Known Peer Addrs: {}", known);
                    println!(
                        "  Relay Messages:   {} ({} bytes)",
                        relay_stored, relay_bytes
                    );
                    println!("  Rate Limiting:    Active (Token Bucket per remote socket)");
                    println!(
                        "  Debug Logging:    {}",
                        if node.is_debug() { "ON" } else { "OFF" }
                    );
                }
                "/debug" => {
                    if parts.len() < 2 {
                        println!("Usage: /debug on | /debug off");
                    } else {
                        match parts[1] {
                            "on" => {
                                node.set_debug(true);
                                println!("Debug mode ENABLED.");
                            }
                            "off" => {
                                node.set_debug(false);
                                println!("Debug mode DISABLED.");
                            }
                            _ => println!("Usage: /debug on | /debug off"),
                        }
                    }
                }
                "/add" => {
                    if parts.len() < 3 {
                        println!("Usage: /add <NAME> <PUBLIC_KEY_HEX>");
                    } else {
                        let name = parts[1];
                        let pubkey_hex = parts[2];
                        match node.add_contact(name, pubkey_hex) {
                            Ok(c) => {
                                println!("Added contact '{}' (Peer ID: {})", c.name, c.peer_id)
                            }
                            Err(e) => println!("Failed to add contact: {e}"),
                        }
                    }
                }
                "/contacts" => {
                    let list = node.list_contacts();
                    if list.is_empty() {
                        println!("No contacts added. Use /add <NAME> <PUBKEY> to add a friend.");
                    } else {
                        println!("Contacts ({}):", list.len());
                        for c in list {
                            println!(
                                "  * {} | Fingerprint: {} | Verified: {}",
                                c.name,
                                c.fingerprint(),
                                c.verified
                            );
                            println!("    Pubkey:  {}", c.public_key_hex());
                            println!("    Peer ID: {}", c.peer_id);
                        }
                    }
                }
                "/send" => {
                    if parts.len() < 3 {
                        println!("Usage: /send <CONTACT_NAME> <MESSAGE...>");
                    } else {
                        let contact_name = parts[1];
                        let msg_text = parts[2..].join(" ");
                        match node.send_chat(contact_name, &msg_text) {
                            Ok(sent) => {
                                println!("Message created: {}", sent.short_id());
                                println!("Status: {}", sent.status);
                            }
                            Err(e) => println!("Error sending message: {e}"),
                        }
                    }
                }
                "/inbox" => {
                    let inbox = node.get_inbox();
                    if inbox.is_empty() {
                        println!("Inbox is empty.");
                    } else {
                        println!("Inbox ({} messages):", inbox.len());
                        for msg in inbox {
                            println!(
                                "  [{}] From {}: {}",
                                msg.short_id(),
                                msg.sender_name,
                                msg.content
                            );
                        }
                    }
                }
                "/outbox" => {
                    let outbox = node.get_outbox();
                    if outbox.is_empty() {
                        println!("Outbox is empty.");
                    } else {
                        println!("Outbox ({} messages):", outbox.len());
                        for msg in outbox {
                            println!(
                                "  [{}] To {}: \"{}\" | Status: {}",
                                msg.short_id(),
                                msg.recipient_name,
                                msg.content,
                                msg.status
                            );
                        }
                    }
                }
                "/file-send" => {
                    if parts.len() < 3 {
                        println!("Usage: /file-send <CONTACT_NAME> <FILE_PATH>");
                    } else {
                        let contact_name = parts[1];
                        let file_path = Path::new(parts[2]);
                        match node.send_file(contact_name, file_path) {
                            Ok(file_id) => {
                                println!("File transfer initiated! File ID: {}", to_hex(&file_id));
                            }
                            Err(e) => println!("Error sending file: {e}"),
                        }
                    }
                }
                "/file-status" => {
                    let transfers = node.get_file_transfers();
                    if transfers.is_empty() {
                        println!("No active or recent file transfers.");
                    } else {
                        println!("File Transfers ({}):", transfers.len());
                        for t in transfers {
                            println!("  * {t}");
                        }
                    }
                }
                "/file-cancel" => {
                    if parts.len() < 2 {
                        println!("Usage: /file-cancel <FILE_ID_HEX>");
                    } else {
                        match parse_32_bytes(parts[1]) {
                            Ok(id) => {
                                if node.cancel_file_transfer(&id) {
                                    println!("File transfer cancelled.");
                                } else {
                                    println!("Transfer not found or already completed.");
                                }
                            }
                            Err(e) => println!("Invalid file ID hex: {e}"),
                        }
                    }
                }
                "/group-create" => {
                    if parts.len() < 2 {
                        println!("Usage: /group-create <GROUP_NAME>");
                    } else {
                        let name = parts[1..].join(" ");
                        match node.create_group(&name) {
                            Ok(g) => {
                                println!("Created group '{}'", g.name);
                                println!("  Group ID: {}", to_hex(&g.group_id));
                                println!("  Epoch:    {}", g.epoch);
                                println!("  Members:  {}", g.members.len());
                            }
                            Err(e) => println!("Failed to create group: {e}"),
                        }
                    }
                }
                "/group-list" => {
                    let groups = node.list_groups();
                    if groups.is_empty() {
                        println!("No groups found. Use /group-create <NAME> to create one.");
                    } else {
                        println!("Groups ({}):", groups.len());
                        for g in groups {
                            println!(
                                "  * '{}' (ID: {}) | Epoch: {} | Members: {}",
                                g.name,
                                to_hex(&g.group_id),
                                g.epoch,
                                g.members.len()
                            );
                        }
                    }
                }
                "/group-add" => {
                    if parts.len() < 3 {
                        println!("Usage: /group-add <GROUP_ID_HEX> <CONTACT_NAME>");
                    } else {
                        match parse_32_bytes(parts[1]) {
                            Ok(group_id) => {
                                let contact = parts[2];
                                match node.add_group_member(&group_id, contact) {
                                    Ok(()) => println!("Added '{contact}' to group!"),
                                    Err(e) => println!("Failed to add member: {e}"),
                                }
                            }
                            Err(e) => println!("Invalid group ID hex: {e}"),
                        }
                    }
                }
                "/group-remove" => {
                    if parts.len() < 3 {
                        println!("Usage: /group-remove <GROUP_ID_HEX> <CONTACT_NAME>");
                    } else {
                        match parse_32_bytes(parts[1]) {
                            Ok(group_id) => {
                                let contact = parts[2];
                                match node.remove_group_member(&group_id, contact) {
                                    Ok(()) => println!("Removed '{contact}' from group!"),
                                    Err(e) => println!("Failed to remove member: {e}"),
                                }
                            }
                            Err(e) => println!("Invalid group ID hex: {e}"),
                        }
                    }
                }
                "/group-send" => {
                    if parts.len() < 3 {
                        println!("Usage: /group-send <GROUP_ID_HEX> <MESSAGE...>");
                    } else {
                        match parse_32_bytes(parts[1]) {
                            Ok(group_id) => {
                                let msg = parts[2..].join(" ");
                                match node.send_group_chat(&group_id, &msg) {
                                    Ok(()) => println!("Group message broadcast successfully!"),
                                    Err(e) => println!("Failed to send group message: {e}"),
                                }
                            }
                            Err(e) => println!("Invalid group ID hex: {e}"),
                        }
                    }
                }
                "/relay" => {
                    if parts.len() < 2 {
                        println!("Usage: /relay on | /relay off");
                    } else {
                        match parts[1] {
                            "on" => {
                                node.set_relay(true);
                                println!("Relay storage ENABLED.");
                            }
                            "off" => {
                                node.set_relay(false);
                                println!("Relay storage DISABLED.");
                            }
                            _ => println!("Usage: /relay on | /relay off"),
                        }
                    }
                }
                "/relay-status" => {
                    let (bytes, count) = node.get_relay_stats();
                    println!("Relay Storage Status:");
                    println!("  Enabled:        {}", node.is_relay_enabled());
                    println!("  Stored Count:   {} envelopes", count);
                    println!("  Storage Used:   {} bytes", bytes);
                }
                "/storage" => {
                    println!("Storage Directory: {:?}", node.config.data_directory);
                    let (bytes, count) = node.get_relay_stats();
                    println!("Relay messages: {count} ({bytes} bytes)");
                }
                "/quit" | "/exit" => {
                    println!("Shutting down P2P node...");
                    node.stop();
                    println!("Goodbye!");
                    break;
                }
                other => {
                    println!("Unknown command: {other}. Type /help for available commands.");
                }
            }
        } else {
            println!("To send a message, use: /send <CONTACT> <MESSAGE>");
        }

        print!("> ");
        let _ = stdout.flush();
    }
}

fn parse_32_bytes(hex_str: &str) -> Result<[u8; 32], String> {
    let bytes = from_hex(hex_str)?;
    if bytes.len() != 32 {
        return Err(format!(
            "Expected 32 bytes (64 hex characters), got {}",
            bytes.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

fn print_help() {
    println!("Available Commands:");
    println!("  Identity & Status:");
    println!("    /id                             Display your Peer ID and Public Key");
    println!("    /status                         Display current node status and connections");
    println!("    /peers                          List connected P2P peers");
    println!("    /connect <IP:PORT>              Connect directly to a peer");
    println!("    /routes                         Show routing table and known addresses");
    println!("    /network-stats                  Show network & rate limiting metrics");
    println!("    /debug on|off                   Enable or disable verbose debug logging");
    println!("  Contacts & Direct Messages:");
    println!("    /add <NAME> <PUBKEY_HEX>        Add a friend's public key to contacts");
    println!("    /contacts                       List your contacts");
    println!("    /send <NAME> <MESSAGE>          Encrypt and send a message to a contact");
    println!("    /inbox                          View received decrypted messages");
    println!("    /outbox                         View sent messages and delivery statuses");
    println!("  File Transfer:");
    println!("    /file-send <CONTACT> <PATH>     Send an encrypted file in 256 KiB chunks");
    println!("    /file-status                    View active and recent file transfers");
    println!("    /file-cancel <FILE_ID_HEX>      Cancel an ongoing file transfer");
    println!("  Encrypted Group Chat:");
    println!("    /group-create <NAME>            Create a new encrypted group chat");
    println!("    /group-list                     List groups you belong to");
    println!("    /group-add <GROUP_ID> <CONTACT> Add a contact and re-key group epoch");
    println!("    /group-remove <GROUP_ID> <CT>   Remove contact and re-key group epoch");
    println!("    /group-send <GROUP_ID> <MSG>    Send encrypted message to group");
    println!("  Relay & Storage:");
    println!("    /relay on|off                   Enable or disable relay store-and-forward");
    println!("    /relay-status                   View relay storage metrics");
    println!("    /storage                        View disk storage details");
    println!("    /quit                           Safely stop the node and exit");
}
