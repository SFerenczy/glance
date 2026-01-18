# Roadmap

> Living document tracking planned features and milestones.

---

## v0.1 — MVP: Chat with Postgres

The minimum viable product: connect to a Postgres database and query it via natural language.

### Core Features

- [ ] **Connection management** — Connect via connection string or config file
- [ ] **Schema discovery** — Auto-introspect tables, columns, types, foreign keys
- [ ] **Chat interface** — Natural language input with streaming responses
- [ ] **SQL generation** — LLM generates SQL from user questions
- [ ] **Query safety classification**
  - `SELECT` / `EXPLAIN` → Auto-execute
  - `INSERT` / `UPDATE` → Confirm before execution
  - `DELETE` / `DROP` / `TRUNCATE` → Confirm with warning
- [ ] **Query log sidebar** — Shows generated SQL, execution time, row count
- [ ] **Result table** — Paginated, scrollable, keyboard-navigable
- [ ] **Manual SQL mode** — Escape hatch to write raw SQL
- [ ] **Basic error handling** — Clear error messages for connection/query failures

### LLM Integration

- [ ] OpenAI API support (gpt-5, gpt-5-mini)
- [ ] Anthropic API support (claude-3.5-sonnet)
- [ ] API key configuration via environment variable or config file

### Configuration

- [ ] Config file at `~/.config/glance/config.toml`
- [ ] Connection profiles (name, host, port, database, user)
- [ ] LLM provider selection

---

## v0.2 — Polish & Usability

- [ ] Query history persistence (SQLite local storage)
- [ ] Copy query to clipboard
- [ ] Copy results to clipboard (as table, CSV, or JSON)
- [ ] Export results to file (CSV, JSON)
- [ ] Keyboard shortcut help overlay (`?`)
- [ ] Connection selector UI (if multiple profiles)
- [ ] Improved table rendering (column width auto-sizing, truncation)

---

## v0.3 — Multi-Provider LLM

- [ ] Ollama support (local models)
- [ ] Azure OpenAI support
- [ ] Google Gemini support
- [ ] Configurable model selection per provider
- [ ] Token usage tracking and display

---

## v0.4 — Schema Tools

- [ ] Schema design assistant ("Help me design a schema for X")
- [ ] Migration generation ("Add a soft delete column to users")
- [ ] Schema documentation generator
- [ ] Table/column description viewer

---

## v0.5 — Analysis & Quality

- [ ] Query performance analysis (`EXPLAIN ANALYZE` integration)
- [ ] Index suggestions based on query patterns
- [ ] Data quality checks ("Find orphaned records", "Check for NULLs")
- [ ] Slow query identification

---

## Future Considerations

- [ ] SQLite support
- [ ] MySQL support
- [ ] Multiple simultaneous connections
- [ ] Saved queries / bookmarks
- [ ] Query templates
- [ ] Team sharing features
- [ ] Plugin system for custom tools
