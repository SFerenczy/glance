//! SQL completion popup widget for the TUI.
//!
//! Provides schema-aware SQL completions based on context.

#![allow(dead_code)] // Will be integrated with App state in future iteration

use crate::db::Schema;
use crate::tui::sql_autocomplete::{parse_sql_context, SqlContext};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};
use std::collections::HashSet;

/// Type of completion item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionKind {
    /// A table name.
    Table,
    /// A column name.
    Column,
    /// A SQL keyword.
    Keyword,
    /// A SQL function.
    Function,
    /// A SQL operator.
    Operator,
    /// Non-insertable hint (informational only).
    Hint,
}

impl CompletionKind {
    /// Returns a short label for the completion kind.
    fn label(&self) -> &'static str {
        match self {
            Self::Table => "tbl",
            Self::Column => "col",
            Self::Keyword => "kw",
            Self::Function => "fn",
            Self::Operator => "op",
            Self::Hint => "...",
        }
    }

    /// Returns the color for the completion kind.
    fn color(&self) -> Color {
        match self {
            Self::Table => Color::Yellow,
            Self::Column => Color::Cyan,
            Self::Keyword => Color::Magenta,
            Self::Function => Color::Green,
            Self::Operator => Color::Blue,
            Self::Hint => Color::DarkGray,
        }
    }

    /// Returns true if this is a non-insertable hint.
    pub fn is_hint(&self) -> bool {
        matches!(self, Self::Hint)
    }
}

/// A single completion item.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The text to insert.
    pub text: String,
    /// The kind of completion.
    pub kind: CompletionKind,
    /// Optional detail (e.g., column type).
    pub detail: Option<String>,
}

impl CompletionItem {
    /// Creates a new completion item.
    pub fn new(text: impl Into<String>, kind: CompletionKind) -> Self {
        Self {
            text: text.into(),
            kind,
            detail: None,
        }
    }

