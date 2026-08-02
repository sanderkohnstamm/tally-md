"""One-time Google OAuth bootstrap: run on a machine with a browser, then copy
the token JSON to the Pi.

    python -m app.google_auth [credentials.json] [token-out.json]
"""

import sys
from pathlib import Path

from google_auth_oauthlib.flow import InstalledAppFlow

from .config import config

SCOPES = ["https://www.googleapis.com/auth/calendar.readonly"]


def main() -> None:
    creds_path = Path(sys.argv[1]) if len(sys.argv) > 1 else config.google_credentials
    token_path = Path(sys.argv[2]) if len(sys.argv) > 2 else config.google_token
    flow = InstalledAppFlow.from_client_secrets_file(str(creds_path), SCOPES)
    creds = flow.run_local_server(port=0)
    token_path.parent.mkdir(parents=True, exist_ok=True)
    token_path.write_text(creds.to_json())
    print(f"token written to {token_path} — copy it to the Pi's ~/.tally-server/")


if __name__ == "__main__":
    main()
