# Implementation Plan

> Step-by-step guide to building Glance v0.1

---

## Phase 0: Development Environment Setup

**Goal**: Any developer or AI agent can clone the repo and immediately build, test, and run the application.

### 0.1: Prerequisites Documentation

Create `docs/DEVELOPMENT.md` with:

- Required tools: Rust 1.75+, Docker (for test database)
- Optional tools: Ollama (for LLM integration tests)
- Environment setup instructions

### 0.2: Project Scaffold

```bash
cargo init
```

Create `Cargo.toml` with all dependencies:

- tokio, ratatui, crossterm, sqlx, reqwest, serde, etc.
- Dev dependencies: tokio-test, pretty_assertions

### 0.3: Docker Compose for Test Database

Create `docker compose.yml`:

```yaml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_USER: glance
      POSTGRES_PASSWORD: glance
      POSTGRES_DB: glance_test
    ports:
      - "5432:5432"
    volumes:
      - ./tests/fixtures/seed.sql:/docker-entrypoint-initdb.d/seed.sql
```

Create `tests/fixtures/seed.sql` with test schema:

- `users` table (id, email, name, created_at)
- `orders` table (id, user_id, total, status, created_at)
- Foreign key relationship
- Sample data (10-20 rows per table)

### 0.4: Test Infrastructure

**LLM Testing Strategy:**

```
┌─────────────────────────────────────────────────────────────┐
│                     LLM Client Trait                        │
├─────────────────────────────────────────────────────────────┤
│  MockLlmClient     │  OllamaClient    │  OpenAiClient      │
│  (unit tests)      │  (integration)   │  (production)      │
│  - Deterministic   │  - Local, free   │  - Real API        │
│  - Instant         │  - Optional      │  - Costs money     │
└─────────────────────────────────────────────────────────────┘
```

1. **MockLlmClient**: Returns predefined responses based on input patterns
   - "show me all users" → `SELECT * FROM users`
   - "count orders" → `SELECT COUNT(*) FROM orders`
   - Used for all unit tests

2. **OllamaClient**: Real local LLM (optional)
   - Skip tests if Ollama not running
   - Use small model (llama3.2:3b or similar)
   - For integration tests only

3. **Real API**: Manual testing only, never in automated tests

### 0.5: CI Configuration

Create `.github/workflows/ci.yml`:

- Build on push/PR
- Run `cargo fmt --check`
- Run `cargo clippy`
- Run `cargo test` (with Postgres via service container)
- No LLM API calls in CI

### 0.6: Makefile / Justfile

Create `justfile` (or Makefile) for common commands:

```just
# Start test database
db:
    docker compose up -d postgres

# Stop test database
db-down:
    docker compose down

# Run all tests
test: db
    cargo test

# Run tests with Ollama integration
test-integration: db
    OLLAMA_AVAILABLE=1 cargo test

# Format and lint
check:
    cargo fmt --check
    cargo clippy -- -D warnings

# Run the application
run:
    cargo run

# Run with test database
dev: db
    DATABASE_URL="postgres://glance:glance@localhost:5432/glance_test" cargo run
```

### 0.7: Environment Configuration

Create `.env.example`:

```bash
# Database (required for running)
DATABASE_URL=postgres://glance:glance@localhost:5432/glance_test

# LLM Provider (required for chat features)
OPENAI_API_KEY=sk-...
# or
ANTHROPIC_API_KEY=sk-ant-...

# Optional: Ollama for local testing
OLLAMA_URL=http://localhost:11434
```

Create `.gitignore`:

```
/target
.env
*.log
```

---

## Phase 1: Core Infrastructure

**Goal**: Basic application skeleton that compiles and runs.

### 1.1: Error Types

Create `src/error.rs`:

- Define `GlanceError` enum with variants:
  - `Connection(String)`
  - `Query(String)`
  - `Llm(String)`
  - `Config(String)`
  - `Internal(String)`
- Implement `std::error::Error`, `Display`
- Use `thiserror` derive macros

**Tests**: Unit tests for error formatting

### 1.2: Configuration

Create `src/config.rs`:

- `Config` struct matching spec (FR-7.2)
- `ConnectionConfig` for database connections
- `LlmConfig` for LLM settings
- Load from TOML file
- Load from environment variables
- Merge with CLI arguments

**Tests**:

- Parse valid config
- Handle missing optional fields
- Validate required fields
- Environment variable override

### 1.3: CLI Argument Parsing