    /// Sets the detail for the completion item.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// State for the SQL completion popup.
#[derive(Debug, Default)]
pub struct SqlCompletionState {
    /// Whether the popup is currently visible.
    pub visible: bool,
    /// Available completion items.
    pub items: Vec<CompletionItem>,
    /// Currently selected index.
    pub selected: usize,
    /// The filter text (current word being typed).
    pub filter: String,
    /// Recently used completions for recency ranking (within session).
    pub recent_completions: HashSet<String>,
}

impl SqlCompletionState {
    /// Creates a new SQL completion state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates completions based on the current input and schema.
    pub fn update(&mut self, input: &str, cursor_pos: usize, schema: Option<&Schema>) {
        let result = parse_sql_context(input, cursor_pos);
        self.filter = result.current_word.clone();
        self.items.clear();

        // Generate completions based on context
        match &result.context {
            SqlContext::Start => {
                // Suggest common starting keywords
                self.add_keywords(&["SELECT", "INSERT", "UPDATE", "DELETE", "WITH"]);
            }
            SqlContext::SelectColumns => {
                // Suggest columns from referenced tables, or * if no tables yet
                self.items
                    .push(CompletionItem::new("*", CompletionKind::Keyword));
                if let Some(schema) = schema {
                    self.add_columns_from_tables(schema, &result.tables);
                }
                self.add_functions();
            }
            SqlContext::FromTable => {
                // Suggest table names
                if let Some(schema) = schema {
                    self.add_tables(schema);
                } else {
                    // Graceful degradation: show hint when no schema
                    self.items.push(CompletionItem::new(
                        "(connect to see tables)",
                        CompletionKind::Hint,
                    ));
                }
            }
            SqlContext::JoinTable => {
                // Suggest tables with FK-based join suggestions first
                if let Some(schema) = schema {
                    self.add_fk_join_suggestions(schema, &result.tables);
                    self.add_tables(schema);
                } else {
                    // Graceful degradation: show hint when no schema
                    self.items.push(CompletionItem::new(
                        "(connect to see tables)",
                        CompletionKind::Hint,
                    ));
                }
            }
            SqlContext::WhereClause => {
                // Suggest columns from referenced tables (not operators)
                if let Some(schema) = schema {
                    self.add_columns_from_tables(schema, &result.tables);
                } else {
                    // Graceful degradation: show hint when no schema
                    self.items.push(CompletionItem::new(
                        "(connect to see columns)",
                        CompletionKind::Hint,
                    ));
                }
            }
            SqlContext::WhereOperator => {
                // Suggest comparison operators
                self.add_operators();
            }
            SqlContext::WhereValue => {
                // Suggest common values
                self.add_keywords(&["NULL", "TRUE", "FALSE"]);
                self.items.push(
                    CompletionItem::new("(SELECT ...", CompletionKind::Keyword)
                        .with_detail("subquery"),
                );
            }
            SqlContext::WhereContinuation => {
                // Suggest logical operators and clause continuations
                self.add_keywords(&["AND", "OR", "ORDER BY", "GROUP BY", "LIMIT"]);
            }
            SqlContext::JoinCondition => {
                // Suggest columns from referenced tables
                if let Some(schema) = schema {
                    self.add_columns_from_tables(schema, &result.tables);
                }
            }
            SqlContext::OrderBy | SqlContext::GroupBy => {
                // Suggest columns
                if let Some(schema) = schema {
                    self.add_columns_from_tables(schema, &result.tables);
                }
                if matches!(result.context, SqlContext::OrderBy) {
                    self.add_keywords(&["ASC", "DESC", "NULLS", "FIRST", "LAST"]);
                }
            }
            SqlContext::AliasDot { alias } => {
                // Suggest columns from the aliased table
                if let Some(schema) = schema {
                    if let Some(table_name) = result.aliases.get(alias) {
                        self.add_columns_from_table(schema, table_name);
                    } else {
                        // Maybe the alias is actually a table name
                        self.add_columns_from_table(schema, alias);
                    }
                }
            }
            SqlContext::SetClause | SqlContext::InsertColumns => {
                // Suggest columns from the target table
                if let Some(schema) = schema {
                    if let Some(table) = result.tables.first() {
                        self.add_columns_from_table(schema, table);
                    }
                }
            }
        }

        // Filter items by current word (fuzzy matching)
        if !self.filter.is_empty() {
            let filter_lower = self.filter.to_lowercase();
            self.items.retain(|item| {
                let text_lower = item.text.to_lowercase();
                // Keep if: prefix match, substring match, or fuzzy match
                text_lower.starts_with(&filter_lower)
                    || text_lower.contains(&filter_lower)
                    || fuzzy_match(&text_lower, &filter_lower)
            });
        }

        // Sort by ranking per FR-3.5:
        // 1. Exact prefix match
        // 2. Case-insensitive prefix match
        // 3. Substring match
        // 4. Fuzzy match
        // 5. Recency of use (within session)
        let filter_lower = self.filter.to_lowercase();
        let recent = &self.recent_completions;
        self.items.sort_by(|a, b| {
            let a_score = rank_completion(&a.text, &self.filter, &filter_lower, recent);
            let b_score = rank_completion(&b.text, &self.filter, &filter_lower, recent);
            a_score
                .cmp(&b_score)
                .then_with(|| a.text.to_lowercase().cmp(&b.text.to_lowercase()))
        });

        // Update visibility
        self.visible = !self.items.is_empty();
        self.selected = 0;
    }

    /// Records a completion as recently used for recency ranking.
    pub fn record_completion(&mut self, text: &str) {
        self.recent_completions.insert(text.to_lowercase());
    }

    /// Adds table names from the schema.
    fn add_tables(&mut self, schema: &Schema) {
        for table in &schema.tables {
            // Skip tables already added as FK suggestions
            if !self.items.iter().any(|i| i.text == table.name) {
                self.items
                    .push(CompletionItem::new(&table.name, CompletionKind::Table));
            }
        }
    }

