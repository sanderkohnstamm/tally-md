"""Claude agent loop: tools over the vault, todos, and agenda.

One code path serves both chat (streamed) and voice-note classification.
"""

import datetime
import json
import logging
from collections.abc import AsyncIterator

import anthropic

from . import calendar_sync, notes, todos
from .config import config

log = logging.getLogger("tally.llm")

TOOLS = [
    {
        "name": "search_notes",
        "description": "Full-text search over the markdown vault. Returns paths and snippets. "
                       "Issue multiple targeted queries rather than one broad one.",
        "input_schema": {
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": ["query"],
        },
    },
    {
        "name": "read_note",
        "description": "Read a note by vault-relative path (as returned by search_notes).",
        "input_schema": {
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"],
        },
    },
    {
        "name": "append_note",
        "description": "Append text to a vault note (created if missing). Use for capturing "
                       "thoughts/notes. Default to the inbox note unless the content clearly "
                       "belongs to an existing note.",
        "input_schema": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Vault-relative path, e.g. 'Inbox.md'"},
                "text": {"type": "string"},
                "heading": {"type": "string", "description": "Optional heading for the block"},
            },
            "required": ["path", "text"],
        },
    },
    {
        "name": "list_todos",
        "description": "List items from todo.md, today.md and done.md with line numbers and sections.",
        "input_schema": {"type": "object", "properties": {}},
    },
    {
        "name": "add_todo",
        "description": "Add a todo item. Set to_today=true only for things that must happen today.",
        "input_schema": {
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Item text, markdown allowed, no leading '- [ ]'"},
                "section": {"type": "string", "description": "Existing section heading to file under"},
                "to_today": {"type": "boolean"},
            },
            "required": ["text"],
        },
    },
    {
        "name": "move_todo",
        "description": "Move an item forward (todo→today→done) or back. Use list_todos first "
                       "to get the file and line number.",
        "input_schema": {
            "type": "object",
            "properties": {
                "file": {"type": "string", "enum": ["todo", "today", "done"]},
                "line_no": {"type": "integer"},
                "direction": {"type": "string", "enum": ["forward", "back"]},
            },
            "required": ["file", "line_no", "direction"],
        },
    },
    {
        "name": "get_agenda",
        "description": "Get upcoming calendar events (Google + iCloud, read-only).",
        "input_schema": {
            "type": "object",
            "properties": {"days": {"type": "integer", "description": "Days ahead, default 1"}},
        },
    },
]

# Tools that change state; voice classification reports these as its outcome.
WRITE_TOOLS = {"append_note", "add_todo", "move_todo"}


def run_tool(name: str, args: dict) -> str:
    try:
        if name == "search_notes":
            return json.dumps(notes.search_notes(args["query"]))
        if name == "read_note":
            return notes.read_note(args["path"])
        if name == "append_note":
            rel = notes.append_note(args["path"], args["text"], args.get("heading"))
            return f"appended to {rel}"
        if name == "list_todos":
            return json.dumps({
                f: [{"line": i.line_no, "text": i.text, "section": i.section}
                    for i in todos.list_items(f)]
                for f in todos.FILES
            })
        if name == "add_todo":
            return todos.add_todo(args["text"], args.get("section"), args.get("to_today", False))
        if name == "move_todo":
            return todos.move_item(args["file"], args["line_no"], args["direction"])
        if name == "get_agenda":
            return json.dumps(calendar_sync.get_agenda(args.get("days", 1)))
        return f"unknown tool {name}"
    except Exception as exc:
        log.exception("tool %s failed", name)
        return f"error: {exc}"


