//! Query safety classification module.
//!
//! Parses SQL and classifies queries as safe, mutating, or destructive
//! to determine whether user confirmation is required before execution.

mod parser;

#[allow(unused_imports)] // Will be used in Phase 8
pub use parser::{classify_sql, SqlClassifier};

use std::fmt;

/// Safety level classification for SQL queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)] // Will be used in Phase 8
pub enum SafetyLevel {
    /// Read-only queries that can be auto-executed (SELECT, EXPLAIN, SHOW).
    Safe,
    /// Data modification queries that require confirmation (INSERT, UPDATE).
    Mutating,
    /// Potentially destructive queries requiring confirmation with warning
    /// (DELETE, DROP, TRUNCATE, ALTER).
    Destructive,
}

#[allow(dead_code)] // Will be used in Phase 8
impl SafetyLevel {
    /// Returns true if this safety level requires user confirmation.
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, Self::Mutating | Self::Destructive)
    }

    /// Returns true if this safety level should show a warning.
    pub fn requires_warning(&self) -> bool {
        matches!(self, Self::Destructive)
    }
}

impl fmt::Display for SafetyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Safe => write!(f, "Safe"),
            Self::Mutating => write!(f, "Mutating"),
            Self::Destructive => write!(f, "Destructive"),
        }
    }
}

/// The type of SQL statement detected.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Will be used in Phase 8
pub enum StatementType {
    Select,
    Insert,
    Update,
    Delete,
    Drop,
    Truncate,
    Alter,
    Create,
    Grant,
    Revoke,
    Explain,
    Show,
    With,
    Merge,
    /// Multiple statements detected; contains the most dangerous type.
    Multiple(Box<StatementType>),
    /// Statement type could not be determined.
    Unknown,
}

impl fmt::Display for StatementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Select => write!(f, "SELECT"),
            Self::Insert => write!(f, "INSERT"),
            Self::Update => write!(f, "UPDATE"),
            Self::Delete => write!(f, "DELETE"),
            Self::Drop => write!(f, "DROP"),
            Self::Truncate => write!(f, "TRUNCATE"),
            Self::Alter => write!(f, "ALTER"),
            Self::Create => write!(f, "CREATE"),
            Self::Grant => write!(f, "GRANT"),
            Self::Revoke => write!(f, "REVOKE"),
            Self::Explain => write!(f, "EXPLAIN"),
            Self::Show => write!(f, "SHOW"),
            Self::With => write!(f, "WITH (CTE)"),
            Self::Merge => write!(f, "MERGE"),
            Self::Multiple(inner) => write!(f, "Multiple ({})", inner),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Result of classifying a SQL query.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Will be used in Phase 8
pub struct ClassificationResult {
    /// The determined safety level.
    pub level: SafetyLevel,
    /// The type of statement(s) detected.
    pub statement_type: StatementType,
    /// Optional warning message for the user.
    pub warning: Option<String>,
}

#[allow(dead_code)] // Will be used in Phase 8
impl ClassificationResult {
    /// Creates a new classification result.
    pub fn new(level: SafetyLevel, statement_type: StatementType) -> Self {
        Self {
            level,
            statement_type,
            warning: None,
        }
    }

    /// Creates a classification result with a warning message.
    pub fn with_warning(
        level: SafetyLevel,
        statement_type: StatementType,
        warning: impl Into<String>,
    ) -> Self {
        Self {
            level,
            statement_type,
            warning: Some(warning.into()),
        }
    }

    /// Returns true if user confirmation is required.
    pub fn requires_confirmation(&self) -> bool {
        self.level.requires_confirmation()
    }

    /// Returns true if a warning should be displayed.
    pub fn requires_warning(&self) -> bool {
        self.level.requires_warning()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safety_level_display() {
        assert_eq!(SafetyLevel::Safe.to_string(), "Safe");
        assert_eq!(SafetyLevel::Mutating.to_string(), "Mutating");
        assert_eq!(SafetyLevel::Destructive.to_string(), "Destructive");
    }

    #[test]
    fn test_safety_level_requires_confirmation() {
        assert!(!SafetyLevel::Safe.requires_confirmation());
        assert!(SafetyLevel::Mutating.requires_confirmation());
        assert!(SafetyLevel::Destructive.requires_confirmation());
    }

    #[test]
    fn test_safety_level_requires_warning() {
        assert!(!SafetyLevel::Safe.requires_warning());
        assert!(!SafetyLevel::Mutating.requires_warning());
        assert!(SafetyLevel::Destructive.requires_warning());
    }

    #[test]
    fn test_statement_type_display() {
        assert_eq!(StatementType::Select.to_string(), "SELECT");
        assert_eq!(StatementType::Insert.to_string(), "INSERT");
        assert_eq!(StatementType::Delete.to_string(), "DELETE");
        assert_eq!(
            StatementType::Multiple(Box::new(StatementType::Delete)).to_string(),
            "Multiple (DELETE)"
        );
    }

    #[test]
    fn test_classification_result_new() {
        let result = ClassificationResult::new(SafetyLevel::Safe, StatementType::Select);
        assert_eq!(result.level, SafetyLevel::Safe);
        assert_eq!(result.statement_type, StatementType::Select);
        assert!(result.warning.is_none());
        assert!(!result.requires_confirmation());
        assert!(!result.requires_warning());
    }

    #[test]
    fn test_classification_result_with_warning() {
        let result = ClassificationResult::with_warning(
            SafetyLevel::Destructive,
            StatementType::Delete,
            "This will delete data",
        );
        assert_eq!(result.level, SafetyLevel::Destructive);
        assert_eq!(result.statement_type, StatementType::Delete);
        assert_eq!(result.warning, Some("This will delete data".to_string()));
        assert!(result.requires_confirmation());
        assert!(result.requires_warning());
    }
}
