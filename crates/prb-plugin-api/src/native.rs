//! Native plugin FFI types and helpers.
//!
//! This module provides the C ABI types and macros for native (shared library) plugins.

use std::os::raw::{c_char, c_void};

/// Plugin metadata returned by `prb_plugin_info()`.
///
/// All string fields are null-terminated UTF-8 C strings. The plugin is
/// responsible for keeping these strings alive for the lifetime of the plugin.
#[repr(C)]
pub struct PluginInfo {
    /// Plugin name (null-terminated UTF-8).
    pub name: *const c_char,
    /// Plugin version (null-terminated UTF-8, semver).
    pub version: *const c_char,
    /// Plugin description (null-terminated UTF-8).
    pub description: *const c_char,
    /// API version this plugin was compiled against.
    pub api_version: *const c_char,
    /// Protocol ID this plugin handles (null-terminated UTF-8).
    pub protocol_id: *const c_char,
}

/// Result of a detect call.
#[repr(C)]
pub struct DetectResultFfi {
    /// Whether the plugin can handle this data (0 = no, 1 = yes).
    pub detected: u8,
    /// Confidence (0.0 - 1.0).
    pub confidence: f32,
}

/// Buffer for passing data across the FFI boundary.
///
/// The buffer is not owned by the recipient; the data pointer is only
/// valid for the duration of the function call.
#[repr(C)]
pub struct ByteBuffer {
    pub ptr: *const u8,
    pub len: usize,
}

impl ByteBuffer {
    /// Create a ByteBuffer from a byte slice.
    ///
    /// # Safety
    /// The returned buffer is only valid as long as the slice remains valid.
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            ptr: data.as_ptr(),
            len: data.len(),
        }
    }

    /// Convert the buffer to a byte slice.
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid and the length is correct.
    pub unsafe fn as_slice<'a>(&self) -> &'a [u8] {
        if self.ptr.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }
}

/// Owned buffer allocated by the plugin, freed by the host.
///
/// The plugin allocates this buffer using `Vec::into_raw_parts()` and the
/// host frees it by calling `prb_plugin_buffer_free()`.
#[repr(C)]
pub struct OwnedBuffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

impl OwnedBuffer {
    /// Create an owned buffer from a Vec.
    ///
    /// # Safety
    /// The caller must ensure the buffer is freed using `prb_plugin_buffer_free()`.
    pub fn from_vec(mut vec: Vec<u8>) -> Self {
        let ptr = vec.as_mut_ptr();
        let len = vec.len();
        let capacity = vec.capacity();
        std::mem::forget(vec); // Don't drop the vec, let the host free it
        Self { ptr, len, capacity }
    }

    /// Convert the owned buffer back to a Vec.
    ///
    /// # Safety
    /// The caller must ensure the buffer was allocated via `Vec::into_raw_parts()`
    /// or `OwnedBuffer::from_vec()`, and this must only be called once.
    pub unsafe fn into_vec(self) -> Vec<u8> {
        unsafe { Vec::from_raw_parts(self.ptr, self.len, self.capacity) }
    }

    /// Create an empty owned buffer.
    pub fn empty() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }
}

/// Opaque decoder handle.
///
/// The plugin creates this handle in `prb_plugin_decoder_create()` and
/// the host passes it to subsequent `prb_plugin_decode()` calls.
pub type DecoderHandle = *mut c_void;

/// Safe Rust trait that native plugin authors implement.
///
/// The `prb_export_plugin!` macro generates the C FFI wrappers around
/// implementations of this trait.
pub trait PluginDecoder: Send {
    /// Return plugin metadata.
    fn info() -> crate::PluginMetadata
    where
        Self: Sized;

    /// Detect whether this plugin can decode the given data.
    ///
    /// Returns `Some(confidence)` where confidence is in [0.0, 1.0] if
    /// the plugin can handle this data, or `None` if it cannot.
    fn detect(ctx: &crate::DetectContext) -> Option<f32>
    where
        Self: Sized;

    /// Create a new decoder instance.
    fn new() -> Self
    where
        Self: Sized;

    /// Decode a byte stream chunk.
    ///
    /// Returns a vector of decoded events, or an error message on failure.
    fn decode(
        &mut self,
        data: &[u8],
        ctx: &crate::DecodeCtx,
    ) -> Result<Vec<crate::DebugEventDto>, String>;
}

