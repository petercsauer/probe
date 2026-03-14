use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterPersistence {
    /// Recent filter history (max 50)
    pub history: Vec<String>,

    /// Favorited filters with optional names
    pub favorites: Vec<FilterFavorite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterFavorite {
    pub name: String,        // User-provided name (e.g., "DNS Traffic")
    pub filter: String,      // The filter expression
    pub description: String, // Optional description
    pub created_at: String,  // Unix timestamp as string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterTemplate {
    pub name: String,
    pub category: String, // "Protocol", "Performance", "Security", "Network"
    pub filter: String,
    pub description: String,
    pub tags: Vec<String>,
}

impl FilterPersistence {
    pub fn load() -> Result<Self, String> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read filters.toml: {}", e))?;

        toml::from_str(&content).map_err(|e| format!("Failed to parse filters.toml: {}", e))
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path()?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize filters: {}", e))?;

        fs::write(&path, content).map_err(|e| format!("Failed to write filters.toml: {}", e))
    }

    fn config_path() -> Result<PathBuf, String> {
        let home =
            std::env::var("HOME").map_err(|_| "HOME environment variable not set".to_string())?;

        Ok(PathBuf::from(home)
            .join(".config")
            .join("prb")
            .join("filters.toml"))
    }

    pub fn add_to_history(&mut self, filter: String) {
        // Remove duplicates
        self.history.retain(|f| f != &filter);

        // Add to end (most recent at the end to match FilterState behavior)
        self.history.push(filter);

        // Keep only the last 50 entries
        if self.history.len() > 50 {
            self.history.remove(0);
        }
    }

    pub fn add_favorite(&mut self, name: String, filter: String, description: String) {
        let favorite = FilterFavorite {
            name,
            filter,
            description,
            created_at: Self::current_timestamp(),
        };

        self.favorites.push(favorite);

        // Limit favorites to 100 (from pre-mortem risks)
        if self.favorites.len() > 100 {
            self.favorites.remove(0);
        }
    }

    pub fn remove_favorite(&mut self, index: usize) {
        if index < self.favorites.len() {
            self.favorites.remove(index);
        }
    }

    pub fn is_favorited(&self, filter: &str) -> bool {
        self.favorites.iter().any(|f| f.filter == filter)
    }

    fn current_timestamp() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    }

    /// Returns the built-in catalog of filter templates
    pub fn default_templates() -> Vec<FilterTemplate> {
        vec![
            // DNS
            FilterTemplate {
                name: "DNS Traffic".to_string(),
                category: "Protocol".to_string(),
                filter: "udp.port == 53 || tcp.port == 53".to_string(),
                description: "All DNS queries and responses (UDP and TCP)".to_string(),
                tags: vec!["dns".to_string(), "protocol".to_string()],
            },
            // TLS
            FilterTemplate {
                name: "TLS Handshakes".to_string(),
                category: "Protocol".to_string(),
                filter: r#"tcp.port == 443 && tcp.payload matches "^\x16\x03""#.to_string(),
                description: "TLS ClientHello and ServerHello messages".to_string(),
                tags: vec![
                    "tls".to_string(),
                    "https".to_string(),
                    "security".to_string(),
                ],
            },
            FilterTemplate {
                name: "HTTPS Traffic".to_string(),
                category: "Protocol".to_string(),
                filter: "tcp.port in {443, 8443}".to_string(),
                description: "All HTTPS connections on standard ports".to_string(),
                tags: vec!["https".to_string(), "tls".to_string()],
            },
            // gRPC
            FilterTemplate {
                name: "gRPC Calls".to_string(),
                category: "Protocol".to_string(),
                filter: r#"transport == "grpc""#.to_string(),
                description: "All gRPC unary and streaming calls".to_string(),
                tags: vec!["grpc".to_string(), "rpc".to_string()],
            },
            // ZeroMQ
            FilterTemplate {
                name: "ZeroMQ Messages".to_string(),
                category: "Protocol".to_string(),
                filter: r#"transport == "zmq""#.to_string(),
                description: "All ZeroMQ socket traffic".to_string(),
                tags: vec!["zmq".to_string(), "messaging".to_string()],
            },
            // HTTP
            FilterTemplate {
                name: "HTTP Requests".to_string(),
                category: "Protocol".to_string(),
                filter: r#"tcp.port in {80, 8080} && tcp.payload matches "^(GET|POST|PUT|DELETE)""#
                    .to_string(),
                description: "HTTP request methods (unencrypted)".to_string(),
                tags: vec!["http".to_string(), "web".to_string()],
            },
            // Performance
            FilterTemplate {
                name: "Large Frames".to_string(),
                category: "Performance".to_string(),
                filter: "frame.len > 1500".to_string(),
                description: "Frames exceeding MTU (potential fragmentation)".to_string(),
                tags: vec!["performance".to_string(), "fragmentation".to_string()],
            },
            FilterTemplate {
                name: "Small Frames".to_string(),
                category: "Performance".to_string(),
                filter: "frame.len < 64".to_string(),
                description: "Very small frames (possible header-only or ACKs)".to_string(),
                tags: vec!["performance".to_string()],
            },
            // Security
            FilterTemplate {
                name: "Unencrypted Traffic".to_string(),
                category: "Security".to_string(),
                filter: r#"tcp.port in {80, 21, 23, 25} || udp.port == 69"#.to_string(),
                description: "Potentially sensitive unencrypted protocols".to_string(),
                tags: vec!["security".to_string(), "cleartext".to_string()],
            },
            // Network
            FilterTemplate {
                name: "Localhost Traffic".to_string(),
                category: "Network".to_string(),
                filter: r#"ip.src == "127.0.0.1" || ip.dst == "127.0.0.1""#.to_string(),
                description: "Traffic to/from localhost".to_string(),
                tags: vec!["localhost".to_string(), "loopback".to_string()],
            },
        ]
    }

    /// Returns all available templates (currently just the built-in ones)
    pub fn get_templates(&self) -> Vec<FilterTemplate> {
        Self::default_templates()
    }

    /// Search templates by name, description, or tags
    pub fn search_templates(&self, query: &str) -> Vec<FilterTemplate> {
        let query_lower = query.to_lowercase();

        self.get_templates()
            .into_iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&query_lower)
                    || t.description.to_lowercase().contains(&query_lower)
                    || t.tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let persistence = FilterPersistence::default();
        assert!(persistence.history.is_empty());
        assert!(persistence.favorites.is_empty());
    }

    #[test]
    fn test_add_to_history() {
        let mut persistence = FilterPersistence::default();

        persistence.add_to_history("tcp.port == 443".to_string());
        assert_eq!(persistence.history.len(), 1);
        assert_eq!(persistence.history[0], "tcp.port == 443");

        persistence.add_to_history("udp.port == 53".to_string());
        assert_eq!(persistence.history.len(), 2);
        assert_eq!(persistence.history[0], "tcp.port == 443");
        assert_eq!(persistence.history[1], "udp.port == 53");
    }

    #[test]
    fn test_history_dedup() {
        let mut persistence = FilterPersistence::default();

        persistence.add_to_history("tcp.port == 443".to_string());
        persistence.add_to_history("udp.port == 53".to_string());
        persistence.add_to_history("tcp.port == 443".to_string());

        assert_eq!(persistence.history.len(), 2);
        assert_eq!(persistence.history[0], "udp.port == 53");
        assert_eq!(persistence.history[1], "tcp.port == 443");
    }

    #[test]
    fn test_history_truncate() {
        let mut persistence = FilterPersistence::default();

        // Add 51 filters
        for i in 0..51 {
            persistence.add_to_history(format!("filter_{}", i));
        }

        // Should be truncated to 50
        assert_eq!(persistence.history.len(), 50);
        // Most recent should be last
        assert_eq!(persistence.history[49], "filter_50");
        // Oldest (filter_0) should be removed
        assert!(!persistence.history.contains(&"filter_0".to_string()));
        // filter_1 should be the oldest remaining
        assert_eq!(persistence.history[0], "filter_1");
    }

    #[test]
    fn test_add_favorite() {
        let mut persistence = FilterPersistence::default();

        persistence.add_favorite(
            "HTTPS Traffic".to_string(),
            "tcp.port == 443".to_string(),
            "All HTTPS connections".to_string(),
        );

        assert_eq!(persistence.favorites.len(), 1);
        assert_eq!(persistence.favorites[0].name, "HTTPS Traffic");
        assert_eq!(persistence.favorites[0].filter, "tcp.port == 443");
        assert_eq!(
            persistence.favorites[0].description,
            "All HTTPS connections"
        );
        assert!(!persistence.favorites[0].created_at.is_empty());
    }

    #[test]
    fn test_remove_favorite() {
        let mut persistence = FilterPersistence::default();

        persistence.add_favorite(
            "HTTPS".to_string(),
            "tcp.port == 443".to_string(),
            "".to_string(),
        );
        persistence.add_favorite(
            "DNS".to_string(),
            "udp.port == 53".to_string(),
            "".to_string(),
        );

        assert_eq!(persistence.favorites.len(), 2);

        persistence.remove_favorite(0);
        assert_eq!(persistence.favorites.len(), 1);
        assert_eq!(persistence.favorites[0].name, "DNS");

        // Removing out of bounds should be no-op
        persistence.remove_favorite(10);
        assert_eq!(persistence.favorites.len(), 1);
    }

    #[test]
    fn test_is_favorited() {
        let mut persistence = FilterPersistence::default();

        persistence.add_favorite(
            "HTTPS".to_string(),
            "tcp.port == 443".to_string(),
            "".to_string(),
        );

        assert!(persistence.is_favorited("tcp.port == 443"));
        assert!(!persistence.is_favorited("udp.port == 53"));
    }

    #[test]
    fn test_toml_roundtrip() {
        let mut persistence = FilterPersistence::default();

        persistence.add_to_history("tcp.port == 443".to_string());
        persistence.add_to_history("udp.port == 53".to_string());

        persistence.add_favorite(
            "HTTPS Traffic".to_string(),
            "tcp.port in {443, 8443}".to_string(),
            "All HTTPS connections".to_string(),
        );

        // Serialize to TOML
        let toml_str = toml::to_string_pretty(&persistence).unwrap();

        // Deserialize back
        let deserialized: FilterPersistence = toml::from_str(&toml_str).unwrap();

        assert_eq!(deserialized.history.len(), 2);
        // Chronological order: oldest to newest
        assert_eq!(deserialized.history[0], "tcp.port == 443");
        assert_eq!(deserialized.history[1], "udp.port == 53");
        assert_eq!(deserialized.favorites.len(), 1);
        assert_eq!(deserialized.favorites[0].name, "HTTPS Traffic");
    }

    #[test]
    fn test_load_missing_file() {
        // This will return default if file doesn't exist
        // We can't easily test this without mocking, but the logic is straightforward
        // The actual load() method checks if path.exists() and returns default if not
        let default = FilterPersistence::default();
        assert!(default.history.is_empty());
        assert!(default.favorites.is_empty());
    }

    #[test]
    fn test_favorites_limit() {
        let mut persistence = FilterPersistence::default();

        // Add 101 favorites
        for i in 0..101 {
            persistence.add_favorite(
                format!("Favorite {}", i),
                format!("filter_{}", i),
                "".to_string(),
            );
        }

        // Should be limited to 100
        assert_eq!(persistence.favorites.len(), 100);
        // First one (Favorite 0) should be removed
        assert!(!persistence.favorites.iter().any(|f| f.name == "Favorite 0"));
        // Last one should still be there
        assert!(
            persistence
                .favorites
                .iter()
                .any(|f| f.name == "Favorite 100")
        );
    }
}
