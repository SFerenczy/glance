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

### Docker Worktree Support

**Status**: Idea
**Effort**: Low-Medium
**Value**: Medium (developer experience)

Make Docker development setup more robust for git worktree workflows:

- **Shared volume configuration** - Handle multiple worktree checkouts gracefully
- **Path-independent setup** - Don't assume fixed repo paths
- **Worktree detection** - Auto-detect and configure for worktree environments

**Use Cases**:
- Developers using git worktrees for parallel feature development
- CI environments with dynamic checkout paths

---

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
