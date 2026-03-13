//! HTTP-level integration tests for AI explanation engine using wiremock.
//!
//! These tests validate the HTTP layer interaction with OpenAI-compatible APIs
//! without requiring actual LLM calls. They cover success paths (streaming and
//! non-streaming), error paths, and request validation.

use bytes::Bytes;
use prb_ai::error::AiError;
use prb_ai::{AiConfig, explain_event, explain_event_stream};
use prb_core::{DebugEvent, Direction, EventSource, Payload, Timestamp, TransportKind};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

/// Create a minimal test event for explanation.
fn make_test_event() -> DebugEvent {
    DebugEvent::builder()
        .id(prb_core::EventId::from_raw(1))
        .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
        .source(EventSource {
            adapter: "pcap".into(),
            origin: "test.pcap".into(),
            network: Some(prb_core::NetworkAddr {
                src: "10.0.0.1:52341".into(),
                dst: "10.0.0.2:50051".into(),
            }),
        })
        .transport(TransportKind::Grpc)
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: Bytes::from_static(b"test payload"),
        })
        .build()
}

/// Helper: Build a mock OpenAI chat completion response.
fn mock_chat_response(content: &str) -> serde_json::Value {
    json!({
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 1710000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 50,
            "completion_tokens": 20,
            "total_tokens": 70
        }
    })
}

/// Helper: Build a mock OpenAI streaming chunk.
fn mock_stream_chunk(content: &str) -> String {
    let chunk = json!({
        "id": "chatcmpl-test",
        "object": "chat.completion.chunk",
        "created": 1710000000,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "delta": {
                "content": content
            },
            "finish_reason": null
        }]
    });
    format!("data: {}\n\n", chunk)
}

// ═══════════════════════════════════════════════════════════════════════════
// SUCCESS PATHS
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_explain_http_success_non_streaming() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::to_string(&mock_chat_response(
        "This is a gRPC request with test payload.",
    ))
    .unwrap();

    // Note: async-openai appends the path to the base_url directly,
    // so we need to include "/v1" in the base URL for the client to hit "/v1/chat/completions"
    let base_url = format!("{}/v1", mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(response_body)
                .insert_header("content-type", "application/json"),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = AiConfig::default()
        .with_base_url(base_url)
        .with_api_key("test-key")
        .with_stream(false);

    let events = vec![make_test_event()];
    let result = explain_event(&events, 0, &config).await;

    assert!(result.is_ok(), "Error: {:?}", result.err());
    let explanation = result.unwrap();
    assert!(explanation.contains("gRPC request"));
    assert!(explanation.contains("test payload"));
}

#[tokio::test]
async fn test_explain_http_success_streaming() {
    let mock_server = MockServer::start().await;

    // Build SSE response with multiple chunks
    let mut sse_body = String::new();
    sse_body.push_str(&mock_stream_chunk("This "));
    sse_body.push_str(&mock_stream_chunk("is "));
    sse_body.push_str(&mock_stream_chunk("streaming!"));
    sse_body.push_str("data: [DONE]\n\n");

    let base_url = format!("{}/v1", mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(sse_body.into_bytes(), "text/event-stream"),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = AiConfig::default()
        .with_base_url(base_url)
        .with_api_key("test-key")
        .with_stream(true);

    let events = vec![make_test_event()];
    let mut chunks = Vec::new();
    let result = explain_event_stream(&events, 0, &config, |chunk| {
        chunks.push(chunk.to_string());
    })
    .await;

    assert!(result.is_ok(), "Streaming error: {:?}", result.err());
    let full_text = result.unwrap();
    assert_eq!(full_text, "This is streaming!");
    assert_eq!(chunks, vec!["This ", "is ", "streaming!"]);
}

// ═══════════════════════════════════════════════════════════════════════════
// ERROR PATHS
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_explain_http_error_empty_choices() {
    let mock_server = MockServer::start().await;
    let base_url = format!("{}/v1", mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1710000000,
            "model": "gpt-4o-mini",
            "choices": [],
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 0,
                "total_tokens": 50
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = AiConfig::default()
        .with_base_url(base_url)
        .with_api_key("test-key")
        .with_stream(false);

    let events = vec![make_test_event()];
    let result = explain_event(&events, 0, &config).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AiError::ApiRequest(msg) => assert!(msg.contains("empty response")),
        _ => panic!("Expected ApiRequest error"),
    }
}

#[tokio::test]
async fn test_explain_http_error_null_content() {
    let mock_server = MockServer::start().await;
    let base_url = format!("{}/v1", mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1710000000,
            "model": "gpt-4o-mini",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 50,
                "completion_tokens": 0,
                "total_tokens": 50
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = AiConfig::default()
        .with_base_url(base_url)
        .with_api_key("test-key")
        .with_stream(false);

    let events = vec![make_test_event()];
    let result = explain_event(&events, 0, &config).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AiError::ApiRequest(msg) => assert!(msg.contains("no content")),
        _ => panic!("Expected ApiRequest error"),
    }
}

#[tokio::test]
async fn test_explain_http_error_500() {
    let mock_server = MockServer::start().await;
    let base_url = format!("{}/v1", mock_server.uri());

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": {
                "message": "Internal server error",
                "type": "server_error",
                "code": "internal_error"
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = AiConfig::default()
        .with_base_url(base_url)
        .with_api_key("test-key")
        .with_stream(false);

    let events = vec![make_test_event()];
    let result = explain_event(&events, 0, &config).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AiError::ApiRequest(_) => {}
        _ => panic!("Expected ApiRequest error for 500"),
    }
}

