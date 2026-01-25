//! Command-line argument parsing for Glance.
//!
//! Uses clap to parse CLI arguments per FR-8.1 specification.

use crate::config::ConnectionConfig;
use crate::error::Result;
use clap::Parser;
use std::path::PathBuf;

/// Output format for headless mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Plain text output of the final screen.
    #[default]
    Text,
    /// JSON output with screen, state, and metadata.
    Json,
    /// Frame-by-frame output showing state after each event.
    Frames,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "frames" => Ok(Self::Frames),
            _ => Err(format!(
                "Invalid output format: {s}. Expected: text, json, or frames"
            )),
        }
    }
}

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

    /// Use named connection from config
    #[arg(short = 'c', long, value_name = "NAME")]
    pub connection: Option<String>,

    /// Config file path
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    // === Headless mode options ===
    /// Run in headless mode (no terminal UI, for testing/automation)
    #[arg(long)]
    pub headless: bool,

    /// Use mock database (in-memory, for testing)
    #[arg(long)]
    pub mock_db: bool,

    /// Comma-separated events to execute in headless mode (e.g., "type:hello,key:enter")
    #[arg(long, value_name = "EVENTS")]
    pub events: Option<String>,

    /// Path to script file with events (use "-" for stdin)
    #[arg(long, value_name = "PATH")]
    pub script: Option<String>,

    /// Screen size for headless mode (WIDTHxHEIGHT, e.g., "80x24")
    #[arg(long, value_name = "SIZE", default_value = "80x24")]
    pub size: String,

    /// Output format for headless mode
    #[arg(long, value_name = "FORMAT", default_value = "text")]
    pub output: String,

    /// Write output to file instead of stdout
    #[arg(long, value_name = "PATH")]
    pub output_file: Option<PathBuf>,

    /// Stop on first assertion failure
    #[arg(long)]
    pub fail_fast: bool,

    /// SQL seed file for mock database
    #[arg(long, value_name = "PATH")]
    pub seed: Option<PathBuf>,

    /// LLM provider to use (overrides config in headless mode)
    #[arg(long, value_name = "PROVIDER")]
    pub llm: Option<String>,

    /// Allow storing secrets in plaintext (when OS keyring is unavailable)
    #[arg(long)]
    pub allow_plaintext: bool,
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
                ..Default::default()
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

    /// Returns true if headless mode is enabled.
    pub fn is_headless(&self) -> bool {
        self.headless
    }

    /// Parses the screen size from the --size argument.
    /// Returns (width, height) or an error.
    pub fn parse_screen_size(&self) -> std::result::Result<(u16, u16), String> {
        let parts: Vec<&str> = self.size.split('x').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid size format: '{}'. Expected WIDTHxHEIGHT (e.g., 80x24)",
                self.size
            ));
        }
        let width = parts[0]
            .parse::<u16>()
            .map_err(|_| format!("Invalid width: '{}'", parts[0]))?;
        let height = parts[1]
            .parse::<u16>()
            .map_err(|_| format!("Invalid height: '{}'", parts[1]))?;
        Ok((width, height))
    }

    /// Parses the output format from the --output argument.
    pub fn parse_output_format(&self) -> std::result::Result<OutputFormat, String> {
        self.output.parse()
    }

    /// Returns true if --allow-plaintext flag is set.
    pub fn allow_plaintext(&self) -> bool {
        self.allow_plaintext
    }

    /// Validates headless mode arguments.
    /// Returns an error message if validation fails.
    pub fn validate_headless(&self) -> std::result::Result<(), String> {
        if !self.headless {
            return Ok(());
        }

        // Headless mode requires either --events or --script
        if self.events.is_none() && self.script.is_none() {
            return Err("--headless requires --events or --script".to_string());
        }

        // Validate screen size
        self.parse_screen_size()?;

        // Validate output format
        self.parse_output_format()?;

        Ok(())
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

    // === Headless mode tests ===

    #[test]
    fn test_parse_headless_flag() {
        let cli = parse_args(&["glance", "--headless", "--mock-db", "--events", "key:esc"]);
        assert!(cli.headless);
        assert!(cli.mock_db);
        assert_eq!(cli.events, Some("key:esc".to_string()));
    }

    #[test]
    fn test_parse_headless_with_script() {
        let cli = parse_args(&["glance", "--headless", "--mock-db", "--script", "test.txt"]);
        assert!(cli.headless);
        assert_eq!(cli.script, Some("test.txt".to_string()));
    }

    #[test]
    fn test_parse_screen_size() {
        let cli = parse_args(&["glance", "--size", "120x40"]);
        let (w, h) = cli.parse_screen_size().unwrap();
        assert_eq!(w, 120);
        assert_eq!(h, 40);
    }

    #[test]
    fn test_parse_screen_size_invalid() {
        let cli = parse_args(&["glance", "--size", "invalid"]);
        assert!(cli.parse_screen_size().is_err());
    }

    #[test]
    fn test_parse_output_format() {
        let cli = parse_args(&["glance", "--output", "json"]);
        assert_eq!(cli.parse_output_format().unwrap(), OutputFormat::Json);

        let cli = parse_args(&["glance", "--output", "text"]);
        assert_eq!(cli.parse_output_format().unwrap(), OutputFormat::Text);

        let cli = parse_args(&["glance", "--output", "frames"]);
        assert_eq!(cli.parse_output_format().unwrap(), OutputFormat::Frames);
    }

    #[test]
    fn test_validate_headless_requires_events_or_script() {
        let cli = parse_args(&["glance", "--headless", "--mock-db"]);
        let result = cli.validate_headless();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("requires --events or --script"));
    }

    #[test]
    fn test_validate_headless_with_events() {
        let cli = parse_args(&["glance", "--headless", "--mock-db", "--events", "key:esc"]);
        assert!(cli.validate_headless().is_ok());
    }

    #[test]
    fn test_validate_headless_with_script() {
        let cli = parse_args(&["glance", "--headless", "--mock-db", "--script", "-"]);
        assert!(cli.validate_headless().is_ok());
    }

    #[test]
    fn test_headless_output_file() {
        let cli = parse_args(&[
            "glance",
            "--headless",
            "--mock-db",
            "--events",
            "key:esc",
            "--output-file",
            "result.json",
        ]);
        assert_eq!(cli.output_file, Some(PathBuf::from("result.json")));
    }

    #[test]
    fn test_headless_fail_fast() {
        let cli = parse_args(&[
            "glance",
            "--headless",
            "--mock-db",
            "--events",
            "key:esc",
            "--fail-fast",
        ]);
        assert!(cli.fail_fast);
    }

    #[test]
    fn test_headless_seed_file() {
        let cli = parse_args(&[
            "glance",
            "--headless",
            "--mock-db",
            "--events",
            "key:esc",
            "--seed",
            "tests/fixtures/seed.sql",
        ]);
        assert_eq!(cli.seed, Some(PathBuf::from("tests/fixtures/seed.sql")));
    }

    #[test]
    fn test_headless_llm_override() {
        let cli = parse_args(&[
            "glance",
            "--headless",
            "--mock-db",
            "--events",
            "key:esc",
            "--llm",
            "anthropic",
        ]);
        assert_eq!(cli.llm, Some("anthropic".to_string()));
    }
}
