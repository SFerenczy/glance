//! Configuration management for Glance.
//!
//! Handles loading configuration from TOML files and environment variables,
//! with support for named database connections and LLM provider settings.

use crate::error::{GlanceError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Re-export url for connection string parsing
use url::Url;

/// Main configuration structure for Glance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// LLM provider configuration.
    #[serde(default)]
    pub llm: LlmConfig,

    /// Named database connections.
    #[serde(default)]
    pub connections: HashMap<String, ConnectionConfig>,
}

/// LLM provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// LLM provider: "openai" or "anthropic".
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Model name (e.g., "gpt-5", "claude-3-5-sonnet-latest").
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_provider() -> String {
    "openai".to_string()
}

fn default_model() -> String {
    "gpt-5".to_string()
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
        }
    }
}

/// Database connection configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionConfig {
    /// Database host.
    pub host: Option<String>,

    /// Database port.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Database name.
    pub database: Option<String>,

    /// Database user.
    pub user: Option<String>,

    /// Database password (not recommended to store in config).
    pub password: Option<String>,
}

fn default_port() -> u16 {
    5432
}

#[allow(dead_code)] // Methods will be used in later phases
impl ConnectionConfig {
    /// Creates a new connection config from a connection string.
    ///
    /// Format: `postgres://user:pass@host:port/database`
    pub fn from_connection_string(conn_str: &str) -> Result<Self> {
        let url = Url::parse(conn_str)
            .map_err(|e| GlanceError::config(format!("Invalid connection string: {e}")))?;

        if url.scheme() != "postgres" && url.scheme() != "postgresql" {
            return Err(GlanceError::config(format!(
                "Invalid scheme '{}'. Expected 'postgres' or 'postgresql'",
                url.scheme()
            )));
        }

        let host = url.host_str().map(String::from);
        let port = url.port().unwrap_or(5432);
        let database = url.path().strip_prefix('/').map(String::from);
        let user = if url.username().is_empty() {
            None
        } else {
            Some(url.username().to_string())
        };
        let password = url.password().map(String::from);

        Ok(Self {
            host,
            port,
            database,
            user,
            password,
        })
    }

    /// Converts the connection config to a connection string.
    pub fn to_connection_string(&self) -> Result<String> {
        let host = self.host.as_deref().unwrap_or("localhost");
        let database = self
            .database
            .as_deref()
            .ok_or_else(|| GlanceError::config("Database name is required"))?;

        let mut conn_str = String::from("postgres://");

        if let Some(user) = &self.user {
            conn_str.push_str(user);
            if let Some(password) = &self.password {
                conn_str.push(':');
                conn_str.push_str(password);
            }
            conn_str.push('@');
        }

        conn_str.push_str(host);
        conn_str.push(':');
        conn_str.push_str(&self.port.to_string());
        conn_str.push('/');
        conn_str.push_str(database);

        Ok(conn_str)
    }

    /// Merges another config into this one, with the other taking precedence.
    pub fn merge(&mut self, other: &ConnectionConfig) {
        if other.host.is_some() {
            self.host = other.host.clone();
        }
        if other.port != default_port() {
            self.port = other.port;
        }
        if other.database.is_some() {
            self.database = other.database.clone();
        }
        if other.user.is_some() {
            self.user = other.user.clone();
        }
        if other.password.is_some() {
            self.password = other.password.clone();
        }
    }

    /// Applies environment variables (PGHOST, PGPORT, etc.) as defaults.
    pub fn apply_env_defaults(&mut self) {
        if self.host.is_none() {
            self.host = std::env::var("PGHOST").ok();
        }
        if self.port == default_port() {
            if let Ok(port_str) = std::env::var("PGPORT") {
                if let Ok(port) = port_str.parse() {
                    self.port = port;
                }
            }
        }
        if self.database.is_none() {
            self.database = std::env::var("PGDATABASE").ok();
        }
        if self.user.is_none() {
            self.user = std::env::var("PGUSER").ok();
        }
        if self.password.is_none() {
            self.password = std::env::var("PGPASSWORD").ok();
        }
    }

    /// Returns a display-safe string (no password) for UI purposes.
    pub fn display_string(&self) -> String {
        let host = self.host.as_deref().unwrap_or("localhost");
        let database = self.database.as_deref().unwrap_or("unknown");
        format!("{database} @ {host}:{}", self.port)
    }
}

