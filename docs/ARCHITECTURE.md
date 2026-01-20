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

## Architectural Principles

Glance follows a **pragmatic layered architecture** with dependency inversion at key boundaries. This approach provides testability and modularity without the ceremony of full hexagonal/ports-and-adapters.

### Core Principles

1. **Dependency inversion at infrastructure boundaries**
   - External systems (databases, LLM APIs) are accessed through traits (`dyn DatabaseClient`, `dyn LlmClient`)
   - Enables mock implementations for testing without real services
   - Allows swapping providers (OpenAI ↔ Anthropic ↔ Ollama) at runtime

2. **Thin orchestrator, focused components**
   - The `Orchestrator` coordinates but doesn't implement business logic
   - Each component has a single responsibility:
     - `CommandRouter` — parse and dispatch commands
     - `QueryExecutor` — classify and execute SQL
     - `LlmService` — prompt building and tool handling
     - `ConnectionManager` — connection lifecycle

3. **Front-end agnostic core**
   - TUI, CLI, and headless testing share the same command infrastructure
   - `InputResult` enum provides a uniform response type for all front-ends
   - No UI concerns leak into business logic

4. **Explicit over clever**
   - Pass dependencies explicitly rather than using global state
   - Prefer composition over inheritance
   - Avoid magic—code paths should be traceable

### What We Don't Do

- **No formal port interfaces** — Trait objects suffice; we don't need `QueryRepository` abstractions for SQLite persistence that won't change
- **No dependency injection framework** — Manual wiring is sufficient at this scale
- **No event sourcing** — Simple state mutations are appropriate for a TUI tool

### Testing Strategy

The architecture enables focused testing at each layer:

| Layer                | Test Type   | Dependencies      |
| -------------------- | ----------- | ----------------- |
| Command parsing      | Unit        | None              |
| Query classification | Unit        | None              |
| Command handlers     | Integration | Mock DB, Mock LLM |
| Full orchestrator    | Integration | Mock DB, Mock LLM |
| TUI behavior         | Headless    | Mock DB, Mock LLM |

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
├── main.rs                   # Entry point, CLI argument parsing
├── app.rs                    # Thin orchestrator (~300 lines)
├── config.rs                 # Configuration loading and management
├── error.rs                  # Error types (thiserror)
│
├── commands/                 # Command parsing and dispatch
│   ├── mod.rs
│   ├── router.rs             # Parse input → Command enum
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── connection.rs     # /connect, /connections, /conn
│   │   ├── history.rs        # /history
│   │   ├── llm_settings.rs   # /llm provider|model|key
│   │   ├── queries.rs        # /savequery, /queries, /usequery
│   │   └── system.rs         # /help, /clear, /schema, /quit
│   └── help.rs               # Help text constants
│
├── connection/               # Connection lifecycle management
│   ├── mod.rs
│   └── manager.rs            # Connect, switch, close
│
├── query/                    # Query execution pipeline
│   ├── mod.rs
│   ├── executor.rs           # Classify → execute → format
│   └── formatter.rs          # Result formatting
│
├── session/                  # Chat session state
│   ├── mod.rs
│   └── chat.rs               # Conversation, prompt cache
│
├── tui/                      # Terminal UI (ratatui)
│   ├── mod.rs                # TUI initialization and main loop
│   ├── app.rs                # TUI application state
│   ├── ui.rs                 # Layout and rendering
│   ├── events.rs             # Keyboard/mouse event handling
│   ├── headless/             # Headless testing support
│   └── widgets/              # UI components
│       ├── chat.rs
│       ├── sidebar.rs
│       ├── table.rs
│       └── input.rs
│
├── llm/                      # LLM integration
│   ├── mod.rs                # LlmClient trait
│   ├── service.rs            # LLM orchestration (prompts, tools)
│   ├── openai.rs             # OpenAI adapter
│   ├── anthropic.rs          # Anthropic adapter
│   ├── ollama.rs             # Ollama adapter
│   ├── mock.rs               # Mock for testing
│   ├── prompt.rs             # System prompts
│   ├── tools.rs              # Tool definitions
│   └── types.rs              # Message types, responses
│
├── db/                       # Database integration
│   ├── mod.rs                # DatabaseClient trait
│   ├── postgres.rs           # Postgres adapter
│   ├── mock.rs               # Mock for testing
│   └── types.rs              # Schema, QueryResult
│
├── persistence/              # Local state (SQLite)
│   ├── mod.rs                # StateDb wrapper
│   ├── connections.rs        # Connection profiles
│   ├── history.rs            # Query history
│   ├── saved_queries.rs      # Saved queries
│   ├── llm_settings.rs       # LLM configuration
│   ├── secrets.rs            # Keyring integration
│   └── migrations.rs         # Schema migrations
│
└── safety/                   # Query safety
    ├── mod.rs                # Classification API
    └── parser.rs             # SQL parsing (sqlparser-rs)
```

### Component Responsibilities

| Component           | Responsibility                    | Dependencies                    |
| ------------------- | --------------------------------- | ------------------------------- |
| `Orchestrator`      | Route input, compose results      | All components                  |
| `CommandRouter`     | Parse `/commands` into typed enum | None                            |
| `QueryExecutor`     | Classify SQL, execute, format     | `DatabaseClient`, `safety`      |
| `LlmService`        | Build prompts, handle tool calls  | `LlmClient`, `persistence`      |
| `ConnectionManager` | Connection lifecycle              | `DatabaseClient`, `persistence` |
| `ChatSession`       | Conversation state, prompt cache  | None                            |

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

Configuration file: `~/.config/db-glance/config.toml`

```toml
[llm]
provider = "openai"          # or "anthropic"
model = "gpt-5"
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
