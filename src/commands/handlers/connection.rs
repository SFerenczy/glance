//! Connection command handlers (/connections, /connect, /conn).

use std::sync::Arc;

use super::{CommandContext, CommandResult};
use crate::commands::router::{ConnectionAddArgs, ConnectionEditArgs};
use crate::config::ConnectionConfig;
use crate::db::{DatabaseBackend, DatabaseClient, Schema};
use crate::persistence::{self, ConnectionProfile, StateDb};

/// Handle /connections command - list saved connections.
pub async fn handle_connections_list(ctx: &CommandContext<'_>) -> CommandResult {
    let state_db = match ctx.state_db {
        Some(db) => db,
        None => {
            return CommandResult::error("State database not available.");
        }
    };

    let connections = match persistence::connections::list_connections(state_db.pool()).await {
        Ok(c) => c,
        Err(e) => return CommandResult::error(e.to_string()),
    };

    if connections.is_empty() {
        return CommandResult::system("No saved connections. Use /conn add <name> to add one.");
    }

    let mut output = String::from("Saved connections:\n");
    for conn in &connections {
        let last_used = conn.last_used_at.as_deref().unwrap_or("never");
        output.push_str(&format!(
            "  • {} - {} @ {}:{} (last used: {})\n",
            conn.name,
            conn.database,
            conn.redacted_host(),
            conn.port,
            last_used
        ));
    }

    CommandResult::system(output.trim_end().to_string())
}

/// Handle /connect <name> command - switch to a saved connection.
/// Returns the new database client, schema, and messages.
#[allow(dead_code)]
pub async fn handle_connect(
    name: &str,
    state_db: &Arc<StateDb>,
) -> Result<ConnectResult, CommandResult> {
    if name.is_empty() {
        return Err(CommandResult::error("Usage: /connect <name>"));
    }

    let profile = match persistence::connections::get_connection(state_db.pool(), name).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return Err(CommandResult::error(format!(
                "Connection '{}' not found.",
                name
            )));
        }
        Err(e) => return Err(CommandResult::error(e.to_string())),
    };

    let password = match persistence::connections::get_connection_password(
        state_db.pool(),
        name,
        state_db.secrets(),
    )
    .await
    {
        Ok(p) => p,
        Err(e) => return Err(CommandResult::error(e.to_string())),
    };

    let config = ConnectionConfig {
        backend: profile.backend,
        host: profile.host.clone(),
        port: profile.port,
        database: Some(profile.database.clone()),
        user: profile.username.clone(),
        password: password.clone(),
        sslmode: profile.sslmode.clone(),
        extras: profile.extras.clone(),
    };

    tracing::debug!(
        "Connecting with: host={:?}, port={}, db={:?}, user={:?}, has_password={}",
        config.host,
        config.port,
        config.database,
        config.user,
        config.password.is_some()
    );

    let db = match crate::db::connect(&config).await {
        Ok(db) => db,
        Err(e) => {
            return Err(CommandResult::error(format!("Failed to connect: {}", e)));
        }
    };

    let schema = match db.introspect_schema().await {
        Ok(s) => s,
        Err(e) => {
            return Err(CommandResult::error(format!(
                "Failed to introspect schema: {}",
                e
            )));
        }
    };

    if let Err(e) = persistence::connections::touch_connection(state_db.pool(), name).await {
        tracing::warn!("Failed to update last_used_at: {}", e);
    }

    Ok(ConnectResult {
        db,
        schema,
        name: name.to_string(),
        database: profile.database,
    })
}

/// Result of a successful connection switch.
#[allow(dead_code)]
pub struct ConnectResult {
    /// The new database client.
    pub db: Box<dyn DatabaseClient>,
    /// The database schema.
    pub schema: Schema,
    /// Connection name.
    pub name: String,
    /// Database name.
    pub database: String,
}

