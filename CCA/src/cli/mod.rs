// src/cli/mod.rs
//! Terminal and command-line interface module.

pub mod commands;
pub mod terminal;

pub use commands::CliCommand;
pub use terminal::run_interactive;
