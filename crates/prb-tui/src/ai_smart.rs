//! AI Smart Features: NL filter generation, anomaly detection, and protocol hints.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessage,
    ChatCompletionRequestUserMessage, CreateChatCompletionRequest, Role,
};
use prb_core::{DebugEvent, TransportKind};
use prb_query::Filter;

/// Rate limiter to prevent API abuse.
struct RateLimiter {
    requests: Mutex<Vec<Instant>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    fn new(max_requests: usize, window: Duration) -> Self {
        RateLimiter {
            requests: Mutex::new(Vec::new()),
            max_requests,
            window,
        }
    }

    fn check_rate_limit(&self) -> Result<(), String> {
        let mut requests = self.requests.lock().unwrap();
        let now = Instant::now();

        // Remove old requests outside the window
        requests.retain(|&t| now.duration_since(t) < self.window);

        if requests.len() >= self.max_requests {
            return Err(format!(
                "Rate limit exceeded: {} requests per {:?}",
                self.max_requests, self.window
            ));
        }

        requests.push(now);
        Ok(())
    }
}

lazy_static::lazy_static! {
    static ref RATE_LIMITER: RateLimiter = RateLimiter::new(10, Duration::from_secs(60));
}

/// Context about the capture for smart features.
#[derive(Debug, Clone)]
pub struct CaptureContext {
    pub total_events: usize,
    pub transports: Vec<TransportKind>,
    pub available_fields: Vec<String>,
    pub sample_metadata: HashMap<String, Vec<String>>,
}

impl CaptureContext {
    /// Build capture context from events.
    pub fn build(events: &[DebugEvent]) -> Self {
        let mut transports = Vec::new();
        let mut available_fields = Vec::new();
        let mut sample_metadata: HashMap<String, Vec<String>> = HashMap::new();

        for event in events.iter().take(100) {
            // Sample first 100 events
            if !transports.contains(&event.transport) {
                transports.push(event.transport);
            }

            for (key, value) in &event.metadata {
                available_fields.push(key.clone());
                sample_metadata
                    .entry(key.clone())
                    .or_default()
                    .push(value.clone());
            }
        }

        available_fields.sort();
        available_fields.dedup();

        // Limit sample values to 5 per field
        for values in sample_metadata.values_mut() {
            values.sort();
            values.dedup();
            values.truncate(5);
        }

        CaptureContext {
            total_events: events.len(),
            transports,
            available_fields,
            sample_metadata,
        }
    }

    /// Format available fields as a string for prompt.
    pub fn format_fields(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Available fields:".to_string());
        lines.push("  - transport (values: gRPC, ZMQ, DDS, TCP, UDP)".to_string());
        lines.push("  - direction (values: Inbound, Outbound)".to_string());

        for field in &self.available_fields {
            if let Some(samples) = self.sample_metadata.get(field) {
                let sample_str = samples
                    .iter()
                    .take(3)
                    .map(|s| format!("\"{}\"", s))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("  - {} (examples: {})", field, sample_str));
            } else {
                lines.push(format!("  - {}", field));
            }
        }

        lines.join("\n")
    }
}

/// Generate a filter from natural language query using AI.
pub async fn generate_filter(
    nl_query: &str,
    context: &CaptureContext,
    config: &prb_ai::AiConfig,
) -> Result<String, String> {
    // Check rate limit
    RATE_LIMITER
        .check_rate_limit()
        .map_err(|e| format!("Rate limit: {}", e))?;

    let system_prompt = r#"You are a query language expert for Probe network analysis tool.
Convert natural language queries into Probe filter expressions.

Filter Syntax:
- Comparison operators: ==, !=, <, >, <=, >=
- String operators: contains, startswith, endswith
- Logical operators: and, or, not
- Field paths use dot notation: grpc.status, grpc.method
- String values must be quoted: "value"
- Numbers are unquoted: 123, 0.5

Examples:
- "show errors" → grpc.status != "0"
- "failed requests" → grpc.status != "0"
- "GET requests" → grpc.method contains "Get"
- "slow calls" → duration > 1000
- "from service X" → grpc.service == "X"

IMPORTANT:
- Output ONLY the filter expression, no explanation
- Use exact field names from the available fields
- If query is ambiguous, use the most common interpretation
- For errors: grpc.status != "0" or http.status >= 400
"#;

    let user_message = format!(
        "Convert this query to a filter: \"{}\"\n\n{}\n\nFilter expression:",
        nl_query,
        context.format_fields()
    );

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
        .map_err(|e| format!("API key error: {}", e))?;
    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&config.base_url);
    let client = Client::with_config(openai_config);

    // Create request
    let request = CreateChatCompletionRequest {
        model: config.model.clone(),
        messages,
        temperature: Some(0.2), // Lower temperature for more consistent output
        max_tokens: Some(200),
        stream: Some(false),
        ..Default::default()
    };

    // Call LLM
    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    // Extract response
    let choice = response
        .choices
        .first()
        .ok_or_else(|| "Empty response from LLM".to_string())?;

    let content = choice
        .message
        .content
        .as_ref()
        .ok_or_else(|| "No content in response".to_string())?;

    // Clean up the response - remove quotes, trim whitespace
    let filter_expr = content
        .trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .trim()
        .to_string();

    // Validate the filter
    Filter::parse(&filter_expr).map_err(|e| format!("Invalid filter generated: {}", e))?;

    Ok(filter_expr)
}

