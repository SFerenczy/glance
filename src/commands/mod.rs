//! Command parsing and dispatch for Glance.
//!
//! This module provides a clean separation between command parsing and execution,
//! enabling unit testing of command parsing without requiring database setup.

pub mod definitions;
pub mod handlers;
pub mod help;
pub mod output;
pub mod router;
pub mod tokenizer;

#[allow(unused_imports)]
pub use definitions::{CommandCategory, CommandDef, COMMANDS};
#[allow(unused_imports)]
pub use handlers::{CommandContext, CommandResult};
#[allow(unused_imports)]
pub use output::{CommandOutput, ControlAction};
pub use router::{Command, CommandRouter};
#[allow(unused_imports)]
pub use tokenizer::{ParseError, Token};
