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
}

impl CompletionKind {
    /// Returns a short label for the completion kind.
    fn label(&self) -> &'static str {
        match self {
            Self::Table => "tbl",
            Self::Column => "col",
            Self::Keyword => "kw",
            Self::Function => "fn",
        }
    }

    /// Returns the color for the completion kind.
    fn color(&self) -> Color {
        match self {
            Self::Table => Color::Yellow,
            Self::Column => Color::Cyan,
            Self::Keyword => Color::Magenta,
            Self::Function => Color::Green,
        }
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
            SqlContext::FromTable | SqlContext::JoinTable => {
                // Suggest table names
                if let Some(schema) = schema {
                    self.add_tables(schema);
                }
            }
            SqlContext::WhereClause | SqlContext::JoinCondition => {
                // Suggest columns from referenced tables
                if let Some(schema) = schema {
                    self.add_columns_from_tables(schema, &result.tables);
                }
                self.add_keywords(&["AND", "OR", "NOT", "IN", "LIKE", "BETWEEN", "IS", "NULL"]);
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

        // Filter items by current word
        if !self.filter.is_empty() {
            let filter_lower = self.filter.to_lowercase();
            self.items.retain(|item| {
                item.text.to_lowercase().starts_with(&filter_lower)
                    || item.text.to_lowercase().contains(&filter_lower)
            });
        }

        // Sort: prefix matches first, then alphabetically
        let filter_lower = self.filter.to_lowercase();
        self.items.sort_by(|a, b| {
            let a_prefix = a.text.to_lowercase().starts_with(&filter_lower);
            let b_prefix = b.text.to_lowercase().starts_with(&filter_lower);
            match (a_prefix, b_prefix) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.text.to_lowercase().cmp(&b.text.to_lowercase()),
            }
        });

        // Update visibility
        self.visible = !self.items.is_empty();
        self.selected = 0;
    }

    /// Adds table names from the schema.
    fn add_tables(&mut self, schema: &Schema) {
        for table in &schema.tables {
            self.items
                .push(CompletionItem::new(&table.name, CompletionKind::Table));
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

    /// Calculates the area for the popup.
    pub fn popup_area(input_area: Rect, max_items: usize) -> Rect {
        let width = input_area.width.min(60);
        let height = (max_items as u16 + 2).min(12); // +2 for borders

        let x = input_area.x + 1;
        let y = input_area.y.saturating_sub(height);

        Rect::new(x, y, width, height)
    }
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
}
