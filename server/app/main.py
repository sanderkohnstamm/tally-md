"""Tally assistant server."""

import asyncio
import datetime
import json
import logging
from pathlib import Path

from fastapi import FastAPI, Request
from fastapi.responses import HTMLResponse, RedirectResponse, StreamingResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates

from . import calendar_sync, llm, notes, settings, todos
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
    settings.apply_to_config()
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


# ---------- settings ----------

GOOGLE_SCOPES = ["https://www.googleapis.com/auth/calendar.readonly"]
_oauth_states: set[str] = set()


def _base_url(request: Request) -> str:
    """External base URL. Behind tailscale serve the app sees plain http on
    127.0.0.1, so trust the Host header and force https except for localhost."""
    host = request.headers.get("host", "localhost:8321")
    scheme = "http" if host.split(":")[0] in ("localhost", "127.0.0.1") else "https"
    return f"{scheme}://{host}"


async def _settings_ctx(request: Request, status: dict | None = None) -> dict:
    return {
        "tab": "settings",
        "masked_key": settings.masked_key(),
        "model": config.model,
        "vault_ok": config.vault_path.exists(),
        "vault_path": str(config.vault_path),
        "sync": await asyncio.to_thread(settings.obsidian_sync_status),
        "google": settings.google_status(),
        "icloud_set": bool(config.icloud_username and config.icloud_app_password),
        "redirect_uri": f"{_base_url(request)}/oauth/google/callback",
        "folders": settings.vault_folders(),
        "focus_dir": config.focus_dir,
        "status": status,
    }


@app.get("/settings", response_class=HTMLResponse)
async def settings_page(request: Request):
    return templates.TemplateResponse(request, "settings.html", await _settings_ctx(request))


@app.post("/settings", response_class=HTMLResponse)
async def save_settings(request: Request):
    form = await request.form()
    values = {k: str(form.get(k, "")).strip() for k in settings.SECRET_FIELDS}
    values["focus_dir"] = str(form.get("focus_dir", "")).strip()
    settings.save_secrets(values)
    ok, message = await llm.test_connection()
    return templates.TemplateResponse(
        request, "settings.html",
        await _settings_ctx(request, status={"ok": ok, "message": message}),
    )


@app.get("/oauth/google/start")
async def google_oauth_start(request: Request):
    from google_auth_oauthlib.flow import Flow

    client_id, client_secret = settings.google_client()
    if not (client_id and client_secret):
        return RedirectResponse("/settings", status_code=303)
    flow = Flow.from_client_config(
        {"web": {
            "client_id": client_id,
            "client_secret": client_secret,
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
        }},
        scopes=GOOGLE_SCOPES,
        redirect_uri=f"{_base_url(request)}/oauth/google/callback",
    )
    auth_url, state = flow.authorization_url(access_type="offline", prompt="consent")
    _oauth_states.add(state)
    return RedirectResponse(auth_url, status_code=303)


@app.get("/oauth/google/callback")
async def google_oauth_callback(request: Request):
    from google_auth_oauthlib.flow import Flow

    state = request.query_params.get("state", "")
    if state not in _oauth_states:
        return HTMLResponse("stale oauth state — go back to settings and retry", status_code=400)
    _oauth_states.discard(state)

    client_id, client_secret = settings.google_client()
    flow = Flow.from_client_config(
        {"web": {
            "client_id": client_id,
            "client_secret": client_secret,
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
        }},
        scopes=GOOGLE_SCOPES,
        state=state,
        redirect_uri=f"{_base_url(request)}/oauth/google/callback",
    )
    # Behind the proxy the incoming URL is http; oauthlib insists on https
    response_url = str(request.url).replace("http://", "https://", 1) \
        if _base_url(request).startswith("https") else str(request.url)
    flow.fetch_token(authorization_response=response_url)
    config.google_token.parent.mkdir(parents=True, exist_ok=True)
    config.google_token.write_text(flow.credentials.to_json())
    config.google_token.chmod(0o600)
    await asyncio.to_thread(calendar_sync.sync_google)
    return RedirectResponse("/settings", status_code=303)


@app.get("/health")
async def health():
    return {"ok": True, "vault": config.vault_path.exists(), "key": bool(config.anthropic_api_key)}
