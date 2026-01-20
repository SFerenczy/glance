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
    /// After column name in WHERE - suggest operators.
    WhereOperator,
    /// After operator in WHERE - suggest values.
    WhereValue,
    /// After a complete condition - suggest AND/OR/ORDER BY.
    WhereContinuation,
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
    let token_strs = tokens_to_strs(&tokens);

    let mut context = SqlContext::Start;
    let mut aliases: HashMap<String, String> = HashMap::new();
    let mut tables: Vec<String> = Vec::new();
    let mut current_word = String::new();

    // Extract current word being typed
    // Only set current_word if the cursor is immediately after the token (no trailing space)
    if let Some(Token::Ident(s)) = tokens.last() {
        if !is_keyword(s) && sql_before_cursor.ends_with(s) {
            current_word = s.to_string();
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
    let mut in_where = false;
    let mut where_state = WhereState::NeedColumn;

    while i < tokens.len() {
        let token_upper = token_strs[i].to_uppercase();

        match token_upper.as_str() {
            "SELECT" => {
                context = SqlContext::SelectColumns;
                in_where = false;
            }
            "FROM" => {
                context = SqlContext::FromTable;
                in_where = false;
                // Parse table and potential alias
                if i + 1 < tokens.len() {
                    if let Token::Ident(table_str) = &tokens[i + 1] {
                        let table = table_str.to_lowercase();
                        if !is_keyword(&table) {
                            tables.push(table.clone());
                            // Check for alias
                            if i + 2 < tokens.len() {
                                if let Token::Ident(next_str) = &tokens[i + 2] {
                                    let next = next_str.to_uppercase();
                                    if next == "AS" {
                                        if i + 3 < tokens.len() {
                                            if let Token::Ident(alias_str) = &tokens[i + 3] {
                                                let alias = alias_str.to_lowercase();
                                                aliases.insert(alias, table);
                                                i += 3;
                                            }
                                        }
                                    } else if !is_keyword(next_str) {
                                        aliases.insert(next_str.to_lowercase(), table);
                                        i += 2;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "JOIN" | "INNER" | "LEFT" | "RIGHT" | "OUTER" | "CROSS" => {
                in_where = false;
                if token_upper == "JOIN"
                    || (i + 1 < tokens.len() && token_strs[i + 1].to_uppercase() == "JOIN")
                {
                    context = SqlContext::JoinTable;
                    // Skip to after JOIN if we're on a modifier
                    if token_upper != "JOIN" {
                        i += 1;
                    }
                    // Parse table and potential alias
                    if i + 1 < tokens.len() {
                        if let Token::Ident(table_str) = &tokens[i + 1] {
                            let table = table_str.to_lowercase();
                            if !is_keyword(&table) {
                                tables.push(table.clone());
                                if i + 2 < tokens.len() {
                                    if let Token::Ident(next_str) = &tokens[i + 2] {
                                        let next = next_str.to_uppercase();
                                        if next == "AS" {
                                            if i + 3 < tokens.len() {
                                                if let Token::Ident(alias_str) = &tokens[i + 3] {
                                                    let alias = alias_str.to_lowercase();
                                                    aliases.insert(alias, table);
                                                }
                                            }
                                        } else if !is_keyword(next_str) && next != "ON" {
                                            aliases.insert(next_str.to_lowercase(), table);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "ON" => {
                context = SqlContext::JoinCondition;
                in_where = false;
            }
            "WHERE" => {
                context = SqlContext::WhereClause;
                in_where = true;
                where_state = WhereState::NeedColumn;
            }
            "AND" | "OR" if in_where => {
                context = SqlContext::WhereClause;
                where_state = WhereState::NeedColumn;
            }
            "ORDER" => {
                if i + 1 < tokens.len() && token_strs[i + 1].to_uppercase() == "BY" {
                    context = SqlContext::OrderBy;
                    in_where = false;
                    i += 1;
                }
            }
            "GROUP" => {
                if i + 1 < tokens.len() && token_strs[i + 1].to_uppercase() == "BY" {
                    context = SqlContext::GroupBy;
                    in_where = false;
                    i += 1;
                }
            }
            "LIMIT" | "OFFSET" | "HAVING" => {
                in_where = false;
            }
            "UPDATE" => {
                in_where = false;
                if i + 1 < tokens.len() {
                    if let Token::Ident(table_str) = &tokens[i + 1] {
                        let table = table_str.to_lowercase();
                        if !is_keyword(&table) {
                            tables.push(table);
                        }
                    }
                }
            }
            "SET" => {
                context = SqlContext::SetClause;
                in_where = false;
            }
            "INSERT" => {
                in_where = false;
                if i + 1 < tokens.len() && token_strs[i + 1].to_uppercase() == "INTO" {
                    if i + 2 < tokens.len() {
                        if let Token::Ident(table_str) = &tokens[i + 2] {
                            let table = table_str.to_lowercase();
                            if !is_keyword(&table) {
                                tables.push(table);
                            }
                        }
                    }
                    context = SqlContext::InsertColumns;
                    i += 1;
                }
            }
            _ if in_where => {
                // WHERE state machine
                match where_state {
                    WhereState::NeedColumn => {
                        // We have a column (or identifier), now need operator
                        if let Token::Ident(_) = &tokens[i] {
                            where_state = WhereState::NeedOperator;
                            context = SqlContext::WhereOperator;
                        }
                    }
                    WhereState::NeedOperator => {
                        // Check if this is an operator
                        let is_op = tokens[i].is_operator()
                            || OPERATOR_KEYWORDS.contains(&token_upper.as_str());
                        if is_op {
                            where_state = WhereState::NeedValue;
                            context = SqlContext::WhereValue;
                            // Handle BETWEEN specially - needs two values
                            if token_upper == "BETWEEN" {
                                where_state = WhereState::InBetween;
                            }
                            // Handle IS NULL / IS NOT NULL
                            if token_upper == "IS" {
                                where_state = WhereState::AfterIs;
                            }
                            // Handle IN (...)
                            if token_upper == "IN" {
                                where_state = WhereState::InList;
                            }
                        }
                    }
                    WhereState::NeedValue => {
                        // We have a value, now need AND/OR or end
                        match &tokens[i] {
                            Token::StringLiteral(_) | Token::Number(_) | Token::Ident(_) => {
                                where_state = WhereState::Complete;
                                context = SqlContext::WhereContinuation;
                            }
                            _ => {}
                        }
                    }
                    WhereState::AfterIs => {
                        // After IS, expect NULL, NOT NULL, TRUE, FALSE
                        if token_upper == "NOT" {
                            // Stay in AfterIs, waiting for NULL
                        } else if token_upper == "NULL"
                            || token_upper == "TRUE"
                            || token_upper == "FALSE"
                        {
                            where_state = WhereState::Complete;
                            context = SqlContext::WhereContinuation;
                        }
                    }
                    WhereState::InBetween => {
                        // After BETWEEN value AND value
                        if token_upper == "AND" {
                            // This AND is part of BETWEEN, not a condition separator
                            where_state = WhereState::NeedValue;
                            context = SqlContext::WhereValue;
                        } else {
                            // First value of BETWEEN
                            where_state = WhereState::BetweenAnd;
                        }
                    }
                    WhereState::BetweenAnd => {
                        if token_upper == "AND" {
                            where_state = WhereState::NeedValue;
                            context = SqlContext::WhereValue;
                        }
                    }
                    WhereState::InList => {
                        // Skip until we're out of the IN list
                        // For now, just move to complete after any value
                        where_state = WhereState::Complete;
                        context = SqlContext::WhereContinuation;
                    }
                    WhereState::Complete => {
                        // Already complete, stay in continuation
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Final context adjustment based on WHERE state
    if in_where {
        context = match where_state {
            WhereState::NeedColumn => SqlContext::WhereClause,
            WhereState::NeedOperator => SqlContext::WhereOperator,
            WhereState::NeedValue
            | WhereState::AfterIs
            | WhereState::InBetween
            | WhereState::BetweenAnd
            | WhereState::InList => SqlContext::WhereValue,
            WhereState::Complete => SqlContext::WhereContinuation,
        };
    }

    SqlParseResult {
        context,
        aliases,
        tables,
        current_word,
    }
}

/// State machine for WHERE clause parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WhereState {
    /// Need a column name.
    NeedColumn,
    /// Have column, need operator.
    NeedOperator,
    /// Have operator, need value.
    NeedValue,
    /// After IS keyword.
    AfterIs,
    /// In BETWEEN expression, before AND.
    InBetween,
    /// In BETWEEN expression, after first value, waiting for AND.
    BetweenAnd,
    /// In IN (...) list.
    InList,
    /// Condition complete, can add AND/OR.
    Complete,
}

/// A token from SQL parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token<'a> {
    /// An identifier or keyword.
    Ident(&'a str),
    /// An operator (=, !=, <>, <, >, <=, >=).
    Operator(&'a str),
    /// A string literal (content without quotes).
    StringLiteral(&'a str),
    /// A number literal.
    Number(&'a str),
}

impl<'a> Token<'a> {
    /// Returns the text of the token.
    fn as_str(&self) -> &'a str {
        match self {
            Token::Ident(s) | Token::Operator(s) | Token::StringLiteral(s) | Token::Number(s) => s,
        }
    }

    /// Returns true if this token is an operator.
    fn is_operator(&self) -> bool {
        matches!(self, Token::Operator(_))
    }
}

/// SQL comparison operators.
const OPERATORS: &[&str] = &["<>", "!=", "<=", ">=", "=", "<", ">"];

/// Keywords that act as operators in WHERE clauses.
const OPERATOR_KEYWORDS: &[&str] = &["IS", "IN", "LIKE", "BETWEEN", "NOT"];

/// Tokenizer that splits SQL into words, operators, and literals.
fn tokenize(sql: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Skip whitespace
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // String literals (single quotes)
        if c == '\'' {
            let start = i + 1;
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                // Handle escaped quotes
                if chars[i] == '\'' && i + 1 < chars.len() && chars[i + 1] == '\'' {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            let end = i;
            if start < end && start <= sql.len() && end <= sql.len() {
                let byte_start = chars[..start].iter().map(|c| c.len_utf8()).sum();
                let byte_end = chars[..end].iter().map(|c| c.len_utf8()).sum();
                tokens.push(Token::StringLiteral(&sql[byte_start..byte_end]));
            }
            i += 1; // Skip closing quote
            continue;
        }

        // Double-quoted identifiers
        if c == '"' {
            let start = i + 1;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            let end = i;
            if start < end && start <= sql.len() && end <= sql.len() {
                let byte_start = chars[..start].iter().map(|c| c.len_utf8()).sum();
                let byte_end = chars[..end].iter().map(|c| c.len_utf8()).sum();
                tokens.push(Token::Ident(&sql[byte_start..byte_end]));
            }
            i += 1; // Skip closing quote
            continue;
        }

        // Check for multi-char operators first
        let remaining: String = chars[i..].iter().collect();
        let mut found_op = false;
        for op in OPERATORS {
            if remaining.starts_with(op) {
                let byte_start: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
                tokens.push(Token::Operator(&sql[byte_start..byte_start + op.len()]));
                i += op.len();
                found_op = true;
                break;
            }
        }
        if found_op {
            continue;
        }

        // Numbers
        if c.is_ascii_digit() || (c == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            if c == '-' {
                i += 1;
            }
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let byte_start: usize = chars[..start].iter().map(|c| c.len_utf8()).sum();
            let byte_end: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
            tokens.push(Token::Number(&sql[byte_start..byte_end]));
            continue;
        }

        // Identifiers and keywords
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let byte_start: usize = chars[..start].iter().map(|c| c.len_utf8()).sum();
            let byte_end: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
            tokens.push(Token::Ident(&sql[byte_start..byte_end]));
            continue;
        }

        // Skip other characters (parentheses, commas, semicolons, etc.)
        i += 1;
    }

    tokens
}

/// Converts tokens to string slices for backward compatibility.
fn tokens_to_strs<'a>(tokens: &[Token<'a>]) -> Vec<&'a str> {
    tokens.iter().map(|t| t.as_str()).collect()
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

    // New tests for WHERE state machine

    #[test]
    fn test_where_column_context() {
        let result = parse_sql_context("SELECT * FROM users WHERE ", 26);
        assert_eq!(result.context, SqlContext::WhereClause);
    }

    #[test]
    fn test_where_operator_context() {
        let result = parse_sql_context("SELECT * FROM users WHERE status ", 33);
        assert_eq!(result.context, SqlContext::WhereOperator);
    }

    #[test]
    fn test_where_value_context() {
        let result = parse_sql_context("SELECT * FROM users WHERE status = ", 35);
        assert_eq!(result.context, SqlContext::WhereValue);
    }

    #[test]
    fn test_where_continuation_context() {
        let result = parse_sql_context("SELECT * FROM users WHERE status = 'active' ", 44);
        assert_eq!(result.context, SqlContext::WhereContinuation);
    }

    #[test]
    fn test_where_continuation_after_number() {
        let result = parse_sql_context("SELECT * FROM users WHERE id = 123 ", 35);
        assert_eq!(result.context, SqlContext::WhereContinuation);
    }

    #[test]
    fn test_where_after_and() {
        let result = parse_sql_context("SELECT * FROM users WHERE status = 'active' AND ", 48);
        assert_eq!(result.context, SqlContext::WhereClause);
    }

    #[test]
    fn test_where_is_null() {
        let result = parse_sql_context("SELECT * FROM users WHERE email IS NULL ", 40);
        assert_eq!(result.context, SqlContext::WhereContinuation);
    }

    #[test]
    fn test_where_is_not_null() {
        let result = parse_sql_context("SELECT * FROM users WHERE email IS NOT NULL ", 44);
        assert_eq!(result.context, SqlContext::WhereContinuation);
    }

    #[test]
    fn test_where_between_value() {
        let result = parse_sql_context("SELECT * FROM users WHERE age BETWEEN ", 38);
        assert_eq!(result.context, SqlContext::WhereValue);
    }

    #[test]
    fn test_where_in_operator() {
        let result = parse_sql_context("SELECT * FROM users WHERE status IN ", 36);
        assert_eq!(result.context, SqlContext::WhereValue);
    }

    #[test]
    fn test_where_like_operator() {
        let result = parse_sql_context("SELECT * FROM users WHERE name LIKE ", 36);
        assert_eq!(result.context, SqlContext::WhereValue);
    }

    // Tokenizer tests

    #[test]
    fn test_tokenize_operators() {
        let tokens = tokenize("a = b");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[0], Token::Ident("a")));
        assert!(matches!(tokens[1], Token::Operator("=")));
        assert!(matches!(tokens[2], Token::Ident("b")));
    }

    #[test]
    fn test_tokenize_string_literal() {
        let tokens = tokenize("status = 'active'");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[2], Token::StringLiteral("active")));
    }

    #[test]
    fn test_tokenize_number() {
        let tokens = tokenize("id = 123");
        assert_eq!(tokens.len(), 3);
        assert!(matches!(tokens[2], Token::Number("123")));
    }

    #[test]
    fn test_tokenize_comparison_operators() {
        let tokens = tokenize("a >= b AND c <> d");
        assert!(tokens.iter().any(|t| matches!(t, Token::Operator(">="))));
        assert!(tokens.iter().any(|t| matches!(t, Token::Operator("<>"))));
    }
}
