# Testing Guide

> Manual test cases and verification procedures for Glance

---

## Test Environment Setup

### Prerequisites

1. **Test Database**: Start the PostgreSQL container

   ```bash
   just db
   ```

2. **Environment Variables**: Copy and configure `.env`

   ```bash
   cp .env.example .env
   # Add your LLM API key (OPENAI_API_KEY or ANTHROPIC_API_KEY)
   ```

3. **Build**: Compile the application
   ```bash
   cargo build
   ```

---

## Automated Tests

### Unit Tests

Run all unit tests (no external dependencies required):

```bash
cargo test
```

### Integration Tests

Run integration tests (requires test database):

```bash
just db
cargo test --test integration_tests
```

### Full Test Suite

```bash
just test
```

### With Ollama Integration

```bash
ollama serve  # In another terminal
just test-integration
```

---

## Manual Test Cases

### US-1: Connect to Database

#### TC-1.1: Connection String

**Steps:**

1. Run `cargo run -- "postgres://glance:glance@localhost:5432/glance_test"`
2. Verify application starts
3. Verify header shows `[db: glance_test @ localhost]`

**Expected:** Application connects and displays database info in header.

#### TC-1.2: Named Connection

**Steps:**

1. Create `~/.config/glance/config.toml`:
   ```toml
   [connections.default]
   host = "localhost"
   port = 5432
   database = "glance_test"
   user = "glance"
   password = "glance"
   ```
2. Run `cargo run`
3. Verify connection succeeds

**Expected:** Application uses default connection from config.

#### TC-1.3: Invalid Host

**Steps:**

1. Run `cargo run -- "postgres://user:pass@invalid.host:5432/db"`

**Expected:** Clear error message: "Cannot connect to invalid.host:5432..."

#### TC-1.4: Invalid Credentials

**Steps:**

1. Run `cargo run -- "postgres://wrong:wrong@localhost:5432/glance_test"`

**Expected:** Error message: "Authentication failed for user 'wrong'..."

#### TC-1.5: Invalid Database

**Steps:**

1. Run `cargo run -- "postgres://glance:glance@localhost:5432/nonexistent"`

**Expected:** Error message: "Database 'nonexistent' does not exist."

---

### US-2: Ask Questions in Natural Language

#### TC-2.1: Simple Query

**Steps:**

1. Connect to test database
2. Type: "Show me all users"
3. Press Enter

**Expected:**

- Question appears in chat with "You:" label
- LLM response streams in with "Glance:" label
- SQL visible in sidebar
- Results table displayed

#### TC-2.2: Complex Query

**Steps:**

1. Type: "How many orders does each user have?"
2. Press Enter

**Expected:**

- LLM generates appropriate JOIN query
- Results show user/order count pairs

#### TC-2.3: Ambiguous Query

**Steps:**

1. Type: "Show me everything"
2. Press Enter

**Expected:**

- LLM asks for clarification or makes reasonable assumption
- Response is helpful, not an error

---

### US-3: View Query Results

#### TC-3.1: Table Display

**Steps:**

1. Run query that returns multiple rows
2. Observe table formatting

**Expected:**

- Column headers match database columns
- Borders use Unicode box-drawing characters
- Row count and execution time shown below table

#### TC-3.2: NULL Values

**Steps:**

1. Query: `/sql SELECT * FROM users WHERE name IS NULL`

**Expected:**

- NULL values display as "NULL" in dim/italic style
- Distinguishable from the string "NULL"

#### TC-3.3: Long Values

**Steps:**

1. Insert a row with a very long value (>40 chars)
2. Query that row

**Expected:**

- Long values truncate with "..." at 40 characters
- Table remains readable

#### TC-3.4: Wide Tables

**Steps:**

1. Query a table with many columns
2. Observe horizontal handling

**Expected:**

- Columns scale to fit terminal width
- Table remains usable

#### TC-3.5: Large Result Sets

**Steps:**

1. Query: `/sql SELECT generate_series(1, 1500)`

**Expected:**

- Results truncated to 1000 rows
- Warning message about truncation displayed

---

### US-4: Safe Query Execution

#### TC-4.1: SELECT Auto-Execute

**Steps:**

1. Ask a question that generates a SELECT query

**Expected:**

- Query executes immediately without confirmation
- Results display in chat

#### TC-4.2: INSERT Confirmation

**Steps:**

1. Type: "Add a new user with email test@test.com"
2. Observe confirmation dialog

**Expected:**

- Yellow warning dialog appears
- Shows the INSERT statement
- Prompts for confirmation

#### TC-4.3: DELETE Warning

**Steps:**

1. Type: "Delete the user with id 999"
2. Observe confirmation dialog

**Expected:**

- Red warning dialog appears
- Shows "WARNING: This query may cause data loss"
- Requires explicit confirmation

#### TC-4.4: Cancel Mutation

**Steps:**

1. Trigger a mutation query
2. Press `n` or `Esc` at confirmation

**Expected:**

