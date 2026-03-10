"""SQLite state management for orchestration runs."""

from __future__ import annotations

import json
import sqlite3
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

_SCHEMA = """
CREATE TABLE IF NOT EXISTS segments (
    num         INTEGER PRIMARY KEY,
    slug        TEXT NOT NULL,
    title       TEXT NOT NULL,
    wave        INTEGER NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    attempts    INTEGER NOT NULL DEFAULT 0,
    started_at  REAL,
    finished_at REAL,
    result_json TEXT
);

CREATE TABLE IF NOT EXISTS events (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    ts        REAL NOT NULL,
    kind      TEXT NOT NULL,
    detail    TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS run_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"""


@dataclass
class SegmentRow:
    num: int
    slug: str
    title: str
    wave: int
    status: str = "pending"
    attempts: int = 0
    started_at: float | None = None
    finished_at: float | None = None
    result_json: str | None = None

    def to_dict(self) -> dict[str, Any]:
        d = asdict(self)
        if self.result_json:
            try:
                d["result"] = json.loads(self.result_json)
            except json.JSONDecodeError:
                d["result"] = self.result_json
        return d


class StateDB:
    """Thread-safe, WAL-mode SQLite state store."""

    def __init__(self, db_path: Path):
        self._path = db_path
        self._conn = sqlite3.connect(
            str(db_path),
            isolation_level="DEFERRED",
            check_same_thread=False,
        )
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._conn.execute("PRAGMA busy_timeout=5000")
        self._conn.executescript(_SCHEMA)
        self._conn.commit()

    def close(self) -> None:
        self._conn.close()

    # ── Segment CRUD ──

    def init_segments(self, segments: list) -> None:
        """Populate segment rows from planner Segment objects."""
        with self._conn:
            for seg in segments:
                self._conn.execute(
                    """INSERT OR IGNORE INTO segments
                       (num, slug, title, wave, status)
                       VALUES (?, ?, ?, ?, ?)""",
                    (seg.num, seg.slug, seg.title, seg.wave, "pending"),
                )

    def get_segment(self, num: int) -> SegmentRow | None:
        cur = self._conn.execute("SELECT * FROM segments WHERE num=?", (num,))
        row = cur.fetchone()
        if not row:
            return None
        return SegmentRow(*row)

    def get_all_segments(self) -> list[SegmentRow]:
        cur = self._conn.execute("SELECT * FROM segments ORDER BY num")
        return [SegmentRow(*r) for r in cur.fetchall()]

    def set_status(self, num: int, status: str, **kwargs: Any) -> None:
        sets = ["status=?"]
        vals: list[Any] = [status]
        for k, v in kwargs.items():
            sets.append(f"{k}=?")
            vals.append(v)
        vals.append(num)
        with self._conn:
            self._conn.execute(
                f"UPDATE segments SET {', '.join(sets)} WHERE num=?", vals
            )

    def increment_attempts(self, num: int) -> int:
        with self._conn:
            self._conn.execute(
                "UPDATE segments SET attempts = attempts + 1 WHERE num=?", (num,)
            )
        row = self.get_segment(num)
        return row.attempts if row else 0

    # ── Events ──

    def log_event(self, kind: str, detail: str = "") -> None:
        with self._conn:
            self._conn.execute(
                "INSERT INTO events (ts, kind, detail) VALUES (?, ?, ?)",
                (time.time(), kind, detail),
            )

    def get_events(self, limit: int = 100, after_id: int = 0) -> list[dict]:
        cur = self._conn.execute(
            "SELECT id, ts, kind, detail FROM events WHERE id > ? ORDER BY id DESC LIMIT ?",
            (after_id, limit),
        )
        return [
            {"id": r[0], "ts": r[1], "kind": r[2], "detail": r[3]}
            for r in cur.fetchall()
        ]

    # ── Run metadata ──

    def set_meta(self, key: str, value: str) -> None:
        with self._conn:
            self._conn.execute(
                "INSERT OR REPLACE INTO run_meta (key, value) VALUES (?, ?)",
                (key, value),
            )

    def get_meta(self, key: str) -> str | None:
        cur = self._conn.execute(
            "SELECT value FROM run_meta WHERE key=?", (key,)
        )
        row = cur.fetchone()
        return row[0] if row else None

    # ── Progress queries ──

    def progress(self) -> dict[str, int]:
        """Return counts by status."""
        cur = self._conn.execute(
            "SELECT status, COUNT(*) FROM segments GROUP BY status"
        )
        return dict(cur.fetchall())

    def wave_segments(self, wave: int) -> list[SegmentRow]:
        cur = self._conn.execute(
            "SELECT * FROM segments WHERE wave=? ORDER BY num", (wave,)
        )
        return [SegmentRow(*r) for r in cur.fetchall()]

    def max_wave(self) -> int:
        cur = self._conn.execute("SELECT MAX(wave) FROM segments")
        row = cur.fetchone()
        return row[0] if row and row[0] else 0

    def reset_stale_running(self) -> int:
        """Reset segments stuck as 'running' from a previous crashed run."""
        with self._conn:
            cur = self._conn.execute(
                "UPDATE segments SET status='pending' WHERE status='running'"
            )
            count = cur.rowcount
        if count:
            self.log_event("reset_stale", f"Reset {count} stale running segments to pending")
        return count

    def migrate_from_json(self, json_path: Path) -> int:
        """Import segment statuses from the old bash script's execution-state.json.

        Returns the number of segments updated.
        """
        if not json_path.exists():
            return 0
        with open(json_path, encoding="utf-8") as f:
            data = json.load(f)

        count = 0
        for key, info in data.get("segments", {}).items():
            # Keys are like "S01", "S02", etc.
            try:
                num = int(key.lstrip("S"))
            except (ValueError, IndexError):
                continue
            old_status = info.get("status", "pending")
            # Map old statuses: "running" from a dead bash session → "pending" for retry
            if old_status == "running":
                old_status = "pending"
            attempts = info.get("attempts", 0)
            completed = info.get("completed")

            finished_at = None
            if completed:
                from datetime import datetime, timezone
                try:
                    dt = datetime.fromisoformat(completed.replace("Z", "+00:00"))
                    finished_at = dt.timestamp()
                except (ValueError, AttributeError):
                    pass

            existing = self.get_segment(num)
            if existing and existing.status == "pending" and old_status in ("pass", "partial", "blocked"):
                self.set_status(
                    num, old_status,
                    attempts=attempts,
                    finished_at=finished_at,
                )
                count += 1
        if count:
            self.log_event("state_migrated", f"Imported {count} segment statuses from {json_path.name}")
        return count

    def all_as_dict(self) -> dict[str, Any]:
        """Full snapshot for the monitor API."""
        segments = [s.to_dict() for s in self.get_all_segments()]
        return {
            "plan_title": self.get_meta("plan_title") or "",
            "plan_goal": self.get_meta("plan_goal") or "",
            "started_at": self.get_meta("started_at") or "",
            "current_wave": self.get_meta("current_wave") or "0",
            "max_wave": self.max_wave(),
            "progress": self.progress(),
            "segments": segments,
            "events": self.get_events(limit=50),
        }
