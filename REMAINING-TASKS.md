# v0.2b Remaining Tasks

**Status**: 27/36 tasks complete (75%)
**Remaining**: 9 tasks (25%) - All require TUI layer changes

---

## Overview

All backend infrastructure for v0.2b has been implemented and tested. The remaining work involves TUI layer integration to expose existing functionality to users.

**What's Complete**:
- ✅ All data persistence layers
- ✅ All command handlers and routing
- ✅ All security features (redaction, secret handling)
- ✅ All database operations
- ✅ All LLM backend features

**What Remains**:
- ⏸️ TUI interaction patterns for new features
- ⏸️ User-facing prompts and confirmations
- ⏸️ Visual feedback (toasts, badges)
- ⏸️ Testing and verification

---

## Task Breakdown

### 1. History Selection-to-Load (Phase 3)
**Priority**: High
**Complexity**: Medium
**Files**: `src/tui/app.rs`, `src/tui/mod.rs`

**Description**:
Make history entries selectable in the TUI. When a user selects a history entry, load its SQL into the input bar.

**Implementation Notes**:
- Add keyboard navigation for history entries (up/down arrows)
- On selection (Enter), populate input bar with SQL from history entry
- Clear any pending input before loading
- Update UI to show selected history entry visually
- Consider adding a preview mode before loading

**Acceptance Criteria**:
- [ ] User can navigate history entries with keyboard
- [ ] Selected entry is visually highlighted
- [ ] Pressing Enter loads SQL into input bar
- [ ] Input history is preserved when switching between modes
- [ ] Works in both TUI and headless modes

---

### 2. Masked Input for /llm key (Phase 5)
**Priority**: High
**Complexity**: Medium
**Files**: `src/tui/app.rs`, `src/tui/mod.rs`, `src/commands/handlers/llm_settings.rs`

**Description**:
Add secure password-style input for API keys when using `/llm key` command.

**Implementation Notes**:
- Create a masked input mode in TUI (show `*` instead of characters)
- Trigger masked input when `/llm key` is invoked without a value
- Pass masked input value to `handle_llm_key` handler
- Backend already supports receiving the key value
- Consider adding confirmation prompt for plaintext storage

**Backend Ready**:
- `handle_llm_key` in `src/commands/handlers/llm_settings.rs` ready to receive input
- Persistence layer handles keyring vs plaintext storage

**Acceptance Criteria**:
- [ ] `/llm key` without value triggers masked input prompt
- [ ] Input characters display as `*` or `•`
- [ ] Pasted values are also masked
- [ ] API key is securely passed to handler
- [ ] User receives feedback on successful storage
- [ ] Plaintext storage warning if keyring unavailable

---

### 3. Database Recovery Toast (Phase 6)
**Priority**: Medium
**Complexity**: Low
**Files**: `src/tui/app.rs`, `src/tui/mod.rs`

**Description**:
Show a toast notification when the database was recovered from corruption.

**Implementation Notes**:
- Check `state_db.was_recovered()` on startup
- Display prominent toast if true: "Database recovered from corruption. Backup saved to state.db.bak"
- Toast should auto-dismiss after ~10 seconds or on user action
- Use existing `App::show_toast()` infrastructure

**Backend Ready**:
- `StateDb::was_recovered()` method available
- Recovery flag set automatically during database opening
- Backup file created at `state.db.bak`

**Acceptance Criteria**:
- [ ] Toast appears on startup if DB was recovered
- [ ] Toast message is clear and informative
- [ ] Toast mentions backup file location
- [ ] Toast auto-dismisses or can be dismissed by user
- [ ] No toast shown on normal startup

---

### 4. Secret Storage Warning Badge (Phase 6)
**Priority**: Medium
**Complexity**: Low
**Files**: `src/tui/app.rs`, `src/tui/mod.rs`

**Description**:
Display a dismissible header badge warning when secrets are stored in plaintext.

**Implementation Notes**:
- Check `state_db.secret_storage_status()` to get status
- If status is `PlaintextConsent`, show badge in header
- Badge text: "⚠️ Secrets stored in plaintext (keyring unavailable)"
- Make badge dismissible (save preference to avoid showing again)
- Consider using yellow/warning color

**Backend Ready**:
- `StateDb::secret_storage_status()` returns `SecretStorageStatus` enum
- Status persisted across sessions

**Acceptance Criteria**:
- [ ] Badge appears in header when secrets are in plaintext
- [ ] Badge has clear warning icon and message
- [ ] Badge can be dismissed by user
- [ ] Dismissal persists across sessions
- [ ] Badge reappears if new secrets are stored in plaintext
- [ ] No badge shown if keyring is available

---

### 5. Testing & Verification (Phase 7)
**Priority**: High
**Complexity**: High
**Files**: Various, `tests/`

#### 5.1 Connection Switch Testing
**Description**: Verify connection switch cancellation and state reset

