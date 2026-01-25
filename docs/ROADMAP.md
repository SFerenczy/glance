# Roadmap

> Living document tracking planned features and milestones.

---

## v0.1 — MVP: Chat with Postgres (Complete)

The minimum viable product: connect to a Postgres database and query it via natural language.

### Core Features

- [x] **Connection management** — Connect via connection string or config file
- [x] **Schema discovery** — Auto-introspect tables, columns, types, foreign keys
- [x] **Chat interface** — Natural language input with streaming responses
- [x] **SQL generation** — LLM generates SQL from user questions
- [x] **Query safety classification**
  - `SELECT` / `EXPLAIN` → Auto-execute
  - `INSERT` / `UPDATE` → Confirm before execution
  - `DELETE` / `DROP` / `TRUNCATE` → Confirm with warning
- [x] **Query log sidebar** — Shows generated SQL, execution time, row count
- [x] **Result table** — Paginated, scrollable, keyboard-navigable
- [x] **Manual SQL mode** — Escape hatch to write raw SQL
- [x] **Basic error handling** — Clear error messages for connection/query failures

### LLM Integration

- [x] OpenAI API support (gpt-5, gpt-5-mini)
- [x] Anthropic API support (claude-3.5-sonnet)
- [x] API key configuration via environment variable or config file

### Configuration

- [x] Config file at `~/.config/glance/config.toml`
- [x] Connection profiles (name, host, port, database, user)
- [x] LLM provider selection

---

## v0.2 — Polish & Usability (Complete)

- [x] Query history persistence (SQLite local storage)
- [x] Copy query to clipboard
- [x] Copy results to clipboard (as table, CSV, or JSON)
- [x] Export results to file (CSV, JSON)
- [x] Keyboard shortcut help overlay (`?`)
- [x] Connection selector UI (if multiple profiles)
- [x] Improved table rendering (column width auto-sizing, truncation)

---

## What's Next

See [BACKLOG.md](BACKLOG.md) for unscheduled features and future ideas.