/// Handle /conn add command with inline parameters.
pub async fn handle_conn_add(args: &ConnectionAddArgs, state_db: &Arc<StateDb>) -> CommandResult {
    if args.name.is_empty() {
        return CommandResult::system(
            "To add a connection, provide details in format:\n\
             /conn add <name> [backend=postgres] host=<host> port=<port> database=<db> user=<user> [password=<pwd>] [sslmode=<mode>]\n\n\
             Example: /conn add mydb host=localhost port=5432 database=mydb user=postgres"
        );
    }

    if args.database.is_none() {
        return CommandResult::error("Connection name and database are required.");
    }

    let db_name = args.database.clone().unwrap();

    // Parse backend, defaulting to postgres
    let backend = match &args.backend {
        Some(b) => match DatabaseBackend::parse(b) {
            Some(backend) => backend,
            None => {
                return CommandResult::error(format!(
                    "Unknown backend '{}'. Supported: postgres",
                    b
                ));
            }
        },
        None => DatabaseBackend::default(),
    };

    // Use backend-specific default port if not specified
    let port = if args.port == 5432 && args.backend.is_some() {
        backend.default_port()
    } else {
        args.port
    };

    if args.test {
        let test_config = ConnectionConfig {
            backend,
            host: args.host.clone(),
            port,
            database: args.database.clone(),
            user: args.user.clone(),
            password: args.password.clone(),
            sslmode: args.sslmode.clone(),
            extras: None,
        };

        match crate::db::connect(&test_config).await {
            Ok(db) => {
                let _ = db.close().await;
            }
            Err(e) => {
                return CommandResult::error(format!(
                    "Connection test failed: {}. Connection not saved.",
                    e
                ));
            }
        }
    }

    let profile = ConnectionProfile {
        name: args.name.clone(),
        backend,
        database: db_name,
        host: args.host.clone(),
        port,
        username: args.user.clone(),
        sslmode: args.sslmode.clone(),
        extras: None,
        password_storage: persistence::connections::PasswordStorage::None,
        created_at: String::new(),
        updated_at: String::new(),
        last_used_at: None,
    };

    match persistence::connections::create_connection(
        state_db.pool(),
        &profile,
        args.password.as_deref(),
        state_db.secrets(),
    )
    .await
    {
        Ok(()) => {
            let test_msg = if args.test {
                " (connection tested successfully)"
            } else {
                ""
            };
            CommandResult::system(format!(
                "Connection '{}' saved{}. Use /connect {} to use it.",
                args.name, test_msg, args.name
            ))
        }
        Err(e) => CommandResult::error(e.to_string()),
    }
}

/// Handle /conn edit command with inline parameters.
pub async fn handle_conn_edit(args: &ConnectionEditArgs, state_db: &Arc<StateDb>) -> CommandResult {
    if args.name.is_empty() {
        return CommandResult::error("Connection name is required.");
    }

    // Check if any fields are being updated
    let has_updates = args.backend.is_some()
        || args.host.is_some()
        || args.port.is_some()
        || args.database.is_some()
        || args.user.is_some()
        || args.password.is_some()
        || args.sslmode.is_some();

    if !has_updates {
        return CommandResult::system(format!(
            "To edit connection '{}', use:\n\
             /conn edit {} <field>=<value> ...\n\n\
             Fields: backend, host, port, database, user, password, sslmode",
            args.name, args.name
        ));
    }

    let existing = match persistence::connections::get_connection(state_db.pool(), &args.name).await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            return CommandResult::error(format!("Connection '{}' not found.", args.name));
        }
        Err(e) => return CommandResult::error(e.to_string()),
    };

    // Parse backend if provided
    let backend = match &args.backend {
        Some(b) => match DatabaseBackend::parse(b) {
            Some(backend) => backend,
            None => {
                return CommandResult::error(format!(
                    "Unknown backend '{}'. Supported: postgres",
                    b
                ));
            }
        },
        None => existing.backend,
    };

    let updated_profile = ConnectionProfile {
        name: args.name.clone(),
        backend,
        database: args.database.clone().unwrap_or(existing.database),
        host: args.host.clone().or(existing.host),
        port: args.port.unwrap_or(existing.port),
        username: args.user.clone().or(existing.username),
        sslmode: args.sslmode.clone().or(existing.sslmode),
        extras: existing.extras,
        password_storage: existing.password_storage,
        created_at: existing.created_at,
        updated_at: String::new(),
        last_used_at: existing.last_used_at,
    };

    match persistence::connections::update_connection(
        state_db.pool(),
        &updated_profile,
        args.password.as_deref(),
        state_db.secrets(),
    )
    .await
    {
        Ok(()) => CommandResult::system(format!("Connection '{}' updated.", args.name)),
        Err(e) => CommandResult::error(e.to_string()),
    }
}

/// Handle /conn delete command.
pub async fn handle_conn_delete(name: &str, state_db: &Arc<StateDb>) -> CommandResult {
    if name.is_empty() {
        return CommandResult::error("Usage: /conn delete <name>");
    }

    match persistence::connections::delete_connection(state_db.pool(), name, state_db.secrets())
        .await
    {
        Ok(()) => CommandResult::system(format!("Connection '{}' deleted.", name)),
        Err(e) => CommandResult::error(e.to_string()),
    }
}
