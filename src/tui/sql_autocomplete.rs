//! SQL autocomplete support for the TUI.
//!
//! Provides context-aware SQL completions based on cursor position.

#![allow(dead_code)] // Used in Phase 3.2 and 3.3

use std::collections::HashMap;

/// SQL context at the cursor position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlContext {
    /// At the start of a query or unknown context.
    Start,
    /// After SELECT keyword - suggest columns.
    SelectColumns,
    /// After FROM keyword - suggest tables.
    FromTable,
    /// After JOIN keyword - suggest tables.
    JoinTable,
    /// After WHERE keyword - suggest columns.
    WhereClause,
    /// After ORDER BY - suggest columns.
    OrderBy,
    /// After GROUP BY - suggest columns.
    GroupBy,
    /// After a table alias dot (e.g., `u.`) - suggest columns from that table.
    AliasDot { alias: String },
    /// After ON keyword in JOIN - suggest columns.
    JoinCondition,
    /// After SET in UPDATE - suggest columns.
    SetClause,
    /// After INSERT INTO table - suggest columns or VALUES.
    InsertColumns,
}

/// A parsed table alias mapping.
#[derive(Debug, Clone)]
pub struct TableAlias {
    /// The table name.
    pub table: String,
    /// The alias (if any).
    pub alias: Option<String>,
}

/// Result of parsing SQL for autocomplete context.
#[derive(Debug)]
pub struct SqlParseResult {
    /// The detected context at cursor position.
    pub context: SqlContext,
    /// Table aliases found in the query.
    pub aliases: HashMap<String, String>,
    /// Tables referenced in the query.
    pub tables: Vec<String>,
    /// The word being typed at cursor (for filtering).
    pub current_word: String,
}

/// SQL keywords that indicate context changes.
const KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "JOIN",
    "LEFT",
    "RIGHT",
    "INNER",
    "OUTER",
    "CROSS",
    "ON",
    "AND",
    "OR",
    "ORDER",
    "BY",
    "GROUP",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "AS",
    "DISTINCT",
    "ALL",
    "UNION",
    "EXCEPT",
    "INTERSECT",
    "CREATE",
    "DROP",
    "ALTER",
    "TABLE",
    "INDEX",
    "VIEW",
    "GRANT",
    "REVOKE",
    "NULL",
    "NOT",
    "IN",
    "LIKE",
    "BETWEEN",
    "EXISTS",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "ASC",
    "DESC",
    "NULLS",
    "FIRST",
    "LAST",
    "TRUE",
    "FALSE",
    "IS",
    "CAST",
    "COALESCE",
    "NULLIF",
];

/// Common SQL functions.
const FUNCTIONS: &[&str] = &[
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "COALESCE",
    "NULLIF",
    "CAST",
    "CONCAT",
    "LENGTH",
    "LOWER",
    "UPPER",
    "TRIM",
    "SUBSTRING",
    "REPLACE",
    "NOW",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "DATE_TRUNC",
    "EXTRACT",
    "TO_CHAR",
    "TO_DATE",
    "TO_NUMBER",
    "ROUND",
    "FLOOR",
    "CEIL",
    "ABS",
    "RANDOM",
    "ROW_NUMBER",
    "RANK",
    "DENSE_RANK",
    "LAG",
    "LEAD",
    "FIRST",
    "LAST",
    "ARRAY_AGG",
    "STRING_AGG",
    "JSON_AGG",
    "JSONB_AGG",
];

