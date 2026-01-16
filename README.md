# Glance

A fast, terminal-native, LLM-first database companion written in Rust.

> Chat with your Postgres database using natural language.

## Status

🚧 **Early Development** — Design phase complete, implementation starting.

## Features (Planned)

- **Natural language queries** — Ask questions, get SQL + results
- **Auto-execute reads** — SELECT queries run automatically
- **Confirm writes** — Mutations require explicit approval
- **Schema-aware** — LLM knows your tables and relationships
- **Fast** — Rust-native, instant startup, minimal footprint

## Documentation

- [Vision](docs/VISION.md) — Product vision and positioning
- [Roadmap](docs/ROADMAP.md) — Feature milestones
- [Architecture](docs/ARCHITECTURE.md) — Technical design
- [Tech Stack](docs/TECH_STACK.md) — Crate selections
- [Agent Rules](docs/AGENT_RULES.md) — Development principles

## Development

```bash
# Format code
cargo fmt

# Lint
cargo clippy

# Test
cargo test

# Run
cargo run
```

## License

MIT
