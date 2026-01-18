# v0.2a Implementation Plan

> Terminal Polish — Implementation Roadmap

This document breaks down the v0.2a specification into discrete, testable implementation phases.

---

## Phase 1: Core Infrastructure

### 1.1 Vim Mode Toggle

**Goal**: Make vim-style navigation opt-in via `/vim` command (disabled by default).

**Tasks**:

- [x] Add `vim_mode_enabled: bool` to App state (default: `false`)
- [x] Add `/vim` command to toggle vim mode on/off
- [x] When disabled: standard input behavior, no mode switching, Esc clears input
- [x] When enabled: Normal/Insert mode switching with `Esc`/`i`
- [x] Mode indicator only shown when vim mode is enabled
- [x] Update help text to document `/vim` command

**Files**: `src/tui/app.rs`, `src/app.rs`

**Tests**:

- `/vim` toggles vim_mode_enabled
- Mode indicator hidden when vim mode disabled
- Esc behavior differs based on vim mode state

### 1.2 Mode System (Vim Mode Only)

**Goal**: Normal/Insert mode switching when vim mode is enabled.

**Tasks**:

- [x] `InputMode` enum (`Normal`, `Insert`) in TUI state
- [x] Display mode indicator in status area (`-- INSERT --` / `-- NORMAL --`)
- [x] `Esc` transitions from Insert → Normal (only when vim mode enabled)
- [x] `i` or any typing transitions from Normal → Insert
- [x] Input only accepted in Insert mode (when vim mode enabled)

**Files**: `src/tui/app.rs`, `src/tui/events.rs`

**Tests**:

- Mode transitions on correct keypresses
- Mode indicator renders correctly
- Input only accepted in Insert mode

---

### 1.2 Input History

**Goal**: Session-based input history with arrow key navigation.

**Tasks**:

- [ ] Create `InputHistory` struct with circular buffer (max 100 entries)
- [ ] Store submitted inputs (skip empty, skip consecutive duplicates)
- [ ] Up/Down arrow navigation through history
- [ ] Preserve unsaved input as temporary entry when navigating
- [ ] Restore unsaved input when returning to newest position

**Files**: `src/tui/app.rs` (new `history.rs` module recommended)

**Tests**:

- History stores entries correctly
- Navigation cycles through entries
- Unsaved input preserved during navigation
- Empty/duplicate entries rejected

---/

## Phase 2: Command Autocomplete

### 2.1 Command Palette UI

**Goal**: Floating overlay showing available slash commands.

**Tasks**:

- [ ] Create `CommandPalette` widget
- [ ] Trigger on `/` in empty input or at cursor start
- [ ] Render floating overlay above input bar
- [ ] List all commands with descriptions

**Files**: `src/tui/widgets/command_palette.rs` (new)

**Tests**:

- Palette appears on `/` trigger
- Correct positioning above input

---

### 2.2 Fuzzy Matching

**Goal**: Filter commands as user types with ranked results.

**Tasks**:

- [ ] Implement fuzzy matching algorithm (prefix > substring > fuzzy)
- [ ] Match against command name and description
- [ ] Update results on each keystroke
- [ ] Highlight matched characters in suggestions

**Files**: `src/tui/widgets/command_palette.rs`

**Tests**:

- Prefix matches rank highest
- Partial matches work correctly
- Empty filter shows all commands

---

### 2.3 Palette Navigation

**Goal**: Keyboard navigation within command palette.

**Tasks**:

- [ ] Up/Down arrow navigates suggestions
- [ ] Tab accepts selection, keeps palette open
- [ ] Enter accepts selection, closes palette
- [ ] Esc closes palette, preserves input
- [ ] Ctrl+C closes palette, clears input

**Files**: `src/tui/events.rs`, `src/tui/widgets/command_palette.rs`

**Tests**:

- Each key behaves as specified
- Selection state maintained correctly

---

## Phase 3: SQL Autocomplete

### 3.1 SQL Parser for Context Detection

**Goal**: Determine SQL context for appropriate completions.

**Tasks**:

- [ ] Create lightweight SQL tokenizer
- [ ] Detect context: after SELECT, FROM, JOIN, WHERE, ORDER BY, etc.
- [ ] Parse table aliases (`users u` → `u` maps to `users`)
- [ ] Track cursor position within query

**Files**: `src/tui/sql_autocomplete.rs` (new)

**Tests**:

- Correct context detection for each SQL clause
- Alias parsing works for various formats
- Handles incomplete/malformed SQL gracefully

