# tally assistant (server)

Self-hosted personal assistant for the Obsidian vault. Runs on the Pi (`mm4`), opened on a
phone over Tailscale. See the repo-root `DESIGN.md` for the full picture.

## Run locally

```bash
cd server
python3 -m venv venv && ./venv/bin/pip install -r requirements.txt
TALLY_VAULT=~/Documents/Space ANTHROPIC_API_KEY=sk-… ./venv/bin/uvicorn app.main:app --port 8321
```

## Deploy to the Pi

```bash
# from the repo root on the Pi (cloned to ~/tally):
server/deploy/setup.sh
```

Then the one-time manual steps:

1. `sudo tailscale up` → click the auth link (once per Pi).
2. `ob login` → `ob sync-setup --vault "Space"` (Obsidian Sync headless; needs Sync subscription).
3. Put `ANTHROPIC_API_KEY` in `~/tally/.env`.
4. Calendars (optional, read-only agenda):
   - **iCloud:** create an app-specific password at appleid.apple.com → `ICLOUD_USERNAME` +
     `ICLOUD_APP_PASSWORD` in `.env`.
   - **Google:** create an OAuth desktop client (Calendar readonly scope), **publish the consent
     screen to "In production"** (otherwise refresh tokens die after 7 days), download the client
     JSON to `~/.tally-server/google-credentials.json`, then run
     `./venv/bin/python -m app.google_auth` once on a machine with a browser and copy the
     resulting token JSON to `~/.tally-server/google-token.json`.
5. `sudo systemctl restart tally-server` → open `https://mm4.<tailnet>.ts.net` on the phone →
   share sheet → *Add to Home Screen*.

## Environment

| var | default | meaning |
|---|---|---|
| `TALLY_VAULT` | `~/vault` | path to the synced Obsidian vault |
| `TALLY_TODO_DIR` | `Work` | vault folder containing todo/today/done.md |
| `TALLY_INBOX_NOTE` | `Inbox.md` | where captures that are notes get appended |
| `TALLY_MODEL` | `claude-sonnet-5` | Claude model for chat + capture |
| `TALLY_DATA` | `~/.tally-server` | SQLite index + tokens (disposable) |
