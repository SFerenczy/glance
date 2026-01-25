//! Command parsing and routing for Glance.
//!
//! Parses user input into structured commands that can be dispatched to handlers.

use super::tokenizer::{tokenize, Token};

/// Parses a duration string like "7d", "12h", "15m" into days as a float.
///
/// Supports:
/// - "Nd" for days (e.g., "7d" = 7.0 days)
/// - "Nh" for hours (e.g., "12h" = 0.5 days)
/// - "Nm" for minutes (e.g., "30m" ≈ 0.021 days)
/// - Plain numbers are interpreted as days
fn parse_duration_to_days(s: &str) -> Option<i64> {
    let s = s.trim();

    if let Ok(days) = s.parse::<i64>() {
        // Plain number, interpret as days
        return Some(days);
    }

    // Try to parse with suffix
    if s.len() < 2 {
        return None;
    }

    let (num_part, suffix) = s.split_at(s.len() - 1);
    let num: f64 = num_part.parse().ok()?;

    match suffix {
        "d" | "D" => Some(num.ceil() as i64),
        "h" | "H" => Some((num / 24.0).ceil() as i64),
        "m" | "M" => Some((num / 1440.0).ceil() as i64), // 1440 minutes in a day
        _ => None,
    }
}

/// Arguments for connection add command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionAddArgs {
    /// Connection name.
    pub name: String,
    /// Database backend (postgres, mysql, etc.).
    pub backend: Option<String>,
    /// Host address.
    pub host: Option<String>,
    /// Port number.
    pub port: u16,
    /// Database name.
    pub database: Option<String>,
    /// Username.
    pub user: Option<String>,
    /// Password.
    pub password: Option<String>,
    /// SSL mode.
    pub sslmode: Option<String>,
    /// Extra connection parameters as key-value pairs.
    pub extras: Option<serde_json::Value>,
    /// Whether to test the connection before saving.
    pub test: bool,
}

/// Arguments for connection delete command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDeleteArgs {
    /// Connection name to delete.
    pub name: String,
    /// Whether deletion is confirmed.
    pub confirmed: bool,
}

/// Arguments for connection edit command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionEditArgs {
    /// Connection name.
    pub name: String,
    /// Database backend (if updating).
    pub backend: Option<String>,
    /// Host address (if updating).
    pub host: Option<String>,
    /// Port number (if updating).
    pub port: Option<u16>,
    /// Database name (if updating).
    pub database: Option<String>,
    /// Username (if updating).
    pub user: Option<String>,
    /// Password (if updating).
    pub password: Option<String>,
    /// SSL mode (if updating).
    pub sslmode: Option<String>,
    /// Extra connection parameters (if updating).
    pub extras: Option<serde_json::Value>,
    /// Whether to test the connection after updating.
    pub test: bool,
}

/// Arguments for history command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryArgs {
    /// Filter by connection name.
    pub connection: Option<String>,
    /// Filter by text search.
    pub text: Option<String>,
    /// Limit number of results.
    pub limit: Option<i64>,
    /// Filter by days since.
    pub since_days: Option<i64>,
    /// Whether clear operation is confirmed.
    pub confirmed: bool,
}

/// Arguments for save query command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveQueryArgs {
    /// Query name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Tags for the query.
    pub tags: Vec<String>,
}

/// Arguments for queries list command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueriesListArgs {
    /// Filter by tag.
    pub tag: Option<String>,
    /// Filter by text search.
    pub text: Option<String>,
    /// Filter by connection name.
    pub connection: Option<String>,
    /// Show all connections.
    pub all: bool,
}

/// Arguments for query delete command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDeleteArgs {
    /// Query name to delete.
    pub name: String,
    /// Whether deletion is confirmed.
    pub confirmed: bool,
}

/// Arguments for LLM provider command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmProviderArgs {
    /// Show current provider.
    Show,
    /// Set provider to a new value.
    Set(String),
}

/// Arguments for LLM model command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmModelArgs {
    /// Show current model.
    Show,
    /// Set model to a new value.
    Set(String),
}

/// Arguments for LLM key command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmKeyArgs {
    /// Show current key status.
    Show,
    /// Set key to a new value.
    Set(String),
}