---

### 3.2 Schema-Aware Completions

**Goal**: Suggest tables, columns, and keywords based on context.

**Tasks**:

- [ ] Fetch and cache schema metadata (tables, columns, foreign keys)
- [ ] After FROM/JOIN: suggest table names
- [ ] After SELECT/WHERE/ORDER BY: suggest column names
- [ ] After alias dot (`u.`): suggest columns from aliased table
- [ ] Include SQL keywords and functions where appropriate

**Files**: `src/tui/sql_autocomplete.rs`, `src/db/schema.rs`

**Tests**:

- Correct suggestions for each context
- Alias resolution works
- Foreign key relationships inform JOIN suggestions

---

### 3.3 Completion UI

**Goal**: Render SQL completion suggestions.

**Tasks**:

- [ ] Create `SqlCompletionPopup` widget
- [ ] Show type indicator (table, column, keyword, function)
- [ ] Maximum 8 visible suggestions with scroll
- [ ] Ranking: prefix > case-insensitive prefix > substring > fuzzy > recency

**Files**: `src/tui/widgets/sql_completion.rs` (new)

**Tests**:

- Correct rendering and scrolling
- Type indicators display correctly
- Ranking order verified

---

## Phase 4: Clipboard Support

### 4.1 Platform Clipboard Integration

**Goal**: Cross-platform clipboard access.

**Tasks**:

- [ ] Add `arboard` or `copypasta` crate for clipboard
- [ ] Abstract clipboard operations behind trait
- [ ] Linux: xclip/xsel with OSC 52 fallback
- [ ] macOS: pbcopy
- [ ] Windows: native API
- [ ] Graceful error handling if clipboard unavailable

**Files**: `src/clipboard.rs` (new), `Cargo.toml`

**Tests**:

- Clipboard write succeeds on supported platforms
- Fallback behavior works
- Errors handled gracefully

---

### 4.2 Copy Actions

**Goal**: Implement copy keybindings.

**Tasks**:

- [ ] `y` (Normal mode) copies last executed SQL
- [ ] Show "Copied to clipboard" toast (2s duration)
- [ ] Track last executed SQL in app state

**Files**: `src/tui/events.rs`, `src/tui/app.rs`

**Tests**:

- `y` copies correct SQL
- Toast appears and auto-dismisses

---

### 4.3 Text Selection (Stretch)

**Goal**: Mouse-based text selection and copy.

**Tasks**:

- [ ] Track mouse drag for text selection
- [ ] Highlight selected text with inverted colors
- [ ] Ctrl+C copies selection (Normal mode)
- [ ] Click clears selection

**Files**: `src/tui/events.rs`, `src/tui/app.rs`

**Tests**:

- Selection renders correctly
- Copy captures selected text
- Selection clears on click

---

## Phase 5: Navigation Enhancements

### 5.1 Vim-Style Scrolling

**Goal**: Vim keybindings for chat navigation.

**Tasks**:

- [ ] `j`/`k` scroll one line (Normal mode)
- [ ] `g`/`G` jump to top/bottom
- [ ] `Ctrl+d`/`Ctrl+u` scroll half page
- [ ] Keys only active in Normal mode

**Files**: `src/tui/events.rs`

**Tests**:

- Each key scrolls correctly
- Keys ignored in Insert mode

---

### 5.2 Sticky Scroll

**Goal**: Smart auto-scroll behavior.

**Tasks**:

- [ ] Auto-scroll to bottom on new content
- [ ] Disable auto-scroll when user scrolls up
- [ ] Show "↓ New messages" indicator when not at bottom
- [ ] Clicking indicator or `G` re-enables auto-scroll

**Files**: `src/tui/app.rs`, `src/tui/widgets/`

**Tests**:

- Auto-scroll works by default
- Scrolling up disables auto-scroll
- Indicator appears/disappears correctly

---

### 5.3 Query Log Navigation

**Goal**: Click query log entries to jump to results.

**Tasks**:

- [ ] Track query → chat position mapping
- [ ] Click handler for query log entries
- [ ] Scroll chat to selected query's results
- [ ] Highlight selected query in log

**Files**: `src/tui/app.rs`, `src/tui/widgets/`

**Tests**:

- Click jumps to correct position
- Highlight renders correctly

---

## Phase 6: Visual Feedback

### 6.1 LLM Thinking Indicator

**Goal**: Animated indicator while awaiting LLM response.

**Tasks**:

