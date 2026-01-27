# Backlog & Future Ideas

> Unscheduled features and improvements awaiting evaluation and prioritization

---

## Performance & Monitoring

### Performance Instrumentation (v0.2c Phase 4)

**Status**: Optional verification infrastructure
**Effort**: Medium
**Value**: Low-Medium (nice-to-have monitoring)

Add lightweight performance tracking for non-functional requirements:

- **PerfStats struct** - Track frame intervals, draw times, input latency, cancel latency
- **Metrics in TUI** - Timestamp before/after each draw, measure event handling duration
- **Headless JSON output** - Expose metrics in `--headless --output json` mode
- **Non-flaky tests** - Tests verify metrics are present and update (not specific values)
- **Manual checklist** - Document manual verification procedures in TESTING.md

**Rationale**: Core functionality complete. Instrumentation is useful for debugging performance regressions but not required for user-facing features.

---

## Conversational AI Improvements

### Agentic AI Database Interaction

**Status**: Exploratory idea
**Effort**: High
**Value**: High (core feature enhancement)

Build autonomous AI agent capabilities for database interaction:

- **Multi-step reasoning** - AI plans and executes sequences of queries
- **Self-correction** - Detect errors and retry with adjusted approach
- **Context accumulation** - Build understanding across multiple interactions
- **Goal-oriented exploration** - AI explores schema to answer complex questions

**Open Questions**:
- How much autonomy should the agent have for mutations?
- How to handle long-running exploration sessions?
- What guardrails prevent runaway query sequences?

---

### Free-Text AI Responses (v0.3 idea)

**Status**: Exploratory idea
**Effort**: High
**Value**: High (better UX)

Rework AI chat to be more conversational:

- **Free-text responses** - AI explains results in natural language
- **Hyperlinked values** - Link to source queries in responses
- **Query result views** - Press Enter on query log item to see full table
- **Query editor mode** - Edit and save queries interactively
- **AI-initiated saving** - AI suggests saving useful queries

**Open Questions**:
- How to balance conversational responses with quick data access?
- Should query results be inline or in a separate view?
- How to handle multi-step query refinement?

---

## Developer Experience

## Architecture & Testing

### Orchestrator Actor Pattern (v0.2d)

**Status**: Planned
**Effort**: High
**Value**: High (architectural improvement)

Refactor to actor-based orchestration:

- **Non-blocking UI** - Orchestrator runs in dedicated task
- **Message channels** - TUI communicates via mpsc channels
- **Request queuing** - Queue requests while operation in progress
- **Clear operation status** - Visual indicators for LLM vs DB operations
- **Cancellation** - Clean cancel handling via actor messages

**Rationale**: Current background task approach works but actor pattern provides cleaner separation and natural request management.

See `docs/specs/v0.2d.md` for detailed specification.

### Headless Test Mode (v0.2e)

**Status**: Partially implemented
**Effort**: Medium (core exists, needs enhancement)
**Value**: High (AI agent debugging, CI testing)

Enhance headless mode for AI-assisted debugging:

- **Scripted event sequences** - Execute pre-defined test scenarios
- **Screen output capture** - Render to text for AI inspection
- **Programmatic state inspection** - JSON output of app state
- **Reproducible scenarios** - Deterministic test execution
- **CI integration** - Run TUI tests in non-TTY environments

**Status Note**: Basic headless mode exists (`--headless --mock-db`). Needs enhancement for full AI agent interaction.

See `docs/specs/v0.2e.md` for detailed specification.

---

## UX Improvements

### Per-Message Inline Spinners

**Status**: Deferred from v0.2a
**Effort**: Medium
**Value**: Medium (visual feedback improvement)

Replace global spinner with per-message inline status indicators:

- **Inline spinner after SQL** - Show spinner next to the user's SQL query while executing
- **Thinking indicator** - Display "thinking..." while LLM processes query
- **Streaming replacement** - Replace thinking indicator with streaming response text
- **Per-message placeholders** - Add message-level status in `src/tui/app.rs`
- **Chat rendering** - Render status inline in `src/tui/widgets/chat.rs`

**Rationale**: Current global spinner in header works but doesn't show which specific operation is in progress when multiple requests are queued. Per-message indicators provide clearer visual feedback.

