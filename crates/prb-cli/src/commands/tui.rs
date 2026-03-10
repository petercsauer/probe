use crate::cli::TuiArgs;
use anyhow::{Context, Result};
use prb_tui::loader::load_events;
use prb_tui::App;

pub fn run_tui(args: TuiArgs) -> Result<()> {
    let path = std::path::PathBuf::from(args.input.as_str());
    let store = load_events(&path).context("Failed to load events")?;

    tracing::info!("Loaded {} events", store.len());

    let mut app = App::new(store, args.where_clause);
    app.run()
}
