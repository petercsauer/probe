use crate::error::AiError;
use serde::{Deserialize, Serialize};

/// AI provider selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    /// Local Ollama instance (default, privacy-first).
    Ollama,
    /// OpenAI API.
    OpenAi,
    /// Any OpenAI-compatible endpoint.
    Custom,
}

impl std::fmt::Display for AiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ollama => write!(f, "ollama"),
            Self::OpenAi => write!(f, "openai"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for AiProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ollama" => Ok(Self::Ollama),
            "openai" => Ok(Self::OpenAi),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("unknown provider: {s} (expected: ollama, openai, custom)")),
        }
    }
}

/// Configuration for the AI explanation engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: AiProvider,
    pub model: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
    pub context_window: usize,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: AiProvider::Ollama,
            model: "llama3.2".into(),
            base_url: "http://localhost:11434/v1".into(),
            api_key: None,
            temperature: 0.3,
            max_tokens: 2048,
            stream: true,
            context_window: 5,
        }
    }
}

impl AiConfig {
    pub fn for_provider(provider: AiProvider) -> Self {
        match provider {
            AiProvider::Ollama => Self::default(),
            AiProvider::OpenAi => Self {
                provider: AiProvider::OpenAi,
                model: "gpt-4o-mini".into(),
                base_url: "https://api.openai.com/v1".into(),
                ..Default::default()
            },
            AiProvider::Custom => Self {
                provider: AiProvider::Custom,
                model: "default".into(),
                base_url: "http://localhost:8080/v1".into(),
                ..Default::default()
            },
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    pub fn with_context_window(mut self, n: usize) -> Self {
        self.context_window = n;
        self
    }

    /// Resolve API key from config or environment.
    pub fn resolve_api_key(&self) -> Result<String, AiError> {
        if let Some(ref key) = self.api_key {
            return Ok(key.clone());
        }
        if let Ok(key) = std::env::var("PRB_AI_API_KEY") {
            return Ok(key);
        }
        match self.provider {
            // Ollama doesn't need a real API key
            AiProvider::Ollama => Ok("ollama".into()),
            _ => Err(AiError::MissingApiKey(self.provider.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults_ollama() {
        let config = AiConfig::default();
        assert_eq!(config.provider, AiProvider::Ollama);
        assert_eq!(config.base_url, "http://localhost:11434/v1");
        assert_eq!(config.model, "llama3.2");
        assert!(config.stream);
        assert_eq!(config.context_window, 5);
    }

    #[test]
    fn test_config_openai_requires_key() {
        let config = AiConfig::for_provider(AiProvider::OpenAi);
        std::env::remove_var("PRB_AI_API_KEY");
        assert!(config.resolve_api_key().is_err());
    }

    #[test]
    fn test_config_from_env() {
        std::env::set_var("PRB_AI_API_KEY", "test-key-123");
        let config = AiConfig::for_provider(AiProvider::OpenAi);
        assert_eq!(config.resolve_api_key().unwrap(), "test-key-123");
        std::env::remove_var("PRB_AI_API_KEY");
    }

    #[test]
    fn test_config_ollama_no_key_needed() {
        std::env::remove_var("PRB_AI_API_KEY");
        let config = AiConfig::default();
        assert_eq!(config.resolve_api_key().unwrap(), "ollama");
    }

    #[test]
    fn test_provider_from_str() {
        assert_eq!("ollama".parse::<AiProvider>().unwrap(), AiProvider::Ollama);
        assert_eq!("openai".parse::<AiProvider>().unwrap(), AiProvider::OpenAi);
        assert_eq!("custom".parse::<AiProvider>().unwrap(), AiProvider::Custom);
        assert!("invalid".parse::<AiProvider>().is_err());
    }

    #[test]
    fn test_config_builder_chain() {
        let config = AiConfig::for_provider(AiProvider::OpenAi)
            .with_model("gpt-4o")
            .with_api_key("sk-test")
            .with_temperature(0.7)
            .with_max_tokens(4096)
            .with_stream(false)
            .with_context_window(10);
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.api_key, Some("sk-test".into()));
        assert!((config.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(config.max_tokens, 4096);
        assert!(!config.stream);
        assert_eq!(config.context_window, 10);
    }
}
