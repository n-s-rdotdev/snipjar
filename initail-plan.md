# Snipjar v1 Plan: macOS Key-Value Launcher with Tauri + React

## Summary
Build a macOS-first desktop launcher in `Tauri + React + Vite` that opens with `Control + Option + Space`, lets users search local key-value pairs by `key` and `tags`, and pastes the selected value into the active app on `Enter`.

The app will be a single floating launcher window with inline modal forms for add/edit, local SQLite storage, full CRUD, and keyboard-first navigation. It is optimized for plain local snippets, not secrets, and will behave like a lightweight Raycast-style utility rather than a full settings-heavy manager.

## Implementation Changes
- Stack:
  - `Tauri 2` shell, `React + Vite` frontend, Rust backend.
  - Rust owns persistence, search, global shortcut handling, clipboard, and paste automation.
- macOS app behavior:
  - Register global shortcut `Control + Option + Space`.
  - Show/hide one centered, borderless, always-on-top launcher window.
  - `Esc` hides the launcher; closing the window hides it instead of quitting.
  - App remains running after manual launch so the shortcut continues working; `Cmd+Q` quits.
- Data model:
  - SQLite database stored in the app’s local application data directory.
  - `entries` table: `id`, `key`, `value`, `created_at`, `updated_at`.
  - `entry_tags` table: `entry_id`, `tag`.
  - `key` is unique and required.
  - Tags are free-form text, normalized to lowercase for search and deduped per entry.
- Search and ranking:
  - Search only `key` and `tags`; do not search inside `value`.
  - Ranking order: key prefix match, key substring match, tag match, then recent update as tie-breaker.
  - Empty query shows recent entries.
- Launcher UX:
  - Main view shows search input and result list.
  - Arrow keys move selection.
  - `Enter` copies the selected value to clipboard, then attempts to paste it into the previously active app, then hides the launcher.
  - Inline modal form for add/edit with fields: `key`, `value`, `tags`.
  - Keyboard actions: `Cmd+N` add, `Cmd+E` edit selected, `Cmd+Backspace` delete selected with confirmation.
- Rust command surface exposed to the frontend:
  - `search_entries(query) -> EntrySummary[]`
  - `create_entry(input) -> Entry`
  - `update_entry(id, input) -> Entry`
  - `delete_entry(id) -> ()`
  - `get_recent_entries() -> EntrySummary[]`
  - `paste_entry(id) -> PasteResult`
- macOS paste handling:
  - Copy selected value to clipboard first.
  - Attempt paste via simulated `Cmd+V` using a Rust-side input automation library.
  - If Accessibility permission is missing or paste fails, keep the value on the clipboard and show a notice: copied successfully, auto-paste unavailable.

## Public Interfaces / Types
- Frontend types:
  - `Entry = { id: string; key: string; value: string; tags: string[]; createdAt: string; updatedAt: string }`
  - `EntrySummary = { id: string; key: string; tags: string[]; updatedAt: string }`
  - `EntryInput = { key: string; value: string; tags: string[] }`
  - `PasteResult = { mode: "pasted" | "copied_only"; message: string }`
- Validation rules:
  - `key` must be non-empty and unique.
  - `value` must be non-empty.
  - Empty tags are dropped; duplicate tags collapse to one normalized tag.

## Test Plan
- Launcher behavior:
  - Global shortcut opens and hides the launcher reliably.
  - `Esc` hides without quitting.
  - Closing the window leaves the app running.
- CRUD:
  - Create, edit, delete, and duplicate-key rejection work correctly.
  - Tag normalization and deduplication behave as expected.
- Search:
  - Key prefix matches rank above substring matches.
  - Tag matches appear when key does not match.
  - Value text never affects results.
  - Empty query returns recent entries.
- Paste flow:
  - `Enter` copies, attempts paste, and hides the launcher.
  - Missing Accessibility permission returns `copied_only` and shows the fallback notice.
- Persistence:
  - Entries survive app restart.
  - Timestamps update correctly on edit.

## Assumptions and Defaults
- v1 is macOS-only in behavior and polish, even if Tauri remains cross-platform-capable structurally.
- No encryption, Keychain integration, import/export, sync, folders, or settings UI in v1.
- The default shortcut is fixed to `Control + Option + Space` in v1; configurability is deferred.
- The app ships as a regular macOS app bundle with manual first launch; launch-at-login is deferred.
