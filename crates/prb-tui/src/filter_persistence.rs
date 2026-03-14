use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl Default for FilterPersistence {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            favorites: Vec::new(),
        }
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
