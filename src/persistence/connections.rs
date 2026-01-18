//! Connection profile persistence.
//!
//! CRUD operations for saved database connections.

#![allow(dead_code)]

use crate::error::{GlanceError, Result};
use crate::persistence::secrets::SecretStorage;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::FromRow;

/// Password storage method for a connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PasswordStorage {
    /// No password stored.
    None,
    /// Password stored in OS keyring.
    Keyring,
    /// Password stored as plaintext (with user consent).
    Plaintext,
}

impl PasswordStorage {
    fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Keyring => "keyring",
            Self::Plaintext => "plaintext",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "keyring" => Self::Keyring,
            "plaintext" => Self::Plaintext,
            _ => Self::None,
        }
    }
}

/// Raw database row for a connection profile.
#[derive(Debug, Clone, FromRow)]
pub struct ConnectionProfileRow {
    pub name: String,
    pub database: String,
    pub host: Option<String>,
    pub port: i32,
    pub username: Option<String>,
    pub sslmode: Option<String>,
    pub extras: Option<String>,
    pub password_storage: String,
    pub password_plaintext: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
}

/// A saved database connection profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub name: String,
    pub database: String,
    pub host: Option<String>,
    pub port: u16,
    pub username: Option<String>,
    pub sslmode: Option<String>,
    pub extras: Option<serde_json::Value>,
    pub password_storage: PasswordStorage,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
}

impl ConnectionProfile {
    /// Creates a new connection profile.
    pub fn new(name: String, database: String) -> Self {
        Self {
            name,
            database,
            host: None,
            port: 5432,
            username: None,
            sslmode: None,
            extras: None,
            password_storage: PasswordStorage::None,
            created_at: String::new(),
            updated_at: String::new(),
            last_used_at: None,
        }
    }

    /// Returns a display-safe string (no password, redacted host/user).
    pub fn display_string(&self) -> String {
        let host = self.host.as_deref().unwrap_or("localhost");
        format!("{} @ {}:{}", self.database, host, self.port)
    }

    /// Returns a redacted display string for AI context (no host/user).
    pub fn redacted_display(&self) -> String {
        format!("{} ({})", self.name, self.database)
    }

    /// Returns the redacted host for display.
    pub fn redacted_host(&self) -> String {
        self.host
            .as_ref()
            .map(|_| "******".to_string())
            .unwrap_or_else(|| "localhost".to_string())
    }

    /// Returns the redacted username for display.
    pub fn redacted_username(&self) -> Option<String> {
        self.username.as_ref().map(|_| "******".to_string())
    }
}

impl From<ConnectionProfileRow> for ConnectionProfile {
    fn from(row: ConnectionProfileRow) -> Self {
        let extras = row
            .extras
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());

        Self {
            name: row.name,
            database: row.database,
            host: row.host,
            port: row.port as u16,
            username: row.username,
            sslmode: row.sslmode,
            extras,
            password_storage: PasswordStorage::from_str(&row.password_storage),
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_used_at: row.last_used_at,
        }
    }
}

/// Lists all saved connection profiles.
pub async fn list_connections(pool: &SqlitePool) -> Result<Vec<ConnectionProfile>> {
    let rows: Vec<ConnectionProfileRow> = sqlx::query_as(
        r#"
        SELECT name, database, host, port, username, sslmode, extras,
               password_storage, password_plaintext, created_at, updated_at, last_used_at
        FROM connections
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to list connections: {e}")))?;

    Ok(rows.into_iter().map(ConnectionProfile::from).collect())
}

/// Gets a connection profile by name.
pub async fn get_connection(pool: &SqlitePool, name: &str) -> Result<Option<ConnectionProfile>> {
    let row: Option<ConnectionProfileRow> = sqlx::query_as(
        r#"
        SELECT name, database, host, port, username, sslmode, extras,
               password_storage, password_plaintext, created_at, updated_at, last_used_at
        FROM connections
        WHERE name = ?
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to get connection: {e}")))?;

    Ok(row.map(ConnectionProfile::from))
}

/// Creates a new connection profile.
pub async fn create_connection(
    pool: &SqlitePool,
    profile: &ConnectionProfile,
    password: Option<&str>,
    secrets: &SecretStorage,
) -> Result<()> {
    let extras_json = profile
        .extras
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());

    let (password_storage, password_plaintext) = if let Some(pwd) = password {
        if secrets.is_secure() {
            let key = SecretStorage::connection_password_key(&profile.name);
            secrets.store(&key, pwd)?;
            (PasswordStorage::Keyring, None)
        } else {
            (PasswordStorage::Plaintext, Some(pwd.to_string()))
        }
    } else {
        (PasswordStorage::None, None)
    };

    sqlx::query(
        r#"
        INSERT INTO connections (name, database, host, port, username, sslmode, extras,
                                 password_storage, password_plaintext)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&profile.name)
    .bind(&profile.database)
    .bind(&profile.host)
    .bind(profile.port as i32)
    .bind(&profile.username)
    .bind(&profile.sslmode)
    .bind(&extras_json)
    .bind(password_storage.as_str())
    .bind(&password_plaintext)
    .execute(pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE constraint") {
            GlanceError::persistence(format!("Connection '{}' already exists", profile.name))
        } else {
            GlanceError::persistence(format!("Failed to create connection: {e}"))
        }
    })?;

    Ok(())
}

