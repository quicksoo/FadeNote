# AGENTS.md

## Project Overview
- FadeNote is a Tauri v2 desktop sticky-note app.
- Frontend lives in `src/` and uses plain HTML/CSS/JavaScript without a bundler.
- Backend lives in `src-tauri/src/main.rs` and manages note files, windows, tray actions, lifecycle, and settings.
- Notes are stored as Markdown files under the app data directory, with metadata tracked in `index.json`.

## Development Guidelines
- Keep changes focused and minimal; avoid broad refactors unless explicitly requested.
- Preserve the lightweight, dependency-minimal architecture.
- Do not introduce a frontend framework or build step unless the user explicitly approves it.
- Save note body content as Markdown text, not HTML.
- Do not change note lifecycle state (`pinned`, `archivedAt`, `expireAt`, etc.) for purely UI/window behaviors.
- Temporary window raise/top behavior must restore each window’s original always-on-top state.

## Frontend Notes
- Main note UI is `src/index.html`, `src/main.js`, and `src/styles.css`.
- Archive UI is `src/archive.html` and `src/archive.js`.
- Settings UI is `src/settings.html` and `src/settings.js`.
- Prefer plain browser APIs and Tauri global APIs through `window.__TAURI__`.
- Run `node --check` on changed JavaScript files after edits.

## Tauri/Rust Notes
- Main Rust entry is `src-tauri/src/main.rs`.
- Keep Tauri command names stable unless updating all frontend invocations.
- When adding frontend window APIs, update `src-tauri/capabilities/default.json` with the minimum required permissions.
- Run `cargo check` from `src-tauri/` after Rust or capability changes.

## Validation
- JavaScript syntax checks:
  - `node --check .\src\main.js`
  - `node --check .\src\archive.js`
  - `node --check .\src\settings.js`
- Rust check:
  - `cd src-tauri && cargo check`

## Caution
- Avoid rewriting large files with tools that may change encoding or line endings unnecessarily.
- Do not run formatters that rewrite the whole project unless explicitly requested.
- Do not commit changes unless the user asks.
