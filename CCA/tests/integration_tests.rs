// tests/integration_tests.rs
//! Comprehensive P2P multi-node integration tests on TCP loopback.

use std::fs;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use p2pchat::config::NodeConfig;
use p2pchat::events::NodeEvent;
use p2pchat::messaging::envelope::MessageEnvelope;
use p2pchat::node::Node;

fn get_test_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("p2pchat_integ_{name}_{nanos}"));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Test 1: Two nodes connect over TCP loopback, perform handshake, and exchange peer IDs.
#[test]
fn test_two_nodes_direct_connection() {
    let dir_a = get_test_dir("node_a");
    let dir_b = get_test_dir("node_b");

    let cfg_a = NodeConfig::new("127.0.0.1:17001".parse().unwrap(), &dir_a);
    let node_a = Node::start(cfg_a).expect("start node A");

    let cfg_b = NodeConfig::new("127.0.0.1:17002".parse().unwrap(), &dir_b);
    let node_b = Node::start(cfg_b).expect("start node B");

    // B connects to A
    node_b
        .connect_peer(node_a.config.listen_address)
        .expect("B connects to A");

    thread::sleep(Duration::from_millis(300));

    // Verify both nodes register each other
    let peers_a = node_a.get_connected_peers();
    let peers_b = node_b.get_connected_peers();

    assert_eq!(peers_a.len(), 1, "Node A should have 1 peer");
    assert_eq!(peers_b.len(), 1, "Node B should have 1 peer");
    assert_eq!(peers_a[0], *node_b.identity.peer_id());
    assert_eq!(peers_b[0], *node_a.identity.peer_id());

    node_a.stop();
    node_b.stop();
    let _ = fs::remove_dir_all(dir_a);
    let _ = fs::remove_dir_all(dir_b);
}

/// Test 2: Direct end-to-end encrypted messaging between Alice and Bob.
#[test]
fn test_direct_encrypted_messaging() {
    let dir_a = get_test_dir("alice_msg");
    let dir_b = get_test_dir("bob_msg");

    let cfg_a = NodeConfig::new("127.0.0.1:17011".parse().unwrap(), &dir_a);
    let alice = Node::start(cfg_a).unwrap();

    let cfg_b = NodeConfig::new("127.0.0.1:17012".parse().unwrap(), &dir_b);
    let bob = Node::start(cfg_b).unwrap();

    // Alice and Bob add each other to contacts
    alice
        .add_contact("Bob", &bob.identity.public_key_hex())
        .unwrap();
    bob.add_contact("Alice", &alice.identity.public_key_hex())
        .unwrap();

    // Connect
    alice.connect_peer(bob.config.listen_address).unwrap();
    thread::sleep(Duration::from_millis(300));

    // Alice sends encrypted message to Bob
    let sent_msg = alice
        .send_chat("Bob", "Hello Bob, secret P2P text!")
        .expect("send message");
    assert_eq!(sent_msg.content, "Hello Bob, secret P2P text!");

    thread::sleep(Duration::from_millis(400));

    // Bob inspects inbox
    let inbox_b = bob.get_inbox();
    assert_eq!(inbox_b.len(), 1, "Bob should have 1 message in inbox");
    assert_eq!(inbox_b[0].content, "Hello Bob, secret P2P text!");
    assert_eq!(inbox_b[0].sender_name, "Alice");
    assert_eq!(inbox_b[0].sender_peer_id, *alice.identity.peer_id());

    alice.stop();
    bob.stop();
    let _ = fs::remove_dir_all(dir_a);
    let _ = fs::remove_dir_all(dir_b);
}

