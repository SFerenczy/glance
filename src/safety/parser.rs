//! SQL parsing and classification logic.
//!
//! Uses sqlparser-rs with PostgreSQL dialect to parse SQL and classify
//! statements by their safety level.

use sqlparser::ast::{Query, Select, SetExpr, Statement, TableFactor, TableWithJoins};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::error::{GlanceError, Result};

use super::{ClassificationResult, SafetyLevel, StatementType};

/// SQL classifier that parses and classifies SQL queries.
#[derive(Debug)]
#[allow(dead_code)] // Will be used in Phase 8
pub struct SqlClassifier {
    dialect: PostgreSqlDialect,
}

impl Default for SqlClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SqlClassifier {
    /// Creates a new SQL classifier.
    pub fn new() -> Self {
        Self {
            dialect: PostgreSqlDialect {},
        }
    }

    /// Classifies a SQL string and returns the classification result.
    ///
    /// If the SQL cannot be parsed, it is treated as destructive (conservative default)
    /// with a warning message.
    pub fn classify(&self, sql: &str) -> ClassificationResult {
        match self.parse_and_classify(sql) {
            Ok(result) => result,
            Err(_) => ClassificationResult::with_warning(
                SafetyLevel::Destructive,
                StatementType::Unknown,
                "Could not parse SQL. Please review carefully.",
            ),
        }
    }

    fn parse_and_classify(&self, sql: &str) -> Result<ClassificationResult> {
        let statements = Parser::parse_sql(&self.dialect, sql)
            .map_err(|e| GlanceError::query(format!("SQL parse error: {}", e)))?;

        if statements.is_empty() {
            return Ok(ClassificationResult::with_warning(
                SafetyLevel::Destructive,
                StatementType::Unknown,
                "Empty SQL statement",
            ));
        }

        if statements.len() == 1 {
            let (level, stmt_type) = classify_statement(&statements[0]);
            let result = if level == SafetyLevel::Destructive {
                ClassificationResult::with_warning(
                    level,
                    stmt_type,
                    "This action cannot be undone.",
                )
            } else {
                ClassificationResult::new(level, stmt_type)
            };
            return Ok(result);
        }

        // Multiple statements: use the most dangerous classification
        let mut max_level = SafetyLevel::Safe;
        let mut max_stmt_type = StatementType::Unknown;

        for stmt in &statements {
            let (level, stmt_type) = classify_statement(stmt);
            if level_priority(&level) > level_priority(&max_level) {
                max_level = level;
                max_stmt_type = stmt_type;
            }
        }

        let result = if max_level == SafetyLevel::Destructive {
            ClassificationResult::with_warning(
                max_level,
                StatementType::Multiple(Box::new(max_stmt_type)),
                "This action cannot be undone.",
            )
        } else {
            ClassificationResult::new(max_level, StatementType::Multiple(Box::new(max_stmt_type)))
        };

        Ok(result)
    }
}

/// Convenience function to classify SQL without creating a classifier instance.
#[allow(dead_code)] // Will be used in Phase 8
pub fn classify_sql(sql: &str) -> ClassificationResult {
    SqlClassifier::new().classify(sql)
}

/// Returns a priority value for safety levels (higher = more dangerous).
fn level_priority(level: &SafetyLevel) -> u8 {
    match level {
        SafetyLevel::Safe => 0,
        SafetyLevel::Mutating => 1,
        SafetyLevel::Destructive => 2,
    }
}

