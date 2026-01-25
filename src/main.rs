//! Glance - A lightweight, AI-first database viewer.

mod app;
mod cli;
mod commands;
mod config;
mod connection;
mod db;
mod error;
mod llm;
mod logging;
mod persistence;
mod query;
mod safety;
mod tui;

use cli::Cli;
use config::{Config, ConnectionConfig};
use error::{GlanceError, Result};
use llm::LlmProvider;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() {
    // Load .env file if present (before anything else)
    if let Err(e) = dotenvy::dotenv() {
        // Not an error if .env doesn't exist, only if it exists but can't be read
        if e.not_found() {
            // .env file not found, that's fine
        } else {
            eprintln!("Warning: Failed to load .env file: {}", e);
        }
    }

    // Parse CLI early to determine mode
    let cli = Cli::parse_args();

    // Initialize logging - file-based for TUI mode, stderr for headless
    if cli.is_headless() {
        logging::init_stderr_logging();
    } else {
        logging::init_file_logging();
    }

    if let Err(e) = run(cli).await {
        error!("{}: {}", e.category(), e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {

    // Handle headless mode
    if cli.is_headless() {
        let exit_code = tui::headless::run_headless(&cli).await?;
        if exit_code != 0 {
            std::process::exit(exit_code);
        }
        return Ok(());
    }

    // Load configuration file
    let config_path = cli.config_path();
    info!("Loading config from: {}", config_path.display());
    let config = Config::load_from_file(&config_path)?;

    // Build connection config with precedence:
    // 1. CLI arguments (highest)
    // 2. Named connection from config
    // 3. Default connection from config
    // 4. Environment variables
    let connection = resolve_connection(&cli, &config)?;

    match connection {
        Some(ref conn) => {
            info!("Connection: {}", conn.display_string());

            // Validate and parse LLM provider from config
            let llm_provider = validate_llm_provider(&config.llm.provider, &config_path)?;

            // Run with full orchestrator integration
            tui::run_async(conn, &config.ui, llm_provider).await?;
        }
        None => {
            warn!("No database connection configured. Running in limited mode.");
            // Run without orchestrator (limited functionality)
            tui::run(None, &config.ui)?;
        }
    }

    Ok(())
}

/// Validates and parses the LLM provider from the configuration.
///
/// Returns the parsed provider or an error with helpful message including
/// valid options and the config file path.
fn validate_llm_provider(provider_str: &str, config_path: &std::path::Path) -> Result<LlmProvider> {
    if provider_str.is_empty() {
        return Ok(LlmProvider::OpenAi);
    }

    provider_str.parse::<LlmProvider>().map_err(|_| {
        GlanceError::config(format!(
            "Invalid LLM provider '{}'. Valid options: openai, anthropic, ollama\n\n\
             Check your configuration file at {}",
            provider_str,
            config_path.display()
        ))
    })
}

/// Resolves the final connection configuration from CLI args, config file, and environment.
fn resolve_connection(cli: &Cli, config: &Config) -> Result<Option<ConnectionConfig>> {
    // Start with CLI connection config if provided
    let mut connection = cli.to_connection_config()?;

    // If no CLI connection, try named connection from config
    if connection.is_none() {
        if let Some(name) = cli.connection_name() {
            connection = config.get_connection(Some(name)).cloned();
            if connection.is_none() {
                return Err(GlanceError::config(format!(
                    "Connection '{}' not found in config file",
                    name
                )));
            }
        }
    }

    // If still no connection, try default from config
    if connection.is_none() {
        connection = config.get_connection(None).cloned();
    }

    // Apply environment variable defaults
    if let Some(ref mut conn) = connection {
        conn.apply_env_defaults();
    }

    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_llm_provider_empty_defaults_to_openai() {
        let path = std::path::Path::new("/fake/config.toml");
        let result = validate_llm_provider("", path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), LlmProvider::OpenAi);
    }

    #[test]
    fn test_validate_llm_provider_valid_openai() {
        let path = std::path::Path::new("/fake/config.toml");
        let result = validate_llm_provider("openai", path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), LlmProvider::OpenAi);
    }

    #[test]
    fn test_validate_llm_provider_valid_anthropic() {
        let path = std::path::Path::new("/fake/config.toml");
        let result = validate_llm_provider("anthropic", path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), LlmProvider::Anthropic);
    }

    #[test]
    fn test_validate_llm_provider_valid_ollama() {
        let path = std::path::Path::new("/fake/config.toml");
        let result = validate_llm_provider("ollama", path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), LlmProvider::Ollama);
    }

    #[test]
    fn test_validate_llm_provider_invalid_includes_options() {
        let path = std::path::Path::new("/fake/config.toml");
        let result = validate_llm_provider("opanai", path);
        assert!(result.is_err());

        let error = result.unwrap_err();
        let error_msg = error.to_string();

        // Error should include the invalid provider
        assert!(error_msg.contains("opanai"));

        // Error should list valid options
        assert!(error_msg.contains("openai"));
        assert!(error_msg.contains("anthropic"));
        assert!(error_msg.contains("ollama"));

        // Error should include the config path
        assert!(error_msg.contains("/fake/config.toml"));
    }

    #[test]
    fn test_validate_llm_provider_invalid_case_sensitive() {
        let path = std::path::Path::new("/test/config.toml");
        // LLM provider parsing might be case-insensitive, but invalid values should still error
        let result = validate_llm_provider("InvalidProvider", path);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_llm_provider_error_category() {
        let path = std::path::Path::new("/test/config.toml");
        let result = validate_llm_provider("bad_provider", path);
        assert!(result.is_err());

        let error = result.unwrap_err();
        // Should be a config error
        assert_eq!(error.category(), "Configuration Error");
    }
}
