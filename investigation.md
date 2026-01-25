# Investigation Results

## Major Security Flaws
- Safety classifier treats `EXPLAIN` as safe without inspecting the inner statement. `EXPLAIN ANALYZE DELETE ...` (or `EXPLAIN` on any mutating query) will auto-execute without confirmation because safe queries are executed immediately. refs: `src/safety/parser.rs:121`, `src/safety/parser.rs:122`, `src/app.rs:801`, `src/app.rs:805`
- Data-modifying CTEs are classified as safe because all `Statement::Query` nodes are marked safe. A query like `WITH deleted AS (DELETE FROM users RETURNING *) SELECT * FROM deleted` will bypass confirmation and execute. refs: `src/safety/parser.rs:121`, `src/app.rs:801`, `src/app.rs:805`

## Dead Code / Unconnected Systems
- Query execution pipeline (`QueryExecutor`) is fully implemented but unused by the TUI orchestrator; the same logic exists in `Orchestrator::handle_sql_with_source`. refs: `src/query/executor.rs:15`, `src/query/executor.rs:42`, `src/app.rs:796`
- Connection lifecycle manager duplicates `Orchestrator::handle_connect` and is never referenced. refs: `src/connection/manager.rs:11`, `src/connection/manager.rs:52`, `src/app.rs:992`
- LLM lifecycle manager is built but the orchestrator directly rebuilds its client instead. refs: `src/llm/manager.rs:19`, `src/app.rs:135`

## Spaghetti / Complexity Hotspots
- `src/tui/app.rs` is a 2k-line god object. `handle_event` and `handle_standard_input_key` mix input, focus, palette, selection, scroll, and state transitions in large match blocks. refs: `src/tui/app.rs:1041`, `src/tui/app.rs:1435`
- Actor loop combines queueing, cancellation, progress updates, and shutdown in one dense control path that is hard to reason about or unit test. refs: `src/tui/orchestrator_actor.rs:519`, `src/tui/orchestrator_actor.rs:563`

## Embarrassing for an Alpha
- The CLI exposes `--password`, but the prompt flow is not implemented; the flag is a no-op. refs: `src/cli.rs:62`, `src/cli.rs:161`
- User-facing errors instruct `--allow-plaintext` or confirmation prompts, but there is no CLI flag or interactive flow. Only headless mode auto-consents. refs: `src/persistence/connections.rs:197`, `src/persistence/llm_settings.rs:135`, `src/tui/headless/mod.rs:419`
