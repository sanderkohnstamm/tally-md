"""Configuration from environment / .env file."""

import os
from dataclasses import dataclass, field
from pathlib import Path


def _load_dotenv(path: Path) -> None:
    if not path.exists():
        return
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        os.environ.setdefault(key.strip(), value.strip())


_load_dotenv(Path(os.environ.get("TALLY_ENV_FILE", ".env")))


@dataclass
class Config:
    vault_path: Path = field(
        default_factory=lambda: Path(os.environ.get("TALLY_VAULT", "~/vault")).expanduser()
    )
    # Folder inside the vault holding todo.md / today.md / done.md
    todo_dir: str = os.environ.get("TALLY_TODO_DIR", "Work")
    # Note inside the vault where quick captures are appended
    inbox_note: str = os.environ.get("TALLY_INBOX_NOTE", "Inbox.md")
    # Vault subfolder Claude focuses on ("" = whole vault); changeable in settings
    focus_dir: str = os.environ.get("TALLY_FOCUS_DIR", "")
    data_dir: Path = field(
        default_factory=lambda: Path(os.environ.get("TALLY_DATA", "~/.tally-server")).expanduser()
    )
    anthropic_api_key: str = os.environ.get("ANTHROPIC_API_KEY", "")
    model: str = os.environ.get("TALLY_MODEL", "claude-sonnet-5")
    icloud_username: str = os.environ.get("ICLOUD_USERNAME", "")
    icloud_app_password: str = os.environ.get("ICLOUD_APP_PASSWORD", "")
    google_credentials: Path = field(
        default_factory=lambda: Path(
            os.environ.get("GOOGLE_CREDENTIALS", "~/.tally-server/google-credentials.json")
        ).expanduser()
    )
    google_token: Path = field(
        default_factory=lambda: Path(
            os.environ.get("GOOGLE_TOKEN", "~/.tally-server/google-token.json")
        ).expanduser()
    )
    calendar_poll_seconds: int = int(os.environ.get("TALLY_CAL_POLL", "300"))

    @property
    def db_path(self) -> Path:
        return self.data_dir / "index.db"

    def todo_file(self, name: str) -> Path:
        return self.vault_path / self.todo_dir / f"{name}.md"


config = Config()
config.data_dir.mkdir(parents=True, exist_ok=True)
