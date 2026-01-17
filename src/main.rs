//! Glance - A lightweight, AI-first database viewer.

mod app;
mod cli;
mod config;
mod db;
mod error;
mod llm;
mod safety;
mod tui;

use cli::Cli;
use config::{Config, ConnectionConfig};
use error::{GlanceError, Result};
use llm::LlmProvider;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    if let Err(e) = run().await {
        error!("{}: {}", e.category(), e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse_args();

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

            // Determine LLM provider from config
            let llm_provider = config
                .llm
                .provider
                .parse::<LlmProvider>()
                .unwrap_or(LlmProvider::OpenAi);

            // Run with full orchestrator integration
            tui::run_async(conn, llm_provider).await?;
        }
        None => {
            warn!("No database connection configured. Running in limited mode.");
            // Run without orchestrator (limited functionality)
            tui::run(None)?;
        }
    }

    Ok(())
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
