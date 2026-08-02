"""SQLite: FTS5 notes index, capture history, event cache, chat history.

Everything here is a disposable index — the vault's markdown files are the
source of truth and the whole database can be rebuilt from them + the calendar
APIs at any time.
"""

import sqlite3
from contextlib import contextmanager

from .config import config

SCHEMA = """
CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    path, title, body, tokenize='porter unicode61'
);
CREATE TABLE IF NOT EXISTS notes_meta (
    path TEXT PRIMARY KEY,
    mtime REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS captures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    text TEXT NOT NULL,
    response TEXT,
    actions TEXT               -- json list of write-tool calls Claude made
);
CREATE TABLE IF NOT EXISTS events (
    uid TEXT PRIMARY KEY,
    source TEXT NOT NULL,          -- google|icloud
    calendar TEXT,
    title TEXT,
    start_utc TEXT NOT NULL,
    end_utc TEXT,
    all_day INTEGER NOT NULL DEFAULT 0,
    location TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS events_start ON events(start_utc);
CREATE TABLE IF NOT EXISTS briefings (
    date TEXT PRIMARY KEY,         -- local YYYY-MM-DD
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    text TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS chat_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    role TEXT NOT NULL,            -- user|assistant
    content TEXT NOT NULL
);
"""


@contextmanager
def get_db():
    conn = sqlite3.connect(config.db_path)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL")
    try:
        yield conn
        conn.commit()
    finally:
        conn.close()


def init_db() -> None:
    with get_db() as conn:
        conn.executescript(SCHEMA)
