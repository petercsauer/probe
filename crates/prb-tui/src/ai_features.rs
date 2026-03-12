//! AI-powered smart features for prb-tui.
//!
//! This module provides:
//! - Natural language to filter expression conversion
//! - Capture summary and analysis
//! - Anomaly detection

use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestUserMessage, CreateChatCompletionRequest, Role,
};
use prb_core::DebugEvent;

/// Error types for AI features.
#[derive(Debug)]
pub enum AiFeatureError {
    ApiRequest(String),
    InvalidResponse(String),
    NoEvents,
}

impl std::fmt::Display for AiFeatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiFeatureError::ApiRequest(e) => write!(f, "API request failed: {}", e),
            AiFeatureError::InvalidResponse(e) => write!(f, "Invalid response: {}", e),
            AiFeatureError::NoEvents => write!(f, "No events to analyze"),
        }
    }
}

impl std::error::Error for AiFeatureError {}

/// Convert natural language query to prb-query filter expression.
///
/// This function sends the user's natural language input to an LLM along with
/// the prb-query syntax specification, and asks it to generate a valid filter expression.
pub async fn natural_language_to_filter(
    nl_query: &str,
    config: &prb_ai::AiConfig,
) -> Result<String, AiFeatureError> {
    // Build system prompt with prb-query syntax
    let system_prompt = r#"You are a filter expression generator for prb (protocol debugger).
Your task is to convert natural language queries into valid prb-query filter expressions.

prb-query syntax:
- Field comparisons: field == "value", field != "value", field > 100, field <= 50
- Available fields:
  - transport: "HTTP", "gRPC", "Thrift", "Raw"
  - direction: "Inbound", "Outbound"
  - source.adapter: adapter name (e.g., "pcap", "tshark")
  - source.origin: origin identifier
  - grpc.status: gRPC status code (string)
  - grpc.method: gRPC method name
  - http.status: HTTP status code (number)
  - http.method: HTTP method
  - http.path: HTTP path
- Logical operators: && (AND), || (OR), ! (NOT)
- Parentheses for grouping: (expr1) && (expr2)
- String fields use quotes, numbers don't

Examples:
- "show me gRPC calls" → transport == "gRPC"
- "failed gRPC requests" → transport == "gRPC" && grpc.status != "0"
- "HTTP errors" → transport == "HTTP" && http.status >= 400
- "slow gRPC calls with latency > 100ms" → transport == "gRPC" (note: latency filtering not yet supported)

IMPORTANT: Only output the filter expression, nothing else. No explanations, no markdown, just the expression."#;

    let user_message = format!("Convert to prb-query filter: {}", nl_query);

    let messages = vec![
        ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
            content: system_prompt.to_string(),
            role: Role::System,
            name: None,
        }),
        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            content: user_message.into(),
            role: Role::User,
            name: None,
        }),
    ];

    // Configure client
    let api_key = config
        .resolve_api_key()
        .map_err(|e| AiFeatureError::ApiRequest(e.to_string()))?;
    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&config.base_url);
    let client = Client::with_config(openai_config);

    // Create request
    let request = CreateChatCompletionRequest {
        model: config.model.clone(),
        messages,
        temperature: Some(0.3), // Lower temperature for more deterministic output
        max_tokens: Some(200),
        stream: Some(false),
        ..Default::default()
    };

    // Call LLM
    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| AiFeatureError::ApiRequest(e.to_string()))?;

    // Extract response
    let choice = response
        .choices
        .first()
        .ok_or_else(|| AiFeatureError::InvalidResponse("empty response from LLM".into()))?;

    let content = choice
        .message
        .content
        .as_ref()
        .ok_or_else(|| AiFeatureError::InvalidResponse("no content in response".into()))?;

    // Clean up the response (trim whitespace, remove markdown code blocks if present)
    let filter_expr = content
        .trim()
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    Ok(filter_expr)
}

/// Generate a summary and analysis of captured events.
///
/// This function samples events (if >100) and sends them to an LLM for analysis.
/// The LLM will identify patterns, errors, anomalies, and suggest investigation filters.
pub async fn generate_capture_summary(
    events: &[DebugEvent],
    config: &prb_ai::AiConfig,
    callback: impl FnMut(&str),
) -> Result<String, AiFeatureError> {
    if events.is_empty() {
        return Err(AiFeatureError::NoEvents);
    }

    // Sample events if there are too many
    let sampled: Vec<&DebugEvent> = if events.len() > 100 {
        // Take first 50, last 50
        events
            .iter()
            .take(50)
            .chain(events.iter().skip(events.len() - 50))
            .collect()
    } else {
        events.iter().collect()
    };

    // Build event summary
    let mut event_summaries = Vec::new();
    for (idx, event) in sampled.iter().enumerate() {
        // Generate a simple summary string
        let payload_len = match &event.payload {
            prb_core::Payload::Raw { raw } => raw.len(),
            prb_core::Payload::Decoded { raw, .. } => raw.len(),
        };

        let detail = if let Some(method) = event.metadata.get("grpc.method") {
            format!("method={}", method)
        } else if let Some(status) = event.metadata.get("grpc.status") {
            format!("status={}", status)
        } else {
            format!("{} bytes", payload_len)
        };

        let summary = format!(
            "Event {}: {} {} from {} to {} - {}",
            idx + 1,
            event.transport,
            event.direction,
            event
                .source
                .network
                .as_ref()
                .map(|n| n.src.as_str())
                .unwrap_or("N/A"),
            event
                .source
                .network
                .as_ref()
                .map(|n| n.dst.as_str())
                .unwrap_or("N/A"),
            detail
        );
        event_summaries.push(summary);
    }

    let events_text = event_summaries.join("\n");

    let system_prompt = r#"You are a protocol debugging assistant analyzing network capture data.
Analyze the provided events and provide:
1. Overview of traffic patterns (protocols, endpoints, volume)
2. Notable errors or failures
3. Potential anomalies or suspicious patterns
4. Suggested investigation filters

Be concise but thorough. Focus on actionable insights."#;

    let user_message = format!(
        "Analyze this capture (showing {} of {} total events):\n\n{}",
        sampled.len(),
        events.len(),
        events_text
    );

    // Use streaming for better UX
    generate_ai_response_stream(system_prompt, &user_message, config, callback).await
}

