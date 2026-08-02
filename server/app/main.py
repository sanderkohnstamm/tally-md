"""Tally assistant server."""

import asyncio
import datetime
import json
import logging
from pathlib import Path

from fastapi import FastAPI, Request
from fastapi.responses import HTMLResponse, StreamingResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates

from . import calendar_sync, llm, notes, todos
from .config import config
from .db import get_db, init_db

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(name)s %(message)s")

BASE = Path(__file__).parent.parent
app = FastAPI(title="tally")
app.mount("/static", StaticFiles(directory=BASE / "static"), name="static")
templates = Jinja2Templates(directory=BASE / "templates")


def _mditem(text: str):
    """Minimal inline markdown for todo items: checkbox stripped, bold + code rendered."""
    import re as _re

    from markupsafe import Markup, escape

    text = _re.sub(r"^\[.\] ", "", text)
    s = str(escape(text))
    s = _re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", s)
    s = _re.sub(r"`([^`]+)`", r"<code>\1</code>", s)
    s = _re.sub(r"\*(.+?)\*", r"<em>\1</em>", s)
    return Markup(s)


templates.env.filters["mditem"] = _mditem


@app.on_event("startup")
async def startup() -> None:
    init_db()
    notes.refresh_index(force=True)
    asyncio.create_task(calendar_sync.poll_loop())


# ---------- pages ----------

def _fmt_event(e: dict) -> dict:
    if e["all_day"]:
        e["time"] = "all day"
    else:
        try:
            dt = datetime.datetime.fromisoformat(e["start_utc"]).astimezone()
            e["time"] = dt.strftime("%H:%M")
        except ValueError:
            e["time"] = ""
    return e


def _recent_captures(limit: int = 5) -> list[dict]:
    with get_db() as conn:
        rows = conn.execute(
            "SELECT * FROM captures ORDER BY id DESC LIMIT ?", (limit,)
        ).fetchall()
    return [dict(r) for r in rows]


@app.get("/", response_class=HTMLResponse)
async def today_page(request: Request):
    events = [_fmt_event(e) for e in calendar_sync.get_agenda(days=1)]
    return templates.TemplateResponse(request, "today.html", {
        "tab": "today",
        "events": events,
        "today_items": todos.list_items("today"),
        "date": datetime.date.today().strftime("%A %d %B"),
        "captures": _recent_captures(),
    })


@app.get("/chat", response_class=HTMLResponse)
async def chat_page(request: Request):
    with get_db() as conn:
        history = [dict(r) for r in conn.execute(
            "SELECT * FROM chat_messages ORDER BY id DESC LIMIT 40"
        ).fetchall()][::-1]
    return templates.TemplateResponse(request, "chat.html", {"tab": "chat", "history": history})


@app.get("/todos", response_class=HTMLResponse)
async def todos_page(request: Request):
    return templates.TemplateResponse(request, "todos.html", {
        "tab": "todos",
        "panes": {name: todos.list_items(name) for name in todos.FILES},
    })


# ---------- capture ----------

@app.post("/api/capture", response_class=HTMLResponse)
async def capture(request: Request):
    """Quick capture: plain text in (form, JSON, or raw body — Shortcut-friendly),
    Claude classifies and files it, rendered capture card out."""
    content_type = request.headers.get("content-type", "")
    if "json" in content_type:
        text = (await request.json()).get("text", "")
    elif "form" in content_type:
        form = await request.form()
        text = str(form.get("text", ""))
    else:
        text = (await request.body()).decode(errors="replace")
    text = text.strip()
    if not text:
        return HTMLResponse("", status_code=422)

    result = await llm.handle_capture(text)
    with get_db() as conn:
        cur = conn.execute(
            "INSERT INTO captures (text, response, actions) VALUES (?, ?, ?)",
            (text, result["text"], json.dumps(result["actions"])),
        )
        capture_id = cur.lastrowid
    return templates.TemplateResponse(request, "partials/capture.html", {
        "c": {"id": capture_id, "text": text, "response": result["text"]},
    })


# ---------- chat ----------

@app.post("/api/chat")
async def chat(request: Request):
    body = await request.json()
    user_text = body.get("message", "").strip()
    if not user_text:
        return {"error": "empty"}

    with get_db() as conn:
        rows = [dict(r) for r in conn.execute(
            "SELECT role, content FROM chat_messages ORDER BY id DESC LIMIT 20"
        ).fetchall()][::-1]
        conn.execute(
            "INSERT INTO chat_messages (role, content) VALUES ('user', ?)", (user_text,)
        )
    messages = [{"role": r["role"], "content": r["content"]} for r in rows]
    messages.append({"role": "user", "content": user_text})

    async def stream():
        async for event in llm.agent_events(messages):
            if event["type"] == "done":
                with get_db() as conn:
                    conn.execute(
                        "INSERT INTO chat_messages (role, content) VALUES ('assistant', ?)",
                        (event["text"],),
                    )
                payload = {"type": "done"}
            else:
                payload = event
            yield f"data: {json.dumps(payload)}\n\n"

    return StreamingResponse(stream(), media_type="text/event-stream")


# ---------- todos api ----------

@app.post("/api/todos/move", response_class=HTMLResponse)
async def move_todo(request: Request):
    form = await request.form()
    try:
        todos.move_item(str(form["file"]), int(str(form["line_no"])), str(form["direction"]))
    except ValueError:
        pass  # stale line number (concurrent edit) — just re-render
    referer = request.headers.get("referer", "")
    if referer.endswith("/"):
        return await today_page(request)
    return await todos_page(request)


@app.get("/health")
async def health():
    return {"ok": True, "vault": config.vault_path.exists()}
