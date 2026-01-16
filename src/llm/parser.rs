//! Response parsing for LLM outputs.
//!
//! Extracts SQL from LLM responses that may contain markdown code blocks.

/// Result of parsing an LLM response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedResponse {
    /// Any explanatory text before or after the SQL.
    pub text: String,
    /// Extracted SQL query, if found.
    pub sql: Option<String>,
}

impl ParsedResponse {
    /// Creates a new parsed response with only text (no SQL).
    pub fn text_only(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            sql: None,
        }
    }

    /// Creates a new parsed response with SQL and optional text.
    pub fn with_sql(text: impl Into<String>, sql: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            sql: Some(sql.into()),
        }
    }
}

/// Parses an LLM response to extract SQL from markdown code blocks.
///
/// Looks for SQL in the following formats:
/// - ```sql ... ```
/// - ``` ... ``` (no language specified)
///
/// If multiple code blocks are found, uses the first one.
/// If no code block is found, returns the full text with no SQL.
pub fn parse_llm_response(response: &str) -> ParsedResponse {
    // Try to find a SQL code block first
    if let Some(sql) = extract_code_block(response, "sql") {
        let text = remove_code_block(response, "sql");
        return ParsedResponse::with_sql(text.trim(), sql.trim());
    }

    // Try to find a generic code block
    if let Some(sql) = extract_code_block(response, "") {
        let text = remove_code_block(response, "");
        return ParsedResponse::with_sql(text.trim(), sql.trim());
    }

    // No code block found, return as text only
    ParsedResponse::text_only(response.trim())
}

/// Extracts content from a markdown code block with the specified language.
///
/// Pass an empty string for `lang` to match blocks without a language specifier.
fn extract_code_block(text: &str, lang: &str) -> Option<String> {
    let _pattern = if lang.is_empty() { "```\n" } else { "" };

    // Build the start pattern
    let start_pattern = if lang.is_empty() {
        "```".to_string()
    } else {
        format!("```{}", lang)
    };

    // Find the start of the code block
    let start_idx = text.find(&start_pattern)?;

    // Find the newline after the opening fence
    let content_start = text[start_idx + start_pattern.len()..]
        .find('\n')
        .map(|i| start_idx + start_pattern.len() + i + 1)?;

    // For generic blocks, make sure it's not actually a language-specific block
    if lang.is_empty() {
        let after_fence = &text[start_idx + 3..content_start - 1];
        // If there's text after ``` before newline, it's a language specifier
        if !after_fence.trim().is_empty() {
            return None;
        }
    }

    // Find the closing fence
    let end_idx = text[content_start..].find("```")?;

    Some(text[content_start..content_start + end_idx].to_string())
}

/// Removes the first code block from the text, returning the remaining text.
fn remove_code_block(text: &str, lang: &str) -> String {
    let start_pattern = if lang.is_empty() {
        "```".to_string()
    } else {
        format!("```{}", lang)
    };

    // Find the start of the code block
    let Some(start_idx) = text.find(&start_pattern) else {
        return text.to_string();
    };

    // For generic blocks, verify it's not a language-specific block
    if lang.is_empty() {
        let after_fence_start = start_idx + 3;
        if let Some(newline_idx) = text[after_fence_start..].find('\n') {
            let after_fence = &text[after_fence_start..after_fence_start + newline_idx];
            if !after_fence.trim().is_empty() {
                return text.to_string();
            }
        }
    }

    // Find the closing fence
    let content_start = text[start_idx + start_pattern.len()..]
        .find('\n')
        .map(|i| start_idx + start_pattern.len() + i + 1);

    let Some(content_start) = content_start else {
        return text.to_string();
    };

    let Some(end_offset) = text[content_start..].find("```") else {
        return text.to_string();
    };

    let end_idx = content_start + end_offset + 3;

    // Build the result without the code block
    let before = &text[..start_idx];
    let after = &text[end_idx..];

    format!("{}{}", before.trim_end(), after.trim_start())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sql_code_block() {
        let response = r#"Here's the query:

```sql
SELECT * FROM users;
```

This will return all users."#;

        let parsed = parse_llm_response(response);

        assert_eq!(parsed.sql, Some("SELECT * FROM users;".to_string()));
        assert!(parsed.text.contains("Here's the query:"));
        assert!(parsed.text.contains("This will return all users."));
    }

    #[test]
    fn test_extract_generic_code_block() {
        let response = r#"```
SELECT COUNT(*) FROM orders;
```"#;

        let parsed = parse_llm_response(response);

        assert_eq!(parsed.sql, Some("SELECT COUNT(*) FROM orders;".to_string()));
    }

    #[test]
    fn test_no_code_block() {
        let response = "I don't understand that question. Could you please clarify?";

        let parsed = parse_llm_response(response);

        assert_eq!(parsed.sql, None);
        assert_eq!(parsed.text, response);
    }

    #[test]
    fn test_multiple_code_blocks_uses_first() {
        let response = r#"First query:

```sql
SELECT * FROM users;
```

Alternative:

```sql
SELECT id, name FROM users;
```"#;

        let parsed = parse_llm_response(response);

        assert_eq!(parsed.sql, Some("SELECT * FROM users;".to_string()));
    }

    #[test]
    fn test_sql_block_preferred_over_generic() {
        let response = r#"```
This is not SQL
```

```sql
SELECT * FROM users;
```"#;

        let parsed = parse_llm_response(response);

        // SQL block should be preferred
        assert_eq!(parsed.sql, Some("SELECT * FROM users;".to_string()));
    }

    #[test]
    fn test_multiline_sql() {
        let response = r#"```sql
SELECT 
    u.id,
    u.name,
    COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON o.user_id = u.id
GROUP BY u.id, u.name
ORDER BY order_count DESC;
```"#;

        let parsed = parse_llm_response(response);

        assert!(parsed.sql.is_some());
        let sql = parsed.sql.unwrap();
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("LEFT JOIN"));
        assert!(sql.contains("GROUP BY"));
    }

    #[test]
    fn test_empty_response() {
        let parsed = parse_llm_response("");
        assert_eq!(parsed.sql, None);
        assert_eq!(parsed.text, "");
    }

    #[test]
    fn test_whitespace_handling() {
        let response = "  \n  ```sql\n  SELECT 1;  \n```  \n  ";

        let parsed = parse_llm_response(response);

        assert!(parsed.sql.is_some());
        assert_eq!(parsed.sql.unwrap(), "SELECT 1;");
    }

    #[test]
    fn test_code_block_with_other_language() {
        let response = r#"```python
print("hello")
```"#;

        let parsed = parse_llm_response(response);

        // Should not extract python as SQL
        assert_eq!(parsed.sql, None);
    }

    #[test]
    fn test_parsed_response_constructors() {
        let text_only = ParsedResponse::text_only("Hello");
        assert_eq!(text_only.text, "Hello");
        assert_eq!(text_only.sql, None);

        let with_sql = ParsedResponse::with_sql("Explanation", "SELECT 1");
        assert_eq!(with_sql.text, "Explanation");
        assert_eq!(with_sql.sql, Some("SELECT 1".to_string()));
    }
}
