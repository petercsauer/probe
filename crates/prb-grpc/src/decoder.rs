//! gRPC protocol decoder implementing the `ProtocolDecoder` trait.

use crate::h2::{H2Codec, H2Event};
use crate::lpm::{CompressionAlgorithm, LpmParser};
use bytes::Bytes;
use prb_core::{
    CoreError, CorrelationKey, DebugEvent, DebugEventBuilder, DecodeContext, Direction,
    METADATA_KEY_GRPC_METHOD, METADATA_KEY_H2_STREAM_ID, METADATA_KEY_OTEL_SPAN_ID,
    METADATA_KEY_OTEL_TRACE_FLAGS, METADATA_KEY_OTEL_TRACE_ID, METADATA_KEY_OTEL_TRACE_SAMPLED,
    Payload, ProtocolDecoder, TransportKind, extract_trace_context,
};
use std::collections::HashMap;

/// gRPC protocol decoder.
///
/// Decodes gRPC messages from reassembled TCP streams by:
/// 1. Parsing HTTP/2 frames
/// 2. Decompressing HPACK headers
/// 3. Extracting gRPC Length-Prefixed-Messages
/// 4. Decompressing message payloads
/// 5. Parsing trailers for status information
pub struct GrpcDecoder {
    /// HTTP/2 codec for frame parsing.
    h2_codec: H2Codec,
    /// Per-stream LPM parsers for extracting gRPC messages.
    lpm_parsers: HashMap<u32, LpmParser>,
    /// Sequence counter for events.
    sequence: u64,
}

