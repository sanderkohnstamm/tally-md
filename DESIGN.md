# Tally Assistant — Design

Tally grows from a markdown todo editor into a personal assistant. Same philosophy: markdown
files are the source of truth, no frameworks, keyboard/thumb-first, themeable. The new piece is
`server/` — a web app that runs on the Pi (`mm4`, reachable over Tailscale), is opened on a
phone, and can listen.

## What it does

- **Capture, zero decisions.** One text box at the top of the phone screen — iOS keyboard
  dictation does voice-to-text (no server-side transcription needed). Claude files what you
  said: a todo goes to `todo.md`, a note goes to the vault inbox, a question gets answered.
  You never pick folders or tags. (Pattern borrowed from Saner.AI's capture loop.)
  `POST /api/capture` also accepts raw text, so an iOS Shortcut ("dictate → post to tally")
  works as a two-tap capture path without opening the app.
- **Chat grounded in the vault.** Claude with tool access: full-text search over
  `~/Documents/Space` (Obsidian vault, synced to the Pi via `obsidian-headless`), read notes,
  list/add/move todos, read the agenda.
- **Todos = tally files.** The assistant operates on the vault's `todo.md` / `today.md` /
  `done.md` with tally semantics (move forward Todo → Today → Done, date headers, breadcrumbs).
  No second todo store.
- **Read-only agenda.** Google Calendar (REST + OAuth) and Apple/iCloud (CalDAV + app-specific
  password), normalized into one event model, polled ~5 min, cached in SQLite. Event creation
  is v2.
- **Today view as the landing screen.** Agenda + `today.md` + a short Claude morning brief.

## Architecture

```
phone (PWA, Safari)
   │ HTTPS via `tailscale serve` (real cert for mm4.<tailnet>.ts.net)
   ▼
mm4 (Pi 4, Debian 13)
   ├─ tally-server: FastAPI + uvicorn, one process (systemd)
   │    ├─ SQLite (index.db): FTS5 notes index, transcription jobs,
   │    │    event cache, chat history   ← disposable, rebuildable
   │    ├─ Claude API (agent loop with tools, SSE streaming to UI)
   │    └─ calendar pollers (google REST, icloud caldav)
   ├─ obsidian-headless: syncs ~/vault ↔ Obsidian Sync   ← vault source of truth
   └─ tailscaled
```

- **Stack:** FastAPI + Jinja2 + HTMX + vanilla JS. No build step, ~60MB RAM. Matches the
  wakey deployment pattern (uvicorn + systemd) and tally's no-framework rule.
- **Vault access:** read/write plain files in the synced vault checkout. A `watchdog` observer
  reindexes changed `.md` files into FTS5. Writes are plain file writes — obsidian-headless
  picks them up and syncs to phone/Mac in seconds.
- **RAG:** none. FTS5 + agentic search (Claude issues `search_notes` queries itself), plus the
  vault's own `CLAUDE.md` files injected as standing context with prompt caching. Vector search
  only if keyword search demonstrably misses (sqlite-vec ready as an upgrade path).
- **Capture pipeline:** text in (typed or iOS-dictated) → Claude classifies with tools
  (`add_todo` / `append_note` / answer) → capture card shows what was filed and where;
  anything ambiguous is left as a proposed action to confirm, not silently filed. No
  server-side audio handling at all — the phone's dictation is the STT.
- **Claude:** `claude-sonnet-5` default (config-switchable), tools: `search_notes`,
  `read_note`, `append_note`, `list_todos`, `add_todo`, `move_todo`, `get_agenda`.
  System prompt carries vault conventions. Read-only posture toward external services.

## UI (tally design language)

Same `--bg/--surface/--overlay/--text/...` variable scheme and the same 8 palettes as the
desktop app (Catppuccin default), monospace type, flat panes, no chrome. Phone-first layout:

```
┌──────────────────────┐
│ today · 3 events     │   Today: agenda list + today.md items
│ ────────────────     │
│ chat                 │   Chat: SSE-streamed, tool calls shown as thin status lines
│                      │
│                      │
│  ┌────────────────┐  │
│  │  capture…      │  │   Capture: text box (iOS dictation), thumb zone
│  └────────────────┘  │
│  today  chat  todos  │   Bottom tab bar (thumb nav)
└──────────────────────┘
```

Three tabs = three panes: **today / chat / todos** (todos tab renders the three tally files with
tap-to-advance). PWA manifest (`display: standalone`, apple-touch-icon, safe-area insets),
service worker caches the shell only — no offline data (iOS evicts it anyway; Tailscale means
effectively always online).

## Repo layout

```
desktop/   Tauri desktop app (unchanged)
ios/       SwiftUI app (unchanged)
server/    NEW — the Pi assistant
  app/                FastAPI application package
  static/             css, vanilla js (capture, chat), icons, manifest
  templates/          Jinja2 pages + HTMX partials
  deploy/             setup.sh, tally-server.service, README
```

## Pi provisioning (mm4)

Cleaned 2026-08-02: `wakey`, `netmon`, `go-librespot` stopped + disabled (files kept).
Installed: tailscale (authorized into tailnet by Sander). To install: nodejs (for
`npm i -g obsidian-headless`), python3-venv. See `server/deploy/setup.sh`.

Secrets live in `/home/wakey/tally/.env` (never in the repo): `ANTHROPIC_API_KEY`,
`ICLOUD_USERNAME`/`ICLOUD_APP_PASSWORD`, Google OAuth client + token JSON paths, vault path.

Manual (Sander) steps: `tailscale up` auth click, `ob login` for Obsidian Sync, Anthropic API
key, Google OAuth consent (publish to production to avoid 7-day token expiry), iCloud
app-specific password.

## v2 candidates (explicitly out of scope now)

Event creation from chat, proactive reminders (ntfy push), task breakdown prompts, weekly
review helper (`/weekly` ritual), sqlite-vec hybrid search, email.
