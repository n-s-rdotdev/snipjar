# Snipjar

Snipjar is a macOS-first desktop launcher for local text snippets. It is built with Tauri 2, React, Vite, and a Rust backend.

## What It Does

- Opens a single floating launcher window with `Control + Option + Space`.
- Searches local snippets by `key` and `tags`.
- Stores data in SQLite under the app local data directory.
- Supports keyboard-first create, edit, delete, and paste flows.
- Copies the selected value to the clipboard and attempts an automatic paste into the previously active app.

## Architecture

- Frontend: React + Vite in `src/`
- Native shell and command surface: Tauri 2 in `src-tauri/`
- Persistence, validation, ranking, and paste automation: Rust in `src-tauri/src/storage.rs`

Rust owns the core native responsibilities: persistence, search/ranking, global shortcut handling, clipboard writes, and paste automation.

## v1 Non-Goals

Snipjar v1 is intentionally narrow in scope.

- No encryption
- No Keychain integration
- No import/export
- No sync
- No folders
- No settings UI
- No launch-at-login support
- No configurable shortcut in v1

This is a local snippet launcher, not a secrets manager.

## Development

Install dependencies:

```sh
npm install
```

Run the desktop app in development:

```sh
npm run tauri dev
```

Run Rust tests:

```sh
cd src-tauri && cargo test
```

Build the frontend bundle:

```sh
npm run build
```

## macOS Notes

Automatic paste uses simulated `Cmd+V`, so macOS Accessibility permission may be required. When auto-paste is unavailable, Snipjar keeps the value on the clipboard and reopens the launcher with a fallback message.

## Current Verification

Verified on 2026-03-28:

- `cargo test` passes in `src-tauri/`
- `npm run build` passes at the repo root

Launcher behavior and paste behavior still need native manual verification from a running Tauri app.
