//! Native plugin loader.
//!
//! Loads shared libraries (.so/.dylib/.dll) and validates their exports.

use crate::error::PluginError;
use libloading::{Library, Symbol};
use prb_plugin_api::native::{ByteBuffer, DecoderHandle, DetectResultFfi, OwnedBuffer, PluginInfo};
use prb_plugin_api::{PluginMetadata, validate_api_version};
use std::ffi::CStr;
use std::path::Path;
use std::sync::Arc;

type InfoFn = extern "C" fn() -> PluginInfo;
type DetectFn = extern "C" fn(ByteBuffer, u16, u16, u8) -> DetectResultFfi;
type CreateFn = extern "C" fn() -> DecoderHandle;
type DecodeFn = extern "C" fn(DecoderHandle, ByteBuffer, ByteBuffer) -> OwnedBuffer;
type FreeFn = extern "C" fn(OwnedBuffer);
type DestroyFn = extern "C" fn(DecoderHandle);

/// A loaded native plugin with validated exports.
pub struct LoadedPlugin {
    /// The shared library handle.
    library: Arc<Library>,
    /// Plugin metadata.
    pub metadata: PluginMetadata,
    /// Cached function pointers.
    #[allow(dead_code)] // Kept for completeness, may be used in future
    info_fn: InfoFn,
    detect_fn: DetectFn,
    create_fn: CreateFn,
    decode_fn: DecodeFn,
    free_fn: FreeFn,
    destroy_fn: DestroyFn,
}

impl LoadedPlugin {
    /// Get plugin metadata.
    #[must_use] 
    pub const fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    /// Call the plugin's detect function.
    #[must_use] 
    pub fn detect(
        &self,
        data: &[u8],
        src_port: u16,
        dst_port: u16,
        transport: u8,
    ) -> DetectResultFfi {
        let buffer = ByteBuffer::from_slice(data);
        (self.detect_fn)(buffer, src_port, dst_port, transport)
    }

    /// Create a new decoder instance.
    #[must_use] 
    pub fn create_decoder(&self) -> DecoderHandle {
        (self.create_fn)()
    }

    /// Decode data using a decoder instance.
    pub fn decode(&self, handle: DecoderHandle, data: &[u8], ctx_json: &[u8]) -> OwnedBuffer {
        let data_buf = ByteBuffer::from_slice(data);
        let ctx_buf = ByteBuffer::from_slice(ctx_json);
        (self.decode_fn)(handle, data_buf, ctx_buf)
    }

    /// Free a buffer returned by decode.
    pub fn free_buffer(&self, buffer: OwnedBuffer) {
        (self.free_fn)(buffer);
    }

    /// Destroy a decoder instance.
    pub fn destroy_decoder(&self, handle: DecoderHandle) {
        (self.destroy_fn)(handle);
    }

    /// Get the shared library handle (for keeping the library alive).
    #[must_use] 
    pub fn library(&self) -> Arc<Library> {
        Arc::clone(&self.library)
    }
}

/// Loads and manages native plugins.
pub struct NativePluginLoader {
    loaded_plugins: Vec<Arc<LoadedPlugin>>,
}

impl NativePluginLoader {
    /// Create a new plugin loader.
    #[must_use] 
    pub const fn new() -> Self {
        Self {
            loaded_plugins: Vec::new(),
        }
    }

