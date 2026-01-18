//! Command-line argument parsing for Glance.
//!
//! Uses clap to parse CLI arguments per FR-8.1 specification.

use crate::config::ConnectionConfig;
use crate::error::Result;
use clap::Parser;
use std::path::PathBuf;

/// A lightweight, AI-first database viewer.
#[derive(Parser, Debug)]
#[command(name = "glance")]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// PostgreSQL connection string (e.g., postgres://user:pass@host:port/database)
    #[arg(value_name = "CONNECTION_STRING")]
    pub connection_string: Option<String>,

    /// Database host
    #[arg(short = 'H', long, value_name = "HOST")]
    pub host: Option<String>,

    /// Database port
    #[arg(short = 'p', long, value_name = "PORT", default_value = "5432")]
    pub port: u16,

    /// Database name
    #[arg(short = 'd', long, value_name = "DATABASE")]
    pub database: Option<String>,

    /// Database user
    #[arg(short = 'U', long, value_name = "USER")]
    pub user: Option<String>,

    /// Prompt for password
    #[arg(short = 'W', long)]
    pub password: bool,

    /// Use named connection from config
    #[arg(short = 'c', long, value_name = "NAME")]
    pub connection: Option<String>,

    /// Config file path
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
}

impl Cli {
    /// Parses command-line arguments.
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Converts CLI arguments to a ConnectionConfig.
    ///
    /// This creates a config from CLI args only, without merging with file config.
    pub fn to_connection_config(&self) -> Result<Option<ConnectionConfig>> {
        // If connection string is provided, parse it
        if let Some(conn_str) = &self.connection_string {
            return Ok(Some(ConnectionConfig::from_connection_string(conn_str)?));
        }

        // If any individual connection args are provided, build a config
        if self.host.is_some() || self.database.is_some() || self.user.is_some() {
            return Ok(Some(ConnectionConfig {
                host: self.host.clone(),
                port: self.port,
                database: self.database.clone(),
                user: self.user.clone(),
                password: None, // Password handled separately via prompt
                sslmode: None,
                extras: None,
            }));
        }

        // No CLI connection args provided
        Ok(None)
    }

    /// Returns the config file path to use.
    ///
    /// Uses the --config argument if provided, otherwise the default path.
    pub fn config_path(&self) -> PathBuf {
        self.config
            .clone()
            .unwrap_or_else(crate::config::Config::default_path)
    }

    /// Returns the named connection to use, if specified.
    pub fn connection_name(&self) -> Option<&str> {
        self.connection.as_deref()
    }

    /// Returns true if the user requested a password prompt.
    #[allow(dead_code)] // Will be used when password prompting is implemented
    pub fn prompt_password(&self) -> bool {
        self.password
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Cli {
        Cli::parse_from(args)
    }

    #[test]
    fn test_parse_connection_string() {
        let cli = parse_args(&["glance", "postgres://user:pass@localhost:5432/mydb"]);
        assert_eq!(
            cli.connection_string,
            Some("postgres://user:pass@localhost:5432/mydb".to_string())
        );
    }

    #[test]
    fn test_parse_individual_args() {
        let cli = parse_args(&[
            "glance",
            "--host",
            "localhost",
            "--port",
            "5432",
            "--database",
            "mydb",
            "--user",
            "postgres",
        ]);

        assert_eq!(cli.host, Some("localhost".to_string()));
        assert_eq!(cli.port, 5432);
        assert_eq!(cli.database, Some("mydb".to_string()));
        assert_eq!(cli.user, Some("postgres".to_string()));
    }

    #[test]
    fn test_parse_short_args() {
        let cli = parse_args(&["glance", "-H", "localhost", "-d", "mydb", "-U", "postgres"]);

        assert_eq!(cli.host, Some("localhost".to_string()));
        assert_eq!(cli.database, Some("mydb".to_string()));
        assert_eq!(cli.user, Some("postgres".to_string()));
    }

    #[test]
    fn test_parse_password_flag() {
        let cli = parse_args(&["glance", "-W"]);
        assert!(cli.password);

        let cli = parse_args(&["glance", "--password"]);
        assert!(cli.password);
    }

    #[test]
    fn test_parse_named_connection() {
        let cli = parse_args(&["glance", "--connection", "prod"]);
        assert_eq!(cli.connection, Some("prod".to_string()));

        let cli = parse_args(&["glance", "-c", "staging"]);
        assert_eq!(cli.connection, Some("staging".to_string()));
    }

    #[test]
    fn test_parse_config_path() {
        let cli = parse_args(&["glance", "--config", "/path/to/config.toml"]);
        assert_eq!(cli.config, Some(PathBuf::from("/path/to/config.toml")));
    }

    #[test]
    fn test_default_port() {
        let cli = parse_args(&["glance"]);
        assert_eq!(cli.port, 5432);
    }

    #[test]
    fn test_to_connection_config_from_string() {
        let cli = parse_args(&["glance", "postgres://user:pass@localhost:5432/mydb"]);
        let config = cli.to_connection_config().unwrap().unwrap();

        assert_eq!(config.host, Some("localhost".to_string()));
        assert_eq!(config.port, 5432);
        assert_eq!(config.database, Some("mydb".to_string()));
        assert_eq!(config.user, Some("user".to_string()));
        assert_eq!(config.password, Some("pass".to_string()));
    }

    #[test]
    fn test_to_connection_config_from_args() {
        let cli = parse_args(&[
            "glance",
            "--host",
            "localhost",
            "--database",
            "mydb",
            "--user",
            "postgres",
        ]);
        let config = cli.to_connection_config().unwrap().unwrap();

        assert_eq!(config.host, Some("localhost".to_string()));
        assert_eq!(config.database, Some("mydb".to_string()));
        assert_eq!(config.user, Some("postgres".to_string()));
        assert_eq!(config.password, None);
    }

    #[test]
    fn test_to_connection_config_none() {
        let cli = parse_args(&["glance"]);
        let config = cli.to_connection_config().unwrap();
        assert!(config.is_none());
    }

    #[test]
    fn test_connection_string_precedence() {
        // Connection string should be used even if individual args are also provided
        let cli = parse_args(&[
            "glance",
            "postgres://user:pass@localhost:5432/mydb",
            "--host",
            "other-host",
        ]);
        let config = cli.to_connection_config().unwrap().unwrap();

        // Connection string takes precedence
        assert_eq!(config.host, Some("localhost".to_string()));
    }
}
