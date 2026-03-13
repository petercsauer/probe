---
segment: 7
title: "AI HTTP Mocking"
depends_on: []
risk: 5/10
complexity: Low
cycle_budget: 10
status: merged
commit_message: "test(ai): Add HTTP mocking for LLM integration with wiremock"
---

# Segment 7: AI HTTP Mocking

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Increase explain.rs coverage from 36.73% to 75%+ with HTTP-level mocking using wiremock.

**Depends on:** None (independent)

## Context: Issues Addressed

**Core Problem:** AI streaming and error handling paths are undertested. Lines 174-183 handle streaming interruption with no recovery mechanism. Lines 85-88 handle empty choices array error. Lines 90-94 handle null content. All 49 existing tests skip actual LLM calls. Current coverage 36.73% indicates HTTP paths completely untested. Uses async-openai v0.20 client for OpenAI-compatible APIs.

**Proposed Fix:** Add wiremock dependency for HTTP-level mocking. Create tests for success paths (non-streaming and streaming SSE), error paths (empty choices, null content, 500 error, 429 rate limit, timeout, stream interruption), request validation, and optional contract tests against real APIs.

**Pre-Mortem Risks:**
- Mock SSE format might diverge from real OpenAI API - mitigate with optional contract tests
- Wiremock adds ~5MB to dev-dependencies - acceptable, dev-only dependency
- Tests could be brittle to API format changes - mitigate by centralizing mock response builders

## Scope

- `crates/prb-ai` HTTP mocking tests (no production code changes)
- `crates/prb-ai/tests/explain_http_test.rs` - New HTTP mocking tests
- Add `wiremock` to dev-dependencies

## Key Files and Context

**`crates/prb-ai/src/explain.rs`** (189 lines):
- Lines 174-183: Streaming interruption, no recovery mechanism
- Lines 85-88: Empty choices array error path
- Lines 90-94: Null content handling
- Uses `async-openai` v0.20 client for OpenAI-compatible APIs

**Prior planning:**
- `.claude/plans/coverage-90-hardening-2026-03-10/segments/05-ai-wiremock.md` - explicitly approved wiremock strategy

**Current state:**
- 49 tests pass but all skip actual LLM calls
- 36.73% coverage indicates HTTP paths untested

## Implementation Approach

1. **Add wiremock dependency** to `Cargo.toml`:
   ```toml
   [dev-dependencies]
   bytes = { workspace = true }
   wiremock = "0.6.5"
   ```

2. **Create `tests/explain_http_test.rs`** with mock server for OpenAI API `/v1/chat/completions` endpoint

3. **Test success paths** (2 tests):
   - Non-streaming: Mock 200 response with `choices[0].message.content = "explanation"`
   - Streaming: Mock SSE response with multiple chunks:
     ```
     data: {"choices":[{"delta":{"content":"Hello"}}]}

     data: {"choices":[{"delta":{"content":" world"}}]}

     data: [DONE]

     ```

4. **Test error paths** (6 tests):
   - Empty choices array: `{"choices": []}` → AiError::ApiRequest("empty response")
   - Null content: `{"choices":[{"message":{"content":null}}]}` → AiError::ApiRequest("no content")
   - 500 error: HTTP 500 with error JSON → AiError::ApiRequest
   - 429 rate limit: HTTP 429 → AiError::RateLimited (if error type exists, else ApiRequest)
   - Timeout: Mock with long delay, use tokio::time::timeout
   - Stream interruption: Send partial SSE then close connection → AiError::StreamInterrupted

5. **Test request validation** (1 test):
   - Verify request body contains correct structure:
     - `model`: matches config
     - `messages`: array with system + user messages
     - `temperature`: matches config
     - `max_tokens`: matches config
   - Use wiremock body matchers

6. **Add contract test** (optional, `#[ignore]`):
   ```rust
   #[tokio::test]
   #[ignore] // Run with --ignored flag
   async fn test_real_openai_api() {
       let api_key = std::env::var("OPENAI_API_KEY")
           .expect("Set OPENAI_API_KEY for contract tests");
       let config = AiConfig::for_provider(AiProvider::OpenAi).with_api_key(api_key);
       let events = vec![make_test_event()];
       let result = explain_event(&events, 0, &config).await;
       assert!(result.is_ok());
   }
   ```

## Alternatives Ruled Out

- **Trait abstraction for HTTP client:** Rejected per prior planning - too invasive, adds runtime cost
- **Testing only non-streaming:** Rejected - streaming is primary use case in TUI, must validate SSE parsing

## Pre-Mortem Risks

- Mock SSE format might diverge from real OpenAI API: Mitigate with optional contract tests against real API
- Wiremock adds ~5MB to dev-dependencies: Acceptable - dev-only dependency, no production impact
- Tests could be brittle to API format changes: Mitigate by centralizing mock response builders in helper functions

## Build and Test Commands

- Build: `cargo build -p prb-ai`
- Test (targeted): `cargo test -p prb-ai explain_http`
- Test (regression): `cargo test -p prb-ai`
- Test (full gate): `cargo nextest run -p prb-ai`
- Contract (optional): `cargo test -p prb-ai -- --ignored` (requires $OPENAI_API_KEY or $ANTHROPIC_API_KEY)

## Exit Criteria

1. **Targeted tests:**
   - `explain_http_success` - 2 tests pass (non-streaming + streaming SSE)
   - `explain_http_errors` - 6 tests pass (empty choices, null content, 500 error, 429 rate limit, timeout, stream interruption)
   - `explain_http_validation` - 1 test passes (request body structure validated)
   - Total: 9 new HTTP mocking tests

2. **Regression tests:** All existing prb-ai tests in src/*/tests pass (49 existing tests)

3. **Full build gate:** `cargo build -p prb-ai` succeeds with zero warnings

4. **Full test gate:** `cargo nextest run -p prb-ai` passes (58 total tests: 49 existing + 9 new)

5. **Self-review gate:**
   - No production code changes
   - Only test additions
   - Wiremock properly configured

6. **Scope verification gate:** Only modified:
   - Added wiremock to dev-dependencies in Cargo.toml
   - Created test file `tests/explain_http_test.rs`
   - No src/ modifications