def system_prompt() -> list[dict]:
    today = datetime.date.today()
    focus = (
        f"Primary working area: the vault's `{config.focus_dir}/` folder — search results "
        "from it rank first, and notes/todos belong there unless clearly personal.\n\n"
        if config.focus_dir else ""
    )
    header = (
        "You are Tally, Sander's personal assistant, running on his Raspberry Pi and "
        "operating directly on his Obsidian vault (the markdown files are the source of "
        f"truth). Today is {today:%A} {today.isoformat()}.\n\n" + focus +
        "Style: brief and direct — this is read on a phone. No headers or bullet-point "
        "essays unless asked. When you act (add a todo, append a note), confirm in one line "
        "what you did and where it went.\n\n"
        "Todos follow the tally flow: todo.md (backlog, sectioned) → today.md (today's plan) "
        "→ done.md (dated log). File new todos into the most fitting existing section.\n\n"
        "The vault's own conventions follow — respect them (e.g. Linear/Notion read-only "
        "posture applies to you too):\n\n"
    )
    return [{
        "type": "text",
        "text": header + notes.standing_context(),
        "cache_control": {"type": "ephemeral"},
    }]


def _client() -> anthropic.AsyncAnthropic:
    return anthropic.AsyncAnthropic(api_key=config.anthropic_api_key)


async def test_connection() -> tuple[bool, str]:
    """Cheap 1-token ping so the settings page can confirm the key works."""
    if not config.anthropic_api_key:
        return False, "no API key set"
    try:
        await _client().messages.create(
            model=config.model,
            max_tokens=1,
            messages=[{"role": "user", "content": "hi"}],
        )
        return True, f"connected · {config.model}"
    except anthropic.AuthenticationError:
        return False, "key rejected (authentication failed)"
    except anthropic.NotFoundError:
        return False, f"key works but model '{config.model}' not available to this account"
    except anthropic.PermissionDeniedError:
        return False, "key valid but lacks permission (check org / billing)"
    except Exception as exc:  # network, rate limit, no credits, …
        return False, f"{type(exc).__name__}: {str(exc)[:120]}"


async def agent_events(messages: list[dict], max_turns: int = 12) -> AsyncIterator[dict]:
    """Run the agent loop, yielding events:
    {"type": "text_delta", "text"} | {"type": "tool", "name", "args"} | {"type": "done", "text", "actions"}
    """
    client = _client()
    full_text_parts: list[str] = []
    actions: list[dict] = []

    for _ in range(max_turns):
        content_blocks = []
        async with client.messages.stream(
            model=config.model,
            max_tokens=2048,
            system=system_prompt(),
            tools=TOOLS,
            messages=messages,
        ) as stream:
            async for event in stream:
                if event.type == "content_block_delta" and event.delta.type == "text_delta":
                    full_text_parts.append(event.delta.text)
                    yield {"type": "text_delta", "text": event.delta.text}
            response = await stream.get_final_message()
            content_blocks = response.content

        tool_uses = [b for b in content_blocks if b.type == "tool_use"]
        if not tool_uses:
            break

        messages = messages + [{"role": "assistant", "content": content_blocks}]
        results = []
        for tu in tool_uses:
            yield {"type": "tool", "name": tu.name, "args": tu.input}
            output = run_tool(tu.name, tu.input)
            if tu.name in WRITE_TOOLS:
                actions.append({"tool": tu.name, "args": tu.input, "result": output})
            results.append({
                "type": "tool_result",
                "tool_use_id": tu.id,
                "content": output,
            })
        messages = messages + [{"role": "user", "content": results}]

    yield {"type": "done", "text": "".join(full_text_parts), "actions": actions}


CAPTURE_INSTRUCTION = (
    "The following is a quick capture, often dictated on a phone (expect dictation noise; "
    "infer intent). Decide what it is and act:\n"
    "- a task → add_todo (pick a fitting section; to_today only if it must happen today)\n"
    "- a thought/note/idea → append_note to the inbox note ('{inbox}') with a short heading, "
    "unless it clearly extends a specific existing note\n"
    "- a question or request → just answer it (search the vault if useful)\n"
    "- completing something mentioned → list_todos then move_todo\n"
    "If it's genuinely ambiguous, don't write anything — say what you'd do and ask.\n\n"
    "Capture:\n{text}"
)


async def handle_capture(text: str) -> dict:
    """Classify + act on a quick capture. Returns {'text','actions'}."""
    prompt = CAPTURE_INSTRUCTION.format(inbox=config.inbox_note, text=text)
    final = {}
    async for event in agent_events([{"role": "user", "content": prompt}]):
        if event["type"] == "done":
            final = event
    return {"text": final.get("text", ""), "actions": final.get("actions", [])}