- [ ] Create animated dots component (`. → .. → ... → .`)
- [ ] Show in chat where response will appear
- [ ] Replace with response when streaming begins

**Files**: `src/tui/widgets/`

**Tests**:

- Animation cycles correctly
- Replaced by actual content

---

### 6.2 Query Execution Spinner

**Goal**: Spinner during database query execution.

**Tasks**:

- [ ] Implement braille spinner (`⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏`)
- [ ] Show inline after SQL in chat
- [ ] Replace with results or error on completion

**Files**: `src/tui/widgets/`

**Tests**:

- Spinner animates correctly
- Replaced on completion

---

### 6.3 Result Highlight

**Goal**: Flash new results to draw attention.

**Tasks**:

- [ ] Brief highlight (200ms) with accent background on new results
- [ ] Fade to normal styling

**Files**: `src/tui/widgets/`

**Tests**:

- Highlight appears on new results
- Fades correctly

---

### 6.4 Relative Timestamps

**Goal**: Human-readable timestamps in query log.

**Tasks**:

- [ ] Format: "just now", "2m ago", "1h ago"
- [ ] Update every minute
- [ ] Store absolute timestamp for hover (if supported)

**Files**: `src/tui/widgets/`

**Tests**:

- Correct relative formatting
- Updates over time

---

### 6.5 Connection Status Indicator

**Goal**: Visual connection state in header.

**Tasks**:

- [ ] Green `●` = connected, gray `○` = disconnected
- [ ] Status text: "Connected to mydb@localhost"
- [ ] Update on connection state change

**Files**: `src/tui/widgets/`, `src/tui/app.rs`

**Tests**:

- Indicator reflects actual state
- Updates on reconnect/disconnect

---

## Phase 7: Quick Actions

### 7.1 Re-run & Edit Last Query

**Goal**: Quick access to last query.

**Tasks**:

- [ ] `r` (Normal mode) re-executes last SQL query
- [ ] Apply same safety rules (confirm mutations)
- [ ] `e` (Normal mode) loads last SQL into input with `/sql ` prefix
- [ ] Switch to Insert mode, cursor at end

**Files**: `src/tui/events.rs`, `src/tui/app.rs`

**Tests**:

- `r` executes correct query
- `e` populates input correctly
- Mode switches to Insert

---

### 7.2 Help Overlay

**Goal**: Quick reference for keyboard shortcuts.

**Tasks**:

- [ ] `?` (Normal mode) shows help overlay
- [ ] Overlay covers center of screen
- [ ] List shortcuts grouped by category
- [ ] Any key dismisses overlay

**Files**: `src/tui/widgets/help_overlay.rs` (new)

**Tests**:

- Overlay appears on `?`
- Any key dismisses
- Content matches spec

---

## Phase 8: Input Enhancements

### 8.1 Clear & Cancel

**Goal**: Input clearing and operation cancellation.

**Tasks**:

- [ ] `Ctrl+U` clears entire input line
- [ ] Double-tap `Esc` (within 500ms) cancels pending operation
- [ ] Track pending operation state (LLM response, query execution)

**Files**: `src/tui/events.rs`, `src/tui/app.rs`

**Tests**:

- `Ctrl+U` clears input
- Double-Esc cancels pending operation
- Single Esc only switches mode

---

### 8.2 Multi-line Paste Handling

**Goal**: Smart handling of pasted content with newlines.

**Tasks**:

- [ ] Detect paste with newlines
- [ ] Single statement: convert newlines to spaces
- [ ] Multiple statements: prompt user
- [ ] Preserve paste for review before submit

**Files**: `src/tui/events.rs`

**Tests**:

- Single statement normalized correctly
- Multi-statement prompts user
- Paste preserved in input

---

## Phase 9: Query Log Enhancements

### 9.1 Visual Separators

**Goal**: Group related queries visually.

**Tasks**:

- [ ] Add separator between query groups
- [ ] Group = queries from same natural language question
- [ ] Thin horizontal line or timestamp header

**Files**: `src/tui/widgets/`

**Tests**:

- Separators render between groups
- Grouping logic correct

---

### 9.2 Manual Query Indicator

**Goal**: Distinguish manual vs generated queries.

**Tasks**:

- [ ] Raw SQL queries (via `/sql`) appear in query log
- [ ] Indicator: "manual" vs "generated"

**Files**: `src/tui/widgets/`, `src/tui/app.rs`

**Tests**:

- Manual queries logged
- Indicator displays correctly

---

### 9.3 Dynamic Panel Width

