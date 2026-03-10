---
plan: "Phase 2 — Protocol Auto-Detection & Extensible Decoder Plugin System"
generated: 2026-03-10
---

# Execution Log

| Segment | Title | Status | Started | Completed | Notes |
|---------|-------|--------|---------|-----------|-------|
| 1 | ProtocolDetector Trait + Built-in Detectors | pending | | | |
| 2 | DecoderRegistry + Dispatch Layer | pending | | | |
| 3 | Pipeline Integration | pending | | | |
| 4 | Native Plugin System | pending | | | Parallelizable with 5 |
| 5 | WASM Plugin System | pending | | | Parallelizable with 4 |
| 6 | Plugin Management CLI | pending | | | |

## Pre-flight Checklist

- [ ] All Phase 1 tests pass (`cargo test --workspace`)
- [ ] `guess` crate v0.2 is available on crates.io
- [ ] `extism` crate v1.10+ is available on crates.io
- [ ] `libloading` crate v0.8+ is available on crates.io
- [ ] `wasm32-unknown-unknown` target is installed (`rustup target add wasm32-unknown-unknown`)

## Post-completion Checklist

- [ ] `prb ingest` auto-detects gRPC, ZMQ, DDS without `--protocol` flag
- [ ] Unknown protocols fall back to `RawTcp`/`RawUdp` gracefully
- [ ] `--protocol` override works for all supported protocols
- [ ] Native example plugin compiles, loads, and decodes
- [ ] WASM example plugin compiles, loads, and decodes
- [ ] `prb plugins list` shows all built-in + loaded decoders
- [ ] `prb plugins install/remove` round-trip works
- [ ] All Phase 1 tests still pass (no regression)
- [ ] Detection benchmarks: <1μs per detection call
- [ ] No clippy warnings in any new crate
