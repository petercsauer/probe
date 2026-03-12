"""Comprehensive tests for state.py StateDB — targeting 90%+ coverage."""

from __future__ import annotations

import asyncio
import json
import time
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from .state import StateDB, SegmentRow

if TYPE_CHECKING:
    from collections.abc import AsyncGenerator


@pytest.fixture
async def temp_dir(tmp_path: Path) -> Path:
    """Provide a temporary directory for test databases."""
    return tmp_path


@pytest.fixture
async def mock_state_db(temp_dir: Path) -> AsyncGenerator[StateDB, None]:
    """Provide a StateDB instance with initialized schema."""
    db_path = temp_dir / "test.db"
    db = await StateDB.create(db_path)
    yield db
    await db.close()


@pytest.fixture
async def mock_segments() -> list:
    """Provide mock segment objects for testing."""
    class MockSegment:
        def __init__(self, num: int, slug: str, title: str, wave: int):
            self.num = num
            self.slug = slug
            self.title = title
            self.wave = wave

    return [
        MockSegment(1, "setup", "Setup Infrastructure", 0),
        MockSegment(2, "core", "Core Implementation", 1),
        MockSegment(3, "tests", "Test Suite", 1),
        MockSegment(4, "docs", "Documentation", 2),
    ]


# ── Lifecycle Tests ──


@pytest.mark.asyncio
async def test_create_initializes_schema(temp_dir: Path) -> None:
    """Verify StateDB.create() creates all required tables."""
    db_path = temp_dir / "lifecycle.db"
    db = await StateDB.create(db_path)

    # Verify tables exist by querying sqlite_master
    cur = await db._conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
    )
    tables = [row[0] for row in await cur.fetchall()]

    assert "segments" in tables
    assert "events" in tables
    assert "run_meta" in tables
    assert "notifications" in tables
    assert "segment_attempts" in tables
    assert "segment_interjections" in tables
    assert "gate_attempts" in tables

    await db.close()


@pytest.mark.asyncio
async def test_create_applies_migrations(temp_dir: Path) -> None:
    """Verify StateDB.create() applies migrations."""
    db_path = temp_dir / "migrations.db"
    db = await StateDB.create(db_path)

    # Check that migrated columns exist
    cur = await db._conn.execute("PRAGMA table_info(segments)")
    columns = [row[1] for row in await cur.fetchall()]

    assert "last_seen_at" in columns
    assert "last_activity" in columns
    assert "per_segment_timeout" in columns

    cur = await db._conn.execute("PRAGMA table_info(events)")
    columns = [row[1] for row in await cur.fetchall()]
    assert "severity" in columns

    await db.close()


@pytest.mark.asyncio
async def test_close_cleans_up(mock_state_db: StateDB) -> None:
    """Verify close() properly closes the connection."""
    await mock_state_db.close()
    # After close, operations should fail
    with pytest.raises(Exception):
        await mock_state_db.get_segment(1)


# ── Segment Operations Tests ──


