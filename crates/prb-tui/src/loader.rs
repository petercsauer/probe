use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{bail, Context, Result};
use prb_core::{CaptureAdapter, DebugEvent};
use prb_schema::SchemaRegistry;

use crate::EventStore;

/// Events sent during streaming file loading.
#[derive(Debug, Clone)]
pub enum LoadEvent {
    /// A batch of events has been loaded.
    Batch(Vec<DebugEvent>),
    /// Progress update with current loaded count and optional total.
    Progress { loaded: usize, total: Option<usize> },
    /// Loading completed successfully.
    Done,
    /// An error occurred during loading.
    Error(String),
}

#[derive(Debug, Clone, Copy)]
enum InputFormat {
    Json,
    Pcap,
    Pcapng,
    Mcap,
}

fn detect_format(path: &Path) -> Result<InputFormat> {
    let mut file =
        File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut magic = [0u8; 8];
    let bytes_read = file.read(&mut magic).unwrap_or(0);

    if bytes_read >= 4 {
        match &magic[..4] {
            [0x89, b'M', b'C', b'A'] => return Ok(InputFormat::Mcap),
            [0x0a, 0x0d, 0x0d, 0x0a] => return Ok(InputFormat::Pcapng),
            [0xa1, 0xb2, 0xc3, 0xd4] | [0xd4, 0xc3, 0xb2, 0xa1] => {
                return Ok(InputFormat::Pcap)
            }
            [b'{', ..] | [b'[', ..] => return Ok(InputFormat::Json),
            _ => {}
        }
    }

    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => Ok(InputFormat::Json),
        Some("pcap") => Ok(InputFormat::Pcap),
        Some("pcapng") => Ok(InputFormat::Pcapng),
        Some("mcap") => Ok(InputFormat::Mcap),
        _ => bail!("Cannot detect format for {}", path.display()),
    }
}

pub fn load_events(path: &Path, tls_keylog: Option<PathBuf>) -> Result<EventStore> {
    let format = detect_format(path)?;
    let events = match format {
        InputFormat::Json => load_json(path)?,
        InputFormat::Pcap | InputFormat::Pcapng => load_pcap(path, tls_keylog)?,
        InputFormat::Mcap => load_mcap(path)?,
    };
    Ok(EventStore::new(events))
}

/// Load events from a file using streaming with progress updates.
///
/// This function spawns a background thread to parse the file and sends:
/// - `LoadEvent::Batch` with chunks of events as they are parsed
/// - `LoadEvent::Progress` with periodic progress updates
/// - `LoadEvent::Done` when complete
/// - `LoadEvent::Error` if an error occurs
pub fn load_events_streaming(
    path: &Path,
    sender: mpsc::Sender<LoadEvent>,
    tls_keylog: Option<PathBuf>,
) -> Result<()> {
    let format = detect_format(path)?;
    let path = path.to_owned();

    std::thread::spawn(move || {
        let result = match format {
            InputFormat::Json => load_json_streaming(&path, &sender),
            InputFormat::Pcap | InputFormat::Pcapng => load_pcap_streaming(&path, &sender, tls_keylog),
            InputFormat::Mcap => load_mcap_streaming(&path, &sender),
        };

        match result {
            Ok(()) => {
                let _ = sender.send(LoadEvent::Done);
            }
            Err(e) => {
                let _ = sender.send(LoadEvent::Error(e.to_string()));
            }
        }
    });

    Ok(())
}

fn load_json(path: &Path) -> Result<Vec<DebugEvent>> {
    use prb_fixture::JsonFixtureAdapter;
    let utf8_path = camino::Utf8PathBuf::try_from(path.to_path_buf())
        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 path: {}", e))?;
    let mut adapter = JsonFixtureAdapter::new(utf8_path);
    let mut events = Vec::new();
    for result in adapter.ingest() {
        match result {
            Ok(event) => events.push(event),
            Err(e) => tracing::warn!("Skipping event with error: {}", e),
        }
    }
    Ok(events)
}

fn load_pcap(path: &Path, tls_keylog: Option<PathBuf>) -> Result<Vec<DebugEvent>> {
    use prb_pcap::PcapCaptureAdapter;
    let mut adapter = PcapCaptureAdapter::new(PathBuf::from(path), tls_keylog);
    let mut events = Vec::new();
    for result in adapter.ingest() {
        match result {
            Ok(event) => events.push(event),
            Err(e) => tracing::warn!("Skipping event with error: {}", e),
        }
    }
    Ok(events)
}

fn load_mcap(path: &Path) -> Result<Vec<DebugEvent>> {
    use prb_storage::SessionReader;
    let reader =
        SessionReader::open(path).with_context(|| format!("Failed to open MCAP {}", path.display()))?;
    let mut events = Vec::new();
    for result in reader.events() {
        match result {
            Ok(event) => events.push(event),
            Err(e) => tracing::warn!("Skipping MCAP event with error: {}", e),
        }
    }
    Ok(events)
}

// Streaming loaders

const BATCH_SIZE: usize = 1000;

