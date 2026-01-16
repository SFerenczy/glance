//! Database abstraction layer for Glance.
//!
//! Provides a trait-based interface for database operations, allowing
//! different database backends to be used interchangeably.

mod postgres;
mod schema;
mod types;

#[allow(unused_imports)]
pub use postgres::PostgresClient;
pub use schema::{Column, ForeignKey, Index, Schema, Table};
pub use types::{ColumnInfo, QueryResult, Row, Value};

use crate::config::ConnectionConfig;
use crate::error::Result;
use async_trait::async_trait;

/// Trait defining the interface for database clients.
///
/// All database operations are async and return Results with GlanceError.
#[async_trait]
pub trait DatabaseClient: Send + Sync {
    /// Connects to the database using the provided configuration.
    async fn connect(config: &ConnectionConfig) -> Result<Self>
    where
        Self: Sized;

    /// Introspects the database schema, returning table and relationship information.
    async fn introspect_schema(&self) -> Result<Schema>;

    /// Executes a SQL query and returns the results.
    async fn execute_query(&self, sql: &str) -> Result<QueryResult>;

    /// Closes the database connection.
    async fn close(&self) -> Result<()>;
}