Create `src/cli.rs`:

- Use clap with derive macros
- Arguments per spec (FR-8.1)
- Parse connection string
- Merge with config

**Tests**:

- Parse connection string
- Parse individual arguments
- Argument precedence

### 1.4: Application Entry Point

Create `src/main.rs`:

- Parse CLI arguments
- Load configuration
- Initialize logging (tracing)
- Placeholder for app startup

**Verification**: `cargo run -- --help` works

---

## Phase 2: Database Layer

**Goal**: Connect to Postgres, introspect schema, execute queries.

### 2.1: Database Client Trait

Create `src/db/mod.rs`:

- Define `DatabaseClient` trait:
  ```rust
  #[async_trait]
  pub trait DatabaseClient: Send + Sync {
      async fn connect(config: &ConnectionConfig) -> Result<Self> where Self: Sized;
      async fn introspect_schema(&self) -> Result<Schema>;
      async fn execute_query(&self, sql: &str) -> Result<QueryResult>;
      async fn close(&self) -> Result<()>;
  }
  ```

### 2.2: Schema Types

Create `src/db/schema.rs`:

- `Schema` struct (tables, foreign keys)
- `Table` struct (name, columns, primary key, indexes)
- `Column` struct (name, data_type, nullable, default)
- `ForeignKey` struct
- `Index` struct
- Method to format schema for LLM prompt

**Tests**: Schema serialization to LLM format

### 2.3: Query Result Types

Create `src/db/types.rs`:

- `QueryResult` struct (columns, rows, execution_time, row_count)
- `Column` metadata (name, type)
- `Row` as `Vec<Value>`
- `Value` enum (String, Int, Float, Bool, Null, etc.)

**Tests**: Value type conversions

### 2.4: Postgres Implementation

Create `src/db/postgres.rs`:

- Implement `DatabaseClient` for `PostgresClient`
- Use sqlx for connection pooling
- Schema introspection via `information_schema`
- Query execution with timeout

**Tests** (require test database):

- Connect to database
- Introspect test schema
- Execute SELECT query
- Handle query errors
- Connection timeout

### 2.5: Schema Introspection Queries

SQL queries for introspection:

```sql
-- Tables
SELECT table_name FROM information_schema.tables
WHERE table_schema = 'public' AND table_type = 'BASE TABLE';

-- Columns
SELECT column_name, data_type, is_nullable, column_default
FROM information_schema.columns
WHERE table_schema = 'public' AND table_name = $1;

-- Primary keys
SELECT kcu.column_name
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu
  ON tc.constraint_name = kcu.constraint_name
WHERE tc.table_name = $1 AND tc.constraint_type = 'PRIMARY KEY';

-- Foreign keys
SELECT
    kcu.table_name AS from_table,
    kcu.column_name AS from_column,
    ccu.table_name AS to_table,
    ccu.column_name AS to_column
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu
  ON tc.constraint_name = kcu.constraint_name
JOIN information_schema.constraint_column_usage ccu
  ON tc.constraint_name = ccu.constraint_name
WHERE tc.constraint_type = 'FOREIGN KEY';
```

**Verification**: Connect to test DB, print schema

---

## Phase 3: Query Safety

**Goal**: Parse SQL and classify as safe/mutating/destructive.

### 3.1: Safety Types

Create `src/safety/mod.rs`:

- `SafetyLevel` enum: `Safe`, `Mutating`, `Destructive`
- `ClassificationResult` struct (level, statement_type, warning)

### 3.2: SQL Parser Integration

Create `src/safety/parser.rs`:

- Use `sqlparser` crate with PostgreSQL dialect
- Parse SQL string to AST
- Extract statement type(s)
- Handle parse failures (treat as Destructive)

**Tests**:

- SELECT → Safe
- INSERT → Mutating
- UPDATE → Mutating
- DELETE → Destructive
- DROP → Destructive
- Multi-statement classification
- CTE classification
- Parse failure handling

### 3.3: Classification Logic

```rust
fn classify(statement: &Statement) -> SafetyLevel {
    match statement {
        Statement::Query(_) => SafetyLevel::Safe,
        Statement::Explain { .. } => SafetyLevel::Safe,
        Statement::ShowVariable { .. } => SafetyLevel::Safe,
        Statement::Insert { .. } => SafetyLevel::Mutating,
        Statement::Update { .. } => SafetyLevel::Mutating,
        Statement::Delete { .. } => SafetyLevel::Destructive,
        Statement::Drop { .. } => SafetyLevel::Destructive,
        Statement::Truncate { .. } => SafetyLevel::Destructive,
        Statement::AlterTable { .. } => SafetyLevel::Destructive,
        _ => SafetyLevel::Destructive, // Conservative default
    }
}
```

