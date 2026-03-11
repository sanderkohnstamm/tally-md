# Tauri Rebuild Plan

## Goal
Rebuild the TUI markdown todo editor as a standalone Tauri desktop app with CodeMirror for editing.

## Architecture
- **Backend (Rust/Tauri):** File I/O, todo completion logic, finished.md management
- **Frontend (HTML/JS):** Two-pane CodeMirror editor with markdown preview

## Storage
- Same `~/.todos/` directory with `todo.md` and `finished.md`

## Features (carry over from TUI)
- Two-pane layout: todo.md (left, 65%) + finished.md (right, 35%)
- Full markdown editing via CodeMirror (selection, undo/redo, search, clipboard — all free)
- Markdown preview mode (CodeMirror markdown extension or rendered HTML toggle)
- Complete a `- ` list item (Ctrl+Enter): moves it from todo.md to finished.md under today's date header with parent breadcrumb
- Recover item (Ctrl+Enter in finished pane): moves back to todo.md
- Auto-save or Ctrl+S save
- Fill empty day headers in finished.md on startup
- System clipboard works natively

## Tech Stack
- Tauri 2.x (Rust backend + system WebView)
- CodeMirror 6 (editor)
- Vanilla JS/CSS (no framework needed, keep it simple)

## File Structure
```
src-tauri/
  src/
    main.rs          — Tauri app entry
    commands.rs      — Tauri commands (load, save, complete, recover)
    finished.rs      — Completion/recovery logic (port from current)
  Cargo.toml
  tauri.conf.json
src/                 — Frontend
  index.html
  style.css
  main.js            — CodeMirror setup, Tauri IPC calls
package.json
```

## Tauri Commands (Rust → JS bridge)
1. `load_files()` → returns `{ todo: String, finished: String }`
2. `save_files(todo: String, finished: String)` → writes both files
3. `complete_item(todo: String, cursor_line: usize)` → returns `{ todo: String, finished: String }`
4. `recover_item(finished: String, todo: String, cursor_line: usize)` → returns `{ todo: String, finished: String }`

## Frontend Behavior
1. On load: call `load_files()`, populate both CodeMirror instances
2. Ctrl+S: read both editors, call `save_files()`
3. Ctrl+Enter in todo pane: get cursor line, call `complete_item()`, update both editors
4. Ctrl+Enter in finished pane: get cursor line, call `recover_item()`, update both editors
5. Finished pane is read-only except for recovery

## Build Steps
1. Scaffold Tauri project (`npm create tauri-app` or manual setup)
2. Port `finished.rs` logic to `src-tauri/src/`
3. Set up CodeMirror with markdown support
4. Wire up Tauri IPC commands
5. Style the two-pane layout
6. Test on macOS and Linux
7. Build standalone binary with `cargo tauri build`

## Output
- macOS: `.dmg` / `.app` (~5-10MB)
- Linux: `.deb` / `.AppImage`