/// Test 3: Relay store-and-forward when recipient is offline.
/// Alice sends to Bob via Relay R. Bob starts later, connects to R, and retrieves the message.
#[test]
fn test_offline_relay_store_and_forward() {
    let dir_a = get_test_dir("alice_relay");
    let dir_r = get_test_dir("relay_node");
    let dir_b = get_test_dir("bob_offline");

    // 1. Start Relay Node R on port 17022
    let mut cfg_r = NodeConfig::new("127.0.0.1:17022".parse().unwrap(), &dir_r);
    cfg_r.relay_enabled = true;
    let relay = Node::start(cfg_r).unwrap();

    // 2. Start Alice on port 17021
    let cfg_a = NodeConfig::new("127.0.0.1:17021".parse().unwrap(), &dir_a);
    let alice = Node::start(cfg_a).unwrap();

    // Generate Bob's identity in advance so Alice can encrypt to Bob while Bob is offline
    let bob_kp = p2pchat::identity::KeyPair::generate().unwrap();
    let bob_pubkey_hex = bob_kp.public_key_hex();
    let bob_peer_id = *bob_kp.peer_id();

    // Save Bob's identity to Bob's directory so when Bob starts, he has this exact keypair
    let storage_b = p2pchat::storage::StorageEngine::new(&dir_b).unwrap();
    storage_b.save_identity(&bob_kp).unwrap();

    // Alice adds Bob to contacts
    alice.add_contact("Bob", &bob_pubkey_hex).unwrap();

    // Alice connects to Relay R
    alice.connect_peer(relay.config.listen_address).unwrap();
    thread::sleep(Duration::from_millis(300));

    // Alice sends message to Bob (Bob is currently OFFLINE)
    let sent = alice
        .send_chat("Bob", "Important message while Bob is offline!")
        .unwrap();
    println!("Alice sent message: {}", sent.id_hex());

    thread::sleep(Duration::from_millis(400));

    // Check Relay store: Relay must have stored 1 envelope for Bob
    let (relay_bytes, count) = relay.get_relay_stats();
    assert_eq!(count, 1, "Relay should have stored 1 envelope");
    assert!(relay_bytes > 0);

    // Alice shuts down completely
    alice.stop();
    drop(alice);
    thread::sleep(Duration::from_millis(300));

    // 3. Bob comes online on port 17023
    let cfg_b = NodeConfig::new("127.0.0.1:17023".parse().unwrap(), &dir_b);
    let bob = Node::start(cfg_b).unwrap();
    assert_eq!(*bob.identity.peer_id(), bob_peer_id);

    // Bob adds Alice to contacts
    bob.add_contact(
        "Alice",
        &storage_b.load_identity().unwrap().unwrap().public_key_hex(),
    )
    .unwrap();

    // Bob connects to Relay R
    bob.connect_peer(relay.config.listen_address).unwrap();
    thread::sleep(Duration::from_millis(500));

    // Bob should have fetched the message from Relay R
    let inbox_b = bob.get_inbox();
    assert_eq!(
        inbox_b.len(),
        1,
        "Bob should have retrieved 1 message from relay"
    );
    assert_eq!(
        inbox_b[0].content,
        "Important message while Bob is offline!"
    );

    relay.stop();
    bob.stop();
    let _ = fs::remove_dir_all(dir_a);
    let _ = fs::remove_dir_all(dir_r);
    let _ = fs::remove_dir_all(dir_b);
}

/// Test 4: Malformed packets sent directly to TCP listener are rejected safely without crashing.
#[test]
fn test_malformed_packets_rejected_without_crash() {
    let dir = get_test_dir("node_malformed");
    let cfg = NodeConfig::new("127.0.0.1:17031".parse().unwrap(), &dir);
    let node = Node::start(cfg).unwrap();

    // Connect raw TCP socket and send invalid magic bytes
    let mut sock = TcpStream::connect(node.config.listen_address).unwrap();
    use std::io::Write;
    sock.write_all(b"GARBAGE_PACKET_NOT_P2CH").unwrap();
    sock.flush().unwrap();
    drop(sock);

    thread::sleep(Duration::from_millis(200));

    // Node is still alive and responsive!
    let mut sock2 = TcpStream::connect(node.config.listen_address).unwrap();
    // Send oversized frame length (> 512 KiB)
    sock2.write_all(&p2pchat::types::PROTOCOL_MAGIC).unwrap();
    sock2.write_all(&1u16.to_be_bytes()).unwrap(); // Version
    sock2.write_all(&[0x10, 0x00]).unwrap(); // Type, flags
    sock2.write_all(&(600_000u32).to_be_bytes()).unwrap(); // 600 KiB > 512 KiB
    sock2.flush().unwrap();
    drop(sock2);

    thread::sleep(Duration::from_millis(200));

    // Connect a legitimate node to verify the listener is completely intact
    let dir_legit = get_test_dir("node_legit");
    let cfg_legit = NodeConfig::new("127.0.0.1:17032".parse().unwrap(), &dir_legit);
    let legit_node = Node::start(cfg_legit).unwrap();

    assert!(legit_node.connect_peer(node.config.listen_address).is_ok());
    thread::sleep(Duration::from_millis(200));
    assert_eq!(node.get_connected_peers().len(), 1);

    node.stop();
    legit_node.stop();
    let _ = fs::remove_dir_all(dir);
    let _ = fs::remove_dir_all(dir_legit);
}

/// Test 5: Replay and duplicate message handling.
#[test]
fn test_replay_protection() {
    let alice = p2pchat::identity::KeyPair::generate().unwrap();
    let bob = p2pchat::identity::KeyPair::generate().unwrap();

    let mut cache = p2pchat::messaging::SeenMessageCache::new(100);

    let env =
        MessageEnvelope::encrypt(&alice, bob.public_key(), *bob.peer_id(), b"Msg", 3600).unwrap();

    // First time seen
    assert!(cache.check_and_record(&env.message_id));
    // Second time seen (replay attempt)
    assert!(!cache.check_and_record(&env.message_id));
}

