//! Database abstraction layer for Glance.
//!
//! Provides a trait-based interface for database operations, allowing
//! different database backends to be used interchangeably.

mod mock;
mod postgres;
mod schema;
mod types;

pub use mock::{FailingDatabaseClient, MockDatabaseClient};
#[allow(unused_imports)]
pub use postgres::PostgresClient;
pub use schema::{Column, ForeignKey, Index, Schema, Table};
pub use types::{ColumnInfo, QueryResult, Row, Value};

use crate::config::ConnectionConfig;
use crate::error::Result;
use async_trait::async_trait;

/// Supported database backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseBackend {
    #[default]
    Postgres,
    // Future: MySQL, SQLite, etc.
}

impl DatabaseBackend {
    /// Returns the backend as a string for persistence.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
        }
    }

    /// Parses a backend from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" => Some(Self::Postgres),
            _ => None,
        }
    }

    /// Returns the default port for this backend.
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Postgres => 5432,
        }
    }

    /// Returns the URL scheme for this backend.
    pub fn url_scheme(&self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
        }
    }
}

/// Creates a database client for the given backend and configuration.
///
/// This is the central factory function for database connections.
pub async fn connect(config: &ConnectionConfig) -> Result<Box<dyn DatabaseClient>> {
    match config.backend {
        DatabaseBackend::Postgres => {
            let client = PostgresClient::connect(config).await?;
            Ok(Box::new(client))
        }
    }
}

/// Trait defining the interface for database clients.
///
/// All database operations are async and return Results with GlanceError.
#[async_trait]
pub trait DatabaseClient: Send + Sync {
    /// Introspects the database schema, returning table and relationship information.
    async fn introspect_schema(&self) -> Result<Schema>;

    /// Executes a SQL query and returns the results.
    async fn execute_query(&self, sql: &str) -> Result<QueryResult>;

    /// Closes the database connection.
    async fn close(&self) -> Result<()>;
}
