//! Command definitions for declarative command metadata.
//!
//! This module provides a declarative way to define commands with their
//! arguments, descriptions, and other metadata. This enables:
//! - Auto-generated help text
//! - Consistent argument validation
//! - Command discovery for autocomplete

/// Definition of a command argument.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ArgDef {
    /// Argument name.
    pub name: &'static str,
    /// Short description.
    pub description: &'static str,
    /// Whether this argument is required.
    pub required: bool,
    /// Argument type hint.
    pub arg_type: ArgType,
}

/// Type hint for argument values.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    /// Plain string value.
    String,
    /// Integer value.
    Integer,
    /// Boolean flag (no value needed).
    Flag,
    /// Key=value pair.
    KeyValue,
}

/// Definition of a command.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CommandDef {
    /// Primary command name (without leading /).
    pub name: &'static str,
    /// Alternative names for the command.
    pub aliases: &'static [&'static str],
    /// Short description shown in help.
    pub description: &'static str,
    /// Detailed usage information.
    pub usage: &'static str,
    /// Argument definitions.
    pub args: &'static [ArgDef],
    /// Whether this command requires a database connection.
    pub requires_db: bool,
    /// Whether this command requires the state database.
    pub requires_state_db: bool,
    /// Category for grouping in help.
    pub category: CommandCategory,
}

/// Category for grouping commands in help output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    /// General commands.
    General,
    /// Connection management commands.
    Connection,
    /// History commands.
    History,
    /// Saved queries commands.
    Queries,
    /// LLM settings commands.
    Llm,
}

impl CommandCategory {
    /// Returns the display name for this category.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::General => "General commands",
            Self::Connection => "Connection commands",
            Self::History => "History commands",
            Self::Queries => "Saved queries",
            Self::Llm => "LLM settings",
        }
    }
}

