"""Tally todo operations on the vault's todo.md / today.md / done.md.

Format-compatible with the tally desktop app (desktop/src-tauri/src/finished.rs):
items are `- …` lines (usually `- [ ] …`) grouped under `## Heading` sections;
completing an item appends a `(Heading)` breadcrumb and files it under a
`## YYYY-MM-DD` header in done.md, newest day first.
"""

import datetime
import re
from dataclasses import dataclass

from .config import config

FILES = ("todo", "today", "done")
_ITEM_RE = re.compile(r"^(\s*)- (\[.\] )?(.*)$")


@dataclass
class Item:
    file: str
    line_no: int
    text: str          # without leading "- " marker (checkbox kept)
    section: str


def _read(name: str) -> str:
    path = config.todo_file(name)
    return path.read_text() if path.exists() else ""


def _write(name: str, content: str) -> None:
    if not content.endswith("\n"):
        content += "\n"
    config.todo_file(name).write_text(content)


def list_items(name: str) -> list[Item]:
    items, section = [], ""
    for i, line in enumerate(_read(name).splitlines()):
        stripped = line.strip()
        if stripped.startswith("#"):
            section = stripped.lstrip("#").strip()
        elif (m := _ITEM_RE.match(line)) and not m.group(1):  # top-level items only
            items.append(Item(name, i, stripped[2:], section))
    return items


def raw_files() -> dict[str, str]:
    return {name: _read(name) for name in FILES}


def add_todo(text: str, section: str | None = None, to_today: bool = False) -> str:
    """Add an item to todo.md (or today.md), under `section` if it exists."""
    name = "today" if to_today else "todo"
    content = _read(name)
    lines = content.splitlines()
    entry = f"- [ ] {text}" if not text.startswith("[") else f"- {text}"

    insert_at = None
    if section:
        for i, line in enumerate(lines):
            if line.strip().startswith("##") and line.lstrip("#").strip().lower() == section.lower():
                insert_at = i + 1
                while insert_at < len(lines) and lines[insert_at].strip():
                    insert_at += 1
                break
    if insert_at is None:
        lines.append("") if lines and lines[-1].strip() else None
        lines.append(entry)
    else:
        lines.insert(insert_at, entry)
    _write(name, "\n".join(lines))
    return f"added to {name}.md" + (f" under '{section}'" if insert_at else "")


def _remove_line(name: str, line_no: int) -> str | None:
    lines = _read(name).splitlines()
    if line_no >= len(lines):
        return None
    line = lines.pop(line_no)
    _write(name, "\n".join(lines) if lines else "")
    stripped = line.strip()
    return stripped[2:] if stripped.startswith("- ") else stripped


def _insert_done(entry_text: str) -> None:
    """Insert under today's date header in done.md, creating it at the top."""
    today = datetime.date.today().isoformat()
    header = f"## {today}"
    lines = _read("done").splitlines()
    entry = f"- {entry_text}"

    for i, line in enumerate(lines):
        if line.strip() == header:
            j = i + 1
            while j < len(lines) and lines[j].strip() and not lines[j].startswith("## "):
                j += 1
            lines.insert(j, entry)
            _write("done", "\n".join(lines))
            return

    # No header for today yet: new day block goes above the previous newest day
    first_day = next((i for i, l in enumerate(lines) if l.startswith("## ")), len(lines))
    lines[first_day:first_day] = [header, entry, ""]
    _write("done", "\n".join(lines))


def remove_item(file: str, line_no: int) -> str:
    """Delete an item line outright (no move to done). Obsidian Sync's version
    history is the safety net."""
    if file not in FILES:
        raise ValueError(f"unknown file {file}")
    if not any(i.line_no == line_no for i in list_items(file)):
        raise ValueError("no item at that line")
    text = _remove_line(file, line_no)
    return f"removed from {file}.md: {text}"


def move_item(file: str, line_no: int, direction: str) -> str:
    """Move an item forward (todo→today→done) or back (done→today→todo)."""
    order = ["todo", "today", "done"]
    if file not in order:
        raise ValueError(f"unknown file {file}")
    step = 1 if direction == "forward" else -1
    target_idx = order.index(file) + step
    if not 0 <= target_idx < len(order):
        raise ValueError("cannot move further")
    target = order[target_idx]

    # Capture section for breadcrumb before removing
    section = next((i.section for i in list_items(file) if i.line_no == line_no), "")
    text = _remove_line(file, line_no)
    if text is None:
        raise ValueError("no item at that line")

    if target == "done":
        # strip checkbox, add breadcrumb like the desktop app
        clean = re.sub(r"^\[.\] ", "", text)
        entry = f"{clean} ({section})" if section else clean
        _insert_done(entry)
    else:
        if direction == "back":
            text = re.sub(r" \([^)]*\)$", "", text)  # strip breadcrumb
        if not text.startswith("["):
            text = f"[ ] {text}"
        content = _read(target)
        lines = content.splitlines()
        # insert at first gap after the title, like insert_at_first_gap
        pos = next(
            (i for i, l in enumerate(lines[1:], start=1) if not l.strip()),
            len(lines),
        )
        lines.insert(pos, f"- {text}")
        _write(target, "\n".join(lines))
    return f"moved to {target}.md"