**Deferred Reason**: Core v0.2a functionality complete. This is a nice-to-have visual enhancement that doesn't block other features.

### Interactive Command Input (Form Mode)

**Status**: Idea
**Effort**: Medium
**Value**: Medium-High (UX improvement)

Improve UX for commands with multiple parameters like `/conn add`:

- **Multi-field forms** - Tab through input fields (host, port, user, password, database)
- **Field validation** - Real-time feedback on input validity
- **Default values** - Pre-populate common defaults (port 5432 for Postgres)
- **Inline help** - Show hints for each field

**Implementation Options**:
1. **Modal form overlay** - Popup form with labeled fields
2. **Inline expansion** - Command expands into editable fields below input
3. **Wizard flow** - Step through fields one at a time with prompts

**Open Questions**:
- Which approach best fits the existing TUI patterns?
- Should forms support keyboard shortcuts for common values?

---

## LLM Provider Expansion

### Multi-Provider LLM Support (formerly v0.3)

**Status**: Idea
**Effort**: Medium-High
**Value**: High (broader user accessibility)

Expand LLM provider support beyond OpenAI and Anthropic:

- **Ollama support** - Local models for privacy-conscious users
- **Azure OpenAI support** - Enterprise customers with Azure contracts
- **Google Gemini support** - Alternative cloud provider
- **Configurable model selection** - Choose model per provider
- **Token usage tracking** - Display token consumption and costs

---

## Schema Tools

### Schema Design Assistant (formerly v0.4)

**Status**: Idea
**Effort**: High
**Value**: Medium-High (productivity feature)

AI-assisted schema design and management:

- **Schema design assistant** - "Help me design a schema for X"
- **Migration generation** - "Add a soft delete column to users"
- **Schema documentation generator** - Auto-generate docs from schema
- **Table/column description viewer** - Display comments and metadata

---

## Analysis & Quality

### Query Performance Analysis (formerly v0.5)

**Status**: Idea
**Effort**: High
**Value**: Medium-High (DBA-oriented feature)

Performance analysis and optimization tools:

- **EXPLAIN ANALYZE integration** - Visual query plan analysis
- **Index suggestions** - Recommend indexes based on query patterns
- **Data quality checks** - "Find orphaned records", "Check for NULLs"
- **Slow query identification** - Highlight expensive queries

---

## Future Features

### Schema Refresh Command

**Status**: Idea
**Effort**: Low
**Value**: Medium

Add `/refresh` command to force schema reload without reconnecting.

**Use Cases**:
- Schema changed via external migration
- New tables/columns added
- Permissions changed

### Query Template System

**Status**: Idea
**Effort**: Medium
**Value**: Medium

Parameterized query templates:

```sql
-- Template: user_orders
SELECT * FROM orders WHERE user_id = {{user_id}}
```

Users can save and reuse templates with different parameters.

### Multi-Database Sessions

**Status**: Idea
**Effort**: High
**Value**: Medium

Support multiple simultaneous database connections:

- Switch between connections with `/conn use <name>`
- Run queries across multiple databases
- Compare schemas side-by-side

### SQLite Support

**Status**: Idea
**Effort**: Medium
**Value**: Medium-High (local development, embedded use cases)

Add SQLite as a supported database backend.

### MySQL Support

**Status**: Idea
**Effort**: Medium
**Value**: High (popular database)

Add MySQL/MariaDB as a supported database backend.

### Saved Queries / Bookmarks

**Status**: Idea
**Effort**: Low-Medium
**Value**: Medium

Allow users to save frequently used queries for quick access.

### Plugin System

**Status**: Idea
**Effort**: High
**Value**: Medium (extensibility)

Custom tools and extensions via plugin architecture.

### Team Sharing Features

**Status**: Idea
**Effort**: High
**Value**: Medium (enterprise feature)

Share queries, connections, and insights with team members.

### Export Results

**Status**: Idea
**Effort**: Low
**Value**: Medium

Export query results to CSV, JSON, or SQL INSERT statements.

```
/export csv results.csv
/export json results.json
/export sql inserts.sql
```

---

## Technical Debt & Known Issues

### Flaky Orchestrator Actor Tests

