# Snipjar

Snipjar is a macOS-first desktop launcher for local text snippets. It is built with Tauri 2, React 19, Vite, and a Rust backend that owns persistence, search, clipboard integration, and paste automation.

## Overview

- Opens a single floating launcher with `Control + Option + Space`
- Stores snippets locally in SQLite as `snipjar.db`
- Searches by snippet key and tags
- Supports keyboard-first create, edit, delete, and paste flows
- Copies the selected snippet and attempts to paste it back into the previously active app

Snipjar is a local snippet launcher, not a secrets manager.

## Project Structure

- `src/`: React UI and Tauri command client
- `src-tauri/src/lib.rs`: app lifecycle, launcher visibility, tray integration, and native command registration
- `src-tauri/src/storage.rs`: SQLite storage, validation, search ranking, clipboard, and paste flow

## v1 Scope

Out of scope for v1:

- Encryption
- Keychain integration
- Import and export
- Sync
- Folders
- Settings UI
- Launch at login
- Configurable shortcuts

## Development

Install JavaScript dependencies:

```sh
npm install
```

Run the desktop app in development:

```sh
npm run tauri -- dev
```

Build the web bundle used by Tauri:

```sh
npm run build
```

Build a desktop bundle:

```sh
npm run tauri -- build
```

Run Rust tests:

```sh
cd src-tauri && cargo test
```

## macOS Notes

Automatic paste uses a simulated `Cmd+V`, so macOS Accessibility permission may be required. If Snipjar cannot complete the paste, it keeps the snippet value on the clipboard and restores the launcher with a fallback message.

## Verification

Verified on 2026-03-28:

- `npm run build` passes at the repo root
- `cd src-tauri && cargo test` passes

Launcher visibility, global shortcut behavior, and automatic paste still require manual verification from a running Tauri build on macOS.