/// Anomaly detection result.
#[derive(Debug, Clone)]
pub struct Anomaly {
    pub title: String,
    pub description: String,
    pub severity: AnomalySeverity,
    pub event_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
}

/// Detect anomalies in the event stream.
pub async fn detect_anomalies(
    events: &[DebugEvent],
    config: &prb_ai::AiConfig,
) -> Result<Vec<Anomaly>, String> {
    // Check rate limit
    RATE_LIMITER
        .check_rate_limit()
        .map_err(|e| format!("Rate limit: {}", e))?;

    // Build summary for AI
    let context = CaptureContext::build(events);
    let summary = build_anomaly_summary(events, &context);

    let system_prompt = r#"You are a network traffic analysis expert.
Analyze the packet capture summary and identify anomalies or issues.

Look for:
- High error rates or unusual status codes
- Repeated failures or retries
- Performance issues (high latency, timeouts)
- Unusual traffic patterns
- Security concerns

Output format (JSON array):
[
  {
    "title": "Brief title",
    "description": "Detailed description",
    "severity": "low|medium|high",
    "filter": "Filter to show related events"
  }
]

If no anomalies found, return empty array: []
"#;

    let user_message = format!("Analyze this capture:\n\n{}", summary);

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
        .map_err(|e| format!("API key error: {}", e))?;
    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&config.base_url);
    let client = Client::with_config(openai_config);

    // Create request
    let request = CreateChatCompletionRequest {
        model: config.model.clone(),
        messages,
        temperature: Some(0.3),
        max_tokens: Some(1000),
        stream: Some(false),
        ..Default::default()
    };

    // Call LLM
    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    // Extract response
    let choice = response
        .choices
        .first()
        .ok_or_else(|| "Empty response from LLM".to_string())?;

    let content = choice
        .message
        .content
        .as_ref()
        .ok_or_else(|| "No content in response".to_string())?;

    // Parse JSON response
    parse_anomaly_response(content, events)
}