    /// Adds FK-based join suggestions with ON clause templates.
    fn add_fk_join_suggestions(&mut self, schema: &Schema, current_tables: &[String]) {
        for fk in &schema.foreign_keys {
            // Check if the FK relates to any of the current tables
            let from_in_query = current_tables.iter().any(|t| t == &fk.from_table);
            let to_in_query = current_tables.iter().any(|t| t == &fk.to_table);

            if from_in_query && !to_in_query {
                // Suggest joining to the target table
                let from_col = fk.from_columns.first().map(|s| s.as_str()).unwrap_or("");
                let to_col = fk.to_columns.first().map(|s| s.as_str()).unwrap_or("");
                let suggestion = format!(
                    "{} ON {}.{} = {}.{}",
                    fk.to_table, fk.to_table, to_col, fk.from_table, from_col
                );
                self.items.push(
                    CompletionItem::new(&fk.to_table, CompletionKind::Table)
                        .with_detail(format!("JOIN {} ON ...", fk.to_table)),
                );
                self.items.push(
                    CompletionItem::new(suggestion, CompletionKind::Hint)
                        .with_detail("FK join template"),
                );
            } else if to_in_query && !from_in_query {
                // Suggest joining from the source table
                let from_col = fk.from_columns.first().map(|s| s.as_str()).unwrap_or("");
                let to_col = fk.to_columns.first().map(|s| s.as_str()).unwrap_or("");
                let suggestion = format!(
                    "{} ON {}.{} = {}.{}",
                    fk.from_table, fk.from_table, from_col, fk.to_table, to_col
                );
                self.items.push(
                    CompletionItem::new(&fk.from_table, CompletionKind::Table)
                        .with_detail(format!("JOIN {} ON ...", fk.from_table)),
                );
                self.items.push(
                    CompletionItem::new(suggestion, CompletionKind::Hint)
                        .with_detail("FK join template"),
                );
            }
        }
    }

    /// Adds columns from a specific table.
    fn add_columns_from_table(&mut self, schema: &Schema, table_name: &str) {
        if let Some(table) = schema.tables.iter().find(|t| t.name == table_name) {
            for column in &table.columns {
                self.items.push(
                    CompletionItem::new(&column.name, CompletionKind::Column)
                        .with_detail(&column.data_type),
                );
            }
        }
    }

    /// Adds columns from multiple tables.
    fn add_columns_from_tables(&mut self, schema: &Schema, tables: &[String]) {
        if tables.is_empty() {
            // Add all columns from all tables
            for table in &schema.tables {
                for column in &table.columns {
                    self.items.push(
                        CompletionItem::new(&column.name, CompletionKind::Column)
                            .with_detail(format!("{}.{}", table.name, column.data_type)),
                    );
                }
            }
        } else {
            for table_name in tables {
                self.add_columns_from_table(schema, table_name);
            }
        }
    }

    /// Adds SQL keywords.
    fn add_keywords(&mut self, keywords: &[&str]) {
        for kw in keywords {
            self.items
                .push(CompletionItem::new(*kw, CompletionKind::Keyword));
        }
    }

    /// Adds common SQL functions.
    fn add_functions(&mut self) {
        let functions = [
            "COUNT", "SUM", "AVG", "MIN", "MAX", "COALESCE", "NULLIF", "CONCAT", "LENGTH", "LOWER",
            "UPPER", "NOW",
        ];
        for func in functions {
            self.items
                .push(CompletionItem::new(func, CompletionKind::Function));
        }
    }

    /// Adds SQL comparison operators.
    fn add_operators(&mut self) {
        let operators = [
            ("=", "equals"),
            ("!=", "not equals"),
            ("<>", "not equals"),
            ("<", "less than"),
            (">", "greater than"),
            ("<=", "less than or equal"),
            (">=", "greater than or equal"),
            ("IS", "is null/not null"),
            ("IN", "in list"),
            ("LIKE", "pattern match"),
            ("BETWEEN", "range"),
        ];
        for (op, detail) in operators {
            self.items
                .push(CompletionItem::new(op, CompletionKind::Operator).with_detail(detail));
        }
    }

