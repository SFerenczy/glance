//! Integration tests for Glance.
//!
//! These tests require a running PostgreSQL database.
//! Set DATABASE_URL environment variable to run them.

pub mod connection_test;
pub mod persistence_test;
pub mod query_test;
pub mod schema_test;