**Test Cases**:
- [ ] Start long-running query, switch connection, confirm cancellation
- [ ] Verify chat history clears on switch
- [ ] Verify input history clears on switch
- [ ] Verify pending confirmations are cancelled
- [ ] Verify query log clears on switch
- [ ] Works in both TUI and headless modes

#### 5.2 Connection CRUD Testing
**Description**: Verify connection management improvements

**Test Cases**:
- [ ] Add connection with extras field, verify storage
- [ ] Edit connection with `--test` flag, verify test runs
- [ ] Verify redacted display in `/connections` list
- [ ] Verify header shows redacted connection info
- [ ] Verify host/user/password never appear in UI

#### 5.3 Saved Queries Testing
**Description**: Verify saved query improvements

**Test Cases**:
- [ ] Save query with description, verify storage
- [ ] Update existing query by using same name
- [ ] Delete query without `--confirm`, verify error
- [ ] Delete query with `--confirm`, verify success
- [ ] Create query with `#global:` tag, verify scope
- [ ] Use saved query, verify `saved_query_id` in history
- [ ] Verify default connection tracks history

#### 5.4 LLM Features Testing
**Description**: Verify LLM improvements

**Test Cases**:
- [ ] Query LLM with multiple tags, verify AND filtering
- [ ] Verify connection context in LLM prompt (check logs)
- [ ] Verify no host/user/password in LLM context
- [ ] Test masked input for API key (when implemented)

#### 5.5 Persistence Testing
**Description**: Verify persistence robustness

**Test Cases**:
- [ ] Simulate DB version mismatch, verify error message
- [ ] Trigger DB recovery, verify toast (when implemented)
- [ ] Verify secret storage badge (when implemented)
- [ ] Test concurrent access (multiple operations)

#### 5.6 Regression Testing
**Description**: Verify existing functionality still works

**Test Cases**:
- [ ] Execute basic queries
- [ ] Use `/help` command
- [ ] Switch between vim mode
- [ ] Use schema introspection
- [ ] Test all existing commands

---

## Implementation Sequence (Recommended)

### Stage 1: Critical UX (1-2 days)
1. Masked input for `/llm key` - High security priority
2. History selection-to-load - High user value

### Stage 2: Notifications & Warnings (0.5-1 day)
3. Database recovery toast - Good error feedback
4. Secret storage warning badge - Security awareness

### Stage 3: Quality Assurance (2-3 days)
5. Comprehensive testing and verification
6. Bug fixes and polish
7. Documentation updates

---

## Technical Reference

### Key Files Modified in Backend (Already Complete)
- `src/app.rs` - Orchestrator with saved query tracking
- `src/commands/router.rs` - All command parsing
- `src/commands/handlers/*.rs` - All command handlers
- `src/persistence/*.rs` - All database operations
- `src/llm/service.rs` - LLM integration with redaction
- `src/tui/app.rs` - App state with reset helper
- `src/tui/history.rs` - Input history with clear()

### TUI Files to Modify
- `src/tui/app.rs` - Core app state and rendering
- `src/tui/mod.rs` - Event handling and input processing
- `src/tui/headless/mod.rs` - Headless mode support

### Testing Approach
1. Unit tests for new TUI components
2. Integration tests for user workflows
3. Manual testing with real databases
4. Headless mode testing via scripts
5. Performance testing with large datasets

---

## Success Criteria

The v0.2b release is complete when:
- ✅ All 36 planned tasks are implemented
- ✅ All acceptance criteria are met
- ✅ All tests pass
- ✅ No regressions in existing functionality
- ✅ Documentation is updated
- ✅ User-facing features are polished and intuitive

---

## Notes

**Backend Quality**: All backend code has been implemented with:
- Proper error handling
- Security considerations (redaction, secret storage)
- Test coverage where applicable
- Clear separation of concerns
- Documentation

**TUI Considerations**:
- Maintain consistency with existing TUI patterns
- Ensure keyboard shortcuts don't conflict
- Test with different terminal sizes
- Consider accessibility (color blindness, screen readers)
- Provide clear user feedback for all actions

**Performance**:
- All database queries use proper indexing
- Connection pooling configured
- Caching in place for LLM prompts
- No blocking operations in TUI event loop

---

## Getting Started

1. Review the backend implementation:
   ```bash
   git log --oneline | grep "feat(v0.2b)"
   ```

2. Check current progress:
   ```bash
   cat implementation-status.md
   cat v0.2b-plan.md
   ```

3. Start with masked input (highest priority):
   - Read `src/tui/app.rs` for input handling patterns
   - Look at existing confirmation flows
   - Implement masked input mode

4. Run tests frequently:
   ```bash
   cargo test
   cargo clippy
   ```

5. Test manually with real usage:
   ```bash
   cargo run -- --help
   ```

Good luck! 🚀