impl GrpcDecoder {
    /// Create a new gRPC decoder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            h2_codec: H2Codec::new(),
            lpm_parsers: HashMap::new(),
            sequence: 0,
        }
    }

    /// Enrich an event builder with trace context from request headers.
    fn enrich_with_trace_context(
        mut builder: DebugEventBuilder,
        headers: &HashMap<String, String>,
    ) -> DebugEventBuilder {
        if let Some(trace_ctx) = extract_trace_context(headers) {
            builder = builder
                .metadata(METADATA_KEY_OTEL_TRACE_ID, &trace_ctx.trace_id)
                .metadata(METADATA_KEY_OTEL_SPAN_ID, &trace_ctx.span_id)
                .metadata(
                    METADATA_KEY_OTEL_TRACE_FLAGS,
                    trace_ctx.trace_flags.to_string(),
                )
                .metadata(
                    METADATA_KEY_OTEL_TRACE_SAMPLED,
                    trace_ctx.is_sampled().to_string(),
                )
                .correlation_key(CorrelationKey::TraceContext {
                    trace_id: trace_ctx.trace_id.clone(),
                    span_id: trace_ctx.span_id.clone(),
                });

            if let Some(ref tracestate) = trace_ctx.tracestate {
                builder = builder.metadata("otel.tracestate", tracestate);
            }
        }
        builder
    }

    /// Process HTTP/2 events and generate `DebugEvents`.
    fn process_h2_events(
        &mut self,
        events: Vec<H2Event>,
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        let mut debug_events = Vec::new();

        for event in events {
            match event {
                H2Event::Headers {
                    stream_id,
                    headers,
                    end_stream,
                } => {
                    // Handle headers in a block to avoid borrow issues
                    let should_emit_trailers = {
                        let stream = self.h2_codec.get_stream(stream_id);

                        // Determine if this is a request, response, or trailers
                        if !stream.saw_request_headers {
                            // Initial request headers
                            stream.request_headers = headers;
                            stream.saw_request_headers = true;
                            false
                        } else if !stream.saw_response_headers {
                            // Initial response headers
                            stream.response_headers = headers;
                            stream.saw_response_headers = true;

                            // Check for Trailers-Only response (end_stream with no data)
                            if end_stream {
                                // This is a Trailers-Only response - treat headers as trailers
                                stream.trailers = stream.response_headers.clone();
                                stream.closed = true;
                                true
                            } else {
                                false
                            }
                        } else if end_stream {
                            // Trailing headers with end_stream
                            stream.trailers = headers;
                            stream.closed = true;
                            true
                        } else {
                            false
                        }
                    };

                    // Emit event if needed (after releasing stream borrow)
                    if should_emit_trailers {
                        let stream = self.h2_codec.get_stream(stream_id);
                        let method_name = stream
                            .request_headers
                            .get(":path")
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string());
                        let grpc_status = stream
                            .trailers
                            .get("grpc-status")
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string());
                        let grpc_message = stream
                            .trailers
                            .get("grpc-message")
                            .cloned()
                            .unwrap_or_default();

                        if !grpc_status.is_empty()
                            && grpc_status != "unknown"
                            && let Some(event) = self.create_trailers_event(
                                stream_id,
                                method_name,
                                grpc_status,
                                grpc_message,
                                ctx,
                            )?
                        {
                            debug_events.push(event);
                        }
                    }
                }
                H2Event::Data {
                    stream_id,
                    data,
                    end_stream,
                } => {
                    // Extract compression algorithm first
                    let compression = {
                        let stream = self.h2_codec.get_stream(stream_id);
                        stream
                            .request_headers
                            .get("grpc-encoding")
                            .or_else(|| stream.response_headers.get("grpc-encoding"))
                            .map_or(CompressionAlgorithm::Identity, |s| {
                                CompressionAlgorithm::from_header(s)
                            })
                    };

                    // Get or create LPM parser
                    let parser = self
                        .lpm_parsers
                        .entry(stream_id)
                        .or_insert_with(|| LpmParser::new(compression));

                    // Feed data to LPM parser
                    let messages = parser
                        .feed(&data)
                        .map_err(|e| CoreError::PayloadDecode(e.to_string()))?;

                    // Create DebugEvents for each complete message
                    for message in messages {
                        let stream = self.h2_codec.get_stream(stream_id);
                        let method_name = stream
                            .request_headers
                            .get(":path")
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string());
                        let authority = stream
                            .request_headers
                            .get(":authority")
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_string());
                        // Direction: if we've seen response headers, DATA is for response (Inbound)
                        // Otherwise, DATA is for request (Outbound)
                        let is_request = !stream.saw_response_headers;

                        if let Some(event) = self.create_message_event(
                            stream_id,
                            method_name,
                            authority,
                            message.payload,
                            is_request,
                            ctx,
                        )? {
                            debug_events.push(event);
                        }
                    }

                    // Update end_stream flag
                    if end_stream {
                        let stream = self.h2_codec.get_stream(stream_id);
                        stream.closed = true;
                    }
                }
                H2Event::Settings => {
                    // SETTINGS frames don't produce DebugEvents
                }
                H2Event::RstStream { stream_id } => {
                    // Stream reset - mark closed
                    let stream = self.h2_codec.get_stream(stream_id);
                    stream.closed = true;
                }
                H2Event::GoAway => {
                    // Connection closed
                }
                H2Event::HpackDegraded { reason } => {
                    tracing::warn!("HPACK degradation: {}", reason);
                }
            }
        }

        Ok(debug_events)
    }

    /// Create a `DebugEvent` for a gRPC message.
    fn create_message_event(
        &mut self,
        stream_id: u32,
        method_name: String,
        authority: String,
        payload: Bytes,
        is_request: bool,
        ctx: &DecodeContext,
    ) -> Result<Option<DebugEvent>, CoreError> {
        self.sequence += 1;

        // Infer direction: request (with :method) = Outbound, response = Inbound
        let direction = if is_request {
            Direction::Outbound
        } else {
            Direction::Inbound
        };

        // Build event using context helper
        let mut event_builder = ctx
            .create_event_builder(TransportKind::Grpc)
            .direction(direction)
            .payload(Payload::Raw { raw: payload })
            .metadata(METADATA_KEY_GRPC_METHOD, &method_name)
            .metadata(METADATA_KEY_H2_STREAM_ID, stream_id.to_string())
            .metadata("grpc.authority", &authority)
            .correlation_key(CorrelationKey::StreamId { id: stream_id })
            .sequence(self.sequence);

        // Add HPACK degradation warning if applicable
        if self.h2_codec.is_hpack_degraded() {
            event_builder = event_builder.warning(
                "HPACK degradation: mid-stream capture, header decoding may be incomplete",
            );
        }

        // Extract trace context from request headers
        let stream = self.h2_codec.get_stream(stream_id);
        event_builder = Self::enrich_with_trace_context(event_builder, &stream.request_headers);

        Ok(Some(event_builder.build()))
    }

    /// Create a `DebugEvent` for gRPC trailers (status).
    fn create_trailers_event(
        &mut self,
        stream_id: u32,
        method_name: String,
        grpc_status: String,
        grpc_message: String,
        ctx: &DecodeContext,
    ) -> Result<Option<DebugEvent>, CoreError> {
        self.sequence += 1;

        // Build event for status using context helper
        let mut event_builder = ctx.create_event_builder(TransportKind::Grpc)
            .direction(Direction::Inbound) // Trailers/status are server responses
            .payload(Payload::Raw {
                raw: Bytes::from(format!("status={grpc_status} message={grpc_message}")),
            })
            .metadata(METADATA_KEY_GRPC_METHOD, &method_name)
            .metadata(METADATA_KEY_H2_STREAM_ID, stream_id.to_string())
            .metadata("grpc.status", &grpc_status)
            .metadata("grpc.message", &grpc_message)
            .correlation_key(CorrelationKey::StreamId { id: stream_id })
            .sequence(self.sequence);

        // Extract trace context from request headers
        let stream = self.h2_codec.get_stream(stream_id);
        event_builder = Self::enrich_with_trace_context(event_builder, &stream.request_headers);

        Ok(Some(event_builder.build()))
    }
}

impl Default for GrpcDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolDecoder for GrpcDecoder {
    fn protocol(&self) -> TransportKind {
        TransportKind::Grpc
    }

    fn decode_stream(
        &mut self,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        // Process data through HTTP/2 codec
        let h2_events = self
            .h2_codec
            .process(data)
            .map_err(|e| CoreError::PayloadDecode(e.to_string()))?;

        // Convert HTTP/2 events to DebugEvents
        self.process_h2_events(h2_events, ctx)
    }
}