**Goal**: Expand query log on focus.

**Tasks**:

- [ ] Query log focused: expand to 50% width
- [ ] Chat focused: query log at 30% width
- [ ] Smooth transition (100ms ease-out)

**Files**: `src/tui/app.rs`

**Tests**:

- Width changes on focus
- Transition animates smoothly

---

## Phase 10: Quality of Life

### 10.1 Graceful Resize

**Goal**: Handle terminal resize without artifacts.

**Tasks**:

- [ ] UI reflows on resize
- [ ] No artifacts or corruption
- [ ] Maintain scroll position relative to content
- [ ] Debounce resize events (50ms)

**Files**: `src/tui/app.rs`, `src/tui/events.rs`

**Tests**:

- Resize handled cleanly
- Scroll position maintained

---

### 10.2 Empty State & Row Numbers

**Goal**: Polish result display.

**Tasks**:

- [ ] Empty result: "No results found. Query executed successfully in Xms."
- [ ] Optional row numbers in result tables
- [ ] `/rownumbers` command to toggle
- [ ] Persist setting in config

**Files**: `src/tui/widgets/`, `src/config.rs`

**Tests**:

- Empty state renders correctly
- Row numbers toggle works
- Setting persists

---

### 10.3 Long Query Notification

**Goal**: Alert user when long query completes.

**Tasks**:

- [ ] Terminal bell if query takes >5 seconds
- [ ] Configurable threshold
- [ ] Can be disabled via config

**Files**: `src/tui/app.rs`, `src/config.rs`

**Tests**:

- Bell emits after threshold
- Respects config setting

---

## Phase 11: Configuration

### 11.1 New Config Options

**Goal**: Add v0.2a configuration options.

**Tasks**:

- [ ] `vim_mode`: Enable vim-style navigation (default: true)
- [ ] `row_numbers`: Show row numbers in tables (default: false)
- [ ] `bell_on_completion`: Terminal bell on long queries (default: true)
- [ ] `bell_threshold_seconds`: Threshold for bell (default: 5)
- [ ] `chat_panel_width`: Default chat width ratio (default: 0.7)
- [ ] `query_log_width_focused`: Query log width when focused (default: 0.5)

**Files**: `src/config.rs`

**Tests**:

- Config parses correctly
- Defaults applied when missing
- Values respected by UI

---

## Dependencies

### New Crates

| Crate                    | Purpose                  | Version       |
| ------------------------ | ------------------------ | ------------- |
| `arboard` or `copypasta` | Cross-platform clipboard | Latest stable |

### Internal Dependencies

- Phase 1 (Mode System) must complete before Phases 5, 7
- Phase 2 (Command Autocomplete) can proceed independently
- Phase 3 (SQL Autocomplete) depends on schema access from `src/db/schema.rs`
- Phase 4 (Clipboard) can proceed independently
- Phases 5-10 can largely proceed in parallel after Phase 1

---

## Testing Strategy

### Unit Tests

- Each new module has corresponding test module
- Fuzzy matching, history, SQL parsing thoroughly tested

### Integration Tests

- Full TUI interaction tests using simulated input
- Verify mode transitions, autocomplete flows, clipboard operations

### Manual Testing Checklist

- [ ] All keyboard shortcuts work as documented
- [ ] Autocomplete appears and navigates correctly
- [ ] Clipboard works on target platforms
- [ ] Animations render smoothly
- [ ] Resize handling is clean
- [ ] Config options respected

---

## Estimated Effort

| Phase                      | Effort | Priority |
| -------------------------- | ------ | -------- |
| 1. Core Infrastructure     | Medium | P0       |
| 2. Command Autocomplete    | Medium | P0       |
| 3. SQL Autocomplete        | High   | P1       |
| 4. Clipboard Support       | Low    | P1       |
| 5. Navigation Enhancements | Medium | P0       |
| 6. Visual Feedback         | Medium | P1       |
| 7. Quick Actions           | Low    | P1       |
| 8. Input Enhancements      | Low    | P2       |
| 9. Query Log Enhancements  | Low    | P2       |
| 10. Quality of Life        | Low    | P2       |
| 11. Configuration          | Low    | P1       |

**P0** = Must have for release  
**P1** = Should have  
**P2** = Nice to have

---

## Open Questions (from spec)

1. **Vim mode default**: Opt-in via config with first-run prompt
2. **Autocomplete trigger**: Automatic with debounce, Ctrl+Space forces immediate
3. **History persistence**: Deferred to v0.3