**Verification**: Unit tests pass for all SQL types

---

## Phase 4: LLM Integration

**Goal**: Send prompts to LLM, receive SQL responses.

### 4.1: LLM Client Trait

Create `src/llm/mod.rs`:

- Define `LlmClient` trait:
  ```rust
  #[async_trait]
  pub trait LlmClient: Send + Sync {
      async fn complete(&self, messages: &[Message]) -> Result<String>;
      async fn complete_stream(&self, messages: &[Message]) -> Result<impl Stream<Item = Result<String>>>;
  }
  ```

### 4.2: Message Types

Create `src/llm/types.rs`:

- `Message` struct (role, content)
- `Role` enum (System, User, Assistant)
- `Conversation` struct (messages, add/clear methods)

### 4.3: Prompt Construction

Create `src/llm/prompt.rs`:

- System prompt template (per FR-3.3)
- Function to inject schema into prompt
- Function to build message list from conversation

**Tests**: Prompt generation with sample schema

### 4.4: Mock LLM Client

Create `src/llm/mock.rs`:

- `MockLlmClient` for testing
- Pattern matching on input to return canned SQL:
  ````rust
  fn mock_response(input: &str) -> String {
      if input.contains("all users") {
          "```sql\nSELECT * FROM users;\n```".to_string()
      } else if input.contains("count") && input.contains("orders") {
          "```sql\nSELECT COUNT(*) FROM orders;\n```".to_string()
      } else {
          "I don't understand that question.".to_string()
      }
  }
  ````

**Tests**: Mock returns expected responses

### 4.5: OpenAI Client

Create `src/llm/openai.rs`:

- Implement `LlmClient` for `OpenAiClient`
- Use reqwest for HTTP
- Handle API response format
- Implement streaming with SSE parsing
- Error handling (rate limits, auth, timeouts)

**Tests** (with mock HTTP or skip):

- Request formatting
- Response parsing
- Error handling

### 4.6: Anthropic Client

Create `src/llm/anthropic.rs`:

- Implement `LlmClient` for `AnthropicClient`
- Different API format than OpenAI
- Streaming support

**Tests**: Same as OpenAI

### 4.7: Response Parsing

Create `src/llm/parser.rs`:

- Extract SQL from markdown code blocks
- Handle responses without SQL
- Handle multiple code blocks (use first)

**Tests**:

- Extract from ```sql block
- Extract from ``` block (no language)
- No code block → return full text
- Multiple blocks → use first

### 4.8: Ollama Client (Optional)

Create `src/llm/ollama.rs`:

- Implement `LlmClient` for `OllamaClient`
- OpenAI-compatible API format
- Used for integration testing

**Tests**: Skip if Ollama not available

---

## Phase 5: TUI Foundation

**Goal**: Basic terminal UI with layout, no functionality yet.

### 5.1: TUI Setup

Create `src/tui/mod.rs`:

- Terminal initialization (crossterm)
- Alternate screen, raw mode
- Cleanup on exit (restore terminal)
- Main render loop

**Verification**: App starts, shows blank screen, exits cleanly with Ctrl+C

### 5.2: Application State

Create `src/tui/app.rs`:

- `App` struct holding all state:
  - `messages: Vec<ChatMessage>`
  - `query_log: Vec<QueryLogEntry>`
  - `input: String`
  - `focus: Focus` (Chat, Sidebar, Input)
  - `scroll_offset: usize`
  - `running: bool`

### 5.3: Layout

Create `src/tui/ui.rs`:

- Three-row layout: header, main, input
- Main area split: chat (70%), sidebar (30%)
- Use ratatui `Layout` and `Constraint`

**Verification**: Layout renders with placeholder text

### 5.4: Header Widget

Create `src/tui/widgets/header.rs`:

- App name and version (left)
- Database connection info (right)
- Styled background

### 5.5: Input Widget

Create `src/tui/widgets/input.rs`:

- Text input with cursor
- Prompt character `>`
- Handle text editing (insert, delete, cursor movement)

**Tests**: Input state management

### 5.6: Event Handling

Create `src/tui/events.rs`:

