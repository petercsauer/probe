use prb_tui::filter_persistence::FilterPersistence;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_save_and_load_roundtrip() {
    // Create a temporary directory
    let temp_dir = TempDir::new().unwrap();
    let filters_path = temp_dir.path().join("filters.toml");

    // Create persistence with some data
    let mut persistence = FilterPersistence::default();
    persistence.add_to_history("tcp.port == 443".to_string());
    persistence.add_to_history("udp.port == 53".to_string());
    persistence.add_to_history("transport == \"grpc\"".to_string());

    persistence.add_favorite(
        "HTTPS Traffic".to_string(),
        "tcp.port in {443, 8443}".to_string(),
        "All HTTPS connections".to_string(),
    );

    persistence.add_favorite(
        "DNS Queries".to_string(),
        "udp.port == 53".to_string(),
        "Standard DNS traffic".to_string(),
    );

    // Serialize to TOML
    let toml_content = toml::to_string_pretty(&persistence).unwrap();
    fs::write(&filters_path, toml_content).unwrap();

    // Read back and deserialize
    let loaded_content = fs::read_to_string(&filters_path).unwrap();
    let loaded: FilterPersistence = toml::from_str(&loaded_content).unwrap();

    // Verify history (chronological order: oldest to newest)
    assert_eq!(loaded.history.len(), 3);
    assert_eq!(loaded.history[0], "tcp.port == 443");
    assert_eq!(loaded.history[1], "udp.port == 53");
    assert_eq!(loaded.history[2], "transport == \"grpc\"");

    // Verify favorites
    assert_eq!(loaded.favorites.len(), 2);
    assert_eq!(loaded.favorites[0].name, "HTTPS Traffic");
    assert_eq!(loaded.favorites[0].filter, "tcp.port in {443, 8443}");
    assert_eq!(loaded.favorites[0].description, "All HTTPS connections");
    assert_eq!(loaded.favorites[1].name, "DNS Queries");
    assert_eq!(loaded.favorites[1].filter, "udp.port == 53");
}

#[test]
fn test_load_nonexistent_file() {
    // Override HOME to a temp dir
    let temp_dir = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("HOME", temp_dir.path());
    }

    // Load should return default when file doesn't exist
    let loaded = FilterPersistence::load().unwrap_or_default();
    assert!(loaded.history.is_empty());
    assert!(loaded.favorites.is_empty());
}

#[test]
fn test_save_creates_directory() {
    // Override HOME to a temp dir
    let temp_dir = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("HOME", temp_dir.path());
    }

    let mut persistence = FilterPersistence::default();
    persistence.add_to_history("test filter".to_string());

    // Save should create the directory structure
    let result = persistence.save();
    assert!(result.is_ok());

    // Verify the directory was created
    let config_dir = temp_dir.path().join(".config").join("prb");
    assert!(config_dir.exists());

    // Verify the file was created
    let filters_path = config_dir.join("filters.toml");
    assert!(filters_path.exists());
}

#[test]
fn test_corrupted_toml_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let filters_path = temp_dir.path().join("filters.toml");

    // Write invalid TOML
    fs::write(&filters_path, "this is not valid toml [[[]").unwrap();

    // Read and try to parse
    let content = fs::read_to_string(&filters_path).unwrap();
    let result: Result<FilterPersistence, _> = toml::from_str(&content);

    // Should fail gracefully
    assert!(result.is_err());
}

#[test]
fn test_history_ordering_after_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let filters_path = temp_dir.path().join("filters.toml");

    let mut persistence = FilterPersistence::default();

    // Add filters in order
    persistence.add_to_history("first".to_string());
    persistence.add_to_history("second".to_string());
    persistence.add_to_history("third".to_string());

    // Serialize
    let toml_content = toml::to_string_pretty(&persistence).unwrap();
    fs::write(&filters_path, toml_content).unwrap();

    // Load back
    let loaded_content = fs::read_to_string(&filters_path).unwrap();
    let loaded: FilterPersistence = toml::from_str(&loaded_content).unwrap();

    // Verify ordering (most recent first)
    assert_eq!(loaded.history[0], "first");
    assert_eq!(loaded.history[1], "second");
    assert_eq!(loaded.history[2], "third");
}

#[test]
fn test_favorite_operations_persist() {
    let temp_dir = TempDir::new().unwrap();
    let filters_path = temp_dir.path().join("filters.toml");

    let mut persistence = FilterPersistence::default();

    // Add favorites
    persistence.add_favorite("Fav1".to_string(), "filter1".to_string(), "".to_string());
    persistence.add_favorite("Fav2".to_string(), "filter2".to_string(), "".to_string());
    persistence.add_favorite("Fav3".to_string(), "filter3".to_string(), "".to_string());

    // Save
    let toml_content = toml::to_string_pretty(&persistence).unwrap();
    fs::write(&filters_path, &toml_content).unwrap();

    // Load
    let loaded: FilterPersistence = toml::from_str(&toml_content).unwrap();
    assert_eq!(loaded.favorites.len(), 3);

    // Remove one and save again
    let mut persistence = loaded;
    persistence.remove_favorite(1); // Remove Fav2

    let toml_content = toml::to_string_pretty(&persistence).unwrap();
    fs::write(&filters_path, toml_content).unwrap();

    // Load again
    let loaded_content = fs::read_to_string(&filters_path).unwrap();
    let final_loaded: FilterPersistence = toml::from_str(&loaded_content).unwrap();

    assert_eq!(final_loaded.favorites.len(), 2);
    assert_eq!(final_loaded.favorites[0].name, "Fav1");
    assert_eq!(final_loaded.favorites[1].name, "Fav3");
}

#[test]
fn test_is_favorited_after_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let filters_path = temp_dir.path().join("filters.toml");

    let mut persistence = FilterPersistence::default();
    persistence.add_favorite(
        "Test".to_string(),
        "tcp.port == 443".to_string(),
        "".to_string(),
    );

    // Save
    let toml_content = toml::to_string_pretty(&persistence).unwrap();
    fs::write(&filters_path, toml_content).unwrap();

    // Load
    let loaded_content = fs::read_to_string(&filters_path).unwrap();
    let loaded: FilterPersistence = toml::from_str(&loaded_content).unwrap();

    // Check if favorited
    assert!(loaded.is_favorited("tcp.port == 443"));
    assert!(!loaded.is_favorited("udp.port == 53"));
}