/// Build a summary of events for anomaly detection.
pub fn build_anomaly_summary(events: &[DebugEvent], context: &CaptureContext) -> String {
    let mut summary = Vec::new();

    summary.push(format!("Total events: {}", context.total_events));
    summary.push(format!(
        "Transports: {}",
        context
            .transports
            .iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    // Count errors
    let error_count = events
        .iter()
        .filter(|e| {
            e.metadata
                .get("grpc.status")
                .map(|s| s != "0")
                .unwrap_or(false)
                || e.metadata
                    .get("http.status")
                    .and_then(|s| s.parse::<u16>().ok())
                    .map(|s| s >= 400)
                    .unwrap_or(false)
        })
        .count();
    if error_count > 0 {
        summary.push(format!(
            "Errors: {} ({:.1}%)",
            error_count,
            (error_count as f64 / events.len() as f64) * 100.0
        ));
    }

    // Sample metadata
    summary.push("\nSample metadata:".to_string());
    for (key, values) in context.sample_metadata.iter().take(10) {
        let value_str = values
            .iter()
            .take(3)
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(", ");
        summary.push(format!("  {}: {}", key, value_str));
    }

    summary.join("\n")
}

/// Parse anomaly detection response from AI.
pub fn parse_anomaly_response(
    content: &str,
    events: &[DebugEvent],
) -> Result<Vec<Anomaly>, String> {
    // Try to extract JSON from response
    let json_str = if let Some(start) = content.find('[') {
        if let Some(end) = content.rfind(']') {
            &content[start..=end]
        } else {
            content
        }
    } else {
        content
    };

    let parsed: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;

    let array = parsed
        .as_array()
        .ok_or_else(|| "Response is not an array".to_string())?;

    let mut anomalies = Vec::new();

    for item in array {
        let title = item["title"]
            .as_str()
            .unwrap_or("Unknown anomaly")
            .to_string();
        let description = item["description"]
            .as_str()
            .unwrap_or("No description")
            .to_string();
        let severity_str = item["severity"].as_str().unwrap_or("medium");
        let severity = match severity_str {
            "high" => AnomalySeverity::High,
            "low" => AnomalySeverity::Low,
            _ => AnomalySeverity::Medium,
        };

        // Try to apply filter to find relevant events
        let event_indices = if let Some(filter_str) = item["filter"].as_str() {
            if let Ok(filter) = Filter::parse(filter_str) {
                events
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| filter.matches(e))
                    .map(|(i, _)| i)
                    .take(100) // Limit to 100 events
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        anomalies.push(Anomaly {
            title,
            description,
            severity,
            event_indices,
        });
    }

    Ok(anomalies)
}

/// Protocol identification hint.
#[derive(Debug, Clone)]
pub struct ProtocolHint {
    pub protocol_name: String,
    pub confidence: f32,
    pub description: String,
}

/// Identify unknown protocol from payload.
pub async fn identify_protocol(
    payload_sample: &[u8],
    config: &prb_ai::AiConfig,
) -> Result<Vec<ProtocolHint>, String> {
    // Check rate limit
    RATE_LIMITER
        .check_rate_limit()
        .map_err(|e| format!("Rate limit: {}", e))?;

    // Format payload as hex dump (first 256 bytes)
    let sample_len = payload_sample.len().min(256);
    let hex_dump = payload_sample[..sample_len]
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if i % 16 == 0 {
                format!("\n{:04x}: {:02x}", i, b)
            } else {
                format!(" {:02x}", b)
            }
        })
        .collect::<String>();

    let system_prompt = r#"You are a protocol analysis expert.
Analyze the hex dump and identify the likely protocol.

Consider:
- Magic bytes and headers
- Structure and patterns
- Common protocol signatures
- Binary vs text encoding

Output format (JSON array):
[
  {
    "protocol": "Protocol name",
    "confidence": 0.0-1.0,
    "description": "Why this protocol"
  }
]

Order by confidence (highest first). Return up to 3 suggestions.
"#;

    let user_message = format!("Identify this protocol:\n{}", hex_dump);

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
        .map_err(|e| format!("API key error: {}", e))?;
    let openai_config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(&config.base_url);
    let client = Client::with_config(openai_config);

    // Create request
    let request = CreateChatCompletionRequest {
        model: config.model.clone(),
        messages,
        temperature: Some(0.4),
        max_tokens: Some(500),
        stream: Some(false),
        ..Default::default()
    };

    // Call LLM
    let response = client
        .chat()
        .create(request)
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    // Extract response
    let choice = response
        .choices
        .first()
        .ok_or_else(|| "Empty response from LLM".to_string())?;

    let content = choice
        .message
        .content
        .as_ref()
        .ok_or_else(|| "No content in response".to_string())?;

    // Parse JSON response
    parse_protocol_response(content)
}

/// Parse protocol identification response.
pub fn parse_protocol_response(content: &str) -> Result<Vec<ProtocolHint>, String> {
    // Try to extract JSON from response
    let json_str = if let Some(start) = content.find('[') {
        if let Some(end) = content.rfind(']') {
            &content[start..=end]
        } else {
            content
        }
    } else {
        content
    };

    let parsed: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {}", e))?;

    let array = parsed
        .as_array()
        .ok_or_else(|| "Response is not an array".to_string())?;

    let mut hints = Vec::new();

    for item in array {
        let protocol_name = item["protocol"].as_str().unwrap_or("Unknown").to_string();
        let confidence = item["confidence"].as_f64().unwrap_or(0.5) as f32;
        let description = item["description"]
            .as_str()
            .unwrap_or("No description")
            .to_string();

        hints.push(ProtocolHint {
            protocol_name,
            confidence,
            description,
        });
    }

    Ok(hints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(2, Duration::from_millis(100));
        assert!(limiter.check_rate_limit().is_ok());
        assert!(limiter.check_rate_limit().is_ok());
        assert!(limiter.check_rate_limit().is_err()); // Third should fail

        std::thread::sleep(Duration::from_millis(150));
        assert!(limiter.check_rate_limit().is_ok()); // Should work after window
    }

    #[test]
    fn test_capture_context() {
        use bytes::Bytes;
        use prb_core::{Direction, EventId, EventSource, Payload, Timestamp};
        use std::collections::BTreeMap;

        let mut metadata = BTreeMap::new();
        metadata.insert("grpc.method".to_string(), "/api/Test".to_string());

        let event = DebugEvent {
            id: EventId::from_raw(1),
            timestamp: Timestamp::from_nanos(1_000_000_000),
            source: EventSource {
                adapter: "test".to_string(),
                origin: "test.json".to_string(),
                network: None,
            },
            transport: TransportKind::Grpc,
            direction: Direction::Inbound,
            payload: Payload::Raw { raw: Bytes::new() },
            metadata,
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        };

        let context = CaptureContext::build(&[event]);
        assert_eq!(context.total_events, 1);
        assert!(
            context
                .available_fields
                .contains(&"grpc.method".to_string())
        );
    }
}