/// All command definitions.
pub static COMMANDS: &[CommandDef] = &[
    // General commands
    CommandDef {
        name: "sql",
        aliases: &[],
        description: "Execute raw SQL directly",
        usage: "/sql <query>",
        args: &[ArgDef {
            name: "query",
            description: "SQL query to execute",
            required: true,
            arg_type: ArgType::String,
        }],
        requires_db: true,
        requires_state_db: false,
        category: CommandCategory::General,
    },
    CommandDef {
        name: "clear",
        aliases: &[],
        description: "Clear chat history and LLM context",
        usage: "/clear",
        args: &[],
        requires_db: false,
        requires_state_db: false,
        category: CommandCategory::General,
    },
    CommandDef {
        name: "schema",
        aliases: &[],
        description: "Display database schema",
        usage: "/schema",
        args: &[],
        requires_db: true,
        requires_state_db: false,
        category: CommandCategory::General,
    },
    CommandDef {
        name: "refresh",
        aliases: &[],
        description: "Re-introspect database schema",
        usage: "/refresh schema",
        args: &[],
        requires_db: true,
        requires_state_db: false,
        category: CommandCategory::General,
    },
    CommandDef {
        name: "vim",
        aliases: &[],
        description: "Toggle vim-style navigation mode",
        usage: "/vim",
        args: &[],
        requires_db: false,
        requires_state_db: false,
        category: CommandCategory::General,
    },
    CommandDef {
        name: "help",
        aliases: &[],
        description: "Show this help message",
        usage: "/help",
        args: &[],
        requires_db: false,
        requires_state_db: false,
        category: CommandCategory::General,
    },
    CommandDef {
        name: "quit",
        aliases: &["exit"],
        description: "Exit the application",
        usage: "/quit",
        args: &[],
        requires_db: false,
        requires_state_db: false,
        category: CommandCategory::General,
    },
    // Connection commands
    CommandDef {
        name: "connections",
        aliases: &[],
        description: "List saved connections",
        usage: "/connections",
        args: &[],
        requires_db: false,
        requires_state_db: true,
        category: CommandCategory::Connection,
    },
    CommandDef {
        name: "connect",
        aliases: &[],
        description: "Switch to a saved connection",
        usage: "/connect <name>",
        args: &[ArgDef {
            name: "name",
            description: "Connection name",
            required: true,
            arg_type: ArgType::String,
        }],
        requires_db: false,
        requires_state_db: true,
        category: CommandCategory::Connection,
    },
    CommandDef {
        name: "conn",
        aliases: &[],
        description: "Manage connections (add/edit/delete)",
        usage:
            "/conn add <name> host=<host> database=<db> [user=<user>] [password=\"<pwd>\"] [--test]",
        args: &[
            ArgDef {
                name: "subcommand",
                description: "add, edit, or delete",
                required: true,
                arg_type: ArgType::String,
            },
            ArgDef {
                name: "name",
                description: "Connection name",
                required: true,
                arg_type: ArgType::String,
            },
        ],
        requires_db: false,
        requires_state_db: true,
        category: CommandCategory::Connection,
    },
    // History commands
    CommandDef {
        name: "history",
        aliases: &[],
        description: "Show query history",
        usage: "/history [--conn <name>] [--text <filter>] [--limit N] [--since N]",
        args: &[
            ArgDef {
                name: "--conn",
                description: "Filter by connection name",
                required: false,
                arg_type: ArgType::String,
            },
            ArgDef {
                name: "--text",
                description: "Filter by text search",
                required: false,
                arg_type: ArgType::String,
            },
            ArgDef {
                name: "--limit",
                description: "Limit number of results",
                required: false,
                arg_type: ArgType::Integer,
            },
            ArgDef {
                name: "--since",
                description: "Filter by days since",
                required: false,
                arg_type: ArgType::Integer,
            },
        ],
        requires_db: false,
        requires_state_db: true,
        category: CommandCategory::History,
    },
    // Saved queries commands
    CommandDef {
        name: "savequery",
        aliases: &[],
        description: "Save the last executed query",
        usage: "/savequery <name> [#tags...]",
        args: &[
            ArgDef {
                name: "name",
                description: "Query name",
                required: true,
                arg_type: ArgType::String,
            },
            ArgDef {
                name: "tags",
                description: "Tags prefixed with #",
                required: false,
                arg_type: ArgType::String,
            },
        ],
        requires_db: false,
        requires_state_db: true,
        category: CommandCategory::Queries,
    },
    CommandDef {
        name: "queries",
        aliases: &[],
        description: "List saved queries",
        usage: "/queries [--tag <tag>] [--text <filter>] [--all]",
        args: &[
            ArgDef {
                name: "--tag",
                description: "Filter by tag",
                required: false,
                arg_type: ArgType::String,
            },
            ArgDef {
                name: "--text",
                description: "Filter by text search",
                required: false,
                arg_type: ArgType::String,
            },
            ArgDef {
                name: "--all",
                description: "Show all connections",
                required: false,
                arg_type: ArgType::Flag,
            },
        ],
        requires_db: false,
        requires_state_db: true,
        category: CommandCategory::Queries,
    },
    CommandDef {
        name: "usequery",
        aliases: &[],
        description: "Load a saved query",
        usage: "/usequery <name>",
        args: &[ArgDef {
            name: "name",
            description: "Query name",
            required: true,
            arg_type: ArgType::String,
        }],
        requires_db: false,
        requires_state_db: true,
        category: CommandCategory::Queries,
    },
    CommandDef {
        name: "query",
        aliases: &[],
        description: "Manage saved queries",
        usage: "/query delete <name>",
        args: &[
            ArgDef {
                name: "subcommand",
                description: "delete",
                required: true,
                arg_type: ArgType::String,
            },
            ArgDef {
                name: "name",
                description: "Query name",
                required: true,
                arg_type: ArgType::String,
            },
        ],
        requires_db: false,
        requires_state_db: true,
        category: CommandCategory::Queries,
    },
    // LLM settings commands
    CommandDef {
        name: "llm",
        aliases: &[],
        description: "Manage LLM settings",
        usage: "/llm [provider|model|key] [value]",
        args: &[
            ArgDef {
                name: "subcommand",
                description: "provider, model, or key",
                required: false,
                arg_type: ArgType::String,
            },
            ArgDef {
                name: "value",
                description: "New value to set",
                required: false,
                arg_type: ArgType::String,
            },
        ],
        requires_db: false,
        requires_state_db: true,
        category: CommandCategory::Llm,
    },
];

