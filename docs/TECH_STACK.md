# Tech Stack

> Crate selections and justifications for Glance.

---

## Core Dependencies

### Async Runtime

| Crate     | Version | Purpose                                                |
| --------- | ------- | ------------------------------------------------------ |
| **tokio** | 1.x     | Async runtime — industry standard, excellent ecosystem |

**Justification**: Best support for database drivers (sqlx, tokio-postgres) and HTTP clients. Required by most async Rust libraries.

---

### TUI

| Crate         | Version | Purpose                                     |
| ------------- | ------- | ------------------------------------------- |
| **ratatui**   | 0.28.x  | TUI framework — modern, actively maintained |
| **crossterm** | 0.28.x  | Terminal backend — cross-platform           |

**Justification**: Ratatui is the successor to tui-rs with active development. Crossterm provides cross-platform terminal handling without external dependencies.

---

### Database

| Crate    | Version | Purpose                                                 |
| -------- | ------- | ------------------------------------------------------- |
| **sqlx** | 0.8.x   | Async database driver with compile-time checked queries |

**Justification**:

- Async-native, works with tokio
- Supports Postgres (and SQLite/MySQL for future)
- Compile-time query verification (optional)
- Connection pooling built-in

---

### LLM Integration

| Crate          | Version | Purpose                       |
| -------------- | ------- | ----------------------------- |
| **reqwest**    | 0.12.x  | HTTP client for API calls     |
| **serde**      | 1.x     | Serialization/deserialization |
| **serde_json** | 1.x     | JSON handling                 |

**Justification**: reqwest is the standard async HTTP client. Direct API calls give us full control over request/response handling without depending on SDK crates that may lag behind API changes.

---

### SQL Parsing

| Crate         | Version | Purpose                               |
| ------------- | ------- | ------------------------------------- |
| **sqlparser** | 0.50.x  | SQL parsing for safety classification |

**Justification**: Pure Rust SQL parser supporting multiple dialects including PostgreSQL. Used to classify queries as safe/mutating/destructive.

---

### Error Handling

| Crate         | Version | Purpose                          |
| ------------- | ------- | -------------------------------- |
| **thiserror** | 1.x     | Derive macros for error types    |
| **anyhow**    | 1.x     | Application-level error handling |

**Justification**: `thiserror` for library-style error definitions, `anyhow` for convenient error propagation in the application layer.

---

### Configuration

| Crate    | Version | Purpose                           |
| -------- | ------- | --------------------------------- |
| **toml** | 0.8.x   | TOML parsing for config files     |
| **dirs** | 5.x     | Platform-specific directory paths |

**Justification**: TOML is human-readable and Rust-native. `dirs` provides correct paths for `~/.config/` across platforms.

---

### CLI

| Crate    | Version | Purpose                       |
| -------- | ------- | ----------------------------- |
| **clap** | 4.x     | Command-line argument parsing |

**Justification**: Feature-rich with derive macros, widely used, excellent documentation.

---

### Logging

| Crate                  | Version | Purpose               |
| ---------------------- | ------- | --------------------- |
| **tracing**            | 0.1.x   | Structured logging    |
| **tracing-subscriber** | 0.3.x   | Log output formatting |

**Justification**: Async-aware, structured logging. Integrates well with tokio ecosystem.

---

## Development Dependencies

| Crate                 | Version | Purpose                 |
| --------------------- | ------- | ----------------------- |
| **tokio-test**        | 0.4.x   | Async test utilities    |
| **pretty_assertions** | 1.x     | Better test diff output |

---

## Crates Explicitly Avoided

| Crate                       | Reason                                                |
| --------------------------- | ----------------------------------------------------- |
| **diesel**                  | Sync-only, heavier ORM approach                       |
| **openai-api-rs** / similar | SDK crates lag behind API changes; prefer direct HTTP |
| **cursive**                 | Less flexible than ratatui for custom layouts         |

---

## Version Policy

- Pin exact versions in `Cargo.toml` for reproducibility
- Use `cargo outdated` to check for updates
- Update dependencies deliberately, not automatically
- Run `cargo audit` before releases

---

## Feature Flags

Minimize compile times by disabling unused features:

```toml
[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }
reqwest = { version = "0.12", features = ["json"], default-features = false }
clap = { version = "4", features = ["derive"] }
```
