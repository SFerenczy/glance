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

## Behavior-Driven Development (BDD)

We follow a BDD approach for feature development. This means:

### Workflow

1. **Specify behavior first**: Write Gherkin scenarios before implementation
2. **Scenarios as acceptance criteria**: Each feature has executable specifications
3. **Implement to pass scenarios**: Code is written to make scenarios pass
4. **Scenarios are living documentation**: Keep them updated as behavior evolves

### Gherkin Format

Use standard Gherkin syntax for feature specifications:

```gherkin
Feature: Feature name
  As a <role>
  I want <capability>
  So that <benefit>

  Scenario: Specific behavior
    Given <precondition>
    When <action>
    Then <expected outcome>
    And <additional outcome>
```

### Best Practices

- **One behavior per scenario**: Keep scenarios focused and atomic
- **Declarative over imperative**: Describe _what_, not _how_
- **Use domain language**: Scenarios should be readable by non-developers
- **Avoid implementation details**: Scenarios describe behavior, not code
- **Group related scenarios**: Use Feature blocks to organize related behaviors

### Where to Write Scenarios

- **Implementation plans**: Include scenarios in `plan.md` for upcoming features
- **Spec documents**: Include scenarios in `docs/specs/` for feature specifications
- **Test files**: Translate scenarios into Rust tests in `/tests` directory

### Example

```gherkin
Feature: Query execution
  As a database user
  I want to run SQL queries through natural language
  So that I can explore data without writing SQL

  Scenario: Simple table query
    Given a connected database with a "users" table
    When I type "show me all users"
    And I press Enter
    Then I should see a table with user data
    And the query log should contain the executed SQL
```

---

## Git & Workflow

- Atomic commits with clear messages.
- Keep PRs focused and reviewable.
- Update docs in the same commit as related code changes.
- Before creating a commit run `just precommit` to run all checks.

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
