# Development Guide

> Setting up your development environment for Glance

---

## Prerequisites

### Required

- **Rust 1.75+**: Install via [rustup](https://rustup.rs/)

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustup update
  ```

- **Docker**: For running the test database
  - [Docker Desktop](https://www.docker.com/products/docker-desktop/) (macOS/Windows)
  - [Docker Engine](https://docs.docker.com/engine/install/) (Linux)

- **just** (command runner): For running common tasks
  ```bash
  cargo install just
  ```

### Optional

- **Ollama**: For local LLM integration tests
  - [Install Ollama](https://ollama.ai/)
  - Pull a small model: `ollama pull llama3.2:3b`

---

## Quick Start

```bash
# Clone the repository
git clone https://github.com/SFerenczy/glance.git
cd glance

# Start the test database
just db

# Run all tests
just test

# Run the application against test database
just dev
```

---

## Environment Setup

### 1. Copy the example environment file

```bash
cp .env.example .env
```

### 2. Configure your LLM provider

Edit `.env` and add your API key:

```bash
# For OpenAI
OPENAI_API_KEY=sk-...

# Or for Anthropic
ANTHROPIC_API_KEY=sk-ant-...
```

### 3. Start the test database

```bash
just db
```

This starts a PostgreSQL 16 container with:

- **Host**: localhost
- **Port**: 5432
- **User**: glance
- **Password**: glance
- **Database**: glance_test

The database is seeded with test data from `tests/fixtures/seed.sql`.

---

## Common Commands

| Command                 | Description                            |
| ----------------------- | -------------------------------------- |
| `just db`               | Start test database                    |
| `just db-down`          | Stop test database                     |
| `just test`             | Run all tests                          |
| `just test-integration` | Run tests including Ollama integration |
| `just check`            | Run formatter and linter               |
| `just run`              | Run the application                    |
| `just dev`              | Run against test database              |

---

## Project Structure

```
glance/
├── src/
│   ├── main.rs           # Application entry point
│   ├── error.rs          # Error types
│   ├── config.rs         # Configuration loading
│   ├── cli.rs            # CLI argument parsing
│   ├── db/               # Database layer
│   ├── llm/              # LLM integration
│   ├── safety/           # Query safety classification
│   └── tui/              # Terminal UI
├── tests/
│   ├── fixtures/         # Test data and SQL
│   └── integration/      # Integration tests
├── docs/                 # Documentation
├── Cargo.toml            # Rust dependencies
├── docker compose.yml    # Test database setup
└── justfile              # Task runner commands
```

---

## Testing Strategy

### Unit Tests

Run with mock LLM client—no external dependencies required:

```bash
cargo test
```

### Integration Tests

Require the test database:

```bash
just db
cargo test
```

### LLM Integration Tests

Require Ollama running locally:

```bash
ollama serve  # In another terminal
just test-integration
```

These tests are skipped if Ollama is not available.

---

## Code Quality

Before committing, ensure:

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Run all checks
just check
```

CI will fail if formatting or linting issues are present.

---

## Troubleshooting

### Database connection refused

Ensure Docker is running and the test database is started:

```bash
docker ps  # Should show glance-postgres container
just db    # Start if not running
```

### Permission denied on Docker

Add your user to the docker group:

```bash
sudo usermod -aG docker $USER
# Log out and back in
```

### Ollama tests skipped

This is expected if Ollama is not installed or not running. These tests are optional for local development.

---

## IDE Setup

### VS Code

Recommended extensions:

- rust-analyzer
- Even Better TOML
- crates

### JetBrains (RustRover/CLion)

Install the Rust plugin and configure the toolchain to use rustup.
