"""Vault access: FTS5 index over markdown files + read/append helpers.

The index refreshes lazily (cheap mtime scan) before searches, so external
changes arriving via Obsidian Sync are picked up without a daemon thread.
"""

import time
from pathlib import Path

from .config import config
from .db import get_db

IGNORED_DIRS = {".obsidian", ".trash", ".git"}
_last_scan = 0.0
_SCAN_INTERVAL = 15.0  # seconds between index refresh scans


def _iter_md_files():
    for path in config.vault_path.rglob("*.md"):
        if not any(part in IGNORED_DIRS for part in path.parts):
            yield path


def refresh_index(force: bool = False) -> int:
    """Reindex vault files whose mtime changed. Returns number reindexed."""
    global _last_scan
    now = time.monotonic()
    if not force and now - _last_scan < _SCAN_INTERVAL:
        return 0
    _last_scan = now

    changed = 0
    with get_db() as conn:
        known = {r["path"]: r["mtime"] for r in conn.execute("SELECT * FROM notes_meta")}
        seen = set()
        for path in _iter_md_files():
            rel = str(path.relative_to(config.vault_path))
            seen.add(rel)
            mtime = path.stat().st_mtime
            if known.get(rel) == mtime:
                continue
            try:
                body = path.read_text(errors="replace")
            except OSError:
                continue
            conn.execute("DELETE FROM notes_fts WHERE path = ?", (rel,))
            conn.execute(
                "INSERT INTO notes_fts (path, title, body) VALUES (?, ?, ?)",
                (rel, path.stem, body),
            )
            conn.execute(
                "INSERT INTO notes_meta (path, mtime) VALUES (?, ?) "
                "ON CONFLICT(path) DO UPDATE SET mtime = excluded.mtime",
                (rel, mtime),
            )
            changed += 1
        for gone in set(known) - seen:
            conn.execute("DELETE FROM notes_fts WHERE path = ?", (gone,))
            conn.execute("DELETE FROM notes_meta WHERE path = ?", (gone,))
            changed += 1
    return changed


def search_notes(query: str, limit: int = 8) -> list[dict]:
    refresh_index()
    with get_db() as conn:
        try:
            rows = conn.execute(
                "SELECT path, title, snippet(notes_fts, 2, '»', '«', ' … ', 24) AS snippet "
                "FROM notes_fts WHERE notes_fts MATCH ? ORDER BY rank LIMIT ?",
                (query, limit),
            ).fetchall()
        except Exception:
            # FTS5 chokes on some raw user syntax; retry as quoted phrase terms
            safe = " ".join(f'"{t}"' for t in query.split())
            rows = conn.execute(
                "SELECT path, title, snippet(notes_fts, 2, '»', '«', ' … ', 24) AS snippet "
                "FROM notes_fts WHERE notes_fts MATCH ? ORDER BY rank LIMIT ?",
                (safe, limit),
            ).fetchall()
    return [dict(r) for r in rows]


def read_note(rel_path: str, max_chars: int = 20000) -> str:
    path = (config.vault_path / rel_path).resolve()
    if not path.is_relative_to(config.vault_path.resolve()):
        raise ValueError("path escapes vault")
    if not path.exists():
        raise FileNotFoundError(rel_path)
    text = path.read_text(errors="replace")
    if len(text) > max_chars:
        text = text[:max_chars] + f"\n… [truncated, {len(text)} chars total]"
    return text


def append_note(rel_path: str, text: str, heading: str | None = None) -> str:
    """Append text to a note (created if missing), optionally under a heading."""
    path = (config.vault_path / rel_path).resolve()
    if not path.is_relative_to(config.vault_path.resolve()):
        raise ValueError("path escapes vault")
    path.parent.mkdir(parents=True, exist_ok=True)
    existing = path.read_text() if path.exists() else ""
    block = f"\n## {heading}\n\n{text}\n" if heading else f"\n{text}\n"
    path.write_text(existing.rstrip() + "\n" + block.lstrip("\n") if existing else block.lstrip("\n"))
    refresh_index(force=True)
    return str(path.relative_to(config.vault_path))


def standing_context() -> str:
    """Vault CLAUDE.md files, injected (and prompt-cached) as standing context."""
    parts = []
    for path in sorted(config.vault_path.rglob("CLAUDE.md")):
        if any(part in IGNORED_DIRS for part in path.parts):
            continue
        rel = path.relative_to(config.vault_path)
        parts.append(f"### {rel}\n\n{path.read_text(errors='replace')}")
    return "\n\n".join(parts)