/// Detect anomalies in the event stream.
///
/// Returns a list of event indices that appear anomalous.
pub async fn detect_anomalies(
    events: &[DebugEvent],
    _config: &prb_ai::AiConfig,
) -> Result<Vec<usize>, AiFeatureError> {
    if events.is_empty() {
        return Ok(Vec::new());
    }

    // For now, use simple heuristics rather than LLM (to save API calls)
    // Future enhancement: could use LLM for more sophisticated detection
    let mut anomalies = Vec::new();

    for (idx, event) in events.iter().enumerate() {
        let is_anomaly = match event.transport {
            prb_core::TransportKind::Grpc => {
                // gRPC status != 0 is an error
                event
                    .metadata
                    .get("grpc.status")
                    .map(|s| s.as_str() != "0")
                    .unwrap_or(false)
            }
            _ => {
                // For other protocols, check for generic error indicators
                // Could be enhanced with protocol-specific checks
                false
            }
        };

        if is_anomaly {
            anomalies.push(idx);
        }
    }

    Ok(anomalies)
}

/// Helper function to generate AI response with streaming.
async fn generate_ai_response_stream(
    system_prompt: &str,
    user_message: &str,
    config: &prb_ai::AiConfig,
    mut callback: impl FnMut(&str),
) -> Result<String, AiFeatureError> {
    use futures::StreamExt;

    let messages = vec![
        ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
            content: system_prompt.to_string(),
            role: Role::System,
            name: None,
        }),
        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            content: user_message.to_string().into(),
            role: Role::User,
            name: None,
        }),
    ];

    // Configure client
    let api_key = config
        .resolve_api_key()
        .map_err(|e| AiFeatureError::ApiRequest(e.to_string()))?;
    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&config.base_url);
    let client = Client::with_config(openai_config);

    // Create request
    let request = CreateChatCompletionRequest {
        model: config.model.clone(),
        messages,
        temperature: Some(config.temperature),
        max_tokens: Some(config.max_tokens as u16),
        stream: Some(true),
        ..Default::default()
    };

    // Call LLM with streaming
    let mut stream = client
        .chat()
        .create_stream(request)
        .await
        .map_err(|e| AiFeatureError::ApiRequest(e.to_string()))?;

    let mut full_text = String::new();

    while let Some(result) = stream.next().await {
        let response = result.map_err(|e| AiFeatureError::ApiRequest(e.to_string()))?;

        if let Some(choice) = response.choices.first()
            && let Some(ref delta) = choice.delta.content
        {
            callback(delta);
            full_text.push_str(delta);
        }
    }

    if full_text.is_empty() {
        return Err(AiFeatureError::InvalidResponse(
            "empty stream from LLM".into(),
        ));
    }

    Ok(full_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_anomalies_grpc_errors() {
        use bytes::Bytes;
        use prb_core::{Direction, EventSource, Payload, Timestamp, TransportKind};

        let mut event_ok = prb_core::DebugEvent::builder()
            .id(prb_core::EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_000_000_000))
            .source(EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"test"),
            })
            .build();
        event_ok
            .metadata
            .insert("grpc.status".to_string(), "0".to_string());

        let mut event_err = event_ok.clone();
        event_err.id = prb_core::EventId::from_raw(2);
        event_err
            .metadata
            .insert("grpc.status".to_string(), "2".to_string());

        let events = vec![event_ok, event_err];
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = prb_ai::AiConfig::default();

        let anomalies = rt.block_on(detect_anomalies(&events, &config)).unwrap();
        assert_eq!(anomalies, vec![1]); // Only second event is anomalous
    }

    #[test]
    fn test_detect_anomalies_multiple_events() {
        use bytes::Bytes;
        use prb_core::{Direction, EventSource, Payload, Timestamp, TransportKind};

        let event1 = prb_core::DebugEvent::builder()
            .id(prb_core::EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_000_000_000))
            .source(EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            })
            .transport(TransportKind::Zmq)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"test"),
            })
            .build();

        let event2 = prb_core::DebugEvent::builder()
            .id(prb_core::EventId::from_raw(2))
            .timestamp(Timestamp::from_nanos(2_000_000_000))
            .source(EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            })
            .transport(TransportKind::RawTcp)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"test2"),
            })
            .build();

        let events = vec![event1, event2];
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = prb_ai::AiConfig::default();

        let anomalies = rt.block_on(detect_anomalies(&events, &config)).unwrap();
        assert_eq!(anomalies, Vec::<usize>::new()); // No anomalies in these events
    }
}
