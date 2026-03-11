"""Stream-JSON line parsing utilities: extract human-readable text."""

from __future__ import annotations

import json


def _summarize_tool_call(name: str, inp: dict) -> str:
    """One-line summary of a tool call."""
    if name in ("Read", "read"):
        return inp.get("file_path", inp.get("path", "?"))
    if name in ("Write", "write", "Edit", "edit"):
        path = inp.get("file_path", inp.get("path", "?"))
        return path
    if name in ("Bash", "bash"):
        cmd = inp.get("command", "?")
        return cmd[:120] + "..." if len(cmd) > 120 else cmd
    if name in ("Grep", "grep"):
        return f'"{inp.get("pattern", "?")}" in {inp.get("path", ".")}'
    if name in ("Glob", "glob"):
        return inp.get("pattern", inp.get("glob_pattern", "?"))
    if name in ("TodoWrite",):
        return "(update todos)"
    return json.dumps(inp)[:120]


def _extract_text_from_stream_line(raw_line: str) -> str | None:
    """Extract human-readable text from a single stream-json line."""
    raw_line = raw_line.strip()
    if not raw_line:
        return None
    try:
        msg = json.loads(raw_line)
    except json.JSONDecodeError:
        return raw_line

    msg_type = msg.get("type", "")

    if msg_type == "assistant" and "message" in msg:
        parts = []
        for block in msg["message"].get("content", []):
            if not isinstance(block, dict):
                continue
            btype = block.get("type", "")
            if btype == "text":
                parts.append(block["text"])
            elif btype == "tool_use":
                name = block.get("name", "?")
                inp = block.get("input", {})
                summary = _summarize_tool_call(name, inp)
                parts.append(f"\n→ {name}: {summary}")
        return "\n".join(parts) if parts else None

    if msg_type == "user":
        content = msg.get("message", {}).get("content", [])
        if isinstance(content, list):
            parts = []
            for block in content:
                if isinstance(block, dict) and block.get("type") == "tool_result":
                    text = block.get("content", "")
                    if isinstance(text, list):
                        text = " ".join(
                            b.get("text", "") for b in text if isinstance(b, dict)
                        )
                    if text:
                        truncated = text[:500] + "..." if len(text) > 500 else text
                        parts.append(f"  ← {truncated}")
            return "\n".join(parts) if parts else None

    if msg_type == "content_block_delta":
        delta = msg.get("delta", {})
        if delta.get("type") == "text_delta":
            return delta.get("text", "")

    if msg_type == "result":
        result = msg.get("result", "")
        if result:
            return f"\n--- RESULT ---\n{result}"

    return None