    /// Load a plugin from a shared library file.
    ///
    /// Validates:
    /// 1. Library loads without error
    /// 2. All required symbols are present
    /// 3. `prb_plugin_info()` returns valid metadata
    /// 4. API version is compatible
    pub fn load(&mut self, path: &Path) -> Result<Arc<LoadedPlugin>, PluginError> {
        tracing::info!(path = %path.display(), "Loading native plugin");

        // Load the library
        let library = unsafe { Library::new(path)? };

        // Look up required symbols
        let info_fn: Symbol<InfoFn> = unsafe {
            library
                .get(b"prb_plugin_info")
                .map_err(|_| PluginError::MissingSymbol("prb_plugin_info".to_string()))?
        };
        let detect_fn: Symbol<DetectFn> = unsafe {
            library
                .get(b"prb_plugin_detect")
                .map_err(|_| PluginError::MissingSymbol("prb_plugin_detect".to_string()))?
        };
        let create_fn: Symbol<CreateFn> = unsafe {
            library
                .get(b"prb_plugin_decoder_create")
                .map_err(|_| PluginError::MissingSymbol("prb_plugin_decoder_create".to_string()))?
        };
        let decode_fn: Symbol<DecodeFn> = unsafe {
            library
                .get(b"prb_plugin_decode")
                .map_err(|_| PluginError::MissingSymbol("prb_plugin_decode".to_string()))?
        };
        let free_fn: Symbol<FreeFn> = unsafe {
            library
                .get(b"prb_plugin_buffer_free")
                .map_err(|_| PluginError::MissingSymbol("prb_plugin_buffer_free".to_string()))?
        };
        let destroy_fn: Symbol<DestroyFn> = unsafe {
            library
                .get(b"prb_plugin_decoder_destroy")
                .map_err(|_| PluginError::MissingSymbol("prb_plugin_decoder_destroy".to_string()))?
        };

        // Call prb_plugin_info to get metadata
        let info = info_fn();

        // Convert C strings to Rust strings
        let name = unsafe {
            if info.name.is_null() {
                return Err(PluginError::NullPointer("name".to_string()));
            }
            CStr::from_ptr(info.name).to_str()?.to_string()
        };

        let version = unsafe {
            if info.version.is_null() {
                return Err(PluginError::NullPointer("version".to_string()));
            }
            CStr::from_ptr(info.version).to_str()?.to_string()
        };

        let description = unsafe {
            if info.description.is_null() {
                return Err(PluginError::NullPointer("description".to_string()));
            }
            CStr::from_ptr(info.description).to_str()?.to_string()
        };

        let api_version = unsafe {
            if info.api_version.is_null() {
                return Err(PluginError::NullPointer("api_version".to_string()));
            }
            CStr::from_ptr(info.api_version).to_str()?.to_string()
        };

        let protocol_id = unsafe {
            if info.protocol_id.is_null() {
                return Err(PluginError::NullPointer("protocol_id".to_string()));
            }
            CStr::from_ptr(info.protocol_id).to_str()?.to_string()
        };

        // Validate API version
        validate_api_version(&api_version).map_err(PluginError::IncompatibleVersion)?;

        let metadata = PluginMetadata {
            name,
            version,
            description,
            protocol_id,
            api_version,
        };

        tracing::info!(
            name = %metadata.name,
            version = %metadata.version,
            protocol = %metadata.protocol_id,
            "Plugin loaded successfully"
        );

        // Cache the function pointers by copying them
        let info_fn_cached = *info_fn;
        let detect_fn_cached = *detect_fn;
        let create_fn_cached = *create_fn;
        let decode_fn_cached = *decode_fn;
        let free_fn_cached = *free_fn;
        let destroy_fn_cached = *destroy_fn;

        let plugin = Arc::new(LoadedPlugin {
            library: Arc::new(library),
            metadata,
            info_fn: info_fn_cached,
            detect_fn: detect_fn_cached,
            create_fn: create_fn_cached,
            decode_fn: decode_fn_cached,
            free_fn: free_fn_cached,
            destroy_fn: destroy_fn_cached,
        });

        self.loaded_plugins.push(Arc::clone(&plugin));

        Ok(plugin)
    }

    /// Discover and load all plugins from a directory.
    ///
    /// Scans for files matching the platform's shared library extension:
    /// - Linux: `*.so`
    /// - macOS: `*.dylib`
    /// - Windows: `*.dll`
    pub fn load_directory(&mut self, dir: &Path) -> Vec<Result<Arc<LoadedPlugin>, PluginError>> {
        let extension = if cfg!(target_os = "linux") {
            "so"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else if cfg!(target_os = "windows") {
            "dll"
        } else {
            "so" // fallback
        };

        tracing::info!(
            dir = %dir.display(),
            extension = extension,
            "Scanning for native plugins"
        );

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read plugin directory");
                return vec![Err(PluginError::Io(e))];
            }
        };

        let mut results = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some(extension) {
                tracing::debug!(path = %path.display(), "Found potential plugin");
                results.push(self.load(&path));
            }
        }

        results
    }

    /// Get all loaded plugins.
    #[must_use] 
    pub fn plugins(&self) -> &[Arc<LoadedPlugin>] {
        &self.loaded_plugins
    }
}

impl Default for NativePluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Send and Sync for LoadedPlugin since the function pointers are stateless
unsafe impl Send for LoadedPlugin {}
unsafe impl Sync for LoadedPlugin {}
