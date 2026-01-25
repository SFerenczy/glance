# Coding Guidelines for AI Agents

> Principles and practices for contributors (human and AI) working on Glance

---

## Table of Contents

- [Core Principles](#core-principles)
- [Architecture Patterns](#architecture-patterns)
- [Development Workflow](#development-workflow)
- [Functional Programming](#functional-programming)
- [Immutability](#immutability)
- [Testing Strategy](#testing-strategy)
- [SOLID in Rust](#solid-in-rust)
- [Error Handling](#error-handling)
- [Related Documentation](#related-documentation)

---

## Core Principles

1. **Functional-first** — Prefer functional paradigms over imperative code
2. **Immutable by default** — Avoid mutable state unless absolutely necessary
3. **Test-driven** — Write tests before or alongside implementation
4. **Explicit over clever** — Code should be obvious and traceable
5. **Composition over inheritance** — Use traits and composition, not complex hierarchies

These align with our [architectural goals](docs/ARCHITECTURE.md#architectural-principles).

---

## Architecture Patterns

### Dependency Inversion at Boundaries

External systems (databases, LLM APIs) are accessed through traits:

```rust
pub trait DatabaseClient: Send + Sync {
    async fn execute(&self, sql: &str) -> Result<QueryResult, Error>;
}

pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError>;
}
```

**Benefits:**
- Mock implementations for testing without real services
- Swap providers at runtime (OpenAI ↔ Anthropic ↔ Ollama)
- Clear boundaries between layers

### Thin Orchestrator, Focused Components

The `Orchestrator` coordinates but doesn't implement business logic. Each component has a single responsibility:

- `CommandRouter` — Parse and dispatch commands
- `QueryExecutor` — Classify, execute, and format SQL
- `LlmService` — Build prompts and handle tool calls
- `ConnectionManager` — Manage connection lifecycle

See [ARCHITECTURE.md](docs/ARCHITECTURE.md#component-responsibilities) for the complete component map.

### Front-End Agnostic Core

TUI, CLI, and headless testing share the same command infrastructure:
- `InputResult` enum provides uniform response type for all front-ends
- No UI concerns leak into business logic
- Same components power interactive TUI and automated tests

### What We Don't Do

Avoid over-engineering:
- **No formal port interfaces** — Trait objects suffice
- **No dependency injection framework** — Manual wiring is sufficient
- **No event sourcing** — Simple state mutations work for a TUI tool

Keep it simple. Add abstraction only when needed.

---

## Development Workflow

### Tight Feedback Loops

**Run everything yourself. Don't wait for CI.**

1. Make changes
2. Run `cargo check`
3. Run `cargo test`
4. Run `cargo clippy`
5. Run `cargo fmt`
6. Fix issues and repeat
7. Commit only when all green

### Pre-Commit Checks

Before every commit:

```bash
just precommit
```

This runs:
- `cargo fmt --check` — Verify formatting
- `cargo clippy -- -D warnings` — Catch mistakes
- `cargo test` — Run all tests

**Keep running until green.** Never commit broken code.

### Small, Atomic Commits

Each commit should:
- Address one concern
- Pass all checks
- Have a clear message
- Be reviewable independently

**Good:**
```
feat(llm): add streaming support for Anthropic client
fix(safety): correctly classify multi-statement queries
test(headless): add connection timeout tests
```

**Bad:**
```
Fix stuff
- Fixed bug
- Added feature
- Refactored things
```

### Continuous Verification

- After every function: `cargo check`
- After every feature: `cargo test`
- Before every commit: `just precommit`

Catch issues immediately when context is fresh.

---

## Functional Programming

### Use Iterators Over Loops

**Prefer:**
```rust
let user_ids: Vec<i64> = users
    .iter()
    .filter(|u| u.is_active)
    .map(|u| u.id)
    .collect();
```

**Avoid:**
```rust
let mut user_ids = Vec::new();
for user in &users {
    if user.is_active {
        user_ids.push(user.id);
    }
}
```

### Write Pure Functions

Functions should:
- Return the same output for the same input
- Have no side effects (no I/O, mutation, or global state)
- Take all dependencies as parameters

**Good:**
```rust
fn classify_query(sql: &str) -> QuerySafety {
    match parse_sql(sql) {
        Ok(Statement::Select(_)) => QuerySafety::Safe,
        Ok(Statement::Insert(_)) => QuerySafety::Mutating,
        Ok(Statement::Delete(_)) => QuerySafety::Destructive,
        _ => QuerySafety::Unknown,
    }
}
```

**Bad:**
```rust
fn classify_query(&mut self, sql: &str) -> QuerySafety {
    self.last_query = sql.to_string();  // Side effect!
    // ...
}
```

### Use Combinators

Leverage `Option` and `Result` combinators:

**Prefer:**
```rust
fn get_user_email(user_id: i64) -> Option<String> {
    find_user(user_id)
        .and_then(|user| user.email)
        .map(|email| email.to_lowercase())
}
```

**Avoid:**
```rust
fn get_user_email(user_id: i64) -> Option<String> {
    let user = find_user(user_id)?;
    let email = user.email?;
    Some(email.to_lowercase())
}
```

### Compose, Don't Mutate

Build new values instead of mutating:

**Good:**
```rust
fn add_message(conversation: Conversation, msg: Message) -> Conversation {
    Conversation {
        messages: conversation.messages
            .into_iter()
            .chain(std::iter::once(msg))
            .collect(),
        ..conversation
    }
}
```

**Bad:**
```rust
fn add_message(conversation: &mut Conversation, msg: Message) {
    conversation.messages.push(msg);
}
```

---

## Immutability

**Default to immutable bindings.** Only use `mut` when truly necessary.

### Prefer Transformations

**Good:**
```rust
let queries = original_queries
    .into_iter()
    .filter(|q| q.status == QueryStatus::Success)
    .collect();
```

**Bad:**
```rust
let mut queries = Vec::new();
for q in original_queries {
    if q.status == QueryStatus::Success {
        queries.push(q);
    }
}
```

### When Mutation Is Acceptable

1. **Performance-critical paths** where allocation is prohibitive
2. **Builder patterns** for complex object construction
3. **I/O buffering**
4. **Interior mutability** for caching (`RefCell`, `Mutex`)

**Example:**
```rust
pub struct QueryBuilder {
    sql: String,
    params: Vec<Value>,
}

impl QueryBuilder {
    pub fn add_where(&mut self, condition: &str, value: Value) -> &mut Self {
        self.sql.push_str(&format!(" WHERE {}", condition));
        self.params.push(value);
        self
    }

    pub fn build(self) -> Query {
        Query {
            sql: self.sql,
            params: self.params,
        }
    }
}
```

### Functional Struct Updates

Create new instances instead of mutating:

```rust
let updated = Connection {
    status: ConnectionStatus::Active,
    ..existing_connection
};
```

---

## Testing Strategy

> See [TESTING.md](docs/TESTING.md) for detailed procedures.

### Test Pyramid

| Test Type        | Speed  | Coverage | External Deps |
| ---------------- | ------ | -------- | ------------- |
| **Unit**         | Fast   | 60%      | None          |
| **Integration**  | Medium | 30%      | Mock clients  |
| **Headless TUI** | Slow   | 10%      | Mock clients  |

### Unit Tests

Test pure functions in isolation:

```rust
#[test]
fn should_classify_select_as_safe() {
    let sql = "SELECT * FROM users";
    assert_eq!(classify_query(sql), QuerySafety::Safe);
}

#[test]
fn should_classify_delete_as_destructive() {
    let sql = "DELETE FROM users WHERE id = 1";
    assert_eq!(classify_query(sql), QuerySafety::Destructive);
}
```

### Integration Tests

Test component interactions with mocks:

```rust
#[tokio::test]
async fn should_connect_to_database() {
    let mock_db = MockDatabaseClient::new();
    let router = CommandRouter::new(mock_db);

    let result = router.route("/connect localhost").await;

    assert!(result.is_ok());
}
```

### Headless TUI Tests

Test full application flow without rendering:

```rust
#[tokio::test]
async fn should_execute_natural_language_query() {
    let mock_db = MockDatabaseClient::with_schema(test_schema());
    let mock_llm = MockLlmClient::with_response("SELECT * FROM users");

    let mut app = HeadlessApp::new(mock_db, mock_llm);

    app.send_message("Show me all users").await;

    assert_eq!(app.message_count(), 2); // User + assistant
    assert!(app.last_message().contains("users"));
}
```

**Key advantage:** Test the full TUI flow without rendering. Fast and reliable.

See `src/tui/headless/` for implementation.

### Behavior-Driven Development

Write tests that describe **behavior**, not implementation.

**Test names should read like specifications:**
- `should_reconnect_after_connection_timeout`
- `should_truncate_results_at_1000_rows`
- `should_preserve_null_values_in_results`
- `should_prevent_execution_without_connection`

**Use Arrange-Act-Assert:**
```rust
#[test]
fn should_format_execution_time() {
    // Arrange
    let result = QueryResult { execution_time_ms: 42, ..default() };

    // Act
    let formatted = format_result(&result);

    // Assert
    assert!(formatted.contains("42ms"));
}
```

---

## SOLID in Rust

### Single Responsibility

Each module and struct has one clear purpose:

```rust
// commands/router.rs - Only parses commands
pub fn parse_command(input: &str) -> Result<Command, ParseError>

// commands/handlers/connection.rs - Only handles connections
pub async fn handle_connect(args: &str) -> Result<Response, Error>
```

### Open/Closed

Use traits for extension without modification:

```rust
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError>;
}

// Add new providers without modifying existing code
impl LlmClient for AnthropicClient { /* ... */ }
impl LlmClient for OpenAiClient { /* ... */ }
impl LlmClient for OllamaClient { /* ... */ }
```

### Liskov Substitution

All trait implementations must be interchangeable:

```rust
pub async fn execute_query(
    client: &dyn DatabaseClient,
    sql: &str,
) -> Result<QueryResult, Error> {
    client.execute(sql).await
}

// Works with any implementation
execute_query(&postgres_client, "SELECT 1").await?;
execute_query(&mock_client, "SELECT 1").await?;
```

### Interface Segregation

Keep traits focused and minimal:

**Good:**
```rust
pub trait SchemaProvider {
    fn get_schema(&self) -> Schema;
}

pub trait QueryExecutor {
    async fn execute(&self, sql: &str) -> Result<QueryResult, Error>;
}
```

**Bad:**
```rust
pub trait DatabaseClient {
    fn get_schema(&self) -> Schema;
    async fn execute(&self, sql: &str) -> Result<QueryResult, Error>;
    fn get_connection_info(&self) -> ConnectionInfo;
    fn backup(&self) -> Result<(), Error>;
    fn restore(&self, backup: &Backup) -> Result<(), Error>;
    // Too many responsibilities!
}
```

### Dependency Inversion

Depend on abstractions (traits), not concrete types:

**Good:**
```rust
pub struct Orchestrator {
    db_client: Arc<dyn DatabaseClient>,
    llm_client: Arc<dyn LlmClient>,
}
```

**Bad:**
```rust
pub struct Orchestrator {
    postgres_client: PostgresClient,  // Concrete!
    openai_client: OpenAiClient,      // Concrete!
}
```

**Benefits:**
- Test with mock implementations
- Swap providers at runtime
- Clear module boundaries

---

## Error Handling

Use `thiserror` for error types. Errors bubble up to the orchestrator.

**Error categories:**
- **Connection errors** — Can't reach database
- **Query errors** — SQL syntax or execution failures
- **LLM errors** — API failures, rate limits
- **Config errors** — Missing or invalid configuration

**Example:**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueryError {
    #[error("Connection lost: {0}")]
    ConnectionLost(String),

    #[error("Invalid SQL: {0}")]
    InvalidSql(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}
```

Errors are displayed user-friendly in TUI, with full details in debug mode.

See [ARCHITECTURE.md](docs/ARCHITECTURE.md#error-handling-strategy) for more.

---

## Related Documentation

- **[ARCHITECTURE.md](docs/ARCHITECTURE.md)** — System design and module structure
- **[TESTING.md](docs/TESTING.md)** — Detailed testing procedures
- **[DEVELOPMENT.md](docs/DEVELOPMENT.md)** — Development environment setup
- **[VISION.md](docs/VISION.md)** — Project goals and philosophy
- **[TECH_STACK.md](docs/TECH_STACK.md)** — Technology choices and rationale

---

## Summary

When contributing to Glance:

1. **Functional, immutable code** — Iterators and combinators over loops and mutation
2. **Pure functions** — No side effects, explicit dependencies
3. **Tight feedback loops** — Run `just precommit` until green before committing
4. **Small commits** — One concern per commit, clear messages
5. **Behavior-focused tests** — Use headless TUI mode for full integration testing
6. **SOLID principles** — Traits for abstraction, single responsibility, dependency inversion
7. **Explicit over clever** — Code should be obvious and traceable

These practices keep Glance fast, reliable, and maintainable.
