//! Connection management for Glance.
//!
//! Centralizes connection lifecycle and switching.

pub mod manager;

#[allow(unused_imports)]
pub use manager::{ActiveConnection, ConnectionManager};
