"""Plan loader: parse manifest + segment frontmatter, build DAG, compute waves."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class PlanMeta:
    title: str = ""
    goal: str = ""


@dataclass
class Segment:
    num: int
    slug: str
    title: str
    depends_on: list[int] = field(default_factory=list)
    dependents: list[int] = field(default_factory=list)  # Computed reverse edges
    cycle_budget: int = 15
    risk: int = 5
    complexity: str = "Medium"
    commit_message: str = ""
    file_path: Path = field(default_factory=Path)
    wave: int = 0
    timeout: int = 0


_FRONTMATTER_RE = re.compile(r"^---\s*\n(.*?)\n---", re.DOTALL)


def _parse_frontmatter(path: Path) -> dict:
    """Extract YAML-like frontmatter from a markdown file.

    Hand-rolled to avoid requiring PyYAML — frontmatter in plan files uses
    only simple key: value pairs and lists like [1, 2, 3].
    """
    text = path.read_text(encoding="utf-8", errors="replace")
    m = _FRONTMATTER_RE.match(text)
    if not m:
        return {}
    result: dict = {}
    for line in m.group(1).splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if ":" not in line:
            continue
        key, _, value = line.partition(":")
        key = key.strip()
        value = value.strip()
        # Strip surrounding quotes
        if len(value) >= 2 and value[0] == value[-1] and value[0] in ('"', "'"):
            value = value[1:-1]
        # Detect list values like [1, 2, 3] or []
        if value.startswith("[") and value.endswith("]"):
            inner = value[1:-1].strip()
            if not inner:
                result[key] = []
            else:
                items = [v.strip().strip("\"'") for v in inner.split(",")]
                # Try to parse as ints
                try:
                    result[key] = [int(x) for x in items]
                except ValueError:
                    result[key] = items
            continue
        # Try int
        try:
            result[key] = int(value)
            continue
        except ValueError:
            pass
        result[key] = value
    return result


def _compute_transitive_dependents(segments: list[Segment]) -> None:
    """Compute reverse edges: for each segment, who depends on it.

    Modifies segments in-place to populate .dependents lists.
    This enables transitive skip marking when a segment fails.
    """
    # Clear all dependents lists
    for seg in segments:
        seg.dependents = []

    # Build reverse edges
    for seg in segments:
        for dep_num in seg.depends_on:
            dep = next((s for s in segments if s.num == dep_num), None)
            if dep:
                dep.dependents.append(seg.num)


def _assign_waves(segments: list[Segment]) -> None:
    """Assign wave numbers using Kahn's topological sort algorithm."""
    by_num = {s.num: s for s in segments}
    seg_nums = set(by_num.keys())
    # Filter depends_on to only include segments actually in the plan
    for s in segments:
        s.depends_on = [d for d in s.depends_on if d in seg_nums]

    remaining = set(seg_nums)
    wave = 1
    while remaining:
        # Segments whose dependencies are all already assigned
        ready = [
            n for n in remaining
            if all(by_num[d].wave > 0 for d in by_num[n].depends_on)
        ]
        if not ready:
            unresolved = {n: by_num[n].depends_on for n in remaining}
            raise ValueError(f"Circular dependency detected: {unresolved}")
        for n in ready:
            by_num[n].wave = wave
            remaining.discard(n)
        wave += 1


def load_plan(plan_dir: Path) -> tuple[PlanMeta, list[Segment]]:
    """Parse manifest + all segment files, compute waves via topological sort."""
    manifest_path = plan_dir / "manifest.md"
    if not manifest_path.exists():
        raise FileNotFoundError(f"No manifest.md in {plan_dir}")

    fm = _parse_frontmatter(manifest_path)
    meta = PlanMeta(
        title=fm.get("plan", ""),
        goal=fm.get("goal", ""),
    )

    segments_dir = plan_dir / "segments"
    if not segments_dir.is_dir():
        raise FileNotFoundError(f"No segments/ directory in {plan_dir}")

    segments: list[Segment] = []
    for f in sorted(segments_dir.glob("*.md")):
        sfm = _parse_frontmatter(f)
        if "segment" not in sfm:
            continue
        segments.append(Segment(
            num=sfm["segment"],
            slug=f.stem,
            title=sfm.get("title", f.stem),
            depends_on=sfm.get("depends_on", []),
            cycle_budget=sfm.get("cycle_budget", 15),
            risk=sfm.get("risk", 5),
            complexity=sfm.get("complexity", "Medium"),
            commit_message=sfm.get("commit_message", ""),
            file_path=f,
            timeout=sfm.get("timeout", 0),
        ))

    if not segments:
        raise ValueError(f"No segments found in {segments_dir}")

    # Validate segment numbers are integers (not strings like "10a")
    for seg in segments:
        if not isinstance(seg.num, int):
            raise ValueError(
                f"Invalid segment number in {seg.file_path.name}: {seg.num!r} (type: {type(seg.num).__name__})\n"
                f"Segment numbers must be integers (1, 2, 3...), not strings like '10a' or '10b'.\n"
                f"Use sequential integer numbering instead."
            )

    _assign_waves(segments)
    _compute_transitive_dependents(segments)
    return meta, segments