- Keyboard event processing
- Map keys to actions
- Handle Ctrl+C, Ctrl+Q for exit
- Handle Enter for submit
- Handle Tab for focus switch

**Verification**: Can type, submit, exit

---

## Phase 6: TUI Chat Panel

**Goal**: Display messages and results in chat panel.

### 6.1: Chat Message Types

Extend `src/tui/app.rs`:

- `ChatMessage` enum:
  - `User(String)`
  - `Assistant(String)`
  - `Result(QueryResult)`
  - `Error(String)`
  - `System(String)`

### 6.2: Chat Widget

Create `src/tui/widgets/chat.rs`:

- Render message history
- Different styling per message type
- Scrollable with offset
- Handle overflow

### 6.3: Result Table Widget

Create `src/tui/widgets/table.rs`:

- Render `QueryResult` as table
- Column headers
- Auto-size columns (up to max width)
- Truncate long values
- Style NULL values
- Row count footer

**Tests**: Table rendering with various data

### 6.4: Chat Scrolling

- Arrow keys scroll when chat focused
- Page Up/Down for page scroll
- Home/End for top/bottom
- Auto-scroll to bottom on new message

**Verification**: Can scroll through messages

---

## Phase 7: TUI Sidebar

**Goal**: Display query log in sidebar.

### 7.1: Query Log Entry

Extend `src/tui/app.rs`:

- `QueryLogEntry` struct:
  - `sql: String`
  - `status: QueryStatus` (Success, Error)
  - `execution_time: Duration`
  - `row_count: Option<usize>`
  - `error: Option<String>`

### 7.2: Sidebar Widget

Create `src/tui/widgets/sidebar.rs`:

- List of query entries
- Truncated SQL preview
- Status icon (✓ or ✗)
- Execution time and row count
- Selectable entries
- Highlight selected

### 7.3: Query Detail Modal

- Press Enter on selected query
- Show modal with full SQL
- Press Esc to close

**Verification**: Sidebar shows queries, can select and view details

---

## Phase 8: Core Integration

**Goal**: Wire everything together into working application.

### 8.1: Orchestrator

Create `src/app.rs`:

- `Orchestrator` struct coordinating:
  - Database client
  - LLM client
  - Safety classifier
  - Application state
- Main chat loop logic

### 8.2: Chat Flow Implementation

```rust
async fn handle_user_input(&mut self, input: String) -> Result<()> {
    // 1. Add user message to chat
    self.state.add_message(ChatMessage::User(input.clone()));

    // 2. Check for commands
    if input.starts_with('/') {
        return self.handle_command(&input).await;
    }

    // 3. Send to LLM
    let response = self.llm.complete(&self.build_messages()).await?;

    // 4. Parse SQL from response
    let (text, sql) = parse_llm_response(&response);

    // 5. Add assistant message
    if !text.is_empty() {
        self.state.add_message(ChatMessage::Assistant(text));
    }

    // 6. If SQL found, classify and execute
    if let Some(sql) = sql {
        self.handle_sql(&sql).await?;
    }

    Ok(())
}
```

### 8.3: SQL Execution Flow

```rust
async fn handle_sql(&mut self, sql: &str) -> Result<()> {
    // 1. Classify safety
    let classification = self.safety.classify(sql)?;

    // 2. Check if confirmation needed
    match classification.level {
        SafetyLevel::Safe => {
            self.execute_and_display(sql).await?;
        }
        SafetyLevel::Mutating | SafetyLevel::Destructive => {
            self.state.pending_query = Some(sql.to_string());
            self.state.show_confirmation = true;
            // TUI will show confirmation dialog
        }
    }

    Ok(())
}
```

### 8.4: Command Handling

```rust
async fn handle_command(&mut self, input: &str) -> Result<()> {
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    match parts[0] {
        "/sql" => {
            let sql = parts.get(1).unwrap_or(&"");
            self.handle_sql(sql).await?;
        }
        "/clear" => {
            self.state.clear_chat();
            self.conversation.clear();
        }
        "/schema" => {
            let schema_text = self.schema.format_for_display();
            self.state.add_message(ChatMessage::System(schema_text));
        }
        "/quit" | "/exit" => {
            self.state.running = false;
        }
        "/help" => {
            self.state.add_message(ChatMessage::System(HELP_TEXT.to_string()));
        }
        _ => {
            self.state.add_message(ChatMessage::Error(format!("Unknown command: {}", parts[0])));
        }
    }
    Ok(())
}
```