impl Config {
    /// Returns the default config file path for the current platform.
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("db-glance")
            .join("config.toml")
    }

    /// Loads configuration from a TOML file.
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| GlanceError::config(format!("Failed to read config file: {e}")))?;

        Self::parse_toml(&content, path)
    }

    /// Parses configuration from a TOML string.
    fn parse_toml(content: &str, path: &Path) -> Result<Self> {
        toml::from_str(content).map_err(|e| {
            GlanceError::config(format!(
                "Configuration error in {}:\n  {}",
                path.display(),
                e
            ))
        })
    }

    /// Gets a named connection, or the default connection if name is None.
    pub fn get_connection(&self, name: Option<&str>) -> Option<&ConnectionConfig> {
        let key = name.unwrap_or("default");
        self.connections.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_config() {
        let toml = r#"
[llm]
provider = "anthropic"
model = "claude-3-5-sonnet-latest"

[connections.default]
host = "localhost"
port = 5432
database = "mydb"
user = "postgres"

[connections.prod]
host = "prod.example.com"
port = 5432
database = "myapp"
user = "readonly"
"#;
        let config: Config = toml::from_str(toml).unwrap();

        assert_eq!(config.llm.provider, "anthropic");
        assert_eq!(config.llm.model, "claude-3-5-sonnet-latest");

        let default_conn = config.connections.get("default").unwrap();
        assert_eq!(default_conn.host, Some("localhost".to_string()));
        assert_eq!(default_conn.database, Some("mydb".to_string()));

        let prod_conn = config.connections.get("prod").unwrap();
        assert_eq!(prod_conn.host, Some("prod.example.com".to_string()));
    }

    #[test]
    fn test_missing_optional_fields() {
        let toml = r#"
[connections.default]
database = "mydb"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        let conn = config.connections.get("default").unwrap();

        assert_eq!(conn.host, None);
        assert_eq!(conn.port, 5432);
        assert_eq!(conn.database, Some("mydb".to_string()));
        assert_eq!(conn.user, None);
        assert_eq!(conn.password, None);
    }

    #[test]
    fn test_default_llm_config() {
        let config = Config::default();
        assert_eq!(config.llm.provider, "openai");
        assert_eq!(config.llm.model, "gpt-5");
    }

    #[test]
    fn test_connection_string_parsing() {
        let conn =
            ConnectionConfig::from_connection_string("postgres://user:pass@localhost:5432/mydb")
                .unwrap();

        assert_eq!(conn.host, Some("localhost".to_string()));
        assert_eq!(conn.port, 5432);
        assert_eq!(conn.database, Some("mydb".to_string()));
        assert_eq!(conn.user, Some("user".to_string()));
        assert_eq!(conn.password, Some("pass".to_string()));
    }

    #[test]
    fn test_connection_string_minimal() {
        let conn = ConnectionConfig::from_connection_string("postgres://localhost/mydb").unwrap();

        assert_eq!(conn.host, Some("localhost".to_string()));
        assert_eq!(conn.port, 5432);
        assert_eq!(conn.database, Some("mydb".to_string()));
        assert_eq!(conn.user, None);
        assert_eq!(conn.password, None);
    }

    #[test]
    fn test_connection_string_invalid_scheme() {
        let result = ConnectionConfig::from_connection_string("mysql://localhost/mydb");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid scheme"));
    }

    #[test]
    fn test_to_connection_string() {
        let conn = ConnectionConfig {
            host: Some("localhost".to_string()),
            port: 5432,
            database: Some("mydb".to_string()),
            user: Some("user".to_string()),
            password: Some("pass".to_string()),
        };

        let conn_str = conn.to_connection_string().unwrap();
        assert_eq!(conn_str, "postgres://user:pass@localhost:5432/mydb");
    }

    #[test]
    fn test_to_connection_string_no_auth() {
        let conn = ConnectionConfig {
            host: Some("localhost".to_string()),
            port: 5432,
            database: Some("mydb".to_string()),
            user: None,
            password: None,
        };

        let conn_str = conn.to_connection_string().unwrap();
        assert_eq!(conn_str, "postgres://localhost:5432/mydb");
    }

    #[test]
    fn test_connection_merge() {
        let mut base = ConnectionConfig {
            host: Some("localhost".to_string()),
            port: 5432,
            database: Some("mydb".to_string()),
            user: Some("user".to_string()),
            password: None,
        };

        let override_config = ConnectionConfig {
            host: Some("remote".to_string()),
            port: 5432,
            database: None,
            user: None,
            password: Some("secret".to_string()),
        };

        base.merge(&override_config);

        assert_eq!(base.host, Some("remote".to_string()));
        assert_eq!(base.database, Some("mydb".to_string()));
        assert_eq!(base.user, Some("user".to_string()));
        assert_eq!(base.password, Some("secret".to_string()));
    }

    #[test]
    fn test_display_string() {
        let conn = ConnectionConfig {
            host: Some("localhost".to_string()),
            port: 5432,
            database: Some("mydb".to_string()),
            user: None,
            password: None,
        };

        assert_eq!(conn.display_string(), "mydb @ localhost:5432");
    }

    #[test]
    fn test_get_connection() {
        let toml = r#"
[connections.default]
database = "default_db"

[connections.prod]
database = "prod_db"
"#;
        let config: Config = toml::from_str(toml).unwrap();

        let default = config.get_connection(None).unwrap();
        assert_eq!(default.database, Some("default_db".to_string()));

        let prod = config.get_connection(Some("prod")).unwrap();
        assert_eq!(prod.database, Some("prod_db".to_string()));

        assert!(config.get_connection(Some("nonexistent")).is_none());
    }
}
