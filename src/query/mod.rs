//! Query execution and classification for Glance.
//!
//! This module isolates SQL execution, classification, and result formatting
//! from the main orchestrator.

pub mod executor;

#[allow(unused_imports)]
pub use executor::{ExecutionResult, QueryExecutor, QueryOutcome};