/// Classifies a single parsed statement.
fn classify_statement(statement: &Statement) -> (SafetyLevel, StatementType) {
    match statement {
        // Query: may contain data-modifying CTEs, so recurse
        Statement::Query(query) => classify_query(query),
        Statement::Explain {
            analyze, statement, ..
        } => {
            if *analyze {
                // EXPLAIN ANALYZE executes the query - inherit inner statement's safety level
                let (inner_level, _) = classify_statement(statement);
                (inner_level, StatementType::Explain)
            } else {
                // Plain EXPLAIN only shows the plan, doesn't execute
                (SafetyLevel::Safe, StatementType::Explain)
            }
        }
        Statement::ShowVariable { .. } => (SafetyLevel::Safe, StatementType::Show),
        Statement::ShowTables { .. } => (SafetyLevel::Safe, StatementType::Show),
        Statement::ShowColumns { .. } => (SafetyLevel::Safe, StatementType::Show),
        Statement::ShowCreate { .. } => (SafetyLevel::Safe, StatementType::Show),
        Statement::ShowFunctions { .. } => (SafetyLevel::Safe, StatementType::Show),
        Statement::ShowStatus { .. } => (SafetyLevel::Safe, StatementType::Show),
        Statement::ShowCollation { .. } => (SafetyLevel::Safe, StatementType::Show),

        // Mutating: data modification
        Statement::Insert(_) => (SafetyLevel::Mutating, StatementType::Insert),
        Statement::Update { .. } => (SafetyLevel::Mutating, StatementType::Update),
        Statement::Merge { .. } => (SafetyLevel::Mutating, StatementType::Merge),

        // Destructive: data loss or schema changes
        Statement::Delete(_) => (SafetyLevel::Destructive, StatementType::Delete),
        Statement::Drop { .. } => (SafetyLevel::Destructive, StatementType::Drop),
        Statement::Truncate { .. } => (SafetyLevel::Destructive, StatementType::Truncate),
        Statement::AlterTable { .. } => (SafetyLevel::Destructive, StatementType::Alter),
        Statement::AlterIndex { .. } => (SafetyLevel::Destructive, StatementType::Alter),
        Statement::AlterView { .. } => (SafetyLevel::Destructive, StatementType::Alter),
        Statement::AlterRole { .. } => (SafetyLevel::Destructive, StatementType::Alter),
        Statement::CreateTable { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::CreateIndex { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::CreateView { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::CreateSchema { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::CreateDatabase { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::CreateFunction { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::CreateProcedure { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::CreateRole { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::CreateSequence { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::CreateType { .. } => (SafetyLevel::Destructive, StatementType::Create),
        Statement::Grant { .. } => (SafetyLevel::Destructive, StatementType::Grant),
        Statement::Revoke { .. } => (SafetyLevel::Destructive, StatementType::Revoke),

        // Conservative default: treat unknown statements as destructive
        _ => (SafetyLevel::Destructive, StatementType::Unknown),
    }
}

/// Classifies a Query by recursively inspecting for data-modifying operations.
/// Returns the most dangerous (SafetyLevel, StatementType) found.
fn classify_query(query: &Query) -> (SafetyLevel, StatementType) {
    let mut max_level = SafetyLevel::Safe;
    let mut max_type = StatementType::Select;

    // Check CTEs in WITH clause
    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let (level, stmt_type) = classify_query(&cte.query);
            if level_priority(&level) > level_priority(&max_level) {
                max_level = level;
                max_type = stmt_type;
            }
        }
    }

    // Check the main query body
    let (body_level, body_type) = classify_set_expr(&query.body);
    if level_priority(&body_level) > level_priority(&max_level) {
        max_level = body_level;
        max_type = body_type;
    }

    (max_level, max_type)
}

/// Classifies a SetExpr, detecting mutations and recursing into nested queries.
fn classify_set_expr(set_expr: &SetExpr) -> (SafetyLevel, StatementType) {
    match set_expr {
        // Direct mutations in CTE bodies (wrapped as Statement)
        SetExpr::Delete(stmt) => classify_statement(stmt),
        SetExpr::Update(stmt) => classify_statement(stmt),
        SetExpr::Insert(stmt) => classify_statement(stmt),
        SetExpr::Merge(stmt) => classify_statement(stmt),

        // Nested query - recurse
        SetExpr::Query(query) => classify_query(query),

        // SELECT - check FROM clause for subqueries
        SetExpr::Select(select) => classify_select(select),

        // Set operations (UNION, INTERSECT, EXCEPT) - check both sides
        SetExpr::SetOperation { left, right, .. } => {
            let (left_level, left_type) = classify_set_expr(left);
            let (right_level, right_type) = classify_set_expr(right);
            if level_priority(&left_level) >= level_priority(&right_level) {
                (left_level, left_type)
            } else {
                (right_level, right_type)
            }
        }

        // Values, Table - safe (no subqueries possible)
        SetExpr::Values(_) | SetExpr::Table(_) => (SafetyLevel::Safe, StatementType::Select),
    }
}

/// Classifies a Select by checking its FROM clause for subqueries.
fn classify_select(select: &Select) -> (SafetyLevel, StatementType) {
    let mut max_level = SafetyLevel::Safe;
    let mut max_type = StatementType::Select;

    for table_with_joins in &select.from {
        let (level, stmt_type) = classify_table_with_joins(table_with_joins);
        if level_priority(&level) > level_priority(&max_level) {
            max_level = level;
            max_type = stmt_type;
        }
    }

    (max_level, max_type)
}

/// Classifies a TableWithJoins, checking the main relation and all joins.
fn classify_table_with_joins(twj: &TableWithJoins) -> (SafetyLevel, StatementType) {
    let mut max_level = SafetyLevel::Safe;
    let mut max_type = StatementType::Select;

    // Check the main relation
    let (level, stmt_type) = classify_table_factor(&twj.relation);
    if level_priority(&level) > level_priority(&max_level) {
        max_level = level;
        max_type = stmt_type;
    }

    // Check all joins
    for join in &twj.joins {
        let (level, stmt_type) = classify_table_factor(&join.relation);
        if level_priority(&level) > level_priority(&max_level) {
            max_level = level;
            max_type = stmt_type;
        }
    }

    (max_level, max_type)
}

/// Classifies a TableFactor, recursing into derived tables (subqueries).
fn classify_table_factor(factor: &TableFactor) -> (SafetyLevel, StatementType) {
    match factor {
        TableFactor::Derived { subquery, .. } => classify_query(subquery),
        TableFactor::NestedJoin {
            table_with_joins, ..
        } => classify_table_with_joins(table_with_joins),
        // Other variants (Table, TableFunction, etc.) are safe
        _ => (SafetyLevel::Safe, StatementType::Select),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_classification(sql: &str, expected_level: SafetyLevel, expected_type: StatementType) {
        let result = classify_sql(sql);
        assert_eq!(
            result.level, expected_level,
            "SQL: '{}' - expected level {:?}, got {:?}",
            sql, expected_level, result.level
        );
        assert_eq!(
            result.statement_type, expected_type,
            "SQL: '{}' - expected type {:?}, got {:?}",
            sql, expected_type, result.statement_type
        );
    }

    // Safe queries
    #[test]
    fn test_select_is_safe() {
        assert_classification(
            "SELECT * FROM users",
            SafetyLevel::Safe,
            StatementType::Select,
        );
    }

    #[test]
    fn test_select_with_where_is_safe() {
        assert_classification(
            "SELECT id, name FROM users WHERE active = true",
            SafetyLevel::Safe,
            StatementType::Select,
        );
    }

    #[test]
    fn test_select_with_join_is_safe() {
        assert_classification(
            "SELECT u.name, o.total FROM users u JOIN orders o ON u.id = o.user_id",
            SafetyLevel::Safe,
            StatementType::Select,
        );
    }

    #[test]
    fn test_select_with_subquery_is_safe() {
        assert_classification(
            "SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)",
            SafetyLevel::Safe,
            StatementType::Select,
        );
    }

    #[test]
    fn test_explain_is_safe() {
        assert_classification(
            "EXPLAIN SELECT * FROM users",
            SafetyLevel::Safe,
            StatementType::Explain,
        );
    }

    #[test]
    fn test_explain_analyze_is_safe() {
        assert_classification(
            "EXPLAIN ANALYZE SELECT * FROM users",
            SafetyLevel::Safe,
            StatementType::Explain,
        );
    }

    #[test]
    fn test_explain_delete_without_analyze_is_safe() {
        // Without ANALYZE, EXPLAIN only shows the plan - doesn't execute
        assert_classification(
            "EXPLAIN DELETE FROM users",
            SafetyLevel::Safe,
            StatementType::Explain,
        );
    }

    #[test]
    fn test_explain_analyze_delete_is_destructive() {
        // EXPLAIN ANALYZE executes the query - DELETE is destructive
        assert_classification(
            "EXPLAIN ANALYZE DELETE FROM users",
            SafetyLevel::Destructive,
            StatementType::Explain,
        );
    }

    #[test]
    fn test_explain_analyze_update_is_mutating() {
        assert_classification(
            "EXPLAIN ANALYZE UPDATE users SET name = 'x'",
            SafetyLevel::Mutating,
            StatementType::Explain,
        );
    }

    #[test]
    fn test_explain_analyze_insert_is_mutating() {
        assert_classification(
            "EXPLAIN ANALYZE INSERT INTO users VALUES (1)",
            SafetyLevel::Mutating,
            StatementType::Explain,
        );
    }

    #[test]
    fn test_explain_analyze_drop_is_destructive() {
        // DROP TABLE is destructive
        assert_classification(
            "EXPLAIN ANALYZE DROP TABLE users",
            SafetyLevel::Destructive,
            StatementType::Explain,
        );
    }

    #[test]
    fn test_show_is_safe() {
        assert_classification("SHOW search_path", SafetyLevel::Safe, StatementType::Show);
    }

    // Mutating queries
    #[test]
    fn test_insert_is_mutating() {
        assert_classification(
            "INSERT INTO users (name, email) VALUES ('Alice', 'alice@test.com')",
            SafetyLevel::Mutating,
            StatementType::Insert,
        );
    }

    #[test]
    fn test_insert_select_is_mutating() {
        assert_classification(
            "INSERT INTO users_backup SELECT * FROM users",
            SafetyLevel::Mutating,
            StatementType::Insert,
        );
    }

    #[test]
    fn test_update_is_mutating() {
        assert_classification(
            "UPDATE users SET status = 'inactive' WHERE last_login < '2024-01-01'",
            SafetyLevel::Mutating,
            StatementType::Update,
        );
    }

    #[test]
    fn test_update_all_is_mutating() {
        assert_classification(
            "UPDATE users SET updated_at = NOW()",
            SafetyLevel::Mutating,
            StatementType::Update,
        );
    }

    // Destructive queries
    #[test]
    fn test_delete_is_destructive() {
        assert_classification(
            "DELETE FROM orders WHERE status = 'cancelled'",
            SafetyLevel::Destructive,
            StatementType::Delete,
        );
    }

    #[test]
    fn test_delete_all_is_destructive() {
        assert_classification(
            "DELETE FROM temp_data",
            SafetyLevel::Destructive,
            StatementType::Delete,
        );
    }

    #[test]
    fn test_drop_table_is_destructive() {
        assert_classification(
            "DROP TABLE users",
            SafetyLevel::Destructive,
            StatementType::Drop,
        );
    }

    #[test]
    fn test_drop_table_if_exists_is_destructive() {
        assert_classification(
            "DROP TABLE IF EXISTS temp_table",
            SafetyLevel::Destructive,
            StatementType::Drop,
        );
    }

    #[test]
    fn test_truncate_is_destructive() {
        assert_classification(
            "TRUNCATE TABLE logs",
            SafetyLevel::Destructive,
            StatementType::Truncate,
        );
    }

    #[test]
    fn test_alter_table_is_destructive() {
        assert_classification(
            "ALTER TABLE users ADD COLUMN phone VARCHAR(20)",
            SafetyLevel::Destructive,
            StatementType::Alter,
        );
    }

    #[test]
    fn test_alter_table_drop_column_is_destructive() {
        assert_classification(
            "ALTER TABLE users DROP COLUMN deprecated_field",
            SafetyLevel::Destructive,
            StatementType::Alter,
        );
    }

    #[test]
    fn test_create_table_is_destructive() {
        assert_classification(
            "CREATE TABLE new_table (id SERIAL PRIMARY KEY, name TEXT)",
            SafetyLevel::Destructive,
            StatementType::Create,
        );
    }

    #[test]
    fn test_create_index_is_destructive() {
        assert_classification(
            "CREATE INDEX idx_users_email ON users(email)",
            SafetyLevel::Destructive,
            StatementType::Create,
        );
    }

    #[test]
    fn test_grant_is_destructive() {
        assert_classification(
            "GRANT SELECT ON users TO readonly_user",
            SafetyLevel::Destructive,
            StatementType::Grant,
        );
    }

    #[test]
    fn test_revoke_is_destructive() {
        assert_classification(
            "REVOKE INSERT ON users FROM app_user",
            SafetyLevel::Destructive,
            StatementType::Revoke,
        );
    }

    // CTE (WITH) queries
    #[test]
    fn test_cte_select_is_safe() {
        assert_classification(
            "WITH active_users AS (SELECT * FROM users WHERE active = true) SELECT * FROM active_users",
            SafetyLevel::Safe,
            StatementType::Select,
        );
    }

    #[test]
    fn test_cte_insert_is_mutating() {
        // Note: sqlparser parses "INSERT INTO ... SELECT" with CTE as Insert statement
        let result = classify_sql(
            "INSERT INTO users SELECT * FROM (WITH new_data AS (SELECT * FROM staging) SELECT * FROM new_data) t",
        );
        assert_eq!(result.level, SafetyLevel::Mutating);
        assert_eq!(result.statement_type, StatementType::Insert);
    }

    #[test]
    fn test_cte_delete_is_destructive() {
        // Standard DELETE with subquery
        let result = classify_sql(
            "DELETE FROM orders WHERE id IN (SELECT id FROM orders WHERE created_at < '2020-01-01')",
        );
        assert_eq!(result.level, SafetyLevel::Destructive);
        assert_eq!(result.statement_type, StatementType::Delete);
    }

    // Multi-statement queries
    #[test]
    fn test_multi_statement_uses_most_dangerous() {
        let result = classify_sql("SELECT * FROM users; DELETE FROM logs");
        assert_eq!(result.level, SafetyLevel::Destructive);
        match result.statement_type {
            StatementType::Multiple(inner) => assert_eq!(*inner, StatementType::Delete),
            _ => panic!("Expected Multiple statement type"),
        }
    }

    #[test]
    fn test_multi_statement_select_insert() {
        let result = classify_sql("SELECT * FROM users; INSERT INTO logs (msg) VALUES ('test')");
        assert_eq!(result.level, SafetyLevel::Mutating);
        match result.statement_type {
            StatementType::Multiple(inner) => assert_eq!(*inner, StatementType::Insert),
            _ => panic!("Expected Multiple statement type"),
        }
    }

    #[test]
    fn test_multi_statement_all_safe() {
        let result = classify_sql("SELECT * FROM users; SELECT COUNT(*) FROM orders");
        assert_eq!(result.level, SafetyLevel::Safe);
    }

    // Parse failure handling
    #[test]
    fn test_parse_failure_is_destructive() {
        let result = classify_sql("THIS IS NOT VALID SQL AT ALL");
        assert_eq!(result.level, SafetyLevel::Destructive);
        assert_eq!(result.statement_type, StatementType::Unknown);
        assert!(result.warning.is_some());
        assert!(result.warning.unwrap().contains("Could not parse SQL"));
    }

    #[test]
    fn test_empty_sql_is_destructive() {
        let result = classify_sql("");
        assert_eq!(result.level, SafetyLevel::Destructive);
    }

    // Warning messages
    #[test]
    fn test_destructive_has_warning() {
        let result = classify_sql("DELETE FROM users");
        assert!(result.warning.is_some());
        assert!(result.requires_warning());
    }

    #[test]
    fn test_safe_has_no_warning() {
        let result = classify_sql("SELECT * FROM users");
        assert!(result.warning.is_none());
        assert!(!result.requires_warning());
    }

    #[test]
    fn test_mutating_has_no_warning() {
        let result = classify_sql("INSERT INTO users (name) VALUES ('test')");
        assert!(result.warning.is_none());
        assert!(!result.requires_warning());
    }

    // Confirmation requirements
    #[test]
    fn test_safe_no_confirmation() {
        let result = classify_sql("SELECT 1");
        assert!(!result.requires_confirmation());
    }

    #[test]
    fn test_mutating_requires_confirmation() {
        let result = classify_sql("UPDATE users SET name = 'test'");
        assert!(result.requires_confirmation());
    }

    #[test]
    fn test_destructive_requires_confirmation() {
        let result = classify_sql("DROP TABLE users");
        assert!(result.requires_confirmation());
    }

    // Edge cases
    #[test]
    fn test_whitespace_only_is_destructive() {
        let result = classify_sql("   \n\t  ");
        assert_eq!(result.level, SafetyLevel::Destructive);
    }

    #[test]
    fn test_comment_only() {
        // sqlparser may handle this differently
        let result = classify_sql("-- just a comment");
        // Should be treated as parse failure or empty
        assert!(result.level == SafetyLevel::Destructive || result.level == SafetyLevel::Safe);
    }

    #[test]
    fn test_case_insensitive() {
        assert_classification(
            "select * from users",
            SafetyLevel::Safe,
            StatementType::Select,
        );
        assert_classification(
            "SELECT * FROM USERS",
            SafetyLevel::Safe,
            StatementType::Select,
        );
        assert_classification(
            "SeLeCt * FrOm UsErS",
            SafetyLevel::Safe,
            StatementType::Select,
        );
    }

    // Classifier instance tests
    #[test]
    fn test_classifier_new() {
        let classifier = SqlClassifier::new();
        let result = classifier.classify("SELECT 1");
        assert_eq!(result.level, SafetyLevel::Safe);
    }

    #[test]
    fn test_classifier_default() {
        let classifier = SqlClassifier::default();
        let result = classifier.classify("SELECT 1");
        assert_eq!(result.level, SafetyLevel::Safe);
    }

    // === Data-modifying CTE tests ===

    #[test]
    fn test_cte_with_delete_is_destructive() {
        assert_classification(
            "WITH deleted AS (DELETE FROM users RETURNING *) SELECT * FROM deleted",
            SafetyLevel::Destructive,
            StatementType::Delete,
        );
    }

    #[test]
    fn test_cte_with_update_is_mutating() {
        assert_classification(
            "WITH updated AS (UPDATE users SET active = false RETURNING *) SELECT * FROM updated",
            SafetyLevel::Mutating,
            StatementType::Update,
        );
    }

    #[test]
    fn test_cte_with_insert_is_mutating() {
        assert_classification(
            "WITH inserted AS (INSERT INTO logs (msg) VALUES ('x') RETURNING *) SELECT * FROM inserted",
            SafetyLevel::Mutating,
            StatementType::Insert,
        );
    }

    #[test]
    fn test_multiple_ctes_most_dangerous_wins() {
        assert_classification(
            "WITH a AS (SELECT 1), b AS (DELETE FROM users RETURNING *) SELECT * FROM a, b",
            SafetyLevel::Destructive,
            StatementType::Delete,
        );
    }

    #[test]
    fn test_mixed_mutations_delete_wins() {
        assert_classification(
            "WITH i AS (INSERT INTO logs VALUES ('x') RETURNING *), \
                  d AS (DELETE FROM users RETURNING *) \
             SELECT * FROM i, d",
            SafetyLevel::Destructive,
            StatementType::Delete,
        );
    }

    #[test]
    fn test_pure_cte_select_remains_safe() {
        assert_classification(
            "WITH active AS (SELECT * FROM users WHERE active) SELECT * FROM active",
            SafetyLevel::Safe,
            StatementType::Select,
        );
    }

    #[test]
    fn test_nested_subquery_with_delete_is_destructive() {
        assert_classification(
            "SELECT * FROM (WITH d AS (DELETE FROM users RETURNING *) SELECT * FROM d) sub",
            SafetyLevel::Destructive,
            StatementType::Delete,
        );
    }

    #[test]
    fn test_deeply_nested_mutation_detected() {
        assert_classification(
            "WITH outer AS (
                SELECT * FROM (
                    WITH inner AS (DELETE FROM users RETURNING *)
                    SELECT * FROM inner
                ) sub
             ) SELECT * FROM outer",
            SafetyLevel::Destructive,
            StatementType::Delete,
        );
    }
}
