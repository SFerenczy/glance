//! LLM tool definitions for function calling.
//!
//! Provides read-only tools for the LLM to access saved queries and metadata.

use serde::{Deserialize, Serialize};

/// Tool definition for LLM function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Input parameters for the list_saved_queries tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSavedQueriesInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// Output entry for a saved query (redacted for LLM consumption).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQueryOutput {
    pub name: String,
    pub sql: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub connection_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    pub usage_count: i64,
}

/// Returns the tool definitions available to the LLM.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        name: "list_saved_queries".to_string(),
        description: "List or search saved SQL queries. Returns query names, SQL, descriptions, \
                      tags, and usage statistics. Use this to find reusable queries the user has saved."
            .to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "connection_name": {
                    "type": "string",
                    "description": "Filter by connection name (optional)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Filter by tags (optional)"
                },
                "text": {
                    "type": "string",
                    "description": "Search text in query name, SQL, or description (optional)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default: 10)"
                }
            },
            "required": []
        }),
    }]
}

/// Formats saved queries for LLM tool response.
///
/// Strips sensitive information (host, user, passwords) and formats
/// for LLM consumption.
pub fn format_saved_queries_for_llm(
    queries: &[crate::persistence::SavedQuery],
) -> Vec<SavedQueryOutput> {
    queries
        .iter()
        .map(|q| SavedQueryOutput {
            name: q.name.clone(),
            sql: q.sql.clone(),
            description: q.description.clone(),
            tags: q.tags.clone(),
            connection_label: q.connection_name.as_deref().unwrap_or("global").to_string(),
            last_used_at: q.last_used_at.clone(),
            usage_count: q.usage_count,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_definitions() {
        let tools = get_tool_definitions();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "list_saved_queries");
    }

    #[test]
    fn test_format_saved_queries_for_llm() {
        let queries = vec![crate::persistence::SavedQuery {
            id: 1,
            name: "get_users".to_string(),
            sql: "SELECT * FROM users".to_string(),
            description: Some("Get all users".to_string()),
            connection_name: Some("mydb".to_string()),
            tags: vec!["users".to_string()],
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
            last_used_at: Some("2024-01-02".to_string()),
            usage_count: 5,
        }];

        let output = format_saved_queries_for_llm(&queries);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].name, "get_users");
        assert_eq!(output[0].connection_label, "mydb");
        assert_eq!(output[0].usage_count, 5);
    }
}