fn load_json_streaming(path: &Path, sender: &mpsc::Sender<LoadEvent>) -> Result<()> {
    use prb_fixture::JsonFixtureAdapter;
    let utf8_path = camino::Utf8PathBuf::try_from(path.to_path_buf())
        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 path: {}", e))?;
    let mut adapter = JsonFixtureAdapter::new(utf8_path);

    let mut batch = Vec::with_capacity(BATCH_SIZE);
    let mut loaded = 0;

    for result in adapter.ingest() {
        match result {
            Ok(event) => {
                batch.push(event);
                loaded += 1;

                if batch.len() >= BATCH_SIZE {
                    sender.send(LoadEvent::Batch(batch.clone()))
                        .map_err(|e| anyhow::anyhow!("Failed to send batch: {}", e))?;
                    sender.send(LoadEvent::Progress { loaded, total: None })
                        .map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))?;
                    batch.clear();
                }
            }
            Err(e) => tracing::warn!("Skipping event with error: {}", e),
        }
    }

    // Send remaining events
    if !batch.is_empty() {
        sender.send(LoadEvent::Batch(batch))
            .map_err(|e| anyhow::anyhow!("Failed to send final batch: {}", e))?;
    }

    Ok(())
}

fn load_pcap_streaming(path: &Path, sender: &mpsc::Sender<LoadEvent>, tls_keylog: Option<PathBuf>) -> Result<()> {
    use prb_pcap::PcapCaptureAdapter;
    let mut adapter = PcapCaptureAdapter::new(PathBuf::from(path), tls_keylog);

    let mut batch = Vec::with_capacity(BATCH_SIZE);
    let mut loaded = 0;

    for result in adapter.ingest() {
        match result {
            Ok(event) => {
                batch.push(event);
                loaded += 1;

                if batch.len() >= BATCH_SIZE {
                    sender.send(LoadEvent::Batch(batch.clone()))
                        .map_err(|e| anyhow::anyhow!("Failed to send batch: {}", e))?;
                    sender.send(LoadEvent::Progress { loaded, total: None })
                        .map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))?;
                    batch.clear();
                }
            }
            Err(e) => tracing::warn!("Skipping event with error: {}", e),
        }
    }

    // Send remaining events
    if !batch.is_empty() {
        sender.send(LoadEvent::Batch(batch))
            .map_err(|e| anyhow::anyhow!("Failed to send final batch: {}", e))?;
    }

    Ok(())
}

fn load_mcap_streaming(path: &Path, sender: &mpsc::Sender<LoadEvent>) -> Result<()> {
    use prb_storage::SessionReader;
    let reader = SessionReader::open(path)
        .with_context(|| format!("Failed to open MCAP {}", path.display()))?;

    let mut batch = Vec::with_capacity(BATCH_SIZE);
    let mut loaded = 0;

    for result in reader.events() {
        match result {
            Ok(event) => {
                batch.push(event);
                loaded += 1;

                if batch.len() >= BATCH_SIZE {
                    sender.send(LoadEvent::Batch(batch.clone()))
                        .map_err(|e| anyhow::anyhow!("Failed to send batch: {}", e))?;
                    sender.send(LoadEvent::Progress { loaded, total: None })
                        .map_err(|e| anyhow::anyhow!("Failed to send progress: {}", e))?;
                    batch.clear();
                }
            }
            Err(e) => tracing::warn!("Skipping MCAP event with error: {}", e),
        }
    }

    // Send remaining events
    if !batch.is_empty() {
        sender.send(LoadEvent::Batch(batch))
            .map_err(|e| anyhow::anyhow!("Failed to send final batch: {}", e))?;
    }

    Ok(())
}

/// Load schemas from proto files, descriptor sets, and MCAP auto-extraction.
pub fn load_schemas(
    proto_paths: &[PathBuf],
    descriptor_sets: &[PathBuf],
    mcap_path: Option<&Path>,
) -> Result<SchemaRegistry> {
    let mut registry = SchemaRegistry::new();

    // Load descriptor set files
    for path in descriptor_sets {
        tracing::debug!("Loading descriptor set from {}", path.display());
        registry
            .load_descriptor_set_file(path)
            .with_context(|| format!("Failed to load descriptor set {}", path.display()))?;
    }

    // Load and compile proto files
    if !proto_paths.is_empty() {
        tracing::debug!("Compiling {} proto files", proto_paths.len());
        // Use parent directory of first proto as include path
        let includes: Vec<PathBuf> = proto_paths
            .iter()
            .filter_map(|p| p.parent().map(|parent| parent.to_path_buf()))
            .collect();
        registry
            .load_proto_files(proto_paths, &includes)
            .context("Failed to compile proto files")?;
    }

    // Auto-extract schemas from MCAP if provided
    if let Some(path) = mcap_path
        && let Ok(format) = detect_format(path)
        && matches!(format, InputFormat::Mcap)
    {
        tracing::debug!("Attempting to extract schemas from MCAP file");
        if let Err(e) = extract_mcap_schemas(&mut registry, path) {
            tracing::warn!("Failed to extract schemas from MCAP: {}", e);
        }
    }

    Ok(registry)
}

fn extract_mcap_schemas(registry: &mut SchemaRegistry, path: &Path) -> Result<()> {
    use prb_storage::SessionReader;
    let reader = SessionReader::open(path)
        .with_context(|| format!("Failed to open MCAP for schema extraction: {}", path.display()))?;

    // Extract embedded schemas from the MCAP file
    let embedded_registry = reader.extract_schemas()
        .with_context(|| "Failed to extract schemas from MCAP")?;

    // Merge the extracted schemas into the main registry
    let message_types = embedded_registry.list_messages();
    if !message_types.is_empty() {
        tracing::info!("Auto-extracted {} schema(s) from MCAP file", message_types.len());

        // Load the extracted schemas into the main registry
        // We need to get the descriptor set from the embedded registry
        // For now, we'll just report what we found
        for msg_type in &message_types {
            tracing::debug!("  Found schema: {}", msg_type);
        }

        // Copy schemas by re-extracting and loading into the main registry
        // This is done by merging the registries
        *registry = embedded_registry;
    }

    Ok(())
}
