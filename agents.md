# Agent Rules for Glance

> A lightweight, AI-first database viewer written in Rust.

This document defines the development principles and rules for AI agents working on this codebase. These are living documents—update them as the project evolves.

---

## Core Philosophy

- **AI-first**: The codebase should be readable and navigable by AI agents. Clear naming, modular structure, and comprehensive documentation.
- **No shortcuts**: We build things properly. No hacks, no "fix it later" code.
- **Living documentation**: Docs live in the repository and are continuously updated alongside code changes.

---

## Code Style

### Rust-Specific

- **Declarative over imperative**: Prefer functional control structures (`map`, `filter`, `fold`, iterators) over manual loops with mutation.
- **Immutability by default**: Avoid `mut` unless absolutely necessary. Prefer transformations over in-place mutation.
- **Type safety**: Leverage Rust's type system fully. Use newtypes, enums, and the `Option`/`Result` types idiomatically.
- **No `unwrap()` in production code**: Use proper error handling with `?`, `map_err`, or explicit `match`.

### Linting & Formatting

- **`cargo fmt`**: All code must be formatted. No exceptions.
- **`cargo clippy`**: All clippy warnings must be addressed. Use `#![deny(clippy::all)]`.
- **Documentation**: Public APIs must have doc comments. Use `#![warn(missing_docs)]`.

---

## Architecture Principles

- **Separation of concerns**: Clear boundaries between data access, business logic, and presentation.
- **Small, focused modules**: Each module should do one thing well.
- **Explicit dependencies**: No global state. Pass dependencies explicitly.
- **Testability**: Design for testing. Pure functions where possible.

---

## Error Handling

- Use `thiserror` or similar for defining error types.
- Errors should be informative and actionable.
- Propagate errors up; let the caller decide how to handle them.

---

## Testing

- Unit tests live alongside the code they test.
- Integration tests in `/tests` directory.
- Aim for high coverage on core logic.
- Tests are documentation—make them readable.

---

## Git & Workflow

- Atomic commits with clear messages.
- Keep PRs focused and reviewable.
- Update docs in the same commit as related code changes.

---

## Dependency Management

- **Minimal dependencies**: Justify each crate added.
- Prefer well-maintained, audited crates with active communities.
- Pin versions explicitly in `Cargo.toml`.
- Run `cargo audit` regularly for security vulnerabilities.
- Evaluate transitive dependencies—avoid crates that pull in the world.

---

## Performance

- No premature optimization, but be allocation-aware.
- Prefer `&str` over `String` where ownership isn't needed.
- Use `Cow<str>` for flexible ownership patterns.
- Profile before optimizing—measure, don't guess.
- Be mindful of hot paths in database operations.

---

## Concurrency

- Prefer `async`/`await` for I/O-bound work.
- Use channels over shared mutable state.
- Document any `unsafe` code with explicit safety invariants.
- Avoid blocking in async contexts.

---

## AI-First Development

- **Consistent naming**: Predictable patterns help AI navigation.
- **One concept per file**: Where reasonable, keep files focused.
- **Shallow nesting**: Max 3-4 levels of indentation.
- **Explicit over clever**: Prefer readable code over clever tricks.
- **Searchable code**: Use descriptive names that are easy to grep.

---

## Security

- Sanitize all user inputs.
- **No SQL string concatenation**—always use parameterized queries.
- Secrets never in code or logs.
- Validate connection strings and file paths.
- Follow principle of least privilege for database connections.

---

## Logging & Observability

- Use structured logging with `tracing`.
- Log at appropriate levels—don't spam INFO.
- Include context in error messages.
- Sensitive data must never appear in logs.

---

## What to Update

When modifying this project:

1. Update relevant documentation.
2. Add or update tests.
3. Run `cargo fmt` and `cargo clippy`.
4. Ensure all tests pass.