@pytest.mark.asyncio
async def test_init_segments_inserts_all(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify init_segments() inserts all segments."""
    await mock_state_db.init_segments(mock_segments)

    all_segs = await mock_state_db.get_all_segments()
    assert len(all_segs) == 4
    assert all_segs[0].num == 1
    assert all_segs[0].slug == "setup"
    assert all_segs[0].status == "pending"


@pytest.mark.asyncio
async def test_get_segment_returns_row(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify get_segment() returns correct SegmentRow."""
    await mock_state_db.init_segments(mock_segments)

    seg = await mock_state_db.get_segment(2)
    assert seg is not None
    assert seg.num == 2
    assert seg.slug == "core"
    assert seg.title == "Core Implementation"
    assert seg.wave == 1


@pytest.mark.asyncio
async def test_get_segment_returns_none_for_missing(mock_state_db: StateDB) -> None:
    """Verify get_segment() returns None for non-existent segment."""
    seg = await mock_state_db.get_segment(999)
    assert seg is None


@pytest.mark.asyncio
@pytest.mark.parametrize(
    "status",
    ["pass", "failed", "timeout", "blocked", "running", "pending"],
)
async def test_set_status_updates_row(
    mock_state_db: StateDB, mock_segments: list, status: str
) -> None:
    """Verify set_status() updates segment status correctly."""
    await mock_state_db.init_segments(mock_segments)

    await mock_state_db.set_status(1, status)
    seg = await mock_state_db.get_segment(1)
    assert seg is not None
    assert seg.status == status


@pytest.mark.asyncio
async def test_set_status_with_kwargs(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify set_status() updates additional fields via kwargs."""
    await mock_state_db.init_segments(mock_segments)

    now = time.time()
    await mock_state_db.set_status(
        1, "pass", finished_at=now, result_json='{"success": true}'
    )

    seg = await mock_state_db.get_segment(1)
    assert seg is not None
    assert seg.status == "pass"
    assert seg.finished_at == now
    assert seg.result_json == '{"success": true}'


@pytest.mark.asyncio
async def test_increment_attempts_increments(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify increment_attempts() increments the counter."""
    await mock_state_db.init_segments(mock_segments)

    count1 = await mock_state_db.increment_attempts(1)
    assert count1 == 1

    count2 = await mock_state_db.increment_attempts(1)
    assert count2 == 2

    seg = await mock_state_db.get_segment(1)
    assert seg is not None
    assert seg.attempts == 2


@pytest.mark.asyncio
async def test_reset_stale_running_resets_only_running(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify reset_stale_running() only resets segments with status='running'."""
    await mock_state_db.init_segments(mock_segments)

    await mock_state_db.set_status(1, "running")
    await mock_state_db.set_status(2, "pass")
    await mock_state_db.set_status(3, "running")

    count = await mock_state_db.reset_stale_running()
    assert count == 2

    seg1 = await mock_state_db.get_segment(1)
    seg2 = await mock_state_db.get_segment(2)
    seg3 = await mock_state_db.get_segment(3)

    assert seg1 is not None and seg1.status == "pending"
    assert seg2 is not None and seg2.status == "pass"
    assert seg3 is not None and seg3.status == "pending"


@pytest.mark.asyncio
async def test_update_heartbeat(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify update_heartbeat() updates last_seen_at and last_activity."""
    await mock_state_db.init_segments(mock_segments)

    now = time.time()
    await mock_state_db.update_heartbeat(1, now, "Running tests")

    seg = await mock_state_db.get_segment(1)
    assert seg is not None
    assert seg.last_seen_at == now
    assert seg.last_activity == "Running tests"


@pytest.mark.asyncio
async def test_reset_for_retry(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify reset_for_retry() resets segment back to pending."""
    await mock_state_db.init_segments(mock_segments)

    await mock_state_db.set_status(1, "failed", started_at=time.time(), finished_at=time.time())
    await mock_state_db.increment_attempts(1)

    await mock_state_db.reset_for_retry(1)

    seg = await mock_state_db.get_segment(1)
    assert seg is not None
    assert seg.status == "pending"
    assert seg.attempts == 0
    assert seg.started_at is None
    assert seg.finished_at is None


# ── Events Log Tests ──


@pytest.mark.asyncio
async def test_log_event_inserts(mock_state_db: StateDB) -> None:
    """Verify log_event() inserts event with timestamp."""
    await mock_state_db.log_event("test_event", "Test detail", "info")

    events = await mock_state_db.get_events(limit=10)
    assert len(events) == 1
    assert events[0]["kind"] == "test_event"
    assert events[0]["detail"] == "Test detail"
    assert events[0]["severity"] == "info"
    assert events[0]["ts"] > 0


@pytest.mark.asyncio
@pytest.mark.parametrize("severity", ["info", "warn", "error"])
async def test_log_event_severity_levels(
    mock_state_db: StateDB, severity: str
) -> None:
    """Verify log_event() handles different severity levels."""
    await mock_state_db.log_event("test", "message", severity)

    events = await mock_state_db.get_events()
    assert events[0]["severity"] == severity


@pytest.mark.asyncio
async def test_get_events_limits(mock_state_db: StateDB) -> None:
    """Verify get_events() respects LIMIT clause."""
    for i in range(10):
        await mock_state_db.log_event(f"event_{i}", f"detail_{i}")

    events = await mock_state_db.get_events(limit=5)
    assert len(events) == 5


@pytest.mark.asyncio
async def test_get_events_after_id_filters(mock_state_db: StateDB) -> None:
    """Verify get_events() filters by after_id."""
    await mock_state_db.log_event("event_1", "detail_1")
    await mock_state_db.log_event("event_2", "detail_2")
    events = await mock_state_db.get_events(limit=100)
    # events are returned in DESC order, so [0] is most recent (event_2)
    second_id = events[0]["id"]

    await mock_state_db.log_event("event_3", "detail_3")
    await mock_state_db.log_event("event_4", "detail_4")

    new_events = await mock_state_db.get_events(limit=100, after_id=second_id)
    assert len(new_events) == 2
    assert new_events[1]["kind"] == "event_3"
    assert new_events[0]["kind"] == "event_4"


# ── Attempts Tracking Tests ──


@pytest.mark.asyncio
async def test_record_attempt_inserts(mock_state_db: StateDB) -> None:
    """Verify record_attempt() inserts attempt record."""
    now = time.time()
    await mock_state_db.record_attempt(
        seg_num=1,
        attempt=1,
        started_at=now,
        finished_at=now + 10,
        status="pass",
        summary="Build succeeded",
        tokens_in=1000,
        tokens_out=500,
    )

    attempts = await mock_state_db.get_attempts(1)
    assert len(attempts) == 1
    assert attempts[0]["seg_num"] == 1
    assert attempts[0]["attempt"] == 1
    assert attempts[0]["status"] == "pass"
    assert attempts[0]["summary"] == "Build succeeded"
    assert attempts[0]["tokens_in"] == 1000
    assert attempts[0]["tokens_out"] == 500


@pytest.mark.asyncio
async def test_get_attempts_returns_ordered(mock_state_db: StateDB) -> None:
    """Verify get_attempts() returns attempts ordered by attempt number."""
    now = time.time()
    await mock_state_db.record_attempt(1, 3, now, now + 1, "pass", "Third")
    await mock_state_db.record_attempt(1, 1, now, now + 1, "fail", "First")
    await mock_state_db.record_attempt(1, 2, now, now + 1, "pass", "Second")

    attempts = await mock_state_db.get_attempts(1)
    assert len(attempts) == 3
    assert attempts[0]["attempt"] == 1
    assert attempts[1]["attempt"] == 2
    assert attempts[2]["attempt"] == 3


# ── Notifications Tests ──


@pytest.mark.asyncio
async def test_enqueue_notification(mock_state_db: StateDB) -> None:
    """Verify enqueue_notification() inserts notification."""
    await mock_state_db.enqueue_notification(
        kind="email",
        message="Build failed",
        event_key="build_fail_1",
        priority="high",
    )

    pending = await mock_state_db.get_pending_notifications()
    assert len(pending) == 1
    assert pending[0]["kind"] == "email"
    assert pending[0]["message"] == "Build failed"
    assert pending[0]["priority"] == "high"
    assert pending[0]["sent_at"] is None


@pytest.mark.asyncio
async def test_enqueue_notification_ignores_duplicates(mock_state_db: StateDB) -> None:
    """Verify enqueue_notification() ignores duplicate event_key."""
    await mock_state_db.enqueue_notification(
        "email", "Test", "key_1", "default"
    )
    await mock_state_db.enqueue_notification(
        "email", "Test2", "key_1", "default"
    )

    pending = await mock_state_db.get_pending_notifications()
    assert len(pending) == 1


@pytest.mark.asyncio
async def test_mark_notification_sent(mock_state_db: StateDB) -> None:
    """Verify mark_notification_sent() updates sent_at."""
    await mock_state_db.enqueue_notification("slack", "Message", "key_1")
    pending = await mock_state_db.get_pending_notifications()
    notif_id = pending[0]["id"]

    await mock_state_db.mark_notification_sent(notif_id)

    pending_after = await mock_state_db.get_pending_notifications()
    assert len(pending_after) == 0

    recent = await mock_state_db.get_recent_notifications()
    assert recent[0]["sent_at"] is not None


@pytest.mark.asyncio
async def test_mark_notification_failed(mock_state_db: StateDB) -> None:
    """Verify mark_notification_failed() updates attempts and error."""
    await mock_state_db.enqueue_notification("slack", "Message", "key_1")
    pending = await mock_state_db.get_pending_notifications()
    notif_id = pending[0]["id"]

    await mock_state_db.mark_notification_failed(notif_id, "Connection timeout")

    recent = await mock_state_db.get_recent_notifications()
    assert recent[0]["attempts"] == 1
    assert recent[0]["last_error"] == "Connection timeout"
    assert recent[0]["last_attempt_at"] is not None


@pytest.mark.asyncio
async def test_get_pending_notifications_max_attempts(mock_state_db: StateDB) -> None:
    """Verify get_pending_notifications() filters by max_attempts."""
    await mock_state_db.enqueue_notification("email", "Test", "key_1")
    pending = await mock_state_db.get_pending_notifications()
    notif_id = pending[0]["id"]

    # Fail it 3 times
    for _ in range(3):
        await mock_state_db.mark_notification_failed(notif_id, "Error")

    # Should not appear in pending with max_attempts=3
    pending_after = await mock_state_db.get_pending_notifications(max_attempts=3)
    assert len(pending_after) == 0


# ── Metadata Tests ──


@pytest.mark.asyncio
async def test_set_meta_and_get_meta(mock_state_db: StateDB) -> None:
    """Verify set_meta() and get_meta() store/retrieve metadata."""
    await mock_state_db.set_meta("plan_title", "Test Plan")
    await mock_state_db.set_meta("started_at", "2025-01-01")

    title = await mock_state_db.get_meta("plan_title")
    started = await mock_state_db.get_meta("started_at")

    assert title == "Test Plan"
    assert started == "2025-01-01"


@pytest.mark.asyncio
async def test_get_meta_returns_none_for_missing(mock_state_db: StateDB) -> None:
    """Verify get_meta() returns None for non-existent key."""
    value = await mock_state_db.get_meta("nonexistent_key")
    assert value is None


# ── Progress Queries Tests ──


@pytest.mark.asyncio
async def test_progress(mock_state_db: StateDB, mock_segments: list) -> None:
    """Verify progress() returns counts by status."""
    await mock_state_db.init_segments(mock_segments)
    await mock_state_db.set_status(1, "pass")
    await mock_state_db.set_status(2, "pass")
    await mock_state_db.set_status(3, "failed")

    progress = await mock_state_db.progress()
    assert progress["pass"] == 2
    assert progress["failed"] == 1
    assert progress["pending"] == 1


@pytest.mark.asyncio
async def test_wave_segments(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify wave_segments() returns segments for a specific wave."""
    await mock_state_db.init_segments(mock_segments)

    wave1 = await mock_state_db.wave_segments(1)
    assert len(wave1) == 2
    assert wave1[0].num == 2
    assert wave1[1].num == 3


@pytest.mark.asyncio
async def test_max_wave(mock_state_db: StateDB, mock_segments: list) -> None:
    """Verify max_wave() returns the highest wave number."""
    await mock_state_db.init_segments(mock_segments)

    max_w = await mock_state_db.max_wave()
    assert max_w == 2


# ── Interjections Tests ──


@pytest.mark.asyncio
async def test_enqueue_interject(mock_state_db: StateDB) -> None:
    """Verify enqueue_interject() stores operator message."""
    interject_id = await mock_state_db.enqueue_interject(1, "Please use --verbose")

    assert interject_id > 0

    pending = await mock_state_db.get_pending_interject(1)
    assert pending is not None
    assert pending["seg_num"] == 1
    assert pending["message"] == "Please use --verbose"


@pytest.mark.asyncio
async def test_get_pending_interject_returns_most_recent(
    mock_state_db: StateDB,
) -> None:
    """Verify get_pending_interject() returns most recent unconsumed message."""
    await mock_state_db.enqueue_interject(1, "First message")
    await mock_state_db.enqueue_interject(1, "Second message")

    pending = await mock_state_db.get_pending_interject(1)
    assert pending is not None
    assert pending["message"] == "Second message"


@pytest.mark.asyncio
async def test_consume_interject(mock_state_db: StateDB) -> None:
    """Verify consume_interject() marks message as consumed."""
    interject_id = await mock_state_db.enqueue_interject(1, "Test message")

    await mock_state_db.consume_interject(interject_id)

    pending = await mock_state_db.get_pending_interject(1)
    assert pending is None


@pytest.mark.asyncio
async def test_get_interject_history(mock_state_db: StateDB) -> None:
    """Verify get_interject_history() returns all interjections."""
    id1 = await mock_state_db.enqueue_interject(1, "First")
    await mock_state_db.enqueue_interject(1, "Second")
    await mock_state_db.consume_interject(id1)

    history = await mock_state_db.get_interject_history(1)
    assert len(history) == 2
    assert history[0]["message"] == "Second"
    assert history[1]["message"] == "First"
    assert history[1]["consumed_at"] is not None


# ── Gate Attempts Tests ──


@pytest.mark.asyncio
async def test_record_gate_attempt(mock_state_db: StateDB) -> None:
    """Verify record_gate_attempt() inserts gate execution record."""
    now = time.time()
    gate_id = await mock_state_db.record_gate_attempt(
        wave=1,
        attempt=1,
        started_at=now,
        finished_at=now + 5,
        passed=True,
        exit_code=0,
        log_file="gate_1_1.log",
    )

    assert gate_id > 0

    attempts = await mock_state_db.get_gate_attempts(1)
    assert len(attempts) == 1
    assert attempts[0]["wave"] == 1
    assert attempts[0]["attempt"] == 1
    assert attempts[0]["passed"] is True
    assert attempts[0]["exit_code"] == 0


@pytest.mark.asyncio
async def test_get_gate_attempts_ordered(mock_state_db: StateDB) -> None:
    """Verify get_gate_attempts() returns attempts in reverse order."""
    now = time.time()
    await mock_state_db.record_gate_attempt(1, 1, now, now + 1, False, 1, "log1")
    await mock_state_db.record_gate_attempt(1, 2, now, now + 1, False, 1, "log2")
    await mock_state_db.record_gate_attempt(1, 3, now, now + 1, True, 0, "log3")

    attempts = await mock_state_db.get_gate_attempts(1)
    assert len(attempts) == 3
    assert attempts[0]["attempt"] == 3
    assert attempts[1]["attempt"] == 2
    assert attempts[2]["attempt"] == 1


@pytest.mark.asyncio
async def test_get_latest_gate_attempt(mock_state_db: StateDB) -> None:
    """Verify get_latest_gate_attempt() returns most recent attempt."""
    now = time.time()
    await mock_state_db.record_gate_attempt(1, 1, now, now + 1, False, 1, "log1")
    await mock_state_db.record_gate_attempt(1, 2, now, now + 1, True, 0, "log2")

    latest = await mock_state_db.get_latest_gate_attempt(1)
    assert latest is not None
    assert latest["attempt"] == 2
    assert latest["passed"] is True


# ── Migration Tests ──


@pytest.mark.asyncio
async def test_migrate_from_json(
    mock_state_db: StateDB, mock_segments: list, temp_dir: Path
) -> None:
    """Verify migrate_from_json() imports legacy state."""
    await mock_state_db.init_segments(mock_segments)

    # Create a legacy JSON file
    legacy_data = {
        "segments": {
            "S01": {
                "status": "pass",
                "attempts": 2,
                "completed": "2025-01-01T12:00:00Z",
            },
            "S02": {
                "status": "blocked",
                "attempts": 1,
            },
        }
    }
    json_path = temp_dir / "execution-state.json"
    json_path.write_text(json.dumps(legacy_data))

    count = await mock_state_db.migrate_from_json(json_path)
    assert count == 2

    seg1 = await mock_state_db.get_segment(1)
    seg2 = await mock_state_db.get_segment(2)

    assert seg1 is not None and seg1.status == "pass"
    assert seg1.attempts == 2
    assert seg2 is not None and seg2.status == "blocked"


# ── SegmentRow Tests ──


def test_segment_row_getitem() -> None:
    """Verify SegmentRow.__getitem__() works."""
    row = SegmentRow(1, "test", "Test Segment", 0)
    assert row["num"] == 1
    assert row["slug"] == "test"


def test_segment_row_to_dict() -> None:
    """Verify SegmentRow.to_dict() includes parsed result_json."""
    row = SegmentRow(1, "test", "Test", 0, result_json='{"success": true}')
    d = row.to_dict()
    assert d["result"]["success"] is True


def test_segment_row_to_dict_invalid_json() -> None:
    """Verify SegmentRow.to_dict() handles invalid JSON."""
    row = SegmentRow(1, "test", "Test", 0, result_json="invalid json")
    d = row.to_dict()
    assert d["result"] == "invalid json"


# ── all_as_dict Tests ──


@pytest.mark.asyncio
async def test_all_as_dict(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify all_as_dict() returns full snapshot."""
    await mock_state_db.init_segments(mock_segments)
    await mock_state_db.set_meta("plan_title", "Test Plan")
    await mock_state_db.log_event("test_event", "detail")

    snapshot = await mock_state_db.all_as_dict()

    assert snapshot["plan_title"] == "Test Plan"
    assert snapshot["max_wave"] == 2
    assert len(snapshot["segments"]) == 4
    assert len(snapshot["events"]) == 1
    assert snapshot["segments"][0]["num"] == 1


# ── Concurrent Access Tests ──


@pytest.mark.asyncio
async def test_concurrent_writes(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify StateDB handles concurrent writes correctly (SQLite WAL mode)."""
    await mock_state_db.init_segments(mock_segments)

    async def write_events(prefix: str) -> None:
        for i in range(5):
            await mock_state_db.log_event(f"{prefix}_event_{i}", f"{prefix}_detail_{i}")
            await asyncio.sleep(0.01)

    # Run 4 tasks in parallel
    await asyncio.gather(
        write_events("task1"),
        write_events("task2"),
        write_events("task3"),
        write_events("task4"),
    )

    events = await mock_state_db.get_events(limit=100)
    assert len(events) == 20


@pytest.mark.asyncio
async def test_concurrent_segment_updates(
    mock_state_db: StateDB, mock_segments: list
) -> None:
    """Verify concurrent segment updates work correctly."""
    await mock_state_db.init_segments(mock_segments)

    async def increment_segment(seg_num: int, times: int) -> None:
        for _ in range(times):
            await mock_state_db.increment_attempts(seg_num)
            await asyncio.sleep(0.01)

    # Increment different segments concurrently
    await asyncio.gather(
        increment_segment(1, 3),
        increment_segment(2, 5),
        increment_segment(3, 4),
    )

    seg1 = await mock_state_db.get_segment(1)
    seg2 = await mock_state_db.get_segment(2)
    seg3 = await mock_state_db.get_segment(3)

    assert seg1 is not None and seg1.attempts == 3
    assert seg2 is not None and seg2.attempts == 5
    assert seg3 is not None and seg3.attempts == 4