/// Macro to implement all required C ABI exports from a Rust struct.
///
/// # Usage
///
/// ```no_run
/// use prb_plugin_api::*;
/// use prb_plugin_api::native::*;
///
/// struct MyDecoder { /* ... */ }
///
/// impl PluginDecoder for MyDecoder {
///     fn info() -> PluginMetadata {
///         PluginMetadata {
///             name: "my-decoder".into(),
///             version: "0.1.0".into(),
///             description: "Custom decoder".into(),
///             protocol_id: "custom".into(),
///             api_version: prb_plugin_api::API_VERSION.into(),
///         }
///     }
///
///     fn detect(ctx: &DetectContext) -> Option<f32> {
///         None
///     }
///
///     fn new() -> Self {
///         Self { /* ... */ }
///     }
///
///     fn decode(&mut self, data: &[u8], ctx: &DecodeCtx) -> Result<Vec<DebugEventDto>, String> {
///         Ok(vec![])
///     }
/// }
///
/// prb_export_plugin!(MyDecoder);
/// ```
#[macro_export]
macro_rules! prb_export_plugin {
    ($decoder_type:ty) => {
        use std::ffi::CString;
        use std::os::raw::{c_char, c_void};
        use std::panic::{AssertUnwindSafe, catch_unwind};

        // Static strings for plugin info (must outlive the plugin)
        static mut PLUGIN_NAME: Option<CString> = None;
        static mut PLUGIN_VERSION: Option<CString> = None;
        static mut PLUGIN_DESCRIPTION: Option<CString> = None;
        static mut PLUGIN_API_VERSION: Option<CString> = None;
        static mut PLUGIN_PROTOCOL_ID: Option<CString> = None;

        /// Initialize plugin metadata strings.
        fn init_plugin_strings() {
            use $crate::native::PluginDecoder;

            let info = <$decoder_type>::info();

            unsafe {
                PLUGIN_NAME = Some(CString::new(info.name).expect("null byte in plugin name"));
                PLUGIN_VERSION =
                    Some(CString::new(info.version).expect("null byte in plugin version"));
                PLUGIN_DESCRIPTION =
                    Some(CString::new(info.description).expect("null byte in plugin description"));
                PLUGIN_API_VERSION =
                    Some(CString::new(info.api_version).expect("null byte in API version"));
                PLUGIN_PROTOCOL_ID =
                    Some(CString::new(info.protocol_id).expect("null byte in protocol ID"));
            }
        }

        #[no_mangle]
        pub extern "C" fn prb_plugin_info() -> $crate::native::PluginInfo {
            init_plugin_strings();

            unsafe {
                $crate::native::PluginInfo {
                    name: PLUGIN_NAME
                        .as_ref()
                        .expect("plugin name not initialized")
                        .as_ptr(),
                    version: PLUGIN_VERSION
                        .as_ref()
                        .expect("plugin version not initialized")
                        .as_ptr(),
                    description: PLUGIN_DESCRIPTION
                        .as_ref()
                        .expect("plugin description not initialized")
                        .as_ptr(),
                    api_version: PLUGIN_API_VERSION
                        .as_ref()
                        .expect("API version not initialized")
                        .as_ptr(),
                    protocol_id: PLUGIN_PROTOCOL_ID
                        .as_ref()
                        .expect("protocol ID not initialized")
                        .as_ptr(),
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn prb_plugin_detect(
            data: $crate::native::ByteBuffer,
            src_port: u16,
            dst_port: u16,
            transport: u8,
        ) -> $crate::native::DetectResultFfi {
            use $crate::native::PluginDecoder;

            let result = catch_unwind(AssertUnwindSafe(|| {
                let data_slice = unsafe { data.as_slice() };
                let transport = match transport {
                    0 => $crate::TransportLayer::Tcp,
                    1 => $crate::TransportLayer::Udp,
                    _ => return None,
                };

                let ctx = $crate::DetectContext {
                    initial_bytes: data_slice.to_vec(),
                    src_port,
                    dst_port,
                    transport,
                    #[cfg(feature = "wasm-pdk")]
                    initial_bytes_b64: String::new(),
                };

                <$decoder_type>::detect(&ctx)
            }));

            match result {
                Ok(Some(confidence)) => $crate::native::DetectResultFfi {
                    detected: 1,
                    confidence: confidence.clamp(0.0, 1.0),
                },
                _ => $crate::native::DetectResultFfi {
                    detected: 0,
                    confidence: 0.0,
                },
            }
        }

        #[no_mangle]
        pub extern "C" fn prb_plugin_decoder_create() -> *mut c_void {
            use $crate::native::PluginDecoder;

            let result = catch_unwind(AssertUnwindSafe(|| {
                let decoder = <$decoder_type>::new();
                Box::into_raw(Box::new(decoder)) as *mut c_void
            }));

            result.unwrap_or(std::ptr::null_mut())
        }

        #[no_mangle]
        pub extern "C" fn prb_plugin_decode(
            handle: *mut c_void,
            data: $crate::native::ByteBuffer,
            ctx_json: $crate::native::ByteBuffer,
        ) -> $crate::native::OwnedBuffer {
            use $crate::native::PluginDecoder;

            let result = catch_unwind(AssertUnwindSafe(|| {
                if handle.is_null() {
                    return $crate::native::OwnedBuffer::empty();
                }

                let decoder = unsafe { &mut *(handle as *mut $decoder_type) };
                let data_slice = unsafe { data.as_slice() };
                let ctx_json_slice = unsafe { ctx_json.as_slice() };

                // Deserialize context
                let ctx: $crate::DecodeCtx = match serde_json::from_slice(ctx_json_slice) {
                    Ok(ctx) => ctx,
                    Err(_) => return $crate::native::OwnedBuffer::empty(),
                };

                // Decode
                let events = match decoder.decode(data_slice, &ctx) {
                    Ok(events) => events,
                    Err(_) => return $crate::native::OwnedBuffer::empty(),
                };

                // Serialize events to JSON
                let json = match serde_json::to_vec(&events) {
                    Ok(json) => json,
                    Err(_) => return $crate::native::OwnedBuffer::empty(),
                };

                $crate::native::OwnedBuffer::from_vec(json)
            }));

            result.unwrap_or_else(|_| $crate::native::OwnedBuffer::empty())
        }

        #[no_mangle]
        pub extern "C" fn prb_plugin_buffer_free(buf: $crate::native::OwnedBuffer) {
            if !buf.ptr.is_null() {
                let _ = catch_unwind(AssertUnwindSafe(|| {
                    let _vec = unsafe { buf.into_vec() };
                    // Vec is dropped here
                }));
            }
        }

        #[no_mangle]
        pub extern "C" fn prb_plugin_decoder_destroy(handle: *mut c_void) {
            if !handle.is_null() {
                let _ = catch_unwind(AssertUnwindSafe(|| {
                    let _decoder = unsafe { Box::from_raw(handle as *mut $decoder_type) };
                    // Decoder is dropped here
                }));
            }
        }
    };
}
