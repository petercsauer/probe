//! Explanation engine: orchestrates prompt building and LLM calls.

use crate::config::AiConfig;
use crate::context::ExplainContext;
use crate::error::AiError;
use crate::prompt::{build_system_prompt, build_user_message};
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestUserMessage, CreateChatCompletionRequest,
};
use async_openai::Client;
use futures::StreamExt;
use prb_core::DebugEvent;

/// Explain a single event using the AI provider (non-streaming).
///
/// # Arguments
/// - `events`: All events in the capture/session
/// - `target_idx`: Index of the event to explain
/// - `config`: AI provider configuration
///
/// # Returns
/// Complete explanation text, or an error if the LLM call fails.
pub async fn explain_event(
    events: &[DebugEvent],
    target_idx: usize,
    config: &AiConfig,
) -> Result<String, AiError> {
    if events.is_empty() {
        return Err(AiError::NoEvents);
    }
    if target_idx >= events.len() {
        return Err(AiError::EventNotFound(target_idx as u64));
    }

    // Build context
    let context = ExplainContext::build(events, target_idx, config.context_window);

    // Build messages
    let has_tls = events[target_idx]
        .metadata
        .get("pcap.tls_decrypted")
        .is_some();
    let system_prompt = build_system_prompt(context.transport, has_tls);
    let user_message = build_user_message(&context);

    let messages = vec![
        ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
            content: system_prompt.into(),
            name: None,
        }),
        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            content: user_message.into(),
            name: None,
        }),
    ];

    // Configure client
    let api_key = config.resolve_api_key()?;
    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&config.base_url);
    let client = Client::with_config(openai_config);

    // Create request
    let request = CreateChatCompletionRequest {
        model: config.model.clone(),
        messages,
        temperature: Some(config.temperature as f64),
        max_tokens: Some(config.max_tokens),
        stream: Some(false),
        ..Default::default()
    };

    // Call LLM
    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| AiError::ApiRequest(e.to_string()))?;

    // Extract response
    let choice = response
        .choices
        .first()
        .ok_or_else(|| AiError::ApiRequest("empty response from LLM".into()))?;

    let content = choice
        .message
        .content
        .as_ref()
        .ok_or_else(|| AiError::ApiRequest("no content in response".into()))?;

    Ok(content.clone())
}

/// Explain a single event using the AI provider with streaming output.
///
/// # Arguments
/// - `events`: All events in the capture/session
/// - `target_idx`: Index of the event to explain
/// - `config`: AI provider configuration
/// - `callback`: Called for each chunk of streamed text
///
/// # Returns
/// Complete explanation text (assembled from stream), or an error if the LLM call fails.
pub async fn explain_event_stream<F>(
    events: &[DebugEvent],
    target_idx: usize,
    config: &AiConfig,
    mut callback: F,
) -> Result<String, AiError>
where
    F: FnMut(&str),
{
    if events.is_empty() {
        return Err(AiError::NoEvents);
    }
    if target_idx >= events.len() {
        return Err(AiError::EventNotFound(target_idx as u64));
    }

    // Build context
    let context = ExplainContext::build(events, target_idx, config.context_window);

    // Build messages
    let has_tls = events[target_idx]
        .metadata
        .get("pcap.tls_decrypted")
        .is_some();
    let system_prompt = build_system_prompt(context.transport, has_tls);
    let user_message = build_user_message(&context);

    let messages = vec![
        ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage {
            content: system_prompt.into(),
            name: None,
        }),
        ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage {
            content: user_message.into(),
            name: None,
        }),
    ];

    // Configure client
    let api_key = config.resolve_api_key()?;
    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&config.base_url);
    let client = Client::with_config(openai_config);

    // Create request
    let request = CreateChatCompletionRequest {
        model: config.model.clone(),
        messages,
        temperature: Some(config.temperature as f64),
        max_tokens: Some(config.max_tokens),
        stream: Some(true),
        ..Default::default()
    };

    // Call LLM with streaming
    let mut stream = client
        .chat()
        .create_stream(request)
        .await
        .map_err(|e| AiError::ApiRequest(e.to_string()))?;

    let mut full_text = String::new();

    while let Some(result) = stream.next().await {
        let response = result.map_err(|e| AiError::StreamInterrupted(e.to_string()))?;

        if let Some(choice) = response.choices.first() {
            if let Some(ref delta) = choice.delta.content {
                callback(delta);
                full_text.push_str(delta);
            }
        }
    }

    if full_text.is_empty() {
        return Err(AiError::ApiRequest("empty stream from LLM".into()));
    }

    Ok(full_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::{Direction, EventSource, Payload, Timestamp, TransportKind};

    fn make_test_event(id: u64, transport: TransportKind) -> DebugEvent {
        DebugEvent::builder()
            .id(prb_core::EventId::from_raw(id))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: Some(prb_core::NetworkAddr {
                    src: "10.0.0.1:52341".into(),
                    dst: "10.0.0.2:50051".into(),
                }),
            })
            .transport(transport)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"test payload"),
            })
            .build()
    }

    #[test]
    fn test_explain_event_validates_empty_events() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = AiConfig::default();
        let result = rt.block_on(explain_event(&[], 0, &config));
        assert!(matches!(result, Err(AiError::NoEvents)));
    }

    #[test]
    fn test_explain_event_validates_event_index() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let events = vec![make_test_event(1, TransportKind::Grpc)];
        let config = AiConfig::default();
        let result = rt.block_on(explain_event(&events, 10, &config));
        assert!(matches!(result, Err(AiError::EventNotFound(10))));
    }

    #[test]
    fn test_context_assembly() {
        // Verify that the explanation engine correctly builds context
        // (without actually calling the LLM)
        let events = vec![
            make_test_event(1, TransportKind::Grpc),
            make_test_event(2, TransportKind::Grpc),
            make_test_event(3, TransportKind::Grpc),
        ];

        let config = AiConfig::default().with_context_window(1);
        let context = ExplainContext::build(&events, 1, config.context_window);

        assert_eq!(context.transport, TransportKind::Grpc);
        assert!(context.target_summary.contains("Event 2"));
        assert_eq!(context.surrounding_summaries.len(), 2);
    }
}
