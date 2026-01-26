//! Database schema types for Glance.
//!
//! Represents the structure of a database including tables, columns,
//! foreign keys, and indexes.

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Represents the complete schema of a database.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Schema {
    /// All tables in the schema.
    pub tables: Vec<Table>,

    /// Foreign key relationships between tables.
    pub foreign_keys: Vec<ForeignKey>,
}

#[allow(dead_code)]
impl Schema {
    /// Creates a new empty schema.
    pub fn new() -> Self {
        Self::default()
    }

    /// Formats the schema for inclusion in an LLM system prompt.
    ///
    /// Produces a human-readable representation that helps the LLM
    /// understand the database structure.
    pub fn format_for_llm(&self) -> String {
        let tables_text = self
            .tables
            .iter()
            .map(|table| self.format_table_for_llm(table))
            .collect::<Vec<_>>()
            .join("");

        let foreign_keys_text = if self.foreign_keys.is_empty() {
            String::new()
        } else {
            let fk_lines = self
                .foreign_keys
                .iter()
                .map(|fk| {
                    format!(
                        "  - {}.{} -> {}.{}\n",
                        fk.from_table,
                        fk.from_columns.join(", "),
                        fk.to_table,
                        fk.to_columns.join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            format!("Foreign Keys:\n{}", fk_lines)
        };

        format!("Database Schema:\n\n{}{}", tables_text, foreign_keys_text)
    }

    fn format_table_for_llm(&self, table: &Table) -> String {
        let column_lines = table
            .columns
            .iter()
            .map(|column| self.format_column_for_llm(table, column))
            .collect::<Vec<_>>()
            .join("");

        format!("Table: {}\n{}\n", table.name, column_lines)
    }

    fn format_column_for_llm(&self, table: &Table, column: &Column) -> String {
        let annotations = [
            table.primary_key.contains(&column.name).then_some("PK"),
            (!column.is_nullable).then_some("NOT NULL"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        let base_annotation = annotations.join(", ");

        let fk_lines = self
            .foreign_keys
            .iter()
            .filter(|fk| fk.from_table == table.name && fk.from_columns.contains(&column.name))
            .map(|fk| {
                let fk_ref = format!(
                    "FK -> {}.{}",
                    fk.to_table,
                    fk.to_columns.first().map(String::as_str).unwrap_or("")
                );
                let annotation = if base_annotation.is_empty() {
                    fk_ref
                } else {
                    format!("{}, {}", base_annotation, fk_ref)
                };
                Self::format_column_line(column, Some(annotation.as_str()))
            })
            .collect::<Vec<_>>();

        let base_line = if base_annotation.is_empty() {
            Self::format_column_line(column, None)
        } else {
            Self::format_column_line(column, Some(base_annotation.as_str()))
        };

        fk_lines
            .into_iter()
            .chain(std::iter::once(base_line))
            .collect::<Vec<_>>()
            .join("")
    }

    fn format_column_line(column: &Column, annotation: Option<&str>) -> String {
        let annotation = annotation.filter(|value| !value.is_empty());
        match (annotation, &column.default) {
            (Some(annotation), Some(default)) => format!(
                "  - {}: {} ({}, DEFAULT {})\n",
                column.name, column.data_type, annotation, default
            ),
            (Some(annotation), None) => {
                format!(
                    "  - {}: {} ({})\n",
                    column.name, column.data_type, annotation
                )
            }
            (None, Some(default)) => format!(
                "  - {}: {} (DEFAULT {})\n",
                column.name, column.data_type, default
            ),
            (None, None) => format!("  - {}: {}\n", column.name, column.data_type),
        }
    }

    /// Formats the schema for display in the TUI.
    pub fn format_for_display(&self) -> String {
        self.format_for_llm()
    }

    /// Computes a hash of the schema content for cache invalidation.
    pub fn content_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        // Hash table count and names
        self.tables.len().hash(&mut hasher);
        for table in &self.tables {
            table.name.hash(&mut hasher);
            table.columns.len().hash(&mut hasher);
            for col in &table.columns {
                col.name.hash(&mut hasher);
                col.data_type.hash(&mut hasher);
                col.is_nullable.hash(&mut hasher);
                col.default.hash(&mut hasher);
            }
            table.primary_key.hash(&mut hasher);
        }
        // Hash foreign keys
        self.foreign_keys.len().hash(&mut hasher);
        for fk in &self.foreign_keys {
            fk.from_table.hash(&mut hasher);
            fk.from_columns.hash(&mut hasher);
            fk.to_table.hash(&mut hasher);
            fk.to_columns.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// Represents a database table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Table {
    /// Table name.
    pub name: String,

    /// Columns in the table.
    pub columns: Vec<Column>,

    /// Column names that form the primary key.
    pub primary_key: Vec<String>,

    /// Indexes on the table.
    pub indexes: Vec<Index>,
}

#[allow(dead_code)]
impl Table {
    /// Creates a new table with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            columns: Vec::new(),
            primary_key: Vec::new(),
            indexes: Vec::new(),
        }
    }
}

/// Represents a column in a table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Column {
    /// Column name.
    pub name: String,

    /// Data type (e.g., "integer", "varchar(255)").
    pub data_type: String,

    /// Whether the column allows NULL values.
    pub is_nullable: bool,

    /// Default value expression, if any.
    pub default: Option<String>,
}

#[allow(dead_code)]
impl Column {
    /// Creates a new column with the given name and data type.
    pub fn new(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
            is_nullable: true,
            default: None,
        }
    }

    /// Sets whether the column is nullable.
    pub fn nullable(self, nullable: bool) -> Self {
        Self {
            is_nullable: nullable,
            ..self
        }
    }

    /// Sets the default value.
    pub fn with_default(self, default: impl Into<String>) -> Self {
        Self {
            default: Some(default.into()),
            ..self
        }
    }
}

/// Represents a foreign key relationship between tables.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ForeignKey {
    /// Source table name.
    pub from_table: String,

    /// Source column names.
    pub from_columns: Vec<String>,

    /// Target table name.
    pub to_table: String,

    /// Target column names.
    pub to_columns: Vec<String>,
}

#[allow(dead_code)]
impl ForeignKey {
    /// Creates a new foreign key relationship.
    pub fn new(
        from_table: impl Into<String>,
        from_columns: Vec<String>,
        to_table: impl Into<String>,
        to_columns: Vec<String>,
    ) -> Self {
        Self {
            from_table: from_table.into(),
            from_columns,
            to_table: to_table.into(),
            to_columns,
        }
    }
}

/// Represents an index on a table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Index {
    /// Index name.
    pub name: String,