/// Parsed command with arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Execute raw SQL directly.
    Sql(String),
    /// Clear chat history and LLM context.
    Clear,
    /// Display database schema.
    Schema,
    /// Show help message.
    Help,
    /// Exit the application.
    Quit,
    /// Toggle vim-style navigation mode.
    Vim,
    /// Toggle row numbers in result tables.
    RowNumbers,
    /// List saved connections.
    ConnectionsList,
    /// Switch to a saved connection.
    Connect(String),
    /// Add a new connection.
    ConnectionAdd(ConnectionAddArgs),
    /// Edit an existing connection.
    ConnectionEdit(ConnectionEditArgs),
    /// Delete a connection.
    ConnectionDelete(ConnectionDeleteArgs),
    /// Show query history.
    History(HistoryArgs),
    /// Clear query history (requires --confirm flag).
    HistoryClear { confirmed: bool },
    /// Save the last executed query.
    SaveQuery(SaveQueryArgs),
    /// List saved queries.
    QueriesList(QueriesListArgs),
    /// Load a saved query.
    UseQuery(String),
    /// Delete a saved query.
    QueryDelete(QueryDeleteArgs),
    /// LLM provider command.
    LlmProvider(LlmProviderArgs),
    /// LLM model command.
    LlmModel(LlmModelArgs),
    /// LLM key command.
    LlmKey(LlmKeyArgs),
    /// Show LLM settings.
    LlmSettings,
    /// Refresh the database schema.
    RefreshSchema,
    /// Natural language query (not a slash command).
    NaturalLanguage(String),
    /// Unknown command.
    Unknown(String),
}

/// Command router for parsing user input.
pub struct CommandRouter;

impl CommandRouter {
    /// Parse user input into a Command.
    pub fn parse(input: &str) -> Command {
        let input = input.trim();

        if input.is_empty() {
            return Command::NaturalLanguage(String::new());
        }

        if !input.starts_with('/') {
            return Command::NaturalLanguage(input.to_string());
        }

        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let command = parts[0].to_lowercase();
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match command.as_str() {
            "/sql" => {
                if args.is_empty() {
                    Command::Sql(String::new())
                } else {
                    Command::Sql(args.to_string())
                }
            }
            "/clear" => Command::Clear,
            "/schema" => Command::Schema,
            "/quit" | "/exit" => Command::Quit,
            "/vim" => Command::Vim,
            "/rownumbers" => Command::RowNumbers,
            "/help" => Command::Help,
            "/connections" => Command::ConnectionsList,
            "/connect" => Command::Connect(args.to_string()),
            "/conn" => Self::parse_conn_command(args),
            "/history" => Self::parse_history_command(args),
            "/savequery" => Self::parse_savequery_command(args),
            "/queries" => Self::parse_queries_command(args),
            "/usequery" => Command::UseQuery(args.to_string()),
            "/query" => Self::parse_query_command(args),
            "/llm" => Self::parse_llm_command(args),
            "/refresh" => Self::parse_refresh_command(args),
            _ => Command::Unknown(command),
        }
    }

    /// Parse /refresh subcommands.
    fn parse_refresh_command(args: &str) -> Command {
        let subcommand = args.split_whitespace().next().unwrap_or("").to_lowercase();
        match subcommand.as_str() {
            "schema" | "" => Command::RefreshSchema,
            _ => Command::Unknown("/refresh".to_string()),
        }
    }

    /// Parse /conn subcommands.
    fn parse_conn_command(args: &str) -> Command {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let subcommand = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        let rest = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match subcommand.as_str() {
            "add" => {
                if rest.is_empty() {
                    return Command::ConnectionAdd(ConnectionAddArgs {
                        name: String::new(),
                        backend: None,
                        host: None,
                        port: 5432,
                        database: None,
                        user: None,
                        password: None,
                        sslmode: None,
                        extras: None,
                        test: false,
                    });
                }
                Self::parse_conn_add_args(rest)
            }
            "edit" => {
                if rest.is_empty() {
                    return Command::ConnectionEdit(ConnectionEditArgs {
                        name: String::new(),
                        backend: None,
                        host: None,
                        port: None,
                        database: None,
                        user: None,
                        password: None,
                        sslmode: None,
                        extras: None,
                        test: false,
                    });
                }
                Self::parse_conn_edit_args(rest)
            }
            "delete" => Self::parse_conn_delete_args(rest),
            _ if !subcommand.is_empty() && subcommand.contains('=') => {
                Self::parse_conn_add_args(args)
            }
            _ => Command::Unknown("/conn".to_string()),
        }
    }

