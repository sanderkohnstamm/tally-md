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

## Storage

**Local:** files in `~/.todos/` (configurable).

**Git sync:** connect a repo to sync across machines. Token stored in OS keychain. Settings sync too.

## Themes

8 built-in themes. Cycle with `Ctrl+K`.

![Themes](screenshots/themes.png)

## Tech

[Tauri 2](https://tauri.app/) + [CodeMirror 6](https://codemirror.net/). No frameworks, no Electron.
