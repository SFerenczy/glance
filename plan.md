## v0.2b Implementation Plan

### Objectives

- Add local SQLite-backed persistence for connections, query history, saved queries, and LLM settings without leaking secrets.
- Support full connection lifecycle (add/edit/delete/list/connect) and session switching with clean state resets.
- Persist query history and saved queries (with tags) and expose read-only access to the LLM.
- Allow configuring LLM provider/model/key inside the TUI with immediate effect and secure storage.
- Enforce privacy/redaction, retention, and recovery guarantees outlined in the spec.

### 1) Persistence Layer (`state.db`)

- Add `sqlite` feature to `sqlx` in `Cargo.toml`; add `keyring` crate for OS secure storage.
- Create `src/persistence/` module responsible for opening `~/.config/db-glance/state.db` (or `%APPDATA%\\db-glance\\state.db`), ensuring parent dirs, enabling WAL, and handling lock retries/backoff.
- Implement schema versioning and migrations; include bootstrap migration for tables: `schema_versions`, `connections`, `query_history`, `saved_queries`, `saved_query_tags`, `llm_settings`, plus supporting indices.
- Secrets storage: integrate OS keyring/secure storage for passwords/API keys; fall back to plaintext only with explicit confirmation and warnings; keep ciphertext/handles outside AI context.
- Implement corruption recovery: detect invalid DB, back up to `state.db.bak`, recreate schema, and surface a toast/log message with the action taken.

### 2) Connection Profiles & Commands (US-1, FR-2)

- Define storage model for connections: name, database, host (redaction-friendly), port, user, sslmode (text), extras (JSON blob), password_ref (keyring handle or encrypted blob), created_at, updated_at, last_used_at.
- Extend `ConnectionConfig` in `src/config.rs` with `sslmode` and `extras` fields for parity.
- Implement `/connections` (list all saved connections with redacted host/user), `/conn add <name>`, `/conn edit <name>`, `/conn delete <name>` flows with validation, confirmation on delete, and friendly error messages; never render passwords.
- Add optional “test connection” step during add/edit to validate credentials before saving.
- Ensure host/user are redacted in any AI-facing context; listing shows redacted host/user but not password; persist timestamps on use/update.

### 3) Connection Switching (US-2, FR-3)

- Implement `/connect <name>` that loads profile, fetches password from keyring/plaintext store, establishes DB connection, re-introspects schema, and updates header display label (no secrets).
- On successful switch, cancel in-flight DB/LLM tasks, clear chat history, LLM conversation, query log, input history, and sticky scroll; show toast “Connected to <name>”.
- On failure, keep prior connection active with a clear error; avoid no-connection limbo unless none existed.

### 4) Query History Persistence (US-3, FR-4)

- Record every executed query (manual/LLM) with fields: id, connection_name, submitted_by (`user`|`llm`), sql, status (`success`|`error`|`cancelled`), execution_time_ms, row_count (nullable), error_message (nullable), created_at, saved_query_id (nullable FK to saved_queries).
- Implement retention policy: prune to max 5,000 rows or 90 days (whichever smaller) on insert; add `/history clear` with confirmation.
- Add `/history [--conn <name>] [--text <filter>] [--since <duration>] [--limit N]` command that renders a scrollable list; Enter loads SQL into input (no auto-run).
- Wire query logging from orchestrator/database layer into persistence, including cancellation/error statuses.

### 5) Saved Queries & Tags (US-4, FR-5)

- Storage: saved query record with name (unique per connection/global), sql, description, tags (via join table), connection_name nullable for globals, created_at/updated_at/last_used_at/usage_count.
- Commands: `/savequery <name> [#tags...]` (use current input or last executed SQL), `/queries [--conn <name>|--all] [--tag <tag>] [--text <filter>]`, `/usequery <name>` (loads into input with `/sql ` prefix), `/query delete <name>` with confirmation.
- Tag handling: support multiple tags; prefix `#global:` for global tags; enforce per-connection scoping otherwise.
- Update usage metrics on use/edit; show toasts on save/update/delete.

### 6) LLM Tooling (Read-Only) (US-5, FR-6)

- Expose a tool for the LLM to list/search saved queries with filters (connection_name, tags[], text, limit); response includes name, sql, description, tags, connection label/database, last_used_at, usage_count.
- Strip/omit host/user/passwords and any result data from tool output; ensure connection identifiers are safe display labels only.
- Ensure tool is read-only—no create/update/delete paths exposed to the LLM client.

### 7) Dynamic LLM Provider & Key Setup (US-6, FR-7)

- Extend command handling with `/llm provider <openai|anthropic|ollama>`, `/llm key`, `/llm model <name>`; persist selections to `state.db`.
- Implement masked key prompt; store keys via secure storage when available; only show last 4 chars after save; warn and require opt-in for plaintext storage.
- Apply changes immediately: refresh LLM client on provider/model change and clear conversation history on provider change.

### 8) Privacy, Redaction, and Display (FR-8)

- Audit prompts sent to the LLM to ensure only database name and user-provided connection label are included; exclude hostnames, usernames, passwords, and result rows.
- Redact host/user in on-screen displays where needed (e.g., `mydb @ ******:5432`); ensure history exports to LLM (if any) exclude secrets and result data.
- Add persistent warning badge in header if secure storage unavailable or plaintext consent given.

### 9) Error Handling & Recovery (FR-9)

- Standardize command error responses to clear single-line messages suitable for chat panel.
- Handle lock contention on SQLite with retry/backoff and clear messaging on failure.
- Backup/recreate state DB path on corruption with user-facing notification of path/action.

### 10) Testing & Non-Functional

- Add unit tests for persistence layer (migrations, CRUD, retention pruning, tag scoping), connection switching state resets, and command parsing for new commands.
- Add integration tests (or smoke tests) covering add/edit/connect, query history persistence, saved query flows, LLM tool redaction, and LLM provider switching.
- Verify performance targets: persistence operations <50ms typical; switching (including schema introspection) within ~3s with spinner/loading UI.
- Ensure docs/README updated to reflect new commands and persistence behavior; run `cargo fmt` and `cargo clippy`.

---

## Dependencies to Add

```toml
# In Cargo.toml [dependencies]
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "sqlite", "tls-rustls"] }
keyring = "2"  # OS keyring for secure secret storage
```

## New Module Structure

```
src/
  persistence/
    mod.rs          # Public API, StateDb struct
    migrations.rs   # Schema versioning and migration runner
    connections.rs  # Connection profile CRUD
    history.rs      # Query history CRUD + retention
    saved_queries.rs # Saved queries + tags CRUD
    llm_settings.rs # LLM provider/model/key persistence
```
