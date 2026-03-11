"""Stream-JSON line parsing utilities: extract human-readable text and rich events."""

from __future__ import annotations

import json


def _summarize_tool_call(name: str, inp: dict) -> str:
    """One-line summary of a tool call."""
    if name in ("Read", "read"):
        return inp.get("file_path", inp.get("path", "?"))
    if name in ("Write", "write"):
        return inp.get("file_path", inp.get("path", "?"))
    if name in ("Edit", "StrReplace", "str_replace"):
        return inp.get("file_path", inp.get("path", "?"))
    if name in ("Bash", "bash", "Shell", "shell"):
        cmd = inp.get("command", inp.get("cmd", "?"))
        return cmd[:120] + "…" if len(cmd) > 120 else cmd
    if name in ("Grep", "grep"):
        pat = inp.get("pattern", "?")
        path = inp.get("path", inp.get("target_directory", "."))
        return f'"{pat}" in {path}'
    if name in ("Glob", "glob"):
        return inp.get("pattern", inp.get("glob_pattern", "?"))
    if name in ("TodoWrite",):
        return "(update todos)"
    if name in ("Task",):
        return inp.get("description", inp.get("prompt", "?"))[:80]
    return json.dumps(inp)[:120]


def _tool_icon(name: str) -> str:
    """Return a fitting icon for a tool name."""
    icons = {
        "Read": "📖", "Write": "✏️", "Edit": "✏️", "StrReplace": "✏️",
        "Bash": "⚡", "Shell": "⚡", "Grep": "🔍", "Glob": "🔍",
        "TodoWrite": "📋", "Task": "🤖", "WebSearch": "🌐", "WebFetch": "🌐",
        "Delete": "🗑️",
    }
    for key, icon in icons.items():
        if name.lower().startswith(key.lower()):
            return icon
    return "⏺"


def _parse_stream_line_rich(raw_line: str) -> list[dict]:
    """Parse a stream-json line into a list of rich structured event dicts.

    Each event dict has a ``type`` key. Types:
      text        — assistant prose  {"type":"text","text":"..."}
      thinking    — thinking block   {"type":"thinking","text":"..."}
      tool_use    — tool call        {"type":"tool_use","name":"Read","summary":"...","icon":"📖"}
      tool_result — tool output      {"type":"tool_result","text":"...","is_error":False}
      result      — final outcome    {"type":"result","subtype":"success","text":"...","cost":0.12}
    """
    raw_line = raw_line.strip()
    if not raw_line:
        return []
    try:
        msg = json.loads(raw_line)
    except json.JSONDecodeError:
        return [{"type": "text", "text": raw_line}]

    msg_type = msg.get("type", "")

    # ── Full assistant turn (batched) ──────────────────────────────────────
    if msg_type == "assistant" and "message" in msg:
        events = []
        for block in msg["message"].get("content", []):
            if not isinstance(block, dict):
                continue
            btype = block.get("type", "")
            if btype == "text":
                text = block.get("text", "").strip()
                if text:
                    events.append({"type": "text", "text": text})
            elif btype == "thinking":
                text = block.get("thinking", "").strip()
                if text:
                    events.append({"type": "thinking", "text": text})
            elif btype == "tool_use":
                name = block.get("name", "?")
                inp = block.get("input", {})
                events.append({
                    "type": "tool_use",
                    "name": name,
                    "summary": _summarize_tool_call(name, inp),
                    "icon": _tool_icon(name),
                })
        return events

    # ── Tool results (user turn) ───────────────────────────────────────────
    if msg_type == "user":
        events = []
        content = msg.get("message", {}).get("content", [])
        if isinstance(content, list):
            for block in content:
                if not isinstance(block, dict):
                    continue
                if block.get("type") == "tool_result":
                    is_error = block.get("is_error", False)
                    raw = block.get("content", "")
                    if isinstance(raw, list):
                        raw = "\n".join(b.get("text", "") for b in raw if isinstance(b, dict))
                    if raw:
                        truncated = raw[:400] + "\n…" if len(raw) > 400 else raw
                        events.append({
                            "type": "tool_result",
                            "text": truncated,
                            "is_error": is_error,
                        })
        return events

    # ── Streaming text delta ───────────────────────────────────────────────
    if msg_type == "content_block_delta":
        delta = msg.get("delta", {})
        if delta.get("type") == "text_delta":
            text = delta.get("text", "")
            if text:
                return [{"type": "text_delta", "text": text}]
        if delta.get("type") == "thinking_delta":
            text = delta.get("thinking", "")
            if text:
                return [{"type": "thinking_delta", "text": text}]
        return []

    # ── Streaming tool start ───────────────────────────────────────────────
    if msg_type == "content_block_start":
        block = msg.get("content_block", {})
        if block.get("type") == "tool_use":
            name = block.get("name", "?")
            return [{"type": "tool_start", "name": name, "icon": _tool_icon(name)}]
        return []

    # ── Final result ───────────────────────────────────────────────────────
    if msg_type == "result":
        subtype = msg.get("subtype", "")
        result_text = msg.get("result", "") or subtype
        cost = msg.get("cost_usd")
        usage = msg.get("usage", {})
        return [{"type": "result", "subtype": subtype, "text": result_text,
                 "cost": cost, "usage": usage}]

    return []


def _extract_text_from_stream_line(raw_line: str) -> str | None:
    """Extract human-readable text from a single stream-json line (legacy plain-text path)."""
    events = _parse_stream_line_rich(raw_line)
    parts = []
    for ev in events:
        t = ev.get("type", "")
        if t == "text":
            parts.append(ev["text"])
        elif t == "tool_use":
            parts.append(f"\n→ {ev['name']}: {ev['summary']}")
        elif t == "tool_result":
            truncated = ev["text"]
            parts.append(f"  ← {truncated}")
        elif t == "result":
            parts.append(f"\n--- RESULT ---\n{ev['text']}")
    return "\n".join(parts) if parts else None
