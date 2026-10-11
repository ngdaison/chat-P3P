// src/cli/commands.rs
//! Command line argument parsing and interactive terminal command execution.

use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CliCommand {
    Init {
        data_dir: PathBuf,
    },
    Run {
        data_dir: PathBuf,
        listen_addr: SocketAddr,
        initial_peers: Vec<SocketAddr>,
        relay: bool,
    },
    Help,
}

impl CliCommand {
    /// Parses CLI arguments from `std::env::args()` without external libraries.
    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.len() < 2 {
            return Ok(CliCommand::Help);
        }

        match args[1].as_str() {
            "init" => {
                let mut data_dir = PathBuf::from("./p2p_data");
                let mut i = 2;
                while i < args.len() {
                    match args[i].as_str() {
                        "--data" => {
                            if i + 1 < args.len() {
                                data_dir = PathBuf::from(&args[i + 1]);
                                i += 2;
                            } else {
                                return Err("Missing value for --data".to_string());
                            }
                        }
                        other => return Err(format!("Unknown option for init: {other}")),
                    }
                }
                Ok(CliCommand::Init { data_dir })
            }
            "run" => {
                let mut data_dir = PathBuf::from("./p2p_data");
                let mut listen_addr: SocketAddr = "127.0.0.1:7001".parse().unwrap();
                let mut initial_peers = Vec::new();
                let mut relay = false;

                let mut i = 2;
                while i < args.len() {
                    match args[i].as_str() {
                        "--data" => {
                            if i + 1 < args.len() {
                                data_dir = PathBuf::from(&args[i + 1]);
                                i += 2;
                            } else {
                                return Err("Missing value for --data".to_string());
                            }
                        }
                        "--listen" => {
                            if i + 1 < args.len() {
                                listen_addr = args[i + 1]
                                    .parse()
                                    .map_err(|e| format!("Invalid --listen address: {e}"))?;
                                i += 2;
                            } else {
                                return Err("Missing value for --listen".to_string());
                            }
                        }
                        "--peer" => {
                            if i + 1 < args.len() {
                                let peer_addr: SocketAddr = args[i + 1]
                                    .parse()
                                    .map_err(|e| format!("Invalid --peer address: {e}"))?;
                                initial_peers.push(peer_addr);
                                i += 2;
                            } else {
                                return Err("Missing value for --peer".to_string());
                            }
                        }
                        "--relay" => {
                            relay = true;
                            i += 1;
                        }
                        other => return Err(format!("Unknown option for run: {other}")),
                    }
                }

                Ok(CliCommand::Run {
                    data_dir,
                    listen_addr,
                    initial_peers,
                    relay,
                })
            }
            "--help" | "-h" | "help" => Ok(CliCommand::Help),
            other => Err(format!("Unknown subcommand: {other}")),
        }
    }
}