    /// Moves selection up.
    pub fn select_previous(&mut self) {
        if !self.items.is_empty() {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    /// Moves selection down.
    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1).min(self.items.len() - 1);
        }
    }

    /// Returns the currently selected item.
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.items.get(self.selected)
    }

    /// Closes the completion popup.
    pub fn close(&mut self) {
        self.visible = false;
        self.items.clear();
        self.selected = 0;
        self.filter.clear();
    }
}

/// SQL completion popup widget.
pub struct SqlCompletionPopup<'a> {
    state: &'a SqlCompletionState,
}

impl<'a> SqlCompletionPopup<'a> {
    /// Creates a new SQL completion popup widget.
    pub fn new(state: &'a SqlCompletionState) -> Self {
        Self { state }
    }

    /// Maximum visible items per FR-3.6.
    const MAX_VISIBLE_ITEMS: u16 = 8;

    /// Calculates the area for the popup.
    pub fn popup_area(input_area: Rect, item_count: usize) -> Rect {
        let width = input_area.width.min(60);
        // Cap to 8 visible items per FR-3.6, +2 for borders
        let visible_items = (item_count as u16).min(Self::MAX_VISIBLE_ITEMS);
        let height = visible_items + 2;

        let x = input_area.x + 1;
        let y = input_area.y.saturating_sub(height);

        Rect::new(x, y, width, height)
    }
}

/// Ranks a completion item for sorting (lower is better).
/// Per FR-3.5: exact prefix > case-insensitive prefix > substring > fuzzy > recency
fn rank_completion(text: &str, filter: &str, filter_lower: &str, recent: &HashSet<String>) -> u8 {
    if filter.is_empty() {
        // No filter: rank by recency only
        return if recent.contains(&text.to_lowercase()) {
            0
        } else {
            1
        };
    }

    let text_lower = text.to_lowercase();

    // 1. Exact prefix match (case-sensitive)
    if text.starts_with(filter) {
        return 0;
    }
    // 2. Case-insensitive prefix match
    if text_lower.starts_with(filter_lower) {
        return 1;
    }
    // 3. Substring match
    if text_lower.contains(filter_lower) {
        return 2;
    }
    // 4. Fuzzy match
    if fuzzy_match(&text_lower, filter_lower) {
        return 3;
    }
    // 5. Recency boost for items that don't match well
    if recent.contains(&text_lower) {
        return 4;
    }
    5
}

