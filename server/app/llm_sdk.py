"""Claude subscription backend via the Claude Agent SDK.

Used when a `claude setup-token` OAuth token is configured (settings page or
CLAUDE_CODE_OAUTH_TOKEN) — i.e. login with a claude.ai account instead of a
console API key. Same tools and events as llm.py's API-key loop; the SDK
drives the local `claude` CLI, which owns the auth.

Chat continuity: the SDK keeps its own session transcripts; we persist the
last session id and resume it, so only the newest message is sent per turn.
"""

import json
import logging
from collections.abc import AsyncIterator

from claude_agent_sdk import (
    AssistantMessage,
    ClaudeAgentOptions,
    ClaudeSDKClient,
    ResultMessage,
    TextBlock,
    ToolUseBlock,
    create_sdk_mcp_server,
    query,
    tool,
)

from .config import config

log = logging.getLogger("tally.llm_sdk")

_SESSION_FILE = config.data_dir / "claude-session.json"


def _load_session_id(kind: str) -> str | None:
    try:
        return json.loads(_SESSION_FILE.read_text()).get(kind)
    except (OSError, json.JSONDecodeError):
        return None


def _save_session_id(kind: str, session_id: str) -> None:
    try:
        data = json.loads(_SESSION_FILE.read_text())
    except (OSError, json.JSONDecodeError):
        data = {}
    data[kind] = session_id
    _SESSION_FILE.write_text(json.dumps(data))


def _build_server(actions: list[dict]):
    """Fresh SDK MCP server per run; write-tool calls are recorded in `actions`."""
    from . import llm  # late import — llm.py imports us back

    def make(spec: dict):
        name = spec["name"]

        @tool(name, spec["description"], spec["input_schema"])
        async def handler(args: dict, _name: str = name):
            output = llm.run_tool(_name, args)
            if _name in llm.WRITE_TOOLS:
                actions.append({"tool": _name, "args": args, "result": output})
            return {"content": [{"type": "text", "text": output}]}

        return handler

    return create_sdk_mcp_server(
        name="tally", version="1.0.0", tools=[make(s) for s in llm.TOOLS]
    )


def _options(actions: list[dict], resume: str | None, max_turns: int) -> ClaudeAgentOptions:
    from . import llm

    tally_tools = [f"mcp__tally__{t['name']}" for t in llm.TOOLS]
    # The CLI defers MCP tools behind ToolSearch, so the model won't see them
    # in its context — without this inventory it assumes they don't exist
    # (e.g. "I don't have calendar access").
    tools_note = (
        "\n\nYour tally tools are MCP tools (deferred — load with ToolSearch "
        f"'select:...' before first use): {', '.join(tally_tools)}. "
        "mcp__tally__get_agenda is your calendar (Google/iCloud feeds) — use it for any "
        "scheduling question; never claim you lack calendar access. Use the todo tools "
        "for todo.md/today.md/done.md so the tally format stays intact; Read/Grep/Glob "
        "on the vault are fine for reading. Never edit vault files directly."
    )
    return ClaudeAgentOptions(
        model=config.model,
        cwd=str(config.vault_path),
        mcp_servers={"tally": _build_server(actions)},
        # Read-only vault access via built-ins; all writes go through tally tools.
        allowed_tools=["Read", "Grep", "Glob", *tally_tools],
        disallowed_tools=["Bash", "Write", "Edit", "NotebookEdit", "WebFetch", "WebSearch", "Task"],
        system_prompt={
            "type": "preset",
            "preset": "claude_code",
            "append": llm.system_prompt()[0]["text"] + tools_note,
        },
        setting_sources=[],
        include_partial_messages=True,
        max_turns=max_turns,
        resume=resume,
        env={"CLAUDE_CODE_OAUTH_TOKEN": config.claude_oauth_token},
    )


async def agent_events(
    prompt: str, session_kind: str | None = None, max_turns: int = 24
) -> AsyncIterator[dict]:
    """Same event shapes as llm.agent_events: text_delta / tool / done."""
    actions: list[dict] = []
    text_parts: list[str] = []
    resume = _load_session_id(session_kind) if session_kind else None

    async def run(resume_id: str | None):
        # ClaudeSDKClient (streaming mode) is required: in-process SDK MCP
        # servers — our tally tools — are not available via plain query().
        async with ClaudeSDKClient(options=_options(actions, resume_id, max_turns)) as client:
            await client.query(prompt)
            async for msg in client.receive_response():
                yield msg

    attempts = [resume, None] if resume else [None]
    for i, resume_id in enumerate(attempts):
        try:
            async for msg in run(resume_id):
                if type(msg).__name__ == "StreamEvent":
                    ev = getattr(msg, "event", {}) or {}
                    delta = ev.get("delta") or {}
                    if ev.get("type") == "content_block_delta" and delta.get("type") == "text_delta":
                        yield {"type": "text_delta", "text": delta.get("text", "")}
                elif isinstance(msg, AssistantMessage):
                    for block in msg.content:
                        if isinstance(block, TextBlock):
                            text_parts.append(block.text)
                        elif isinstance(block, ToolUseBlock):
                            yield {"type": "tool", "name": block.name, "args": block.input}
                elif isinstance(msg, ResultMessage):
                    if msg.session_id and session_kind:
                        _save_session_id(session_kind, msg.session_id)
            break
        except Exception:
            # A stale resume id is the common failure — retry once from scratch.
            if i + 1 < len(attempts):
                log.warning("resume %s failed, starting fresh session", resume_id)
                actions.clear()
                text_parts.clear()
                continue
            raise

    yield {"type": "done", "text": "".join(text_parts), "actions": actions}


async def test_connection() -> tuple[bool, str]:
    """One trivial turn through the CLI to confirm the login token works."""
    if not config.claude_oauth_token:
        return False, "no claude login token set"
    try:
        options = ClaudeAgentOptions(
            model=config.model,
            max_turns=1,
            allowed_tools=[],
            system_prompt="Reply with the single word: ok",
            setting_sources=[],
            env={"CLAUDE_CODE_OAUTH_TOKEN": config.claude_oauth_token},
        )
        async for msg in query(prompt="ping", options=options):
            if isinstance(msg, ResultMessage):
                if msg.is_error:
                    return False, f"login token rejected: {str(msg.result)[:120]}"
                return True, f"connected · {config.model} · claude subscription"
        return False, "no result from claude CLI"
    except Exception as exc:
        return False, f"{type(exc).__name__}: {str(exc)[:120]}"
