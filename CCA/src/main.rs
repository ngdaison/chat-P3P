// src/main.rs
//! P2P Chat Core - Main executable entry point.

use p2pchat::cli::{CliCommand, run_interactive};
use p2pchat::config::NodeConfig;
use p2pchat::identity::keypair::KeyPair;
use p2pchat::node::Node;
use p2pchat::storage::StorageEngine;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = match CliCommand::parse(&args) {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("Error: {err}");
            eprintln!("Run `p2pchat --help` for usage information.");
            process::exit(1);
        }
    };

    match command {
        CliCommand::Init { data_dir } => {
            let storage = match StorageEngine::new(&data_dir) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to initialize storage at {:?}: {e}", data_dir);
                    process::exit(1);
                }
            };

            let keypair = match storage.load_identity() {
                Ok(Some(kp)) => {
                    println!("Existing identity loaded from {:?}", data_dir);
                    kp
                }
                Ok(None) => match KeyPair::generate() {
                    Ok(kp) => {
                        if let Err(e) = storage.save_identity(&kp) {
                            eprintln!("Failed to persist new identity: {e}");
                            process::exit(1);
                        }
                        println!("New identity generated and saved to {:?}", data_dir);
                        kp
                    }
                    Err(e) => {
                        eprintln!("Crypto error generating identity: {e}");
                        process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("Storage error loading identity: {e}");
                    process::exit(1);
                }
            };

            println!("Peer ID:    {}", keypair.peer_id());
            println!("Public Key: {}", keypair.public_key_hex());
        }
        CliCommand::Run {
            data_dir,
            listen_addr,
            initial_peers,
            relay,
        } => {
            let mut config = NodeConfig::new(listen_addr, data_dir);
            config.relay_enabled = relay;

            let node = match Node::start(config) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Failed to start P2P node: {e}");
                    process::exit(1);
                }
            };

            // Connect to initial bootstrap peers if provided
            for peer_addr in initial_peers {
                if let Err(e) = node.connect_peer(peer_addr) {
                    println!("Warning: Could not connect to initial peer {peer_addr}: {e}");
                }
            }

            // Start interactive command loop
            run_interactive(node);
        }
        CliCommand::Help => {
            println!("P2P Chat Core - Decentralized Peer-to-Peer Encrypted Chat");
            println!();
            println!("Usage:");
            println!("  cargo run -- init --data <DATA_DIR>");
            println!("      Initialize a new node identity without starting the network.");
            println!();
            println!(
                "  cargo run -- run --data <DATA_DIR> --listen <IP:PORT> [--peer <IP:PORT>] [--relay]"
            );
            println!("      Start the P2P node and open the interactive terminal.");
            println!();
            println!("Examples:");
            println!("  cargo run -- init --data ./node-a");
            println!("  cargo run -- run --data ./node-a --listen 127.0.0.1:7001");
            println!(
                "  cargo run -- run --data ./node-b --listen 127.0.0.1:7002 --peer 127.0.0.1:7001"
            );
            println!("  cargo run -- run --data ./relay  --listen 127.0.0.1:7003 --relay");
        }
    }
}
