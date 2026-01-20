# US-1: Responsive UI During Long Operations

> FR-1: Background Task Architecture

**Spec**: [docs/specs/v0.2c.md](docs/specs/v0.2c.md)  
**User Story**: As a user, I want the UI to remain responsive while queries execute so I can cancel operations or continue reading results.

---

## Problem

Currently, `handle_crossterm_event()` in `src/tui/mod.rs` directly awaits LLM and DB operations:

```rust
match orchestrator.handle_input(&input).await {  // Blocks UI
```

This blocks all UI updates (spinner animation, input, redraws) during operations.

---

## Acceptance Criteria

- UI redraws continue during LLM/DB operations
- Ctrl+C or Esc cancels in-flight operations
- Spinner animation updates smoothly during execution
- Input remains responsive (can type while waiting)

---

## Scenarios

```gherkin
Feature: Responsive UI during long operations
  As a user
  I want the UI to remain responsive during queries
  So that I can cancel operations or continue reading

  Scenario: UI redraws during LLM request
    Given a connected database
    When I send a natural language query
    Then the spinner should animate smoothly
    And the UI should continue to redraw

  Scenario: Cancel operation with Escape
    Given an in-flight LLM or DB operation
    When I press Escape
    Then the operation should be cancelled
    And a cancellation message should appear in chat

  Scenario: Cancel operation with Ctrl+C during processing
    Given an in-flight operation (is_processing = true)
    When I press Ctrl+C
    Then the operation should be cancelled
    And the application should NOT exit
```

---

## Implementation Tasks

- [x] Add `tokio_util` dependency for `CancellationToken`
- [x] Add `active_task` field to `Tui` to track spawned operation
- [x] Refactor input submission to spawn background task instead of awaiting
- [x] Refactor query confirmation to spawn background task
- [x] Send `ProgressMessage::LlmStarted` / `DbStarted` when spawning
- [x] Handle Esc during `is_processing` to cancel active task
- [x] Handle Ctrl+C during `is_processing` to cancel (not exit)
- [x] Send `ProgressMessage::Cancelled` on abort
- [x] Ensure `is_processing` cleared on task completion/cancellation
- [ ] Add headless test for cancellation behavior

---

## Key Changes

### 1. Add dependency (`Cargo.toml`)

```toml
tokio-util = { version = "0.7", features = ["rt"] }
```

### 2. Track active task (`src/tui/mod.rs`)

```rust
pub struct Tui {
    // ... existing fields
    active_task: Option<tokio::task::JoinHandle<()>>,
    cancellation_token: Option<CancellationToken>,
}
```

### 3. Spawn instead of await

Before:

```rust
match orchestrator.handle_input(&input).await {
```

After:

```rust
let token = CancellationToken::new();
self.cancellation_token = Some(token.clone());
let tx = tx.clone();
self.active_task = Some(tokio::spawn(async move {
    tokio::select! {
        result = orchestrator.handle_input(&input) => {
            let _ = tx.send(AsyncMessage::InputResult(result)).await;
        }
        _ = token.cancelled() => {
            let _ = tx.send(AsyncMessage::Progress(ProgressMessage::Cancelled)).await;
        }
    }
}));
```

### 4. Cancel on Esc/Ctrl+C during processing

```rust
if app_state.is_processing {
    if let Some(token) = &self.cancellation_token {
        token.cancel();
    }
    return;
}
```

---

## Files to Modify

| File             | Changes                                                  |
| ---------------- | -------------------------------------------------------- |
| `Cargo.toml`     | Add `tokio-util` dependency                              |
| `src/tui/mod.rs` | Add task tracking, spawn operations, handle cancellation |

---

## Testing

### Headless Test

```bash
# Test that Esc during processing sends cancellation
glance --headless --mock-db --events "type:slow query,key:enter,wait:50,key:esc,wait:100" \
  --output json | jq '.messages[] | select(.type == "system" and .content | contains("cancelled"))'
```

---

## Notes

- The `ProgressMessage` enum and `handle_async_message` already exist
- The `mpsc` channel infrastructure is in place
- Main change is spawning tasks instead of awaiting inline
- `Orchestrator` needs to be `Send + 'static` for spawning (may require `Arc<Mutex<>>` or restructuring)
