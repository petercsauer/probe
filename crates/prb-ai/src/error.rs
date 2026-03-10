/// Errors from the AI explanation engine.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AiError {
    #[error("provider not reachable: {0}")]
    ProviderUnreachable(String),

    #[error("API request failed: {0}")]
    ApiRequest(String),

    #[error("no events to explain")]
    NoEvents,

    #[error("event ID {0} not found in input")]
    EventNotFound(u64),

    #[error("missing API key for provider {0} (set PRB_AI_API_KEY or use --api-key)")]
    MissingApiKey(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("stream interrupted: {0}")]
    StreamInterrupted(String),
}