### 8.5: Confirmation Dialog

Create `src/tui/widgets/confirm.rs`:

- Modal dialog for mutation confirmation
- Show SQL being executed
- Warning level based on classification
- y/Enter to confirm, n/Esc to cancel

### 8.6: Async Event Loop

Main loop handling:

- TUI events (keyboard input)
- LLM streaming responses
- Database query completion
- Use tokio::select! for multiplexing

---

## Phase 9: Polish & Edge Cases

**Goal**: Handle errors gracefully, improve UX.

### 9.1: Connection Error Handling

- Clear error messages per FR-1.4
- Retry logic for transient failures
- Graceful degradation (allow /sql if LLM fails)

### 9.2: Query Error Display

- Parse Postgres error messages
- Show inline in chat
- Include hints when available

### 9.3: LLM Error Handling

- Rate limit detection and backoff
- Timeout handling
- API error messages

### 9.4: Large Result Sets

- Limit to 1000 rows
- Show warning if truncated
- Memory-efficient streaming

### 9.5: Terminal Resize

- Handle SIGWINCH
- Re-render on resize
- Maintain scroll position

### 9.6: Cleanup

- Restore terminal on panic
- Close database connection
- Cancel pending requests

---

## Phase 10: Testing & Documentation

**Goal**: Comprehensive tests, ready for release.

### 10.1: Unit Test Coverage

Target modules:

- `config`: Parsing, validation, merging
- `safety`: All SQL classification cases
- `llm/parser`: Response extraction
- `db/schema`: Schema formatting
- `tui/widgets`: Rendering logic

### 10.2: Integration Tests

Create `tests/integration/`:

- `connection_test.rs`: Connect to test DB
- `schema_test.rs`: Introspect test schema
- `query_test.rs`: Execute queries
- `chat_flow_test.rs`: Full chat flow with mock LLM

### 10.3: Manual Test Script

Create `docs/TESTING.md`:

- Manual test cases for each user story
- Edge cases to verify
- Performance benchmarks

### 10.4: Documentation

Update:

- `README.md`: Installation, quick start
- `docs/DEVELOPMENT.md`: Full dev setup guide
- `docs/USAGE.md`: User guide with examples

---

## Implementation Order Summary

```
Phase 0: Dev Environment     [~2 hours]
    ├── 0.1-0.7: Setup, Docker, CI

Phase 1: Core Infrastructure [~2 hours]
    ├── 1.1: Error types
    ├── 1.2: Configuration
    ├── 1.3: CLI parsing
    └── 1.4: Entry point

Phase 2: Database Layer      [~4 hours]
    ├── 2.1-2.3: Traits and types
    ├── 2.4-2.5: Postgres implementation

Phase 3: Query Safety        [~2 hours]
    ├── 3.1-3.3: Parser and classifier

Phase 4: LLM Integration     [~4 hours]
    ├── 4.1-4.4: Traits, types, mock
    ├── 4.5-4.6: OpenAI, Anthropic
    ├── 4.7-4.8: Response parsing, Ollama

Phase 5: TUI Foundation      [~3 hours]
    ├── 5.1-5.6: Setup, layout, input

Phase 6: TUI Chat Panel      [~3 hours]
    ├── 6.1-6.4: Messages, tables, scrolling

Phase 7: TUI Sidebar         [~2 hours]
    ├── 7.1-7.3: Query log, detail modal

Phase 8: Core Integration    [~4 hours]
    ├── 8.1-8.6: Orchestrator, flows, async

Phase 9: Polish              [~3 hours]
    ├── 9.1-9.6: Error handling, edge cases

Phase 10: Testing & Docs     [~3 hours]
    ├── 10.1-10.4: Tests, documentation

Total Estimated: ~32 hours
```

---

## Testing Commands Reference

```bash
# Start test database
just db

# Run all tests (uses mock LLM)
just test

# Run with Ollama integration tests
just test-integration

# Run specific test
cargo test test_schema_introspection

# Run with logging
RUST_LOG=debug cargo test

# Check formatting and lints
just check

# Run the app against test database
just dev
```

---

## Definition of Done for v0.1

- [ ] All user stories have passing tests
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo fmt --check` passes
- [ ] Integration tests pass with test database
- [ ] Manual testing completed per test script
- [ ] Documentation updated
- [ ] README has installation and usage instructions
- [ ] No panics in normal operation
- [ ] Clean terminal restore on exit
