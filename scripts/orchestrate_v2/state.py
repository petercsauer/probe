"""SQLite state management for orchestration runs — async via aiosqlite."""

from __future__ import annotations

import json
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

import aiosqlite

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
    detail    TEXT NOT NULL DEFAULT '',
    severity  TEXT NOT NULL DEFAULT 'info'
);

CREATE TABLE IF NOT EXISTS run_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notifications (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at      REAL NOT NULL,
    event_key       TEXT NOT NULL UNIQUE,
    kind            TEXT NOT NULL,
    message         TEXT NOT NULL,
    priority        TEXT NOT NULL DEFAULT 'default',
    sent_at         REAL,
    attempts        INTEGER NOT NULL DEFAULT 0,
    last_attempt_at REAL,
    last_error      TEXT
);

CREATE TABLE IF NOT EXISTS segment_attempts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    seg_num     INTEGER NOT NULL,
    attempt     INTEGER NOT NULL,
    started_at  REAL,
    finished_at REAL,
    status      TEXT,
    summary     TEXT,
    tokens_in   INTEGER DEFAULT 0,
    tokens_out  INTEGER DEFAULT 0
);
"""

_MIGRATIONS = [
    "ALTER TABLE segments ADD COLUMN last_seen_at REAL",
    "ALTER TABLE segments ADD COLUMN last_activity TEXT",
    "ALTER TABLE segments ADD COLUMN per_segment_timeout INTEGER",
    "ALTER TABLE events ADD COLUMN severity TEXT DEFAULT 'info'",
]


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
    last_seen_at: float | None = None
    last_activity: str | None = None
    per_segment_timeout: int | None = None

    def to_dict(self) -> dict[str, Any]:
        d = asdict(self)
        if self.result_json:
            try:
                d["result"] = json.loads(self.result_json)
            except json.JSONDecodeError:
                d["result"] = self.result_json
        return d


class StateDB:
    """Async WAL-mode SQLite state store backed by aiosqlite."""

    @classmethod
    async def create(cls, db_path: Path) -> "StateDB":
        conn = await aiosqlite.connect(str(db_path))
        conn.row_factory = aiosqlite.Row
        # PRAGMAs are non-transactional — commit any implicit tx first, then set them
        await conn.commit()
        await conn.execute("PRAGMA journal_mode=WAL")
        await conn.execute("PRAGMA busy_timeout=5000")
        await conn.execute("PRAGMA synchronous=NORMAL")
        # Create schema — split by statement to avoid executescript's implicit COMMIT/BEGIN
        for stmt in _SCHEMA.split(";"):
            stmt = stmt.strip()
            if stmt:
                await conn.execute(stmt)
        await conn.commit()
        # Idempotent migrations (ignore "column already exists" errors)
        for sql in _MIGRATIONS:
            try:
                await conn.execute(sql)
                await conn.commit()
            except Exception:
                await conn.rollback()
        obj = object.__new__(cls)
        obj._conn = conn
        obj._path = db_path
        return obj

    async def close(self) -> None:
        await self._conn.close()

    # ── Segment CRUD ──

    async def init_segments(self, segments: list) -> None:
        """Populate segment rows from planner Segment objects."""
        for seg in segments:
            await self._conn.execute(
                """INSERT OR IGNORE INTO segments
                   (num, slug, title, wave, status)
                   VALUES (?, ?, ?, ?, ?)""",
                (seg.num, seg.slug, seg.title, seg.wave, "pending"),
            )
        await self._conn.commit()

    async def get_segment(self, num: int) -> SegmentRow | None:
        cur = await self._conn.execute(
            "SELECT num, slug, title, wave, status, attempts, started_at, finished_at,"
            " result_json, last_seen_at, last_activity, per_segment_timeout"
            " FROM segments WHERE num=?",
            (num,),
        )
        row = await cur.fetchone()
        if not row:
            return None
        return SegmentRow(*tuple(row))

    async def get_all_segments(self) -> list[SegmentRow]:
        cur = await self._conn.execute(
            "SELECT num, slug, title, wave, status, attempts, started_at, finished_at,"
            " result_json, last_seen_at, last_activity, per_segment_timeout"
            " FROM segments ORDER BY num"
        )
        return [SegmentRow(*tuple(r)) for r in await cur.fetchall()]

    async def set_status(self, num: int, status: str, **kwargs: Any) -> None:
        sets = ["status=?"]
        vals: list[Any] = [status]
        for k, v in kwargs.items():
            sets.append(f"{k}=?")
            vals.append(v)
        vals.append(num)
        await self._conn.execute(
            f"UPDATE segments SET {', '.join(sets)} WHERE num=?", vals
        )
        await self._conn.commit()

    async def increment_attempts(self, num: int) -> int:
        await self._conn.execute(
            "UPDATE segments SET attempts = attempts + 1 WHERE num=?", (num,)
        )
        await self._conn.commit()
        row = await self.get_segment(num)
        return row.attempts if row else 0

    async def update_heartbeat(
        self, num: int, last_seen_at: float, last_activity: str
    ) -> None:
        await self._conn.execute(
            "UPDATE segments SET last_seen_at=?, last_activity=? WHERE num=?",
            (last_seen_at, last_activity, num),
        )
        await self._conn.commit()

    # ── Attempt history ──

    async def record_attempt(
        self,
        seg_num: int,
        attempt: int,
        started_at: float | None,
        finished_at: float | None,
        status: str,
        summary: str,
        tokens_in: int = 0,
        tokens_out: int = 0,
    ) -> None:
        await self._conn.execute(
            """INSERT INTO segment_attempts
               (seg_num, attempt, started_at, finished_at, status, summary, tokens_in, tokens_out)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
            (seg_num, attempt, started_at, finished_at, status, summary, tokens_in, tokens_out),
        )
        await self._conn.commit()

    async def get_attempts(self, seg_num: int) -> list[dict]:
        cur = await self._conn.execute(
            "SELECT id, seg_num, attempt, started_at, finished_at, status, summary,"
            " tokens_in, tokens_out FROM segment_attempts WHERE seg_num=? ORDER BY attempt",
            (seg_num,),
        )
        rows = await cur.fetchall()
        return [dict(r) for r in rows]

    # ── Events ──

    async def log_event(
        self, kind: str, detail: str = "", severity: str = "info"
    ) -> None:
        await self._conn.execute(
            "INSERT INTO events (ts, kind, detail, severity) VALUES (?, ?, ?, ?)",
            (time.time(), kind, detail, severity),
        )
        await self._conn.commit()

    async def get_events(self, limit: int = 100, after_id: int = 0) -> list[dict]:
        cur = await self._conn.execute(
            "SELECT id, ts, kind, detail, severity FROM events"
            " WHERE id > ? ORDER BY id DESC LIMIT ?",
            (after_id, limit),
        )
        return [
            {"id": r[0], "ts": r[1], "kind": r[2], "detail": r[3], "severity": r[4]}
            for r in await cur.fetchall()
        ]

    # ── Notifications ──

    async def enqueue_notification(
        self,
        kind: str,
        message: str,
        event_key: str,
        priority: str = "default",
    ) -> None:
        await self._conn.execute(
            """INSERT OR IGNORE INTO notifications
               (created_at, event_key, kind, message, priority)
               VALUES (?, ?, ?, ?, ?)""",
            (time.time(), event_key, kind, message, priority),
        )
        await self._conn.commit()

    async def get_pending_notifications(self, max_attempts: int = 3) -> list[dict]:
        cur = await self._conn.execute(
            "SELECT id, created_at, event_key, kind, message, priority,"
            " sent_at, attempts, last_attempt_at, last_error"
            " FROM notifications WHERE sent_at IS NULL AND attempts < ?"
            " ORDER BY created_at",
            (max_attempts,),
        )
        return [dict(r) for r in await cur.fetchall()]

    async def mark_notification_sent(self, notif_id: int) -> None:
        await self._conn.execute(
            "UPDATE notifications SET sent_at=?, attempts=attempts+1 WHERE id=?",
            (time.time(), notif_id),
        )
        await self._conn.commit()

    async def mark_notification_failed(self, notif_id: int, error: str) -> None:
        await self._conn.execute(
            "UPDATE notifications SET attempts=attempts+1, last_attempt_at=?,"
            " last_error=? WHERE id=?",
            (time.time(), error, notif_id),
        )
        await self._conn.commit()

    async def get_recent_notifications(self, limit: int = 20) -> list[dict]:
        cur = await self._conn.execute(
            "SELECT id, created_at, event_key, kind, message, priority,"
            " sent_at, attempts, last_attempt_at, last_error"
            " FROM notifications ORDER BY created_at DESC LIMIT ?",
            (limit,),
        )
        return [dict(r) for r in await cur.fetchall()]

    # ── Run metadata ──

    async def set_meta(self, key: str, value: str) -> None:
        await self._conn.execute(
            "INSERT OR REPLACE INTO run_meta (key, value) VALUES (?, ?)",
            (key, value),
        )
        await self._conn.commit()

    async def get_meta(self, key: str) -> str | None:
        cur = await self._conn.execute(
            "SELECT value FROM run_meta WHERE key=?", (key,)
        )
        row = await cur.fetchone()
        return row[0] if row else None

    # ── Progress queries ──

    async def progress(self) -> dict[str, int]:
        """Return counts by status."""
        cur = await self._conn.execute(
            "SELECT status, COUNT(*) FROM segments GROUP BY status"
        )
        return dict(await cur.fetchall())

    async def wave_segments(self, wave: int) -> list[SegmentRow]:
        cur = await self._conn.execute(
            "SELECT num, slug, title, wave, status, attempts, started_at, finished_at,"
            " result_json, last_seen_at, last_activity, per_segment_timeout"
            " FROM segments WHERE wave=? ORDER BY num",
            (wave,),
        )
        return [SegmentRow(*tuple(r)) for r in await cur.fetchall()]

    async def max_wave(self) -> int:
        cur = await self._conn.execute("SELECT MAX(wave) FROM segments")
        row = await cur.fetchone()
        return row[0] if row and row[0] else 0

    async def reset_stale_running(self) -> int:
        """Reset segments stuck as 'running' from a previous crashed run."""
        cur = await self._conn.execute(
            "UPDATE segments SET status='pending' WHERE status='running'"
        )
        count = cur.rowcount
        await self._conn.commit()
        if count:
            await self.log_event(
                "reset_stale", f"Reset {count} stale running segments to pending"
            )
        return count

    async def reset_for_retry(self, num: int) -> None:
        """Reset a single segment back to pending for manual retry."""
        await self._conn.execute(
            "UPDATE segments SET status='pending', started_at=NULL,"
            " finished_at=NULL WHERE num=?",
            (num,),
        )
        await self._conn.commit()

    async def migrate_from_json(self, json_path: Path) -> int:
        """Import segment statuses from the old bash script's execution-state.json.

        Returns the number of segments updated.
        """
        if not json_path.exists():
            return 0
        with open(json_path, encoding="utf-8") as f:
            data = json.load(f)

        count = 0
        for key, info in data.get("segments", {}).items():
            try:
                num = int(key.lstrip("S"))
            except (ValueError, IndexError):
                continue
            old_status = info.get("status", "pending")
            if old_status == "running":
                old_status = "pending"
            attempts = info.get("attempts", 0)
            completed = info.get("completed")

            finished_at = None
            if completed:
                from datetime import datetime  # noqa: PLC0415
                try:
                    dt = datetime.fromisoformat(completed.replace("Z", "+00:00"))
                    finished_at = dt.timestamp()
                except (ValueError, AttributeError):
                    pass

            existing = await self.get_segment(num)
            if existing and existing.status == "pending" and old_status in (
                "pass", "partial", "blocked"
            ):
                await self.set_status(
                    num, old_status,
                    attempts=attempts,
                    finished_at=finished_at,
                )
                count += 1
        if count:
            await self.log_event(
                "state_migrated",
                f"Imported {count} segment statuses from {json_path.name}",
            )
        return count

    async def all_as_dict(self) -> dict[str, Any]:
        """Full snapshot for the monitor API."""
        all_segs = await self.get_all_segments()
        segments = [s.to_dict() for s in all_segs]
        for seg in segments:
            seg["attempts_history"] = await self.get_attempts(seg["num"])
        return {
            "plan_title": await self.get_meta("plan_title") or "",
            "plan_goal": await self.get_meta("plan_goal") or "",
            "started_at": await self.get_meta("started_at") or "",
            "current_wave": await self.get_meta("current_wave") or "0",
            "max_wave": await self.max_wave(),
            "progress": await self.progress(),
            "segments": segments,
            "events": await self.get_events(limit=50),
            "notifications": await self.get_recent_notifications(limit=20),
        }
