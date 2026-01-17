//! Query result types for Glance.
//!
//! Defines the structures used to represent query results from the database.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// Represents the result of executing a SQL query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct QueryResult {
    /// Column metadata for the result set.
    pub columns: Vec<ColumnInfo>,

    /// Rows of data.
    pub rows: Vec<Row>,

    /// Time taken to execute the query.
    #[serde(with = "duration_serde")]
    pub execution_time: Duration,

    /// Number of rows in the result (may be truncated).
    pub row_count: usize,

    /// Total number of rows before truncation (if known).
    pub total_rows: Option<usize>,

    /// Whether the result was truncated due to exceeding MAX_ROWS.
    #[serde(default)]
    pub was_truncated: bool,
}

#[allow(dead_code)]
impl QueryResult {
    /// Creates a new empty query result.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a query result with the given columns and rows.
    pub fn with_data(columns: Vec<ColumnInfo>, rows: Vec<Row>) -> Self {
        let row_count = rows.len();
        Self {
            columns,
            rows,
            execution_time: Duration::ZERO,
            row_count,
            total_rows: Some(row_count),
            was_truncated: false,
        }
    }

    /// Sets the execution time.
    pub fn with_execution_time(mut self, duration: Duration) -> Self {
        self.execution_time = duration;
        self
    }

    /// Returns true if the result set is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Returns a truncation warning message if the result was truncated.
    pub fn truncation_warning(&self) -> Option<String> {
        if self.was_truncated {
            let total = self.total_rows.unwrap_or(self.row_count);
            Some(format!(
                "⚠ Result truncated: showing {} of {} rows",
                self.row_count, total
            ))
        } else {
            None
        }
    }
}

/// Metadata about a column in a result set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ColumnInfo {
    /// Column name.
    pub name: String,

    /// Column data type.
    pub data_type: String,
}

#[allow(dead_code)]
impl ColumnInfo {
    /// Creates a new column info with the given name and type.
    pub fn new(name: impl Into<String>, data_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: data_type.into(),
        }
    }
}

/// A row of data from a query result.
#[allow(dead_code)]
pub type Row = Vec<Value>;

/// Represents a single value from a database query.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub enum Value {
    /// NULL value.
    #[default]
    Null,

    /// Boolean value.
    Bool(bool),

    /// Signed integer (up to i64).
    Int(i64),

    /// Floating point number.
    Float(f64),

    /// Text/string value.
    String(String),

    /// Binary data.
    Bytes(Vec<u8>),
}

#[allow(dead_code)]
impl Value {
    /// Returns true if this value is NULL.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Attempts to convert the value to a string representation.
    pub fn to_display_string(&self) -> String {
        match self {
            Value::Null => "NULL".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::String(s) => s.clone(),
            Value::Bytes(b) => format!("<{} bytes>", b.len()),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

// Conversion implementations for common types
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(v as i64)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl<T> From<Option<T>> for Value
where
    T: Into<Value>,
{
    fn from(v: Option<T>) -> Self {
        match v {
            Some(val) => val.into(),
            None => Value::Null,
        }
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Bytes(v)
    }
}

/// Serde support for Duration (not natively supported by serde).
#[allow(dead_code)]
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_nanos().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let nanos = u128::deserialize(deserializer)?;
        Ok(Duration::from_nanos(nanos as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_display() {
        assert_eq!(Value::Null.to_display_string(), "NULL");
        assert_eq!(Value::Bool(true).to_display_string(), "true");
        assert_eq!(Value::Int(42).to_display_string(), "42");
        assert_eq!(Value::Float(2.71).to_display_string(), "2.71");
        assert_eq!(
            Value::String("hello".to_string()).to_display_string(),
            "hello"
        );
        assert_eq!(Value::Bytes(vec![1, 2, 3]).to_display_string(), "<3 bytes>");
    }

    #[test]
    fn test_value_is_null() {
        assert!(Value::Null.is_null());
        assert!(!Value::Bool(false).is_null());
        assert!(!Value::Int(0).is_null());
    }

    #[test]
    fn test_value_from_conversions() {
        assert_eq!(Value::from(true), Value::Bool(true));
        assert_eq!(Value::from(42i32), Value::Int(42));
        assert_eq!(Value::from(42i64), Value::Int(42));
        assert_eq!(Value::from(2.71f64), Value::Float(2.71));
        assert_eq!(
            Value::from("hello".to_string()),
            Value::String("hello".to_string())
        );
        assert_eq!(Value::from("hello"), Value::String("hello".to_string()));
        assert_eq!(Value::from(None::<i32>), Value::Null);
        assert_eq!(Value::from(Some(42i32)), Value::Int(42));
    }

    #[test]
    fn test_query_result_new() {
        let result = QueryResult::new();
        assert!(result.is_empty());
        assert_eq!(result.row_count, 0);
    }

    #[test]
    fn test_query_result_with_data() {
        let columns = vec![
            ColumnInfo::new("id", "integer"),
            ColumnInfo::new("name", "varchar"),
        ];
        let rows = vec![
            vec![Value::Int(1), Value::String("Alice".to_string())],
            vec![Value::Int(2), Value::String("Bob".to_string())],
        ];

        let result = QueryResult::with_data(columns, rows);

        assert!(!result.is_empty());
        assert_eq!(result.row_count, 2);
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_query_result_with_execution_time() {
        let result = QueryResult::new().with_execution_time(Duration::from_millis(100));
        assert_eq!(result.execution_time, Duration::from_millis(100));
    }

    #[test]
    fn test_column_info_new() {
        let col = ColumnInfo::new("email", "varchar(255)");
        assert_eq!(col.name, "email");
        assert_eq!(col.data_type, "varchar(255)");
    }
}
