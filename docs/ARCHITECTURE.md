# Architecture

> This document describes the high-level architecture of Glance.

**Status**: Initial design phase.

---

## Overview

Glance is a lightweight, LLM-first database companion. It provides a fast, terminal-native interface for querying and exploring Postgres databases using natural language.

---

## Design Goals

1. **Fast** — Rust-native, minimal dependencies, instant startup
2. **LLM-first** — Natural language as the primary interaction mode
3. **Safe** — Auto-execute reads, confirm writes
4. **Transparent** — Always show the SQL being executed
5. **Extensible** — Clean abstractions for future database/LLM providers

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                           TUI                                │
│  ┌─────────────────────────────┐  ┌───────────────────────┐ │
│  │        Chat Panel           │  │     Query Sidebar     │ │
│  │  - Message history          │  │  - Generated SQL      │ │
│  │  - Result tables            │  │  - Execution stats    │ │
│  │  - Input prompt             │  │  - Query log          │ │
│  └─────────────────────────────┘  └───────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                      Core Application                        │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────┐  │
│  │   Orchestrator │  │ Safety       │  │   Config        │  │
│  │   - Chat loop   │  │ Classifier   │  │   Manager       │  │
│  │   - State mgmt  │  │              │  │                 │  │
│  └──────────────┘  └──────────────┘  └───────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                      Service Layer                           │
│  ┌──────────────────────┐  ┌──────────────────────────────┐ │
│  │     LLM Client       │  │       Database Client        │ │
│  │  - OpenAI            │  │  - Connection management     │ │
│  │  - Anthropic         │  │  - Schema introspection      │ │
│  │  - (Ollama future)   │  │  - Query execution           │ │
│  └──────────────────────┘  └──────────────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                      External Systems                        │
│  ┌──────────────────────┐  ┌──────────────────────────────┐ │
│  │   LLM APIs           │  │       PostgreSQL             │ │
│  └──────────────────────┘  └──────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## Module Structure

```
src/
├── main.rs              # Entry point, CLI argument parsing
├── app.rs               # Application state and orchestration
├── config.rs            # Configuration loading and management
│
├── tui/
│   ├── mod.rs           # TUI initialization and main loop
│   ├── app.rs           # TUI application state
│   ├── ui.rs            # Layout and rendering
│   ├── widgets/
│   │   ├── chat.rs      # Chat panel widget
│   │   ├── sidebar.rs   # Query log sidebar widget
│   │   ├── table.rs     # Result table widget
│   │   └── input.rs     # Input prompt widget
│   └── events.rs        # Keyboard/mouse event handling
│
├── llm/
│   ├── mod.rs           # LLM client trait and factory
│   ├── openai.rs        # OpenAI implementation
│   ├── anthropic.rs     # Anthropic implementation
│   ├── prompt.rs        # System prompts and prompt construction
│   └── types.rs         # Message types, responses
│
├── db/
│   ├── mod.rs           # Database client trait and factory
│   ├── postgres.rs      # Postgres implementation
│   ├── schema.rs        # Schema introspection types
│   └── types.rs         # Query results, column types
│
├── safety/
│   ├── mod.rs           # Query safety classification
│   └── parser.rs        # SQL statement parsing
│
└── error.rs             # Error types
```

---

## Data Flow

### Chat Query Flow

```
User Input (natural language)
         │
         ▼
┌─────────────────────┐
│    Orchestrator     │
└─────────────────────┘
         │
         ▼
┌─────────────────────┐     ┌─────────────────────┐
│     LLM Client      │────▶│   Schema Context    │
│  (generate SQL)     │◀────│   (tables, cols)    │
└─────────────────────┘     └─────────────────────┘
         │
         ▼ (SQL string)
┌─────────────────────┐
│  Safety Classifier  │
└─────────────────────┘
         │
         ├─── SELECT/EXPLAIN ──▶ Auto-execute
         │
         ├─── INSERT/UPDATE ───▶ Confirm prompt ──▶ Execute or cancel
         │
         └─── DELETE/DROP ─────▶ Confirm + warn ──▶ Execute or cancel
                                        │
                                        ▼
                              ┌─────────────────────┐
                              │   Database Client   │
                              │   (execute query)   │
                              └─────────────────────┘
                                        │
                                        ▼
                              ┌─────────────────────┐
                              │    TUI Update       │
                              │  - Show results     │
                              │  - Update sidebar   │
                              └─────────────────────┘
```

### Schema Discovery Flow

```
On Connect
    │
    ▼
┌─────────────────────┐
│   Database Client   │
│  (introspect)       │
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│   Schema Cache      │
│  - Tables           │
│  - Columns + types  │
│  - Foreign keys     │
│  - Indexes          │
└─────────────────────┘
    │
    ▼
┌─────────────────────┐
│   LLM System Prompt │
│  (inject schema)    │
└─────────────────────┘
```

---

## Query Safety Classification

The safety classifier parses SQL and categorizes operations:

| Category        | Operations                            | Behavior                 |
| --------------- | ------------------------------------- | ------------------------ |
| **Safe**        | `SELECT`, `EXPLAIN`, `SHOW`           | Auto-execute             |
| **Mutating**    | `INSERT`, `UPDATE`                    | Confirm before execution |
| **Destructive** | `DELETE`, `DROP`, `TRUNCATE`, `ALTER` | Confirm with warning     |

Implementation approach:

- Use `sqlparser-rs` to parse SQL statements
- Extract the statement type from the AST
- Handle multi-statement queries by classifying the most dangerous operation

---

## Configuration

Configuration file: `~/.config/glance/config.toml`

```toml
[llm]
provider = "openai"          # or "anthropic"
model = "gpt-4o"
# api_key loaded from OPENAI_API_KEY or ANTHROPIC_API_KEY env var

[connections.default]
name = "Local Dev"
host = "localhost"
port = 5432
database = "myapp_dev"
user = "postgres"
# password loaded from PGPASSWORD env var or prompted

[connections.prod]
name = "Production"
host = "prod.example.com"
port = 5432
database = "myapp_prod"
user = "readonly"
```

---

## Error Handling Strategy

- Use `thiserror` for defining error types
- Errors bubble up to the orchestrator
- TUI displays user-friendly error messages
- Full error details available in debug mode

Error categories:

- **Connection errors** — Can't reach database
- **Query errors** — SQL syntax or execution errors
- **LLM errors** — API failures, rate limits
- **Config errors** — Missing or invalid configuration
