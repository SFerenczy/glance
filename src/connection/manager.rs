//! Connection manager for database lifecycle and switching.

use std::sync::Arc;

use crate::config::ConnectionConfig;
use crate::db::{DatabaseClient, Schema};
use crate::error::Result;
use crate::persistence::{self, StateDb};

/// An active database connection with its metadata.
pub struct ActiveConnection {
    /// Connection name (if using a saved connection).
    pub name: Option<String>,
    /// Database client.
    pub db: Box<dyn DatabaseClient>,
    /// Database schema.
    #[allow(dead_code)] // Kept for API completeness
    pub schema: Schema,
}

/// Manages database connections and switching between them.
pub struct ConnectionManager {
    active: Option<ActiveConnection>,
    state_db: Option<Arc<StateDb>>,
}

impl ConnectionManager {
    /// Creates a new connection manager.
    pub fn new(state_db: Option<Arc<StateDb>>) -> Self {
        Self {
            active: None,
            state_db,
        }
    }

    /// Creates a connection manager with an existing connection.
    pub fn with_connection(
        db: Box<dyn DatabaseClient>,
        schema: Schema,
        name: Option<String>,
        state_db: Option<Arc<StateDb>>,
    ) -> Self {
        Self {
            active: Some(ActiveConnection { name, db, schema }),
            state_db,
        }
    }

    /// Connect to a database using the given configuration.
    #[allow(dead_code)] // Kept for API completeness
    pub async fn connect(&mut self, config: &ConnectionConfig, name: Option<String>) -> Result<()> {
        let db = crate::db::connect(config).await?;
        let schema = db.introspect_schema().await?;

        if let Some(old) = self.active.take() {
            let _ = old.db.close().await;
        }

        self.active = Some(ActiveConnection { name, db, schema });

        Ok(())
    }

    /// Switch to a saved connection by name.
    pub async fn switch_to(&mut self, name: &str) -> Result<ConnectionSwitchResult> {
        let state_db = self
            .state_db
            .as_ref()
            .ok_or_else(|| crate::error::GlanceError::connection("State database not available"))?;

        let profile = persistence::connections::get_connection(state_db.pool(), name)
            .await?
            .ok_or_else(|| {
                crate::error::GlanceError::connection(format!("Connection '{}' not found", name))
            })?;

        let password = persistence::connections::get_connection_password(
            state_db.pool(),
            name,
            state_db.secrets(),
        )
        .await?;

        let config = ConnectionConfig {
            backend: profile.backend,
            host: profile.host.clone(),
            port: profile.port,
            database: Some(profile.database.clone()),
            user: profile.username.clone(),
            password,
            sslmode: profile.sslmode.clone(),
            extras: profile.extras.clone(),
        };

        let db = crate::db::connect(&config).await?;
        let schema = db.introspect_schema().await?;

        if let Some(old) = self.active.take() {
            let _ = old.db.close().await;
        }

        self.active = Some(ActiveConnection {
            name: Some(name.to_string()),
            db,
            schema: schema.clone(),
        });

        persistence::connections::touch_connection(state_db.pool(), name).await?;

        Ok(ConnectionSwitchResult {
            name: name.to_string(),
            database: profile.database,
            schema,
        })
    }

    /// Get the active database client.
    pub fn db(&self) -> Option<&dyn DatabaseClient> {
        self.active.as_ref().map(|c| c.db.as_ref())
    }

    /// Get the current schema.
    #[allow(dead_code)] // Kept for API completeness
    pub fn schema(&self) -> Option<&Schema> {
        self.active.as_ref().map(|c| &c.schema)
    }

    /// Get the current connection name.
    pub fn current_name(&self) -> Option<&str> {
        self.active.as_ref().and_then(|c| c.name.as_deref())
    }

    /// Check if there's an active connection.
    #[allow(dead_code)] // Kept for API completeness
    pub fn is_connected(&self) -> bool {
        self.active.is_some()
    }

    /// Close the active connection.
    pub async fn close(&mut self) -> Result<()> {
        if let Some(conn) = self.active.take() {
            conn.db.close().await?;
        }
        Ok(())
    }

    /// Take ownership of the active connection.
    #[allow(dead_code)] // Kept for API completeness
    pub fn take_active(&mut self) -> Option<ActiveConnection> {
        self.active.take()
    }

    /// Set a new active connection.
    #[allow(dead_code)] // Kept for API completeness
    pub fn set_active(&mut self, conn: ActiveConnection) {
        self.active = Some(conn);
    }
}

/// Result of switching to a new connection.
pub struct ConnectionSwitchResult {
    /// Connection name.
    pub name: String,
    /// Database name.
    pub database: String,
    /// Database schema.
    pub schema: Schema,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::MockDatabaseClient;

    #[test]
    fn test_new_manager_has_no_connection() {
        let manager = ConnectionManager::new(None);
        assert!(!manager.is_connected());
        assert!(manager.db().is_none());
        assert!(manager.schema().is_none());
        assert!(manager.current_name().is_none());
    }

    #[test]
    fn test_with_connection() {
        let mock_db = MockDatabaseClient::new();
        let schema = Schema::default();
        let manager = ConnectionManager::with_connection(
            Box::new(mock_db),
            schema,
            Some("test".to_string()),
            None,
        );

        assert!(manager.is_connected());
        assert!(manager.db().is_some());
        assert!(manager.schema().is_some());
        assert_eq!(manager.current_name(), Some("test"));
    }

    #[tokio::test]
    async fn test_close_connection() {
        let mock_db = MockDatabaseClient::new();
        let schema = Schema::default();
        let mut manager = ConnectionManager::with_connection(
            Box::new(mock_db),
            schema,
            Some("test".to_string()),
            None,
        );

        assert!(manager.is_connected());
        manager.close().await.unwrap();
        assert!(!manager.is_connected());
    }

    #[test]
    fn test_take_and_set_active() {
        let mock_db = MockDatabaseClient::new();
        let schema = Schema::default();
        let mut manager = ConnectionManager::with_connection(
            Box::new(mock_db),
            schema.clone(),
            Some("test".to_string()),
            None,
        );

        let taken = manager.take_active();
        assert!(taken.is_some());
        assert!(!manager.is_connected());

        let new_conn = ActiveConnection {
            name: Some("new".to_string()),
            db: Box::new(MockDatabaseClient::new()),
            schema,
        };
        manager.set_active(new_conn);
        assert!(manager.is_connected());
        assert_eq!(manager.current_name(), Some("new"));
    }
}
