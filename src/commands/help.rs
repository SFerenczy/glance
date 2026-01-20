//! Help text constants for Glance commands.

/// Help text displayed for the /help command.
pub const HELP_TEXT: &str = r#"Available commands:
  /sql <query>     - Execute raw SQL directly
  /clear           - Clear chat history and LLM context
  /schema          - Display database schema
  /refresh schema  - Re-introspect database schema
  /vim             - Toggle vim-style navigation mode
  /help            - Show this help message
  /quit, /exit     - Exit the application

Connection commands:
  /connections     - List saved connections
  /connect <name>  - Switch to a saved connection
  /conn add <name> host=... database=... [--test]
  /conn edit <name> - Edit an existing connection
  /conn delete <name> - Delete a connection

History commands:
  /history [--conn <name>] [--text <filter>] [--limit N]
  /history clear   - Clear query history

Saved queries:
  /savequery <name> [#tags...] - Save current/last query
  /queries [--tag <tag>] [--text <filter>]
  /usequery <name> - Load a saved query
  /query delete <name> - Delete a saved query

LLM settings:
  /llm provider <openai|anthropic|ollama>
  /llm model <name>
  /llm key         - Set API key (masked input)

Keyboard shortcuts:
  Ctrl+C, Ctrl+Q  - Exit application
  Tab             - Switch focus between panels
  Enter           - Submit input
  Esc             - Clear input (or exit to Normal mode in vim mode)
  ↑/↓             - History navigation or scroll
  Page Up/Down    - Scroll by page"#;