/// Test 6: Persistence across node restarts.
#[test]
fn test_persistence_across_restarts() {
    let dir = get_test_dir("persist_node");

    let peer_id_1;
    let pubkey_1;

    // First run
    {
        let cfg = NodeConfig::new("127.0.0.1:17041".parse().unwrap(), &dir);
        let node = Node::start(cfg).unwrap();
        peer_id_1 = *node.identity.peer_id();
        pubkey_1 = node.identity.public_key_hex();

        // Add a contact
        node.add_contact("Charlie", &pubkey_1).unwrap();
        node.stop();
        drop(node);
    }

    // Allow OS socket cleanup
    thread::sleep(Duration::from_millis(400));

    // Second run: restart node pointing to the same data directory
    {
        let cfg = NodeConfig::new("127.0.0.1:17041".parse().unwrap(), &dir);
        let node = Node::start(cfg).unwrap();

        assert_eq!(
            *node.identity.peer_id(),
            peer_id_1,
            "Peer ID must persist across restart"
        );
        assert_eq!(
            node.identity.public_key_hex(),
            pubkey_1,
            "Public Key must persist across restart"
        );

        let contacts = node.list_contacts();
        assert_eq!(contacts.len(), 1, "Contacts must persist across restart");
        assert_eq!(contacts[0].name, "Charlie");

        node.stop();
    }

    let _ = fs::remove_dir_all(dir);
}

/// Test 7: Multi-chunk end-to-end encrypted file transfer over TCP loopback network.
/// Alice sends a multi-chunk file to Bob. Bob receives manifest, requests chunks sequentially,
/// verifies hashes, writes to disk, and completes file reconstruction.
#[test]
fn test_file_transfer_over_network() {
    let dir_a = get_test_dir("alice_file");
    let dir_b = get_test_dir("bob_file");

    let cfg_a = NodeConfig::new("127.0.0.1:17051".parse().unwrap(), &dir_a);
    let alice = Node::start(cfg_a).expect("start alice");

    let cfg_b = NodeConfig::new("127.0.0.1:17052".parse().unwrap(), &dir_b);
    let bob = Node::start(cfg_b).expect("start bob");

    // Mutual contacts
    alice
        .add_contact("Bob", &bob.identity.public_key_hex())
        .unwrap();
    bob.add_contact("Alice", &alice.identity.public_key_hex())
        .unwrap();

    // Connect Bob to Alice
    bob.connect_peer(alice.config.listen_address).unwrap();
    thread::sleep(Duration::from_millis(300));

    // Create a 600 KiB test file in Alice's directory (spans 3 chunks: 256K, 256K, 88K)
    let src_file_path = dir_a.join("test_payload.bin");
    let mut original_data = vec![0u8; 600 * 1024];
    for (i, byte) in original_data.iter_mut().enumerate() {
        *byte = ((i * 31 + 17) & 0xFF) as u8;
    }
    fs::write(&src_file_path, &original_data).unwrap();

    // Subscribe Bob to FileTransferCompleted event
    let completed_transfers = Arc::new(Mutex::new(Vec::new()));
    let comp_clone = completed_transfers.clone();
    bob.event_bus.subscribe(move |event| {
        if let NodeEvent::FileTransferCompleted {
            file_name,
            path,
            file_id,
        } = event
        {
            comp_clone
                .lock()
                .unwrap()
                .push((file_name.clone(), path.clone(), *file_id));
        }
    });

    // Alice initiates file send to Bob
    let file_id = alice.send_file("Bob", &src_file_path).expect("send_file");

    // Wait for transfer completion over TCP loopback (poll with timeout)
    let start = std::time::Instant::now();
    let mut completed = false;
    let mut received_path = String::new();
    while start.elapsed() < Duration::from_secs(6) {
        let list = completed_transfers.lock().unwrap();
        if let Some((name, path, id)) = list.first() {
            assert_eq!(name, "test_payload.bin");
            assert_eq!(id, &file_id);
            received_path = path.clone();
            completed = true;
            break;
        }
        drop(list);
        thread::sleep(Duration::from_millis(50));
    }

    assert!(completed, "File transfer must complete over loopback TCP");

    // Verify received file matches original byte-for-byte
    let received_data = fs::read(&received_path).expect("read received file");
    assert_eq!(received_data.len(), original_data.len());
    assert_eq!(received_data, original_data);

    alice.stop();
    bob.stop();
    let _ = fs::remove_dir_all(dir_a);
    let _ = fs::remove_dir_all(dir_b);
}

