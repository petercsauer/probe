---
id: "S3-6"
title: "CLI Extension for PCAP Ingest"
risk: 2/10
addressed_by_segments: [5]
---

# Issue S3-6: CLI Extension for PCAP Ingest

**Core Problem:**
The CLI must be extended to support `prb ingest capture.pcapng [--tls-keylog keys.log]` with progress reporting and error summaries for large files.

**Root Cause:**
Subsection 1 establishes CLI with fixture ingest only; PCAP ingest needs new arguments and UX.

**Proposed Fix:**
Add `--tls-keylog <path>` flag to `prb ingest`. Auto-detect file format (JSON fixture vs PCAP/pcapng) from magic bytes. Report progress for large files (packet count, stream count, bytes processed). Summarize warnings at end (skipped packets, failed TLS sessions, incomplete streams).

**Existing Solutions Evaluated:**
N/A -- internal CLI design for project-specific command.

**Alternatives Considered:**
- Separate `prb ingest-pcap` subcommand -- rejected, format auto-detection makes a single `prb ingest` command cleaner UX.

**Pre-Mortem -- What Could Go Wrong:**
- Progress reporting for streaming parsers requires periodic flush; must not slow down ingest.
- Error summary formatting needs care to be useful without overwhelming.

**Risk Factor:** 2/10

**Evidence for Optimality:**
- Project conventions: Subsection 1 establishes `prb ingest` as the entry point; extending it maintains CLI consistency.
- External evidence: Wireshark/tshark use format auto-detection, validating the approach.

**Blast Radius:**
- Direct: CLI binary crate (`prb-cli`)
- Ripple: none