- Query cancelled message appears
- No data modified

#### TC-4.5: Approve Mutation

**Steps:**

1. Trigger a mutation query
2. Press `y` or `Enter` at confirmation

**Expected:**

- Query executes
- Results or affected row count displayed

---

### US-5: View Query History

#### TC-5.1: Query Log Display

**Steps:**

1. Execute several queries
2. Observe sidebar

**Expected:**

- Queries appear in sidebar, most recent at top
- Each shows: truncated SQL, status icon, time, row count

#### TC-5.2: Query Selection

**Steps:**

1. Press Tab to focus sidebar
2. Use arrow keys to navigate
3. Press Enter on a query

**Expected:**

- Selected query highlighted
- Full SQL shown in modal/detail view

#### TC-5.3: Error Indicator

**Steps:**

1. Execute a query with an error
2. Observe sidebar

**Expected:**

- Failed query shows red ✗ indicator
- Error info visible

---

### US-6: Write Raw SQL

#### TC-6.1: Raw SQL Mode

**Steps:**

1. Type: `/sql SELECT COUNT(*) FROM orders`
2. Press Enter

**Expected:**

- Query executes directly (no LLM)
- Results display normally

#### TC-6.2: Raw SQL Safety

**Steps:**

1. Type: `/sql DELETE FROM users WHERE id = 999`

**Expected:**

- Confirmation dialog appears (same as LLM-generated)
- Safety rules still apply

---

### US-7: Exit Application

#### TC-7.1: Ctrl+C Exit

**Steps:**

1. Press Ctrl+C

**Expected:**

- Application exits cleanly
- Terminal restored to normal state

#### TC-7.2: Ctrl+Q Exit

**Steps:**

1. Press Ctrl+Q

**Expected:**

- Application exits cleanly

#### TC-7.3: Command Exit

**Steps:**

1. Type: `/quit` or `/exit`
2. Press Enter

**Expected:**

- Application exits cleanly

---

### Keyboard Shortcuts

#### TC-KB.1: Chat Scrolling

**Steps:**

1. Generate enough content to scroll
2. Use ↑/↓, Page Up/Down, Home/End

**Expected:**

- Chat scrolls appropriately
- Scroll position maintained

#### TC-KB.2: Panel Focus

**Steps:**

1. Press Tab repeatedly

**Expected:**

- Focus cycles between chat and sidebar
- Focused panel has cyan border

#### TC-KB.3: Clear Chat

**Steps:**

1. Press Ctrl+L

**Expected:**

- Chat history cleared
- LLM context reset

---

### Commands

#### TC-CMD.1: Help Command

**Steps:**

1. Type: `/help`

**Expected:**

- List of available commands displayed

#### TC-CMD.2: Schema Command

**Steps:**

1. Type: `/schema`

**Expected:**

- Database schema summary displayed
- Shows tables, columns, relationships

#### TC-CMD.3: Clear Command

**Steps:**

1. Type: `/clear`

**Expected:**

- Chat cleared
- Confirmation message shown

---

## Edge Cases

### EC-1: Empty Database

**Steps:**

1. Connect to a database with no tables

**Expected:**

- Application starts successfully
- Schema shows empty
- LLM informed of empty schema

### EC-2: Very Large Schema

**Steps:**

1. Connect to database with 100+ tables

**Expected:**

- Schema introspection completes (may take a few seconds)
- LLM receives truncated schema if too large
- Warning if schema truncated

### EC-3: Network Interruption

**Steps:**

1. Connect to database
2. Disconnect network briefly
3. Try to execute query

**Expected:**

- Clear error message about connection
- Application remains responsive

### EC-4: LLM API Error

**Steps:**

1. Set invalid API key
2. Try to ask a question

**Expected:**

- Clear error message about API authentication
- Raw SQL mode still works

### EC-5: Terminal Resize

**Steps:**

1. Resize terminal window while running

**Expected:**

- UI re-renders correctly
- No visual artifacts
- Content remains visible

---

## Performance Benchmarks

### PB-1: Startup Time

**Target:** < 500ms to interactive prompt

**Steps:**

1. Time application startup: `time cargo run -- "postgres://..."`

### PB-2: Schema Introspection

**Target:** < 2s for databases with < 100 tables

**Steps:**

1. Connect to test database
2. Observe time to first prompt

### PB-3: Query Execution

**Target:** First rows within 100ms of execution

**Steps:**

1. Execute `SELECT * FROM users`
2. Observe response time

### PB-4: UI Responsiveness

**Target:** < 16ms frame time (60fps)

**Steps:**

1. Scroll rapidly through chat
2. Observe for lag or stuttering

---

## Regression Checklist

Before release, verify:

- [ ] All automated tests pass (`just test`)
- [ ] Manual test cases TC-1.1 through TC-7.3 pass
- [ ] Edge cases EC-1 through EC-5 handled gracefully
- [ ] Performance benchmarks met
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt --check`)
- [ ] Documentation up to date
