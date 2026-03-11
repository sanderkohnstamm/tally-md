# Tally.md

A minimal desktop app for managing your tasks in plain markdown. Three panes, one flow: **Todo → Today → Done**.

Write your backlog in Todo. Pull items into Today when you're ready to work on them. Mark them done when finished. Done items are logged by date — a permanent record of what you accomplished.

All your data is plain `.md` files. No databases, no cloud lock-in.

## Install

```
npm install
npm run package
```

The built binary is in `src-tauri/target/release/`.

## Develop

```
npm install
npm run dev
```

In a separate terminal for frontend hot reload:

```
npm run watch
```

## How It Works

**Three panes:**
- **Todo** — your backlog, everything you need to do
- **Today** — what you're working on right now
- **Done** — completed items, organized by date

**Moving items:**
- `Ctrl+Enter` moves a line forward (Todo → Today, or Today → Done)
- `Ctrl+Shift+Enter` sends a line back (Today → Todo, or Done → Todo)
- `Ctrl+Shift+D` skips Today and sends straight from Todo → Done

Items moved to Done get a breadcrumb showing where they came from, and are filed under today's date header automatically.

## Keyboard Shortcuts

All shortcuts are customizable in Settings → Keyboard Shortcuts.

| Shortcut | Action |
|---|---|
| `Ctrl+Enter` | Move item forward |
| `Ctrl+Shift+Enter` | Send item back |
| `Ctrl+Shift+D` | Skip to done |
| `Ctrl+S` | Save |
| `Ctrl+\` | Next pane |
| `Ctrl+Shift+\` | Previous pane |
| `Ctrl+Shift+B` | Toggle done pane |
| `Ctrl+B` | Bold |
| `Ctrl+I` | Italic |
| `Ctrl+E` | Toggle fold |
| `Ctrl+Shift+E` | Toggle fold all |
| `Ctrl+K` | Cycle theme |
| `Ctrl+Shift+S` | Git sync |
| `Ctrl+,` | Settings |
| `Ctrl+Click` | Open link |

On macOS, use `Cmd` instead of `Ctrl`.

## Storage

**Local (default):** files are stored in a local folder (default `~/.todos/`).

**Git sync:** connect a git repo to sync your files across machines. Tally.md handles clone, pull, push, and merge automatically. Your personal access token is stored securely in your OS keychain.

Settings are saved alongside your data so they sync too.

## Themes

8 built-in themes: White on Black, Black on White, Catppuccin, Rose Pine, Tokyo Night, Soft Ember, Nord, and Moonlight. Cycle with `Ctrl+K` or pick in Settings.

## Settings

Open with `Ctrl+,`. Configure:

- Storage mode (local folder or git repo)
- Theme
- Layout (side by side, stacked, or split)
- Date format for done headers
- Auto-sync interval
- All keyboard shortcuts

## Tech

Built with [Tauri 2](https://tauri.app/) (Rust backend) and [CodeMirror 6](https://codemirror.net/) (editor). No frameworks, no Electron. Small binary, low resource usage.