/// Updates an existing connection profile.
pub async fn update_connection(
    pool: &SqlitePool,
    profile: &ConnectionProfile,
    password: Option<&str>,
    secrets: &SecretStorage,
) -> Result<()> {
    let extras_json = profile
        .extras
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());

    // If password is provided, update it; otherwise keep existing password unchanged
    let result = if let Some(pwd) = password {
        let (password_storage, password_plaintext) = if secrets.is_secure() {
            let key = SecretStorage::connection_password_key(&profile.name);
            secrets.store(&key, pwd)?;
            (PasswordStorage::Keyring, None)
        } else {
            (PasswordStorage::Plaintext, Some(pwd.to_string()))
        };

        sqlx::query(
            r#"
            UPDATE connections
            SET database = ?, host = ?, port = ?, username = ?, sslmode = ?, extras = ?,
                password_storage = ?, password_plaintext = ?, updated_at = datetime('now')
            WHERE name = ?
            "#,
        )
        .bind(&profile.database)
        .bind(&profile.host)
        .bind(profile.port as i32)
        .bind(&profile.username)
        .bind(&profile.sslmode)
        .bind(&extras_json)
        .bind(password_storage.as_str())
        .bind(&password_plaintext)
        .bind(&profile.name)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to update connection: {e}")))?
    } else {
        // Don't touch password fields when password is not being updated
        sqlx::query(
            r#"
            UPDATE connections
            SET database = ?, host = ?, port = ?, username = ?, sslmode = ?, extras = ?,
                updated_at = datetime('now')
            WHERE name = ?
            "#,
        )
        .bind(&profile.database)
        .bind(&profile.host)
        .bind(profile.port as i32)
        .bind(&profile.username)
        .bind(&profile.sslmode)
        .bind(&extras_json)
        .bind(&profile.name)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to update connection: {e}")))?
    };

    if result.rows_affected() == 0 {
        return Err(GlanceError::persistence(format!(
            "Connection '{}' not found",
            profile.name
        )));
    }

    Ok(())
}

/// Deletes a connection profile.
pub async fn delete_connection(
    pool: &SqlitePool,
    name: &str,
    secrets: &SecretStorage,
) -> Result<()> {
    let key = SecretStorage::connection_password_key(name);
    secrets.delete(&key)?;

    let result = sqlx::query("DELETE FROM connections WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to delete connection: {e}")))?;

    if result.rows_affected() == 0 {
        return Err(GlanceError::persistence(format!(
            "Connection '{}' not found",
            name
        )));
    }

    Ok(())
}

/// Updates the last_used_at timestamp for a connection.
pub async fn touch_connection(pool: &SqlitePool, name: &str) -> Result<()> {
    sqlx::query("UPDATE connections SET last_used_at = datetime('now') WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await
        .map_err(|e| GlanceError::persistence(format!("Failed to update connection: {e}")))?;

    Ok(())
}

/// Retrieves the password for a connection.
pub async fn get_connection_password(
    pool: &SqlitePool,
    name: &str,
    secrets: &SecretStorage,
) -> Result<Option<String>> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT password_storage, password_plaintext FROM connections WHERE name = ?",
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|e| GlanceError::persistence(format!("Failed to get connection: {e}")))?;

    match row {
        Some((storage, plaintext)) => {
            let storage_type = PasswordStorage::from_str(&storage);
            tracing::debug!(
                "Connection '{}' password_storage={:?}, has_plaintext={}",
                name,
                storage_type,
                plaintext.is_some()
            );
            match storage_type {
                PasswordStorage::None => Ok(None),
                PasswordStorage::Keyring => {
                    let key = SecretStorage::connection_password_key(name);
                    let result = secrets.retrieve(&key)?;
                    if result.is_none() {
                        tracing::warn!(
                            "Password for connection '{}' stored in keyring but could not be retrieved. \
                             Keyring may be unavailable. Try re-adding the connection with /conn edit {} password=<pwd>",
                            name, name
                        );
                    }
                    Ok(result)
                }
                PasswordStorage::Plaintext => Ok(plaintext),
            }
        }
        None => Err(GlanceError::persistence(format!(
            "Connection '{}' not found",
            name
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::migrations;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        migrations::run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_create_and_get_connection() {
        let pool = test_pool().await;
        let secrets = SecretStorage::new();

        let profile = ConnectionProfile::new("test".to_string(), "mydb".to_string());
        create_connection(&pool, &profile, None, &secrets)
            .await
            .unwrap();

        let retrieved = get_connection(&pool, "test").await.unwrap().unwrap();
        assert_eq!(retrieved.name, "test");
        assert_eq!(retrieved.database, "mydb");
    }

    #[tokio::test]
    async fn test_list_connections() {
        let pool = test_pool().await;
        let secrets = SecretStorage::new();

        let profile1 = ConnectionProfile::new("alpha".to_string(), "db1".to_string());
        let profile2 = ConnectionProfile::new("beta".to_string(), "db2".to_string());

        create_connection(&pool, &profile1, None, &secrets)
            .await
            .unwrap();
        create_connection(&pool, &profile2, None, &secrets)
            .await
            .unwrap();

        let connections = list_connections(&pool).await.unwrap();
        assert_eq!(connections.len(), 2);
        assert_eq!(connections[0].name, "alpha");
        assert_eq!(connections[1].name, "beta");
    }

    #[tokio::test]
    async fn test_delete_connection() {
        let pool = test_pool().await;
        let secrets = SecretStorage::new();

        let profile = ConnectionProfile::new("test".to_string(), "mydb".to_string());
        create_connection(&pool, &profile, None, &secrets)
            .await
            .unwrap();

        delete_connection(&pool, "test", &secrets).await.unwrap();

        let retrieved = get_connection(&pool, "test").await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_duplicate_connection_fails() {
        let pool = test_pool().await;
        let secrets = SecretStorage::new();

        let profile = ConnectionProfile::new("test".to_string(), "mydb".to_string());
        create_connection(&pool, &profile, None, &secrets)
            .await
            .unwrap();

        let result = create_connection(&pool, &profile, None, &secrets).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }
}