/// Test 8: Encrypted group chat with dynamic key distribution over TCP loopback network.
/// Alice creates a group, adds Bob and Charlie, rotates keys, broadcasts encrypted messages,
/// and verifies members receive and decrypt the group messages.
#[test]
fn test_group_chat_over_network() {
    let dir_a = get_test_dir("alice_group");
    let dir_b = get_test_dir("bob_group");
    let dir_c = get_test_dir("charlie_group");

    let cfg_a = NodeConfig::new("127.0.0.1:17061".parse().unwrap(), &dir_a);
    let alice = Node::start(cfg_a).expect("start alice");

    let cfg_b = NodeConfig::new("127.0.0.1:17062".parse().unwrap(), &dir_b);
    let bob = Node::start(cfg_b).expect("start bob");

    let cfg_c = NodeConfig::new("127.0.0.1:17063".parse().unwrap(), &dir_c);
    let charlie = Node::start(cfg_c).expect("start charlie");

    // Setup contacts
    alice
        .add_contact("Bob", &bob.identity.public_key_hex())
        .unwrap();
    alice
        .add_contact("Charlie", &charlie.identity.public_key_hex())
        .unwrap();

    bob.add_contact("Alice", &alice.identity.public_key_hex())
        .unwrap();
    charlie
        .add_contact("Alice", &alice.identity.public_key_hex())
        .unwrap();

    // Connect Bob -> Alice and Charlie -> Alice
    bob.connect_peer(alice.config.listen_address).unwrap();
    charlie.connect_peer(alice.config.listen_address).unwrap();
    thread::sleep(Duration::from_millis(300));

    // Subscribe Bob and Charlie to group messages
    let bob_group_msgs = Arc::new(Mutex::new(Vec::new()));
    let bob_msgs_clone = bob_group_msgs.clone();
    bob.event_bus.subscribe(move |event| {
        if let NodeEvent::GroupMessageReceived {
            group_name,
            sender,
            text,
            ..
        } = event
        {
            bob_msgs_clone
                .lock()
                .unwrap()
                .push((group_name.clone(), *sender, text.clone()));
        }
    });

    let charlie_group_msgs = Arc::new(Mutex::new(Vec::new()));
    let charlie_msgs_clone = charlie_group_msgs.clone();
    charlie.event_bus.subscribe(move |event| {
        if let NodeEvent::GroupMessageReceived {
            group_name,
            sender,
            text,
            ..
        } = event
        {
            charlie_msgs_clone
                .lock()
                .unwrap()
                .push((group_name.clone(), *sender, text.clone()));
        }
    });

    // 1. Alice creates group "Core Team"
    let group = alice.create_group("Core Team").expect("create group");
    let group_id = group.group_id;

    // 2. Alice adds Bob and Charlie to the group
    alice.add_group_member(&group_id, "Bob").expect("add bob");
    alice
        .add_group_member(&group_id, "Charlie")
        .expect("add charlie");
    thread::sleep(Duration::from_millis(400));

    // Verify Bob and Charlie now have the group registered
    let bob_groups = bob.list_groups();
    assert_eq!(bob_groups.len(), 1, "Bob should have 1 group");
    assert_eq!(bob_groups[0].name, "Core Team");

    let charlie_groups = charlie.list_groups();
    assert_eq!(charlie_groups.len(), 1, "Charlie should have 1 group");
    assert_eq!(charlie_groups[0].name, "Core Team");

    // 3. Alice broadcasts group message
    alice
        .send_group_chat(&group_id, "Welcome to Core Team, everyone!")
        .expect("alice sends group chat");

    // Wait and verify both Bob and Charlie received and decrypted the message
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(4) {
        if !bob_group_msgs.lock().unwrap().is_empty()
            && !charlie_group_msgs.lock().unwrap().is_empty()
        {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    let b_msgs = bob_group_msgs.lock().unwrap();
    assert_eq!(b_msgs.len(), 1, "Bob should receive 1 group message");
    assert_eq!(b_msgs[0].2, "Welcome to Core Team, everyone!");
    assert_eq!(b_msgs[0].1, *alice.identity.peer_id());

    let c_msgs = charlie_group_msgs.lock().unwrap();
    assert_eq!(c_msgs.len(), 1, "Charlie should receive 1 group message");
    assert_eq!(c_msgs[0].2, "Welcome to Core Team, everyone!");
    assert_eq!(c_msgs[0].1, *alice.identity.peer_id());

    alice.stop();
    bob.stop();
    charlie.stop();
    let _ = fs::remove_dir_all(dir_a);
    let _ = fs::remove_dir_all(dir_b);
    let _ = fs::remove_dir_all(dir_c);
}
