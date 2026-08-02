#!/usr/bin/env bash
# Provision tally-server on the Pi (run as wakey on mm4). Idempotent.
set -euo pipefail

REPO_DIR="$HOME/tally"

sudo apt-get update -qq
sudo apt-get install -y -qq python3-venv git nodejs npm

# --- python app ---
cd "$REPO_DIR/server"
[ -d venv ] || python3 -m venv venv
./venv/bin/pip install -q -r requirements.txt

# --- obsidian sync (headless) ---
if ! command -v ob >/dev/null; then
  sudo npm install -g obsidian-headless
  echo ">>> Now run: ob login   (then: ob sync-list-remote; ob sync-setup --vault \"Space\")"
fi

# --- env file ---
if [ ! -f "$REPO_DIR/.env" ]; then
  cat > "$REPO_DIR/.env" <<'EOF'
TALLY_VAULT=/home/wakey/Space
TALLY_TODO_DIR=Work
TALLY_INBOX_NOTE=Inbox.md
ANTHROPIC_API_KEY=
# ICLOUD_USERNAME=
# ICLOUD_APP_PASSWORD=
# GOOGLE_CREDENTIALS=/home/wakey/.tally-server/google-credentials.json
# GOOGLE_TOKEN=/home/wakey/.tally-server/google-token.json
EOF
  chmod 600 "$REPO_DIR/.env"
  echo ">>> Fill in $REPO_DIR/.env (at minimum ANTHROPIC_API_KEY)"
fi

# --- systemd ---
sudo cp "$REPO_DIR/server/deploy/tally-server.service" /etc/systemd/system/
sudo cp "$REPO_DIR/server/deploy/obsidian-sync.service" /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now tally-server
# obsidian-sync only makes sense once `ob sync-setup` has been run:
sudo systemctl enable --now obsidian-sync || true

# --- tailscale serve: HTTPS at https://mm4.<tailnet>.ts.net ---
sudo tailscale serve --bg 8321 || echo ">>> tailscale serve failed — is tailscale up?"

echo "done. check: systemctl status tally-server"
