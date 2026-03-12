---
segment: 5
title: "AI explain wiremock tests"
depends_on: []
risk: 4
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "test(prb-ai): add wiremock mock server tests for explain_event and explain_event_stream"
---

# Segment 5: AI explain wiremock tests

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Push prb-ai from 76.1% to 90%+ by adding `wiremock`-based tests that mock the OpenAI API for `explain_event` and `explain_event_stream`.

**Depends on:** None

## Issues Addressed

Issue 5 — prb-ai explain.rs requires live LLM endpoint.

## Scope

- `crates/prb-ai/Cargo.toml` — add `wiremock` and `tokio` test deps
- `crates/prb-ai/tests/explain_test.rs` — new integration test file
- `crates/prb-ai/src/config.rs` — fix `test_config_from_env` (currently failing)

## Key Files and Context

**explain.rs function signatures:**
```rust
pub async fn explain_event(
    events: &[DebugEvent], target_idx: usize, config: &AiConfig
) -> Result<String, AiError>

pub async fn explain_event_stream<F: FnMut(&str)>(
    events: &[DebugEvent], target_idx: usize, config: &AiConfig, callback: F
) -> Result<String, AiError>
```

**explain_event flow (lines ~37-96):**
1. Validates `events.is_empty()` → `AiError::NoEvents`
2. Validates `target_idx >= events.len()` → `AiError::EventNotFound`
3. Builds context via `ExplainContext::build`
4. Builds prompt via `build_system_prompt` + `build_user_message`
5. Configures `async_openai::Client` with `config.base_url` and API key
6. Calls `client.chat().create(request).await`
7. `response.choices.first()` → `None` = `AiError::ApiRequest("empty response from LLM")`
8. `choice.message.content` → `None` = `AiError::ApiRequest("no content in response")`

**explain_event_stream flow (lines ~108-189):**
1. Same validation and setup
2. Calls `client.chat().create_stream(request).await`
3. Iterates stream, accumulates `delta.content`
4. Empty stream → `AiError::ApiRequest("empty stream from LLM")`
5. Stream error → `AiError::StreamInterrupted`

**async-openai uses `reqwest` internally.** `AiConfig::with_base_url(url)` sets the base URL, so pointing at a wiremock server works.

**OpenAI chat completion response format:**
```json
{
  "id": "chatcmpl-xxx",
  "object": "chat.completion",
  "choices": [{
    "index": 0,
    "message": {"role": "assistant", "content": "The explanation..."},
    "finish_reason": "stop"
  }]
}
```

**Streaming SSE format:**
```
data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

**Failing test `test_config_from_env`:** Panics at config.rs:162 with `MissingApiKey("openai")`. The test sets env vars for OpenAI provider but doesn't set `OPENAI_API_KEY` (or the test env leaks). Fix: either set the key in the test or use `for_provider(Ollama)` which doesn't require a key.

## Implementation Approach

1. Add to `crates/prb-ai/Cargo.toml` `[dev-dependencies]`:
   ```toml
   wiremock = "0.6"
   tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
   ```

2. Fix `test_config_from_env` — set `OPENAI_API_KEY` env var in the test or switch to Ollama provider.

3. Create `tests/explain_test.rs` with helper:
   ```rust
   async fn mock_server_with_response(body: serde_json::Value) -> (wiremock::MockServer, AiConfig) {
       let server = wiremock::MockServer::start().await;
       wiremock::Mock::given(wiremock::matchers::method("POST"))
           .and(wiremock::matchers::path("/v1/chat/completions"))
           .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(body))
           .mount(&server)
           .await;
       let config = AiConfig::default()
           .with_base_url(&format!("{}/v1", server.uri()))
           .with_api_key("test-key");
       (server, config)
   }
   ```

4. Tests to write:
   - `test_explain_event_success`: valid response → extracted content string
   - `test_explain_event_empty_choices`: `{"choices": []}` → `AiError::ApiRequest`
   - `test_explain_event_null_content`: `{"choices": [{"message": {"content": null}}]}` → error
   - `test_explain_event_stream_success`: SSE response → accumulated text
   - `test_explain_event_stream_empty`: empty SSE stream → error
   - `test_explain_event_no_events`: empty slice → `AiError::NoEvents`
   - `test_explain_event_bad_index`: out of bounds → `AiError::EventNotFound`

## Alternatives Ruled Out

- `mockito`: sync-first, awkward with tokio. Rejected.
- Trait abstraction over `async_openai::Client`: too invasive for production code. Wiremock is sufficient.
- `httpmock`: viable but wiremock has better async ergonomics. Rejected.

## Pre-Mortem Risks

- `async-openai` may append path segments to `base_url` differently than expected. Test the URL path matches `/v1/chat/completions`.
- Streaming SSE: wiremock's `ResponseTemplate` supports `set_body_string` with `text/event-stream` content type. Ensure each `data:` line ends with `\n\n`.
- `wiremock` 0.6 requires tokio runtime — use `#[tokio::test]`.
- Version compatibility: check latest wiremock version on crates.io.

## Build and Test Commands

- Build: `cargo build -p prb-ai`
- Test (targeted): `cargo test -p prb-ai -- explain`
- Test (regression): `cargo test -p prb-ai`
- Test (full gate): `cargo test -p prb-ai`

## Exit Criteria

1. **Targeted tests:**
   - `test_explain_event_success`: returns Ok with expected content string
   - `test_explain_event_empty_choices`: returns Err(ApiRequest)
   - `test_explain_event_null_content`: returns Err(ApiRequest)
   - `test_explain_event_stream_success`: returns Ok with accumulated text
   - `test_explain_event_stream_empty`: returns Err(ApiRequest)
   - `test_config_from_env`: no longer panics
2. **Regression tests:** All 22 existing prb-ai tests pass (including fixed config test)
3. **Full build gate:** `cargo build -p prb-ai`
4. **Full test gate:** `cargo test -p prb-ai`
5. **Self-review gate:** No dead code, wiremock only in dev-deps
6. **Scope verification gate:** Changes in `crates/prb-ai/` only

**Risk factor:** 4/10
**Estimated complexity:** Medium