**Status**: Currently ignored
**Effort**: Medium
**Priority**: Medium (tests work intermittently, need stabilization)

Two orchestrator actor tests are currently ignored due to timing dependencies with mock orchestrator:

- `test_queue_max_depth` - Mock processes requests too fast, queue empties before overflow test can verify QueueFull response
- `test_cancel_queued_request` - Timing-dependent, may need mock with artificial delays to reliably test cancellation of queued requests

**Location**: `src/tui/orchestrator_actor.rs:994-1086`

**Root Cause**: The mock orchestrator processes requests instantly without I/O delays, making it difficult to test queue states and cancellation timing. Real orchestrator has natural delays from LLM/DB I/O that make these scenarios testable in production.

**Proposed Solutions**:
1. Add configurable delays to mock orchestrator for testing
2. Refactor tests to use synchronization primitives (barriers, latches)
3. Use deterministic async test scheduling (tokio-test)
4. Extract queue logic for unit testing without full actor setup

**Impact**: Tests were passing before v0.2d actor refactor (commit 9e50285), then became flaky with tokio::select! implementation. Functionality works correctly in production, but we lack reliable automated verification.

---

### OS-Typical Word Deletion Shortcuts Missing

**Status**: Reported
**Effort**: Low
**Priority**: Medium

Standard shortcuts for removing words from input don't work:
- `Ctrl+W` - Delete word backward
- `Ctrl+Backspace` - Delete word backward
- `Alt+Backspace` - Delete word backward
- `Ctrl+Delete` - Delete word forward

**Location**: Input handling in `src/tui/widgets/input.rs` or similar

---

### SQL Completion Auto-Trigger Behavior

**Status**: Implemented with workaround
**Effort**: Low (refinement)
**Priority**: Low (works correctly, but logic could be cleaner)

SQL completion visibility logic required special handling to auto-trigger in suggestion contexts without requiring Ctrl+Space:

**Implementation**: Added `auto_trigger_contexts` check in `update()` method that automatically sets `force_opened = true` for contexts like `FromTable`, `SelectColumns`, `WhereClause`, etc.

**Location**: `src/tui/widgets/sql_completion.rs:246-289`

**Issue**: The original v0.2a spec intended completion to only show when:
- Filter is not empty (user typed something), OR
- User pressed Ctrl+Space (force_opened)

However, integration tests expected completions to appear automatically when typing "SELECT * FROM " without any additional trigger. The auto-trigger logic was added to maintain backward compatibility with test expectations.

**Considerations**:
- Current behavior feels intuitive to users (completions appear when expected)
- May want to make this configurable via settings in future
- Consider consolidating trigger logic to avoid dual conditions

### Multi-Query Responses Stack in Chat View

**Status**: Reported
**Effort**: Low-Medium
**Priority**: Medium (confusing UX)

When the AI agent generates multiple queries in a conversation (e.g., user asks several questions in sequence), all SQL queries are appended together in a single code block at the top of the first response area instead of being shown inline with each respective response.

**Current Behavior**:
```
Glance:
  ```sql
  SELECT COUNT(*) FROM users;
  ```
  ```sql
  INSERT INTO users ...;
  ```
  ```sql
  DELETE FROM users ...;
  ```
  [Results only for first query]
```

**Expected Behavior Options**:
1. **Hide SQL entirely in chat** - Since the query log panel on the right already shows executed queries, don't duplicate them in the chat view
2. **Inline per-response** - Each query should appear with its corresponding response/result, not stacked together

**Location**: Likely in chat message rendering (`src/tui/widgets/chat.rs`) or how assistant responses are accumulated

**Impact**: Users see a confusing wall of SQL that doesn't correspond to the visible results, making it hard to understand which query produced which output.

---

## Evaluation Criteria

When prioritizing backlog items, consider:

1. **User Impact** - Does it solve a real user pain point?
2. **Effort** - Development and testing time required
3. **Dependencies** - Does it require other features first?
4. **Maintenance** - Ongoing support burden
5. **Architecture** - Does it improve or complicate the codebase?

---

## Related Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)** - System design and patterns
- **[VISION.md](VISION.md)** - Project goals and philosophy
- **[docs/specs/](specs/)** - Detailed specifications for planned features