/// Parses SQL text and determines the context at the cursor position.
pub fn parse_sql_context(sql: &str, cursor_pos: usize) -> SqlParseResult {
    let sql_before_cursor = &sql[..cursor_pos.min(sql.len())];
    let tokens = tokenize(sql_before_cursor);

    let mut context = SqlContext::Start;
    let mut aliases: HashMap<String, String> = HashMap::new();
    let mut tables: Vec<String> = Vec::new();
    let mut current_word = String::new();

    // Extract current word being typed
    if let Some(last_token) = tokens.last() {
        if !last_token.is_empty() && !is_keyword(last_token) {
            // Check if we're in the middle of typing
            let trimmed = sql_before_cursor.trim_end();
            if trimmed.ends_with(last_token) {
                current_word = last_token.to_string();
            }
        }
    }

    // Check for alias dot pattern (e.g., "u.")
    let trimmed = sql_before_cursor.trim_end();
    if let Some(before_dot) = trimmed.strip_suffix('.') {
        // Find the alias before the dot
        if let Some(alias) = before_dot.split_whitespace().last() {
            return SqlParseResult {
                context: SqlContext::AliasDot {
                    alias: alias.to_lowercase(),
                },
                aliases,
                tables,
                current_word: String::new(),
            };
        }
    }

    // Parse tokens to determine context
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].to_uppercase();

        match token.as_str() {
            "SELECT" => {
                context = SqlContext::SelectColumns;
            }
            "FROM" => {
                context = SqlContext::FromTable;
                // Parse table and potential alias
                if i + 1 < tokens.len() {
                    let table = tokens[i + 1].to_lowercase();
                    if !is_keyword(&table) {
                        tables.push(table.clone());
                        // Check for alias
                        if i + 2 < tokens.len() {
                            let next = tokens[i + 2].to_uppercase();
                            if next == "AS" && i + 3 < tokens.len() {
                                let alias = tokens[i + 3].to_lowercase();
                                aliases.insert(alias, table);
                                i += 3;
                            } else if !is_keyword(&next) {
                                aliases.insert(next.to_lowercase(), table);
                                i += 2;
                            }
                        }
                    }
                }
            }
            "JOIN" | "INNER" | "LEFT" | "RIGHT" | "OUTER" | "CROSS" => {
                if token == "JOIN"
                    || (i + 1 < tokens.len() && tokens[i + 1].to_uppercase() == "JOIN")
                {
                    context = SqlContext::JoinTable;
                    // Skip to after JOIN if we're on a modifier
                    if token != "JOIN" {
                        i += 1;
                    }
                    // Parse table and potential alias
                    if i + 1 < tokens.len() {
                        let table = tokens[i + 1].to_lowercase();
                        if !is_keyword(&table) {
                            tables.push(table.clone());
                            if i + 2 < tokens.len() {
                                let next = tokens[i + 2].to_uppercase();
                                if next == "AS" && i + 3 < tokens.len() {
                                    let alias = tokens[i + 3].to_lowercase();
                                    aliases.insert(alias, table);
                                } else if !is_keyword(&next) && next != "ON" {
                                    aliases.insert(next.to_lowercase(), table);
                                }
                            }
                        }
                    }
                }
            }
            "ON" => {
                context = SqlContext::JoinCondition;
            }
            "WHERE" => {
                context = SqlContext::WhereClause;
            }
            "ORDER" => {
                if i + 1 < tokens.len() && tokens[i + 1].to_uppercase() == "BY" {
                    context = SqlContext::OrderBy;
                    i += 1;
                }
            }
            "GROUP" => {
                if i + 1 < tokens.len() && tokens[i + 1].to_uppercase() == "BY" {
                    context = SqlContext::GroupBy;
                    i += 1;
                }
            }
            "UPDATE" => {
                if i + 1 < tokens.len() {
                    let table = tokens[i + 1].to_lowercase();
                    if !is_keyword(&table) {
                        tables.push(table);
                    }
                }
            }
            "SET" => {
                context = SqlContext::SetClause;
            }
            "INSERT" => {
                if i + 1 < tokens.len() && tokens[i + 1].to_uppercase() == "INTO" {
                    if i + 2 < tokens.len() {
                        let table = tokens[i + 2].to_lowercase();
                        if !is_keyword(&table) {
                            tables.push(table);
                        }
                    }
                    context = SqlContext::InsertColumns;
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    SqlParseResult {
        context,
        aliases,
        tables,
        current_word,
    }
}

/// Simple tokenizer that splits SQL into words.
fn tokenize(sql: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = None;

    for (i, c) in sql.char_indices() {
        if c.is_alphanumeric() || c == '_' {
            if start.is_none() {
                start = Some(i);
            }
        } else {
            if let Some(s) = start {
                tokens.push(&sql[s..i]);
                start = None;
            }
            // Include operators and punctuation as separate tokens
            if !c.is_whitespace() && c != ',' && c != '(' && c != ')' && c != ';' {
                // Skip for now - we mainly care about identifiers
            }
        }
    }

    // Don't forget the last token
    if let Some(s) = start {
        tokens.push(&sql[s..]);
    }

    tokens
}

/// Checks if a string is a SQL keyword.
fn is_keyword(s: &str) -> bool {
    let upper = s.to_uppercase();
    KEYWORDS.contains(&upper.as_str()) || FUNCTIONS.contains(&upper.as_str())
}

/// Returns all SQL keywords for completion.
pub fn sql_keywords() -> &'static [&'static str] {
    KEYWORDS
}

/// Returns all SQL functions for completion.
pub fn sql_functions() -> &'static [&'static str] {
    FUNCTIONS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_after_select() {
        let result = parse_sql_context("SELECT ", 7);
        assert_eq!(result.context, SqlContext::SelectColumns);
    }

    #[test]
    fn test_context_after_from() {
        let result = parse_sql_context("SELECT * FROM ", 14);
        assert_eq!(result.context, SqlContext::FromTable);
    }

    #[test]
    fn test_context_after_where() {
        let result = parse_sql_context("SELECT * FROM users WHERE ", 26);
        assert_eq!(result.context, SqlContext::WhereClause);
    }

    #[test]
    fn test_context_after_join() {
        let result = parse_sql_context("SELECT * FROM users JOIN ", 25);
        assert_eq!(result.context, SqlContext::JoinTable);
    }

    #[test]
    fn test_context_after_order_by() {
        let result = parse_sql_context("SELECT * FROM users ORDER BY ", 29);
        assert_eq!(result.context, SqlContext::OrderBy);
    }

    #[test]
    fn test_alias_detection() {
        let result = parse_sql_context("SELECT * FROM users u WHERE ", 28);
        assert!(result.aliases.contains_key("u"));
        assert_eq!(result.aliases.get("u"), Some(&"users".to_string()));
    }

    #[test]
    fn test_alias_with_as() {
        let result = parse_sql_context("SELECT * FROM users AS u WHERE ", 31);
        assert!(result.aliases.contains_key("u"));
        assert_eq!(result.aliases.get("u"), Some(&"users".to_string()));
    }

    #[test]
    fn test_alias_dot_context() {
        let result = parse_sql_context("SELECT u.", 9);
        assert!(matches!(result.context, SqlContext::AliasDot { alias } if alias == "u"));
    }

    #[test]
    fn test_tables_collected() {
        let result = parse_sql_context(
            "SELECT * FROM users u JOIN orders o ON u.id = o.user_id",
            55,
        );
        assert!(result.tables.contains(&"users".to_string()));
        assert!(result.tables.contains(&"orders".to_string()));
    }

    #[test]
    fn test_current_word() {
        let result = parse_sql_context("SELECT na", 9);
        assert_eq!(result.current_word, "na");
    }

    #[test]
    fn test_is_keyword() {
        assert!(is_keyword("SELECT"));
        assert!(is_keyword("select"));
        assert!(is_keyword("FROM"));
        assert!(!is_keyword("users"));
        assert!(!is_keyword("mycolumn"));
    }
}
