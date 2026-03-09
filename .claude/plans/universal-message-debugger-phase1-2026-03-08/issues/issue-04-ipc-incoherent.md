---
id: "4"
title: "IPC/Shared Memory Incoherent"
risk: 1/10
addressed_by_subsections: [1]
---

# Issue 4: IPC/Shared Memory Protocols Incoherent with Offline Analysis

**Core Problem:**
The plan lists Iceoryx, Unix domain sockets, POSIX shared memory, and named pipes as Phase 1 protocol targets. Phase 1 is scoped to offline analysis of captured traffic, but there is no standard capture file format for IPC/shared memory traffic. You cannot open a `.pcap` of Iceoryx messages.

**Root Cause:**
The protocol coverage list was defined by "what transports exist" rather than "what transports produce capturable offline artifacts."

**Proposed Fix:**
Remove all IPC/shared memory protocols from Phase 1 scope. Move them to Phase 2, which should introduce a live capture agent that can intercept IPC traffic and serialize it into MCAP sessions. Phase 1 supports IPC data only if pre-serialized as JSON fixture files (which the fixture adapter already handles).

**Existing Solutions Evaluated:**
- Iceoryx2 has Rust bindings (eclipse-iceoryx/iceoryx2, pure Rust, actively maintained) that could support live capture in Phase 2.
- `iceoryx-rs` wraps the C++ iceoryx1 library.
- No existing tool captures IPC traffic into pcap-like files. The closest analog is `strace` for syscall-level tracing, which is too low-level.

**Recommendation:** Defer to Phase 2. Phase 1's fixture adapter already provides a path for users who can manually serialize IPC messages to JSON.

**Alternatives Considered:**
- Add a custom IPC capture format. Rejected: designing a capture format is a significant effort orthogonal to the core debugger.
- Wrap `strace`/`dtrace` to capture UDS traffic. Rejected: brittle, OS-specific, requires root, and produces syscall-level noise rather than message-level events.

**Pre-Mortem -- What Could Go Wrong:**
- Users expecting IPC support in Phase 1 are disappointed.
- Deferral creates pressure to rush IPC in Phase 2 without adequate design.

**Risk Factor:** 1/10 (removal is low-risk)

**Evidence for Optimality:**
- External evidence: No existing network analysis tool (Wireshark, tcpdump, tshark) supports shared memory capture. This is a known gap in the ecosystem.
- Existing solutions: Iceoryx2's Rust bindings exist but require live process attachment, not offline file analysis.

**Blast Radius:**
- Direct: protocol coverage list (removal)
- Ripple: none (no code exists yet for these adapters)
