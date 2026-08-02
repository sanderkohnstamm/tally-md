"""Runtime settings: secrets entered via the settings page + status probes.

Secrets are stored outside the repo and the vault, in the data dir with 0600
perms — never committed, never synced. They overlay whatever is in .env.
"""

import json
import os
import subprocess
from pathlib import Path

from .config import config

SECRETS_PATH = config.data_dir / "secrets.json"

SECRET_FIELDS = (
    "claude_oauth_token",
    "anthropic_api_key",
    "model",
    "icloud_username",
    "icloud_app_password",
    "google_client_id",
    "google_client_secret",
)

# Settings where empty is a meaningful value (not "leave unchanged"); these are
# shown editable in the form rather than masked
PLAIN_FIELDS = ("focus_dir", "ics_urls")


def load_secrets() -> dict:
    if SECRETS_PATH.exists():
        try:
            return json.loads(SECRETS_PATH.read_text())
        except (json.JSONDecodeError, OSError):
            return {}
    return {}


def save_secrets(updates: dict) -> None:
    """Merge non-empty updates into the secrets file (0600)."""
    data = load_secrets()
    for key, value in updates.items():
        if key in SECRET_FIELDS and value:  # empty string = "leave unchanged"
            data[key] = value
        elif key in PLAIN_FIELDS and value is not None:
            data[key] = value
    SECRETS_PATH.write_text(json.dumps(data, indent=2))
    os.chmod(SECRETS_PATH, 0o600)
    apply_to_config()


def apply_to_config() -> None:
    """Push stored secrets into the live config object."""
    data = load_secrets()
    if data.get("claude_oauth_token"):
        config.claude_oauth_token = data["claude_oauth_token"]
    if data.get("anthropic_api_key"):
        config.anthropic_api_key = data["anthropic_api_key"]
    if data.get("model"):
        config.model = data["model"]
    if data.get("icloud_username"):
        config.icloud_username = data["icloud_username"]
    if data.get("icloud_app_password"):
        config.icloud_app_password = data["icloud_app_password"]
    if "focus_dir" in data:
        config.focus_dir = data["focus_dir"]
    if "ics_urls" in data:
        config.ics_urls = data["ics_urls"]


def google_client() -> tuple[str, str]:
    data = load_secrets()
    return data.get("google_client_id", ""), data.get("google_client_secret", "")


def masked_key() -> str:
    key = config.anthropic_api_key
    if not key:
        return ""
    return f"{key[:7]}…{key[-4:]}" if len(key) > 12 else "set"


def auth_mode() -> str:
    """Which Claude credential is live: subscription login beats API key."""
    if config.claude_oauth_token:
        return "subscription"
    if config.anthropic_api_key:
        return "api-key"
    return ""


def vault_folders() -> list[str]:
    """Actual vault folders (two levels deep), for the focus-folder dropdown."""
    if not config.vault_path.exists():
        return []
    out: list[str] = []
    for p in sorted(config.vault_path.iterdir(), key=lambda p: p.name.lower()):
        if p.is_dir() and not p.name.startswith("."):
            out.append(p.name)
            out.extend(
                f"{p.name}/{q.name}"
                for q in sorted(p.iterdir(), key=lambda q: q.name.lower())
                if q.is_dir() and not q.name.startswith(".")
            )
    return out


# ---------- status probes ----------

def obsidian_sync_status() -> dict:
    """Is obsidian-headless configured for the vault, and is the sync service running?"""
    configured, detail = False, ""
    try:
        result = subprocess.run(
            ["ob", "sync-status", "--path", str(config.vault_path)],
            capture_output=True, text=True, timeout=10,
        )
        out = (result.stdout or result.stderr).strip()
        configured = result.returncode == 0
        detail = out.splitlines()[0][:80] if out else ("" if configured else "not configured")
    except FileNotFoundError:
        detail = "ob CLI not installed"
    except subprocess.TimeoutExpired:
        detail = "ob sync-status timed out"

    service_active = False
    try:
        result = subprocess.run(
            ["systemctl", "is-active", "obsidian-sync"],
            capture_output=True, text=True, timeout=5,
        )
        service_active = result.stdout.strip() == "active"
    except (FileNotFoundError, subprocess.TimeoutExpired):
        pass  # not linux/systemd (local dev)

    return {
        "ok": configured and service_active,
        "configured": configured,
        "service_active": service_active,
        "detail": detail,
    }


def google_status() -> dict:
    client_id, client_secret = google_client()
    return {
        "client_set": bool(client_id and client_secret),
        "connected": config.google_token.exists(),
    }