/// Simple fuzzy matching: all filter characters appear in order in text.
fn fuzzy_match(text: &str, filter: &str) -> bool {
    let mut text_chars = text.chars().peekable();
    for filter_char in filter.chars() {
        loop {
            match text_chars.next() {
                Some(tc) if tc == filter_char => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

impl Widget for SqlCompletionPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the area first
        Clear.render(area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(" SQL Completions ");

        let inner = block.inner(area);
        block.render(area, buf);

        // Render each completion item
        let mut y = inner.y;
        let visible_items = self.state.items.iter().enumerate().skip(
            self.state
                .selected
                .saturating_sub((inner.height as usize) / 2),
        );

        for (idx, item) in visible_items {
            if y >= inner.y + inner.height {
                break;
            }

            let is_selected = idx == self.state.selected;

            let bg_color = if is_selected {
                Color::DarkGray
            } else {
                Color::Reset
            };

            // Clear line with background
            for x in inner.x..inner.x + inner.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_bg(bg_color);
                }
            }

            // Build the line
            let kind_style = Style::default()
                .fg(item.kind.color())
                .add_modifier(Modifier::BOLD);
            let text_style = if is_selected {
                Style::default().fg(Color::White).bg(bg_color)
            } else {
                Style::default()
            };
            let detail_style = Style::default().fg(Color::DarkGray).bg(bg_color);

            let mut spans = vec![
                Span::styled(format!("[{}] ", item.kind.label()), kind_style),
                Span::styled(&item.text, text_style),
            ];

            if let Some(ref detail) = item.detail {
                spans.push(Span::styled(format!(" : {}", detail), detail_style));
            }

            let line = Line::from(spans);
            let paragraph = Paragraph::new(line);
            let line_area = Rect::new(inner.x, y, inner.width, 1);
            paragraph.render(line_area, buf);

            y += 1;
        }

        // Show "no completions" if empty
        if self.state.items.is_empty() {
            let no_match =
                Paragraph::new("No completions").style(Style::default().fg(Color::DarkGray));
            no_match.render(inner, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Column, Table};

    fn test_schema() -> Schema {
        Schema {
            tables: vec![
                Table {
                    name: "users".to_string(),
                    columns: vec![
                        Column::new("id", "integer"),
                        Column::new("name", "varchar(255)"),
                        Column::new("email", "varchar(255)"),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
                Table {
                    name: "orders".to_string(),
                    columns: vec![
                        Column::new("id", "integer"),
                        Column::new("user_id", "integer"),
                        Column::new("total", "decimal"),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
            ],
            foreign_keys: vec![],
        }
    }

    fn test_schema_with_fk() -> Schema {
        use crate::db::ForeignKey;
        Schema {
            tables: vec![
                Table {
                    name: "users".to_string(),
                    columns: vec![
                        Column::new("id", "integer"),
                        Column::new("name", "varchar(255)"),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
                Table {
                    name: "orders".to_string(),
                    columns: vec![
                        Column::new("id", "integer"),
                        Column::new("user_id", "integer"),
                        Column::new("total", "decimal"),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
            ],
            foreign_keys: vec![ForeignKey::new(
                "orders",
                vec!["user_id".to_string()],
                "users",
                vec!["id".to_string()],
            )],
        }
    }

    #[test]
    fn test_completion_after_from() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();
        state.update("SELECT * FROM ", 14, Some(&schema));

        assert!(state.visible);
        assert!(state.items.iter().any(|i| i.text == "users"));
        assert!(state.items.iter().any(|i| i.text == "orders"));
    }

    #[test]
    fn test_completion_after_select() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();
        state.update("SELECT ", 7, Some(&schema));

        assert!(state.visible);
        assert!(state.items.iter().any(|i| i.text == "*"));
    }

    #[test]
    fn test_completion_with_filter() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();
        state.update("SELECT * FROM us", 16, Some(&schema));

        assert!(state.visible);
        assert!(state.items.iter().any(|i| i.text == "users"));
        assert!(!state.items.iter().any(|i| i.text == "orders"));
    }

    #[test]
    fn test_completion_navigation() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();
        state.update("SELECT * FROM ", 14, Some(&schema));

        assert_eq!(state.selected, 0);
        state.select_next();
        assert_eq!(state.selected, 1);
        state.select_previous();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_completion_close() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();
        state.update("SELECT * FROM ", 14, Some(&schema));

        assert!(state.visible);
        state.close();
        assert!(!state.visible);
        assert!(state.items.is_empty());
    }

    #[test]
    fn test_completion_where_operator_context() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();
        // After column name in WHERE, should suggest operators
        state.update("SELECT * FROM users WHERE status ", 33, Some(&schema));

        assert!(state.visible, "Completion should be visible");
        assert!(
            state.items.iter().any(|i| i.text == "="),
            "Should have '=' operator"
        );
        assert!(
            state.items.iter().any(|i| i.text == "LIKE"),
            "Should have 'LIKE' operator"
        );
    }

    #[test]
    fn test_completion_where_value_context() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();
        // After operator in WHERE, should suggest values
        state.update("SELECT * FROM users WHERE status = ", 35, Some(&schema));

        assert!(state.visible, "Completion should be visible");
        assert!(
            state.items.iter().any(|i| i.text == "TRUE"),
            "Should have 'TRUE' value"
        );
        assert!(
            state.items.iter().any(|i| i.text == "NULL"),
            "Should have 'NULL' value"
        );
    }

    #[test]
    fn test_completion_where_continuation_context() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();
        // After complete condition, should suggest AND/OR
        state.update("SELECT * FROM users WHERE id = 1 ", 33, Some(&schema));

        assert!(state.visible, "Completion should be visible");
        assert!(
            state.items.iter().any(|i| i.text == "AND"),
            "Should have 'AND' keyword"
        );
        assert!(
            state.items.iter().any(|i| i.text == "OR"),
            "Should have 'OR' keyword"
        );
    }

    #[test]
    fn test_fk_join_suggestion_from_orders() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema_with_fk();
        // When orders is in query, should suggest users with FK join template
        state.update("SELECT * FROM orders JOIN ", 26, Some(&schema));

        assert!(state.visible, "Completion should be visible");
        // Should have users as FK-related table suggestion
        assert!(
            state.items.iter().any(|i| i.text == "users"),
            "Should suggest 'users' table via FK"
        );
        // Should have ON clause template
        assert!(
            state
                .items
                .iter()
                .any(|i| i.text.contains("ON") && i.text.contains("user_id")),
            "Should have FK join template with ON clause"
        );
    }

    #[test]
    fn test_fk_join_suggestion_from_users() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema_with_fk();
        // When users is in query, should suggest orders with FK join template
        state.update("SELECT * FROM users JOIN ", 25, Some(&schema));

        assert!(state.visible, "Completion should be visible");
        // Should have orders as FK-related table suggestion
        assert!(
            state.items.iter().any(|i| i.text == "orders"),
            "Should suggest 'orders' table via FK"
        );
    }

    #[test]
    fn test_recency_ranking() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();

        // Record "orders" as recently used
        state.record_completion("orders");

        // Update with no filter
        state.update("SELECT * FROM ", 14, Some(&schema));

        assert!(state.visible);
        // "orders" should be ranked first due to recency
        assert_eq!(
            state.items.first().map(|i| i.text.as_str()),
            Some("orders"),
            "Recently used 'orders' should be first"
        );
    }

    #[test]
    fn test_fuzzy_matching() {
        let mut state = SqlCompletionState::new();
        let schema = test_schema();

        // "usr" should fuzzy match "users" (u-s-r all appear in order)
        state.update("SELECT * FROM usr", 17, Some(&schema));

        assert!(state.visible, "Completion should be visible");
        assert!(
            state.items.iter().any(|i| i.text == "users"),
            "Should fuzzy match 'users' with 'usr'"
        );
    }

    #[test]
    fn test_ranking_order() {
        let mut state = SqlCompletionState::new();
        // Create schema with tables that test ranking
        let schema = Schema {
            tables: vec![
                Table {
                    name: "Users".to_string(), // Exact case
                    columns: vec![],
                    primary_key: vec![],
                    indexes: vec![],
                },
                Table {
                    name: "users_archive".to_string(), // Prefix match
                    columns: vec![],
                    primary_key: vec![],
                    indexes: vec![],
                },
                Table {
                    name: "active_users".to_string(), // Substring match
                    columns: vec![],
                    primary_key: vec![],
                    indexes: vec![],
                },
            ],
            foreign_keys: vec![],
        };

        state.update("SELECT * FROM User", 18, Some(&schema));

        assert!(state.visible);
        // Exact prefix "User" should match "Users" first
        let texts: Vec<&str> = state.items.iter().map(|i| i.text.as_str()).collect();
        assert!(
            texts.iter().position(|t| *t == "Users")
                < texts.iter().position(|t| *t == "users_archive"),
            "Exact prefix match should rank before case-insensitive prefix"
        );
    }

    #[test]
    fn test_popup_height_capped() {
        // Verify MAX_VISIBLE_ITEMS is 8 per FR-3.6
        assert_eq!(
            SqlCompletionPopup::MAX_VISIBLE_ITEMS,
            8,
            "Popup should show max 8 items per FR-3.6"
        );

        // Test popup_area caps height correctly
        let input_area = Rect::new(0, 20, 80, 3);
        let area = SqlCompletionPopup::popup_area(input_area, 20); // 20 items
                                                                   // Height should be 8 + 2 (borders) = 10, not 20 + 2
        assert_eq!(
            area.height, 10,
            "Popup height should be capped at 8 items + 2 borders"
        );
    }
}