    /// Column names included in the index.
    pub columns: Vec<String>,

    /// Whether this is a unique index.
    pub is_unique: bool,
}

#[allow(dead_code)]
impl Index {
    /// Creates a new index with the given name and columns.
    pub fn new(name: impl Into<String>, columns: Vec<String>) -> Self {
        Self {
            name: name.into(),
            columns,
            is_unique: false,
        }
    }

    /// Sets whether the index is unique.
    pub fn unique(self, unique: bool) -> Self {
        Self {
            is_unique: unique,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_schema() -> Schema {
        Schema {
            tables: vec![
                Table {
                    name: "users".to_string(),
                    columns: vec![
                        Column::new("id", "integer").nullable(false),
                        Column::new("email", "varchar(255)").nullable(false),
                        Column::new("name", "varchar(100)"),
                        Column::new("created_at", "timestamp")
                            .nullable(false)
                            .with_default("now()"),
                    ],
                    primary_key: vec!["id".to_string()],
                    indexes: vec![],
                },
                Table {
                    name: "orders".to_string(),
                    columns: vec![
                        Column::new("id", "integer").nullable(false),
                        Column::new("user_id", "integer").nullable(false),
                        Column::new("total", "numeric(10,2)").nullable(false),
                        Column::new("status", "varchar(20)")
                            .nullable(false)
                            .with_default("'pending'"),
                        Column::new("created_at", "timestamp")
                            .nullable(false)
                            .with_default("now()"),
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
    fn test_schema_format_for_llm() {
        let schema = sample_schema();
        let formatted = schema.format_for_llm();

        assert!(formatted.contains("Table: users"));
        assert!(formatted.contains("Table: orders"));
        assert!(formatted.contains("id: integer (PK, NOT NULL)"));
        assert!(formatted.contains("email: varchar(255) (NOT NULL)"));
        assert!(formatted.contains("created_at: timestamp (NOT NULL, DEFAULT now())"));
        assert!(formatted.contains("Foreign Keys:"));
        assert!(formatted.contains("orders.user_id -> users.id"));
    }

    #[test]
    fn test_column_builder() {
        let col = Column::new("email", "varchar(255)")
            .nullable(false)
            .with_default("''");

        assert_eq!(col.name, "email");
        assert_eq!(col.data_type, "varchar(255)");
        assert!(!col.is_nullable);
        assert_eq!(col.default, Some("''".to_string()));
    }

    #[test]
    fn test_table_new() {
        let table = Table::new("users");
        assert_eq!(table.name, "users");
        assert!(table.columns.is_empty());
        assert!(table.primary_key.is_empty());
    }

    #[test]
    fn test_foreign_key_new() {
        let fk = ForeignKey::new(
            "orders",
            vec!["user_id".to_string()],
            "users",
            vec!["id".to_string()],
        );

        assert_eq!(fk.from_table, "orders");
        assert_eq!(fk.from_columns, vec!["user_id"]);
        assert_eq!(fk.to_table, "users");
        assert_eq!(fk.to_columns, vec!["id"]);
    }

    #[test]
    fn test_index_builder() {
        let idx = Index::new("idx_users_email", vec!["email".to_string()]).unique(true);

        assert_eq!(idx.name, "idx_users_email");
        assert_eq!(idx.columns, vec!["email"]);
        assert!(idx.is_unique);
    }

    #[test]
    fn test_empty_schema() {
        let schema = Schema::new();
        let formatted = schema.format_for_llm();

        assert!(formatted.contains("Database Schema:"));
        assert!(!formatted.contains("Foreign Keys:"));
    }
}