/// Generates help text from command definitions.
#[allow(dead_code)]
pub fn generate_help_text() -> String {
    // Group commands by category
    let categories = [
        CommandCategory::General,
        CommandCategory::Connection,
        CommandCategory::History,
        CommandCategory::Queries,
        CommandCategory::Llm,
    ];

    let category_blocks = categories
        .iter()
        .filter_map(|category| {
            let cmds: Vec<_> = COMMANDS
                .iter()
                .filter(|c| c.category == *category)
                .collect();

            if cmds.is_empty() {
                return None;
            }

            let command_lines = cmds
                .iter()
                .map(|cmd| {
                    let aliases = if cmd.aliases.is_empty() {
                        String::new()
                    } else {
                        format!(", /{}", cmd.aliases.join(", /"))
                    };
                    format!("  /{}{:<12} - {}\n", cmd.name, aliases, cmd.description)
                })
                .collect::<Vec<_>>()
                .join("");

            Some(format!("{}:\n{}\n", category.display_name(), command_lines))
        })
        .collect::<Vec<_>>()
        .join("");

    let keyboard_shortcuts = [
        "Keyboard shortcuts:",
        "  Ctrl+C, Ctrl+Q  - Exit application",
        "  Tab             - Switch focus between panels",
        "  Enter           - Submit input",
        "  Esc             - Clear input (or exit to Normal mode in vim mode)",
        "  ↑/↓             - History navigation or scroll",
        "  Page Up/Down    - Scroll by page",
    ]
    .join("\n");

    format!("{}{}", category_blocks, keyboard_shortcuts)
}

/// Finds a command definition by name.
#[allow(dead_code)]
pub fn find_command(name: &str) -> Option<&'static CommandDef> {
    let name_lower = name.to_lowercase();
    COMMANDS
        .iter()
        .find(|c| c.name == name_lower || c.aliases.iter().any(|a| *a == name_lower))
}

/// Returns commands that require the state database.
#[allow(dead_code)]
pub fn commands_requiring_state_db() -> impl Iterator<Item = &'static CommandDef> {
    COMMANDS.iter().filter(|c| c.requires_state_db)
}

/// Returns commands that require a database connection.
#[allow(dead_code)]
pub fn commands_requiring_db() -> impl Iterator<Item = &'static CommandDef> {
    COMMANDS.iter().filter(|c| c.requires_db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_command() {
        assert!(find_command("sql").is_some());
        assert!(find_command("SQL").is_some());
        assert!(find_command("quit").is_some());
        assert!(find_command("exit").is_some()); // alias
        assert!(find_command("nonexistent").is_none());
    }

    #[test]
    fn test_generate_help_text() {
        let help = generate_help_text();
        assert!(help.contains("General commands"));
        assert!(help.contains("/sql"));
        assert!(help.contains("/quit"));
        assert!(help.contains("Keyboard shortcuts"));
    }

    #[test]
    fn test_commands_requiring_state_db() {
        let cmds: Vec<_> = commands_requiring_state_db().collect();
        assert!(cmds.iter().any(|c| c.name == "connections"));
        assert!(cmds.iter().any(|c| c.name == "history"));
        assert!(!cmds.iter().any(|c| c.name == "clear"));
    }

    #[test]
    fn test_commands_requiring_db() {
        let cmds: Vec<_> = commands_requiring_db().collect();
        assert!(cmds.iter().any(|c| c.name == "sql"));
        assert!(cmds.iter().any(|c| c.name == "schema"));
        assert!(!cmds.iter().any(|c| c.name == "help"));
    }

    #[test]
    fn test_category_display_name() {
        assert_eq!(CommandCategory::General.display_name(), "General commands");
        assert_eq!(
            CommandCategory::Connection.display_name(),
            "Connection commands"
        );
    }
}
