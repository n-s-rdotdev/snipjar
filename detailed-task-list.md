# Snipjar v1 Detailed Task List

## 1) Product Scope and Constraints
- [x] Confirm v1 product scope: a macOS-first desktop launcher built with `Tauri + React + Vite`.
- [x] Ensure launcher opens via global shortcut `Control + Option + Space`.
- [x] Ensure launcher searches local key-value pairs by `key` and `tags`.
- [x] Ensure pressing `Enter` on a selected result pastes the selected value into the previously active app.
- [x] Keep the app as a single floating launcher window.
- [x] Use inline modal forms for add/edit.
- [x] Implement local SQLite storage with full CRUD.
- [x] Keep navigation keyboard-first.
- [x] Optimize for plain local snippets only; do not position/store as a secrets manager.
- [x] Keep UX lightweight and Raycast-style; avoid settings-heavy v1 behavior.

## 2) Tech Stack and Ownership Boundaries
- [x] Use `Tauri 2` as shell.
- [x] Use `React + Vite` for frontend.
- [x] Use Rust backend for core native/system responsibilities.
- [x] Keep persistence in Rust.
- [x] Keep search and ranking logic in Rust.
- [x] Keep global shortcut registration/handling in Rust.
- [x] Keep clipboard and paste automation in Rust.

## 3) macOS App Lifecycle and Window Behavior
- [x] Register global shortcut `Control + Option + Space`.
- [x] Implement one centered launcher window.
- [x] Make launcher window borderless.
- [x] Keep launcher window always-on-top.
- [x] Toggle show/hide behavior from shortcut.
- [x] Map `Esc` to hide launcher.
- [x] Intercept close action so window hide occurs instead of app quit.
- [x] Keep app process alive after manual launch so global shortcut continues working.
- [x] Ensure `Cmd+Q` fully quits the app.

## 4) Data Storage and Schema
- [x] Store SQLite database in app local application data directory.
- [x] Create `entries` table with columns: `id`, `key`, `value`, `created_at`, `updated_at`.
- [x] Create `entry_tags` table with columns: `entry_id`, `tag`.
- [x] Enforce `key` as required (non-empty).
- [x] Enforce `key` uniqueness.
- [x] Support free-form tags on input.
- [x] Normalize tags to lowercase for search.
- [x] Deduplicate tags per entry.
- [x] Keep timestamps for create/update lifecycle (`created_at`, `updated_at`).

## 5) Search and Ranking
- [x] Search only `key` and `tags`.
- [x] Explicitly exclude `value` from search matching.
- [x] Implement ranking priority #1: key prefix match.
- [x] Implement ranking priority #2: key substring match.
- [x] Implement ranking priority #3: tag match.
- [x] Use most recent update time as tie-breaker after match class ranking.
- [x] Return recent entries when query is empty.

## 6) Frontend Launcher UX and Keyboard Flows
- [x] Build main launcher view with search input.
- [x] Build result list in main view.
- [x] Support arrow keys to move active selection.
- [x] On `Enter`, copy selected value to clipboard.
- [x] On `Enter`, attempt paste into previously active app after copying.
- [x] On `Enter`, hide launcher after paste attempt flow.
- [x] Provide inline modal form for add/edit.
- [x] Include add/edit fields: `key`, `value`, `tags`.
- [x] Add keyboard action `Cmd+N` to open add flow.
- [x] Add keyboard action `Cmd+E` to edit selected entry.
- [x] Add keyboard action `Cmd+Backspace` to delete selected entry.
- [x] Require confirmation before delete executes.

## 7) Rust Command Surface (Frontend API)
- [x] Expose `search_entries(query) -> EntrySummary[]`.
- [x] Expose `create_entry(input) -> Entry`.
- [x] Expose `update_entry(id, input) -> Entry`.
- [x] Expose `delete_entry(id) -> ()`.
- [x] Expose `get_recent_entries() -> EntrySummary[]`.
- [x] Expose `paste_entry(id) -> PasteResult`.

## 8) Paste Handling (macOS)
- [x] In paste flow, always copy selected value to clipboard first.
- [x] Attempt paste with simulated `Cmd+V` using a Rust-side input automation library.
- [x] Detect missing Accessibility permission or paste failure.
- [x] On permission/failure path, keep value on clipboard.
- [x] On permission/failure path, return/show fallback notice indicating copy succeeded but auto-paste unavailable.

## 9) Public Interfaces and Shared Types
- [x] Implement/align frontend `Entry` type exactly: `{ id: string; key: string; value: string; tags: string[]; createdAt: string; updatedAt: string }`.
- [x] Implement/align frontend `EntrySummary` type exactly: `{ id: string; key: string; tags: string[]; updatedAt: string }`.
- [x] Implement/align frontend `EntryInput` type exactly: `{ key: string; value: string; tags: string[] }`.
- [x] Implement/align frontend `PasteResult` type exactly: `{ mode: "pasted" | "copied_only"; message: string }`.

## 10) Validation Rules
- [x] Validate `key` is non-empty.
- [x] Validate `key` is unique.
- [x] Validate `value` is non-empty.
- [x] Drop empty tags during normalization.
- [x] Collapse duplicate tags to one normalized tag.

## 11) Test Plan Execution Checklist

Automated verification completed on 2026-03-28:
- [x] `cd /Users/nrakshit/Developer/xcode/snipjar/src-tauri && cargo test`
- [x] `cd /Users/nrakshit/Developer/xcode/snipjar && npm run build`

### 11.1 Launcher Behavior Tests
- [ ] Verify global shortcut reliably opens launcher.
- [ ] Verify global shortcut reliably hides launcher.
- [ ] Verify `Esc` hides without quitting app.
- [ ] Verify closing window keeps app running.

### 11.2 CRUD Tests
- [x] Verify create succeeds with valid input.
- [x] Verify edit updates entry correctly.
- [x] Verify delete works with confirmation.
- [x] Verify duplicate-key create/update is rejected.
- [x] Verify tag normalization behavior.
- [x] Verify tag deduplication behavior.

### 11.3 Search Tests
- [x] Verify key prefix matches rank above key substring matches.
- [x] Verify tag matches appear when key does not match.
- [x] Verify `value` text never affects search results.
- [x] Verify empty query returns recent entries.

### 11.4 Paste Flow Tests
- [ ] Verify `Enter` copies value, attempts paste, then hides launcher.
- [ ] Verify missing Accessibility permission returns `copied_only`.
- [ ] Verify fallback notice appears when auto-paste unavailable.

### 11.5 Persistence and Timestamp Tests
- [ ] Verify entries persist across app restart.
- [ ] Verify timestamps update correctly on edit.

## 12) Assumptions, Defaults, and Explicit Non-Goals
- [x] Keep v1 behavior and polish macOS-only, while preserving cross-platform-capable structure in Tauri.
- [x] Exclude encryption in v1.
- [x] Exclude Keychain integration in v1.
- [x] Exclude import/export in v1.
- [x] Exclude sync in v1.
- [x] Exclude folders in v1.
- [x] Exclude settings UI in v1.
- [x] Keep default shortcut fixed to `Control + Option + Space` in v1 (no configurability).
- [ ] Ship as regular macOS app bundle with manual first launch.
- [x] Defer launch-at-login support.