    /// Parse connection add arguments using the tokenizer.
    fn parse_conn_add_args(args: &str) -> Command {
        let mut name = String::new();
        let mut backend = None;
        let mut host = None;
        let mut port = 5432u16;
        let mut database = None;
        let mut user = None;
        let mut password = None;
        let mut sslmode = None;
        let mut test = false;
        let mut extras_map: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::new();

        let tokens = tokenize(args);

        for token in tokens {
            match token {
                Token::KeyValue { key, value } => match key.as_str() {
                    "backend" => backend = Some(value),
                    "host" => host = Some(value),
                    "port" => port = value.parse().unwrap_or(5432),
                    "database" | "db" => database = Some(value),
                    "user" => user = Some(value),
                    "password" | "pwd" => password = Some(value),
                    "sslmode" => sslmode = Some(value),
                    // Collect unknown key-value pairs as extras
                    _ => {
                        extras_map.insert(key, serde_json::Value::String(value));
                    }
                },
                Token::LongFlag(flag) if flag == "test" => test = true,
                Token::ShortFlag('t') => test = true,
                Token::Word(word) if name.is_empty() => name = word,
                _ => {}
            }
        }

        let extras = if extras_map.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(
                extras_map
                    .into_iter()
                    .map(|(k, v)| (k, v))
                    .collect(),
            ))
        };

        Command::ConnectionAdd(ConnectionAddArgs {
            name,
            backend,
            host,
            port,
            database,
            user,
            password,
            sslmode,
            extras,
            test,
        })
    }

    /// Parse connection delete arguments.
    fn parse_conn_delete_args(args: &str) -> Command {
        let mut name = String::new();
        let mut confirmed = false;

        let tokens = tokenize(args);

        for token in tokens {
            match token {
                Token::LongFlag(flag) if flag == "confirm" => confirmed = true,
                Token::ShortFlag('y') => confirmed = true,
                Token::Word(word) if name.is_empty() => name = word,
                _ => {}
            }
        }

        Command::ConnectionDelete(ConnectionDeleteArgs { name, confirmed })
    }

    /// Parse connection edit arguments using the tokenizer.
    fn parse_conn_edit_args(args: &str) -> Command {
        let mut name = String::new();
        let mut backend = None;
        let mut host = None;
        let mut port = None;
        let mut database = None;
        let mut user = None;
        let mut password = None;
        let mut sslmode = None;
        let mut test = false;
        let mut extras_map: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::new();

        let tokens = tokenize(args);

        for token in tokens {
            match token {
                Token::KeyValue { key, value } => match key.as_str() {
                    "backend" => backend = Some(value),
                    "host" => host = Some(value),
                    "port" => port = value.parse().ok(),
                    "database" | "db" => database = Some(value),
                    "user" => user = Some(value),
                    "password" | "pwd" => password = Some(value),
                    "sslmode" => sslmode = Some(value),
                    // Collect unknown key-value pairs as extras
                    _ => {
                        extras_map.insert(key, serde_json::Value::String(value));
                    }
                },
                Token::LongFlag(flag) if flag == "test" => test = true,
                Token::ShortFlag('t') => test = true,
                Token::Word(word) if name.is_empty() => name = word,
                _ => {}
            }
        }

        let extras = if extras_map.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(
                extras_map
                    .into_iter()
                    .map(|(k, v)| (k, v))
                    .collect(),
            ))
        };

        Command::ConnectionEdit(ConnectionEditArgs {
            name,
            backend,
            host,
            port,
            database,
            user,
            password,
            sslmode,
            extras,
            test,
        })
    }

    /// Parse /history command arguments using the tokenizer.
    fn parse_history_command(args: &str) -> Command {
        let trimmed = args.trim();
        if trimmed == "clear" {
            return Command::HistoryClear { confirmed: false };
        }
        if trimmed == "clear --confirm" || trimmed == "clear -y" {
            return Command::HistoryClear { confirmed: true };
        }

        let mut history_args = HistoryArgs::default();
        let tokens = tokenize(args);
        let mut iter = tokens.iter().peekable();

        while let Some(token) = iter.next() {
            match token {
                Token::LongFlag(flag) => {
                    // Look for the next token as the value
                    if let Some(Token::Word(val)) = iter.peek() {
                        match flag.as_str() {
                            "conn" => {
                                history_args.connection = Some(val.clone());
                                iter.next();
                            }
                            "text" => {
                                history_args.text = Some(val.clone());
                                iter.next();
                            }
                            "limit" => {
                                history_args.limit = val.parse().ok();
                                iter.next();
                            }
                            "since" => {
                                history_args.since_days = val.parse().ok();
                                iter.next();
                            }
                            _ => {}
                        }
                    }
                }
                Token::KeyValue { key, value } => match key.as_str() {
                    "conn" => history_args.connection = Some(value.clone()),
                    "text" => history_args.text = Some(value.clone()),
                    "limit" => history_args.limit = value.parse().ok(),
                    "since" => history_args.since_days = parse_duration_to_days(&value),
                    _ => {}
                },
                _ => {}
            }
        }

        Command::History(history_args)
    }

    /// Parse /savequery command arguments.
    fn parse_savequery_command(args: &str) -> Command {
        let mut name = String::new();
        let mut description = None;
        let mut tags = Vec::new();
        let tokens = tokenize(args);

        for token in tokens {
            match token {
                Token::Word(word) => {
                    if word.starts_with('#') {
                        tags.push(word.trim_start_matches('#').to_string());
                    } else if name.is_empty() {
                        name = word;
                    }
                }
                Token::KeyValue { key, value } => {
                    if key == "description" || key == "desc" {
                        description = Some(value);
                    }
                }
                _ => {}
            }
        }

        Command::SaveQuery(SaveQueryArgs {
            name,
            description,
            tags,
        })
    }

    /// Parse /queries command arguments using the tokenizer.
    fn parse_queries_command(args: &str) -> Command {
        let mut queries_args = QueriesListArgs::default();
        let tokens = tokenize(args);
        let mut iter = tokens.iter().peekable();

        while let Some(token) = iter.next() {
            match token {
                Token::LongFlag(flag) => match flag.as_str() {
                    "all" => queries_args.all = true,
                    "tag" | "text" | "conn" => {
                        if let Some(Token::Word(val)) = iter.peek() {
                            match flag.as_str() {
                                "tag" => {
                                    queries_args.tag =
                                        Some(val.trim_start_matches('#').to_string());
                                }
                                "text" => queries_args.text = Some(val.clone()),
                                "conn" => queries_args.connection = Some(val.clone()),
                                _ => {}
                            }
                            iter.next();
                        }
                    }
                    _ => {}
                },
                Token::KeyValue { key, value } => match key.as_str() {
                    "tag" => queries_args.tag = Some(value.trim_start_matches('#').to_string()),
                    "text" => queries_args.text = Some(value.clone()),
                    "conn" => queries_args.connection = Some(value.clone()),
                    _ => {}
                },
                _ => {}
            }
        }

        Command::QueriesList(queries_args)
    }

    /// Parse /query subcommands.
    fn parse_query_command(args: &str) -> Command {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let subcommand = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        let rest = parts.get(1).map(|s| s.trim()).unwrap_or("");

        if subcommand == "delete" {
            let tokens = tokenize(rest);
            let mut name = String::new();
            let mut confirmed = false;

            for token in tokens {
                match token {
                    Token::Word(word) => {
                        if name.is_empty() {
                            name = word;
                        }
                    }
                    Token::LongFlag(flag) if flag == "confirm" => {
                        confirmed = true;
                    }
                    _ => {}
                }
            }

            Command::QueryDelete(QueryDeleteArgs { name, confirmed })
        } else {
            Command::Unknown("/query".to_string())
        }
    }

    /// Parse /llm subcommands.
    fn parse_llm_command(args: &str) -> Command {
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        let subcommand = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        let value = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match subcommand.as_str() {
            "provider" => {
                if value.is_empty() {
                    Command::LlmProvider(LlmProviderArgs::Show)
                } else {
                    Command::LlmProvider(LlmProviderArgs::Set(value.to_string()))
                }
            }
            "model" => {
                if value.is_empty() {
                    Command::LlmModel(LlmModelArgs::Show)
                } else {
                    Command::LlmModel(LlmModelArgs::Set(value.to_string()))
                }
            }
            "key" => {
                if value.is_empty() {
                    Command::LlmKey(LlmKeyArgs::Show)
                } else {
                    Command::LlmKey(LlmKeyArgs::Set(value.to_string()))
                }
            }
            _ => Command::LlmSettings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_input() {
        assert!(matches!(
            CommandRouter::parse(""),
            Command::NaturalLanguage(s) if s.is_empty()
        ));
    }

    #[test]
    fn test_parse_natural_language() {
        assert!(matches!(
            CommandRouter::parse("show me all users"),
            Command::NaturalLanguage(s) if s == "show me all users"
        ));
    }

    #[test]
    fn test_parse_sql_command() {
        assert!(matches!(
            CommandRouter::parse("/sql SELECT 1"),
            Command::Sql(s) if s == "SELECT 1"
        ));
    }

    #[test]
    fn test_parse_sql_command_empty() {
        assert!(matches!(
            CommandRouter::parse("/sql"),
            Command::Sql(s) if s.is_empty()
        ));
    }

    #[test]
    fn test_parse_simple_commands() {
        assert!(matches!(CommandRouter::parse("/clear"), Command::Clear));
        assert!(matches!(CommandRouter::parse("/schema"), Command::Schema));
        assert!(matches!(CommandRouter::parse("/quit"), Command::Quit));
        assert!(matches!(CommandRouter::parse("/exit"), Command::Quit));
        assert!(matches!(CommandRouter::parse("/vim"), Command::Vim));
        assert!(matches!(CommandRouter::parse("/help"), Command::Help));
        assert!(matches!(
            CommandRouter::parse("/connections"),
            Command::ConnectionsList
        ));
    }

    #[test]
    fn test_parse_connect_command() {
        assert!(matches!(
            CommandRouter::parse("/connect prod"),
            Command::Connect(s) if s == "prod"
        ));
    }

    #[test]
    fn test_parse_conn_add() {
        let cmd = CommandRouter::parse("/conn add mydb host=localhost database=test");
        if let Command::ConnectionAdd(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert_eq!(args.host, Some("localhost".to_string()));
            assert_eq!(args.database, Some("test".to_string()));
            assert!(!args.test);
        } else {
            panic!("Expected ConnectionAdd");
        }
    }

    #[test]
    fn test_parse_conn_add_with_test() {
        let cmd = CommandRouter::parse("/conn add mydb host=localhost database=test --test");
        if let Command::ConnectionAdd(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert!(args.test);
        } else {
            panic!("Expected ConnectionAdd");
        }
    }

    #[test]
    fn test_parse_conn_edit() {
        let cmd = CommandRouter::parse("/conn edit mydb host=newhost port=5433");
        if let Command::ConnectionEdit(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert_eq!(args.host, Some("newhost".to_string()));
            assert_eq!(args.port, Some(5433));
        } else {
            panic!("Expected ConnectionEdit");
        }
    }

    #[test]
    fn test_parse_conn_delete() {
        let cmd = CommandRouter::parse("/conn delete mydb");
        if let Command::ConnectionDelete(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert!(!args.confirmed);
        } else {
            panic!("Expected ConnectionDelete");
        }
    }

    #[test]
    fn test_parse_conn_delete_with_confirm() {
        let cmd = CommandRouter::parse("/conn delete mydb --confirm");
        if let Command::ConnectionDelete(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert!(args.confirmed);
        } else {
            panic!("Expected ConnectionDelete");
        }
    }

    #[test]
    fn test_parse_conn_delete_with_short_flag() {
        let cmd = CommandRouter::parse("/conn delete mydb -y");
        if let Command::ConnectionDelete(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert!(args.confirmed);
        } else {
            panic!("Expected ConnectionDelete");
        }
    }

    #[test]
    fn test_parse_history() {
        let cmd = CommandRouter::parse("/history --conn prod --limit 10");
        if let Command::History(args) = cmd {
            assert_eq!(args.connection, Some("prod".to_string()));
            assert_eq!(args.limit, Some(10));
        } else {
            panic!("Expected History");
        }
    }

    #[test]
    fn test_parse_history_clear() {
        assert!(matches!(
            CommandRouter::parse("/history clear"),
            Command::HistoryClear { .. }
        ));
    }

    #[test]
    fn test_parse_savequery() {
        let cmd = CommandRouter::parse("/savequery myquery #tag1 #tag2");
        if let Command::SaveQuery(args) = cmd {
            assert_eq!(args.name, "myquery");
            assert_eq!(args.tags, vec!["tag1", "tag2"]);
        } else {
            panic!("Expected SaveQuery");
        }
    }

    #[test]
    fn test_parse_queries_list() {
        let cmd = CommandRouter::parse("/queries --tag reports --all");
        if let Command::QueriesList(args) = cmd {
            assert_eq!(args.tag, Some("reports".to_string()));
            assert!(args.all);
        } else {
            panic!("Expected QueriesList");
        }
    }

    #[test]
    fn test_parse_usequery() {
        assert!(matches!(
            CommandRouter::parse("/usequery myquery"),
            Command::UseQuery(s) if s == "myquery"
        ));
    }

    #[test]
    fn test_parse_query_delete() {
        assert!(matches!(
            CommandRouter::parse("/query delete myquery"),
            Command::QueryDelete(QueryDeleteArgs { name, confirmed: false }) if name == "myquery"
        ));
    }

    #[test]
    fn test_parse_llm_provider_show() {
        assert!(matches!(
            CommandRouter::parse("/llm provider"),
            Command::LlmProvider(LlmProviderArgs::Show)
        ));
    }

    #[test]
    fn test_parse_llm_provider_set() {
        assert!(matches!(
            CommandRouter::parse("/llm provider anthropic"),
            Command::LlmProvider(LlmProviderArgs::Set(s)) if s == "anthropic"
        ));
    }

    #[test]
    fn test_parse_llm_model() {
        assert!(matches!(
            CommandRouter::parse("/llm model gpt-4"),
            Command::LlmModel(LlmModelArgs::Set(s)) if s == "gpt-4"
        ));
    }

    #[test]
    fn test_parse_llm_key() {
        assert!(matches!(
            CommandRouter::parse("/llm key sk-123"),
            Command::LlmKey(LlmKeyArgs::Set(s)) if s == "sk-123"
        ));
    }

    #[test]
    fn test_parse_llm_settings() {
        assert!(matches!(CommandRouter::parse("/llm"), Command::LlmSettings));
    }

    #[test]
    fn test_parse_unknown_command() {
        assert!(matches!(
            CommandRouter::parse("/unknown"),
            Command::Unknown(s) if s == "/unknown"
        ));
    }

    #[test]
    fn test_case_insensitive_commands() {
        assert!(matches!(CommandRouter::parse("/CLEAR"), Command::Clear));
        assert!(matches!(
            CommandRouter::parse("/SQL SELECT 1"),
            Command::Sql(_)
        ));
        assert!(matches!(CommandRouter::parse("/Help"), Command::Help));
    }

    #[test]
    fn test_parse_refresh_schema() {
        assert!(matches!(
            CommandRouter::parse("/refresh schema"),
            Command::RefreshSchema
        ));
        assert!(matches!(
            CommandRouter::parse("/refresh"),
            Command::RefreshSchema
        ));
        assert!(matches!(
            CommandRouter::parse("/REFRESH SCHEMA"),
            Command::RefreshSchema
        ));
    }

    #[test]
    fn test_parse_refresh_unknown() {
        assert!(matches!(
            CommandRouter::parse("/refresh unknown"),
            Command::Unknown(_)
        ));
    }

    #[test]
    fn test_parse_conn_add_with_quoted_password() {
        let cmd = CommandRouter::parse("/conn add mydb host=localhost password=\"my secret\"");
        if let Command::ConnectionAdd(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert_eq!(args.host, Some("localhost".to_string()));
            assert_eq!(args.password, Some("my secret".to_string()));
        } else {
            panic!("Expected ConnectionAdd");
        }
    }

    #[test]
    fn test_parse_conn_add_with_special_chars_in_password() {
        let cmd = CommandRouter::parse("/conn add mydb password=\"p@ss=word!\"");
        if let Command::ConnectionAdd(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert_eq!(args.password, Some("p@ss=word!".to_string()));
        } else {
            panic!("Expected ConnectionAdd");
        }
    }

    #[test]
    fn test_parse_conn_add_with_backend() {
        let cmd =
            CommandRouter::parse("/conn add mydb backend=postgres host=localhost database=test");
        if let Command::ConnectionAdd(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert_eq!(args.backend, Some("postgres".to_string()));
            assert_eq!(args.host, Some("localhost".to_string()));
            assert_eq!(args.database, Some("test".to_string()));
        } else {
            panic!("Expected ConnectionAdd");
        }
    }

    #[test]
    fn test_parse_conn_edit_with_backend() {
        let cmd = CommandRouter::parse("/conn edit mydb backend=postgres");
        if let Command::ConnectionEdit(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert_eq!(args.backend, Some("postgres".to_string()));
        } else {
            panic!("Expected ConnectionEdit");
        }
    }

    #[test]
    fn test_parse_conn_edit_with_quoted_password() {
        let cmd = CommandRouter::parse("/conn edit mydb password=\"new secret\"");
        if let Command::ConnectionEdit(args) = cmd {
            assert_eq!(args.name, "mydb");
            assert_eq!(args.password, Some("new secret".to_string()));
        } else {
            panic!("Expected ConnectionEdit");
        }
    }
}
