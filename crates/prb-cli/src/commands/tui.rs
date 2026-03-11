use crate::cli::TuiArgs;
use anyhow::{Context, Result};
use prb_tui::loader::{load_events, load_schemas};
use prb_tui::{generate_demo_events, App, EventStore};

pub fn run_tui(args: TuiArgs) -> Result<()> {
    // Load events - either from demo mode or from file
    let store = if args.demo {
        let events = generate_demo_events();
        tracing::info!("Generated {} demo events", events.len());
        EventStore::from_events(events)
    } else {
        let input = args.input.as_ref().context("Input file required (or use --demo)")?;
        let path = std::path::PathBuf::from(input.as_str());
        load_events(&path).context("Failed to load events")?
    };

    tracing::info!("Loaded {} events", store.len());

    // Load schemas if provided (not applicable in demo mode)
    let schema_registry = if !args.demo && (!args.proto.is_empty() || !args.descriptor_set.is_empty()) {
        let input = args.input.as_ref().unwrap(); // Safe: we checked demo mode above
        let path = std::path::PathBuf::from(input.as_str());
        let proto_paths: Vec<std::path::PathBuf> = args.proto.iter().map(|p| p.as_std_path().to_path_buf()).collect();
        let desc_paths: Vec<std::path::PathBuf> = args.descriptor_set.iter().map(|p| p.as_std_path().to_path_buf()).collect();

        let mcap_path = if path.extension().and_then(|e| e.to_str()) == Some("mcap") {
            Some(path.as_path())
        } else {
            None
        };

        match load_schemas(&proto_paths, &desc_paths, mcap_path) {
            Ok(registry) => {
                let msg_count = registry.list_messages().len();
                tracing::info!("Loaded schema registry with {} message types", msg_count);
                Some(registry)
            }
            Err(e) => {
                tracing::warn!("Failed to load schemas: {}", e);
                None
            }
        }
    } else {
        None
    };

    let mut app = App::new(store, args.where_clause, schema_registry);
    app.run()
}
