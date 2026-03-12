---
id: "5"
title: "prb-ai explain.rs requires live LLM endpoint"
risk: 4/10
addressed_by_segments: [5]
---

# Issue 5: prb-ai explain.rs requires live LLM endpoint

## Core Problem

`explain_event()` and `explain_event_stream()` in `explain.rs` (32% coverage, 170 lines) are untested beyond input validation guards. The functions make HTTP calls to an OpenAI-compatible API via `async-openai`. Without a mock server, the LLM response handling (empty choices, null content, stream errors, empty stream) is completely uncovered.

## Root Cause

No mock HTTP server in dev-dependencies. `async-openai` uses `reqwest` under the hood and supports `base_url` override, making it trivially mockable with `wiremock`.

## Proposed Fix

1. Add `wiremock` and `tokio` (with `macros` feature) to `[dev-dependencies]`.
2. Create `tests/explain_test.rs` with a `MockServer` that stubs `POST /v1/chat/completions`.
3. Test paths: valid response → extracted content, empty `choices: []` → `AiError::ApiRequest`, null content → `AiError::ApiRequest`, streaming response with chunks → accumulated text, stream error → `AiError::StreamInterrupted`, empty stream → `AiError::ApiRequest`.
4. Point `AiConfig::with_base_url(server.uri())` at the mock.

## Existing Solutions Evaluated

- **wiremock** (crates.io): Async HTTP mock server, native tokio, actively maintained. Adopted.
- **mockito**: Sync-first API, less ergonomic with async-openai. Rejected.
- **httpmock**: Good but wiremock has better ecosystem fit. Rejected.

## Pre-Mortem

- `async-openai` streaming uses SSE format. Mock must return `text/event-stream` content type with `data: {"choices":[...]}` lines.
- `wiremock` adds ~15 transitive deps. Acceptable for dev-only.
- `AiConfig::resolve_api_key()` must be satisfied — use `with_api_key("test-key")`.

## Risk Factor: 4/10

New dev-dependency, but isolated to test code. No production changes.

## Blast Radius

- Direct: `crates/prb-ai/Cargo.toml`, `crates/prb-ai/tests/explain_test.rs`
- Ripple: None
