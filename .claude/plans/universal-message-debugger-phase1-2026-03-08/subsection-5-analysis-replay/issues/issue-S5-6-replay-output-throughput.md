---
id: "S5-6"
title: "Replay Output Throughput and Formatting"
risk: 2/10
addressed_by_segments: [5]
---
# Issue S5-6: Replay Output Throughput and Formatting

**Core Problem:**
Rust's stdout is line-buffered by default. Each `println!()` triggers a syscall. At 100k events/sec (the stated performance target), this produces 100k syscalls/sec, throttling replay to a fraction of target speed. Research confirms: 500K lines with println takes ~17.6s vs ~3.7s with BufWriter (4.7x speedup).

**Root Cause:**
The parent plan mentions stdout buffering as a concern but does not prescribe a solution or specify output formatting libraries.

**Proposed Fix:**
1. Wrap stdout in `BufWriter` for all replay output. Flush on completion and on Ctrl+C (signal handler).
2. Two output formats:
   - `--format json`: one JSON object per line (NDJSON). Use `serde_json::to_writer()` directly to BufWriter.
   - `--format table` (default): human-readable table using `tabled` crate with derive macros.
3. Pre-format events into a buffer before writing to minimize per-event overhead.
4. For piped output (not a terminal), automatically switch to block buffering and suppress color codes.

CLI: `prb replay session.mcap [--speed 2.0] [--filter 'transport=grpc'] [--format json|table]`

**Existing Solutions Evaluated:**
- `tabled` (crates.io, v0.20.0, 20.6M+ downloads, 597 reverse deps) -- derive-macro table formatting with multiple themes, color, padding. Best fit for structured DebugEvent output. Adopted.
- `comfy-table` (v7.2.2, 60M+ downloads) -- manual table construction with auto-wrapping. More mature but lacks derive macros. Better for dynamic schemas; our events have known structure. Rejected as secondary choice.
- `serde_json` (already in workspace from Subsection 1) -- handles JSON output mode. No additional dependency.

**Alternatives Considered:**
- Custom formatting with `write!()` macros. Rejected: reinvents what tabled provides; error-prone alignment handling.
- Use `indicatif` for progress bars during replay. Deferred to Phase 2: useful but not core functionality.

**Pre-Mortem -- What Could Go Wrong:**
- `tabled` derive macros may conflict with existing `serde::Serialize` derives on DebugEvent. Mitigation: use a separate `EventDisplay` type for table output.
- BufWriter loses data on abnormal termination (Ctrl+C). Mitigation: register a signal handler that flushes before exit.
- JSON output for large events may produce very long lines. Consider `--pretty` flag for indented JSON.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- External evidence: Rust Users Forum "Efficient stdout: buffers all the way down" confirms BufWriter wrapping as the standard approach for high-throughput CLI output.
- Existing solutions: `tabled` is the most-downloaded Rust table formatting library with active maintenance through 2025.

**Blast Radius:**
- Direct: replay output module, CLI argument parsing
- Ripple: `prb flows` command benefits from the same formatting infrastructure
