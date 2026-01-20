//! Command parsing and dispatch for Glance.
//!
//! This module provides a clean separation between command parsing and execution,
//! enabling unit testing of command parsing without requiring database setup.

pub mod handlers;
pub mod help;
pub mod router;

#[allow(unused_imports)]
pub use handlers::{CommandContext, CommandResult};
pub use router::{Command, CommandRouter};
