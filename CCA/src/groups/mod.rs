// src/groups/mod.rs
//! Encrypted Group Chat module (Membership, Epoch Key Rotation, Group Messaging).

pub mod group;
pub mod group_manager;

pub use group::GroupInfo;
pub use group_manager::GroupManager;