#[tokio::test]
async fn test_explain_http_error_429_rate_limit() {
    let mock_server = MockServer::start().await;
    let base_url = format!("{}/v1", mock_server.uri());

    // Note: async-openai may retry on 429, so we don't use .expect() here
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(json!({
                    "error": {
                        "message": "Rate limit exceeded",
                        "type": "rate_limit_error",
                        "code": "rate_limit_exceeded"
                    }
                }))
                .insert_header("retry-after", "1"),
        )
        .mount(&mock_server)
        .await;

    let config = AiConfig::default()
        .with_base_url(base_url)
        .with_api_key("test-key")
        .with_stream(false);

    let events = vec![make_test_event()];

    // Wrap in timeout since async-openai may retry 429 responses
    let timeout_result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        explain_event(&events, 0, &config),
    )
    .await;

    // Either times out (due to retries) or returns an error
    match timeout_result {
        Ok(result) => {
            assert!(result.is_err());
            match result.unwrap_err() {
                AiError::ApiRequest(_) => {}
                _ => panic!("Expected ApiRequest error for 429"),
            }
        }
        Err(_) => {
            // Timeout is also acceptable - client is retrying as expected
        }
    }
}

#[tokio::test]
async fn test_explain_http_error_timeout() {
    let mock_server = MockServer::start().await;
    let base_url = format!("{}/v1", mock_server.uri());

    // Mock with a long delay to trigger timeout
    // Note: No .expect(1) because the request should timeout before reaching the server
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(mock_chat_response("delayed response"))
                .set_delay(std::time::Duration::from_secs(10)),
        )
        .mount(&mock_server)
        .await;

    let config = AiConfig::default()
        .with_base_url(base_url)
        .with_api_key("test-key")
        .with_stream(false);

    let events = vec![make_test_event()];

    // Use tokio::time::timeout to enforce a 1-second timeout
    let timeout_result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        explain_event(&events, 0, &config),
    )
    .await;

    assert!(timeout_result.is_err(), "Expected timeout");
}

#[tokio::test]
async fn test_explain_http_error_stream_interruption() {
    let mock_server = MockServer::start().await;
    let base_url = format!("{}/v1", mock_server.uri());

    // Send partial SSE stream then close connection (no [DONE] marker)
    let mut partial_sse = String::new();
    partial_sse.push_str(&mock_stream_chunk("This is "));
    partial_sse.push_str(&mock_stream_chunk("partial"));
    // Intentionally no [DONE] marker - connection closes abruptly

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(partial_sse.into_bytes(), "text/event-stream"),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = AiConfig::default()
        .with_base_url(base_url)
        .with_api_key("test-key")
        .with_stream(true);

    let events = vec![make_test_event()];
    let result = explain_event_stream(&events, 0, &config, |_| {}).await;

    // Stream ends without [DONE] marker -> triggers StreamInterrupted error
    assert!(result.is_err());
    match result.unwrap_err() {
        AiError::StreamInterrupted(_) => {}
        _ => panic!("Expected StreamInterrupted error"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// REQUEST VALIDATION
// ═══════════════════════════════════════════════════════════════════════════

/// Custom matcher to validate OpenAI API request structure.
struct RequestStructureMatcher {
    expected_model: String,
    expected_temperature: f32,
    expected_max_tokens: u16,
}

impl Match for RequestStructureMatcher {
    fn matches(&self, request: &Request) -> bool {
        let body: serde_json::Value = match serde_json::from_slice(&request.body) {
            Ok(v) => v,
            Err(_) => return false,
        };

        // Validate required fields exist
        if !body.is_object() {
            return false;
        }

        let obj = body.as_object().unwrap();

        // Check model
        if obj.get("model").and_then(|v| v.as_str()) != Some(&self.expected_model) {
            return false;
        }

        // Check messages array exists and has at least 2 entries (system + user)
        let messages = match obj.get("messages").and_then(|v| v.as_array()) {
            Some(arr) if arr.len() >= 2 => arr,
            _ => return false,
        };

        // Verify first message is system role
        if messages[0].get("role").and_then(|v| v.as_str()) != Some("system") {
            return false;
        }

        // Verify second message is user role
        if messages[1].get("role").and_then(|v| v.as_str()) != Some("user") {
            return false;
        }

        // Check temperature
        let temp = obj.get("temperature").and_then(|v| v.as_f64());
        if temp.map(|t| (t - self.expected_temperature as f64).abs() > 0.01) != Some(false) {
            return false;
        }

        // Check max_tokens
        if obj.get("max_tokens").and_then(|v| v.as_u64()) != Some(self.expected_max_tokens as u64) {
            return false;
        }

        // Check stream field exists
        obj.contains_key("stream")
    }
}

#[tokio::test]
async fn test_explain_http_request_validation() {
    let mock_server = MockServer::start().await;
    let base_url = format!("{}/v1", mock_server.uri());

    let request_matcher = RequestStructureMatcher {
        expected_model: "gpt-4o-mini".to_string(),
        expected_temperature: 0.5,
        expected_max_tokens: 1024,
    };

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(request_matcher)
        .respond_with(
            ResponseTemplate::new(200).set_body_json(mock_chat_response("Valid request received.")),
        )
        .expect(1)
        .mount(&mock_server)
        .await;

    let config = AiConfig::default()
        .with_base_url(base_url)
        .with_api_key("test-key")
        .with_model("gpt-4o-mini")
        .with_temperature(0.5)
        .with_max_tokens(1024)
        .with_stream(false);

    let events = vec![make_test_event()];
    let result = explain_event(&events, 0, &config).await;

    assert!(result.is_ok());
    let explanation = result.unwrap();
    assert!(explanation.contains("Valid request received"));
}
