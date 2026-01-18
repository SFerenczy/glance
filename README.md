# Glance

A fast, terminal-native, LLM-first database companion written in Rust.

> Chat with your Postgres database using natural language.

## Status

🚧 **v0.1 Development** — Core features implemented, testing in progress.

## Features

- **Natural language queries** — Ask questions, get SQL + results
- **Auto-execute reads** — SELECT queries run automatically
- **Confirm writes** — Mutations require explicit approval
- **Schema-aware** — LLM knows your tables and relationships
- **Fast** — Rust-native, instant startup, minimal footprint

## Installation

### Prerequisites

- Rust 1.75+ ([install](https://rustup.rs/))
- PostgreSQL 12+
- OpenAI or Anthropic API key

### Build from Source

```bash
git clone https://github.com/SFerenczy/glance.git
cd glance
cargo build --release
```

The binary will be at `target/release/glance`.

## Quick Start

### 1. Set your LLM API key

```bash
export OPENAI_API_KEY=sk-...
# or
export ANTHROPIC_API_KEY=sk-ant-...
```

### 2. Connect to your database

```bash
# Using connection string
glance "postgres://user:password@localhost:5432/mydb"

# Using arguments
glance --host localhost --database mydb --user postgres --password
```

### 3. Ask questions

```
> How many orders were placed this week?
```

Glance generates SQL, executes it safely, and displays results.

## Usage Examples

```
> Show me all users who signed up this month
> What's the average order value by customer?
> /sql SELECT COUNT(*) FROM orders WHERE status = 'pending'
> /schema
> /help
```

## Configuration

Create `~/.config/db-glance/config.toml`:

```toml
[llm]
provider = "openai"
model = "gpt-5"

[connections.default]
host = "localhost"
port = 5432
database = "mydb"
user = "postgres"
```

Then just run `glance` with no arguments.

## Documentation

### User Documentation

- [Usage Guide](docs/USAGE.md) — Complete user guide with examples
- [Testing Guide](docs/TESTING.md) — Manual test cases and verification

### Developer Documentation

- [Development Guide](docs/DEVELOPMENT.md) — Setting up your dev environment
- [Architecture](docs/ARCHITECTURE.md) — Technical design
- [Tech Stack](docs/TECH_STACK.md) — Crate selections
- [Implementation Plan](docs/IMPLEMENTATION_PLAN.md) — Development phases

### Project Documentation

- [Vision](docs/VISION.md) — Product vision and positioning
- [Roadmap](docs/ROADMAP.md) — Feature milestones
- [v0.1 Specification](docs/specs/v0.1.md) — Detailed requirements

## Headless Mode

Glance supports headless mode for AI-assisted testing and automation. Run without a terminal UI, execute scripted events, and capture output for verification.

### Basic Usage

```bash
# Run with inline events
glance --headless --mock-db --events "type:hello,key:enter"

# Run with script file
glance --headless --mock-db --script tests/tui/fixtures/basic_flow.txt

# JSON output for programmatic analysis
glance --headless --mock-db --events "type:test" --output json

# Frame-by-frame debugging
glance --headless --mock-db --events "type:a,type:b" --output frames
```

### Event DSL

| Event              | Example                    | Description                       |
| ------------------ | -------------------------- | --------------------------------- |
| `key:`             | `key:enter`, `key:ctrl+c`  | Key press with optional modifiers |
| `type:`            | `type:hello world`         | Type text into input              |
| `wait:`            | `wait:100ms`, `wait:2s`    | Wait for duration                 |
| `resize:`          | `resize:120x40`            | Resize terminal                   |
| `assert:contains:` | `assert:contains:hello`    | Assert screen contains text       |
| `assert:state:`    | `assert:state:focus=Input` | Assert application state          |

### Exit Codes

| Code | Meaning                          |
| ---- | -------------------------------- |
| 0    | Success (all assertions passed)  |
| 1    | Test failure (assertions failed) |
| 2    | Error (invalid syntax, etc.)     |

See [v0.2e Specification](docs/specs/v0.2e.md) for complete documentation.

## Development

```bash
# Start test database
just db

# Run all tests
just test

# Run with test database
just dev

# Format and lint
just check
```

See [Development Guide](docs/DEVELOPMENT.md) for full setup instructions.

## Keyboard Shortcuts

| Key      | Action               |
| -------- | -------------------- |
| Enter    | Submit input         |
| Ctrl+C/Q | Exit                 |
| Ctrl+L   | Clear chat           |
| Tab      | Switch panel focus   |
| ↑/↓      | Scroll / navigate    |
| Esc      | Cancel / close modal |

## License

MIT
