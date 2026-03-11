# Tally.md

Markdown todo app. Three panes: **Todo → Today → Done**.

![Main view](screenshots/main.png)

## Install

```
cd desktop
npm install
npm run package
```

## Develop

```
cd desktop
npm install
npm run dev        # Tauri dev mode
npm run watch      # frontend hot reload (separate terminal)
```

## Shortcuts

All customizable in Settings (`Ctrl+,`). On macOS use `Cmd`.

| Shortcut | Action |
|---|---|
| `Ctrl+Enter` | Move item forward |
| `Ctrl+Shift+Enter` | Send item back |
| `Ctrl+Shift+D` | Skip to done |
| `Ctrl+S` | Save |
| `Ctrl+\` | Next pane |
| `Ctrl+Shift+B` | Toggle done pane |
| `Ctrl+E` | Fold/unfold |
| `Ctrl+K` | Cycle theme |
| `Ctrl+Shift+S` | Git sync |
| `Ctrl+Click` | Open link |

![Shortcuts](screenshots/shortcuts.png)

## Example

Your data is just markdown files. Here's what they look like:

**todo.md**
```markdown
## Work
- Review PR for auth service
- Write integration tests for payment flow

## Personal
- Research flights to Lisbon
- Fix squeaky door hinge
```

**today.md**
```markdown
- Fix login redirect bug (#342)
- Prep slides for Friday demo
```

**done.md** — items get a breadcrumb showing where they came from:
```markdown
## 2026-03-11
- Merge dark mode PR (Work)
- Order new monitor stand (Personal)

## 2026-03-10
- Ship v1.2 hotfix (Work)
- Book dentist appointment (Personal)
```

See [`example/`](example/) for full sample files.

## Storage

**Local:** files in `~/.todos/` (configurable).

**Git sync:** connect a repo to sync across machines. Token stored in OS keychain. Settings sync too. If a merge conflict overwrites your local changes, Tally.md backs up your files and shows a warning with the backup path.

## Themes

8 built-in themes. Cycle with `Ctrl+K`.

![Themes](screenshots/themes.png)

## Tech

[Tauri 2](https://tauri.app/) + [CodeMirror 6](https://codemirror.net/). No frameworks, no Electron.
