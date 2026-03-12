use crate::context::ExplainContext;
use prb_core::TransportKind;

const ROLE_PREAMBLE: &str = "\
You are Probe, an expert network protocol analyzer built in Rust. \
You explain decoded network events in plain English, helping developers \
debug communication issues between services. \
You are precise, concise, and always ground your explanations in the actual data provided.";

const GROUNDING_RULES: &str = "\
CRITICAL RULES:
- ONLY reference information present in the provided event data
- Cite specific field values (method names, status codes, addresses) when making claims
- If something is uncertain or ambiguous, explicitly say so
- Never invent packet data or field values not present in the context
- Suggest concrete, actionable next debugging steps";

const GRPC_KNOWLEDGE: &str = "\
PROTOCOL KNOWLEDGE — gRPC over HTTP/2:

gRPC Status Codes (from grpc/grpc specification):
- 0 OK: Success
- 1 CANCELLED: Operation cancelled by caller
- 2 UNKNOWN: Unknown error (often from exception in server handler)
- 3 INVALID_ARGUMENT: Client sent invalid argument
- 4 DEADLINE_EXCEEDED: Deadline expired before operation completed
- 5 NOT_FOUND: Requested entity not found
- 6 ALREADY_EXISTS: Entity already exists
- 7 PERMISSION_DENIED: Caller lacks permission
- 8 RESOURCE_EXHAUSTED: Resource quota or rate limit exceeded
- 9 FAILED_PRECONDITION: System not in required state for operation
- 10 ABORTED: Operation aborted (typically concurrency issue)
- 11 OUT_OF_RANGE: Operation attempted past valid range
- 12 UNIMPLEMENTED: Method not implemented
- 13 INTERNAL: Internal server error
- 14 UNAVAILABLE: Service unavailable (transient, retry with backoff)
- 15 DATA_LOSS: Unrecoverable data loss or corruption
- 16 UNAUTHENTICATED: Missing or invalid authentication credentials

HTTP/2 Error Codes (RFC 9113):
- NO_ERROR (0x0): Graceful shutdown
- PROTOCOL_ERROR (0x1): Generic protocol error
- INTERNAL_ERROR (0x2): Implementation fault
- FLOW_CONTROL_ERROR (0x3): Flow control violation
- SETTINGS_TIMEOUT (0x4): Settings not acknowledged in time
- STREAM_CLOSED (0x5): Frame received on closed stream
- FRAME_SIZE_ERROR (0x6): Invalid frame size
- REFUSED_STREAM (0x7): Stream refused before processing
- CANCEL (0x8): Stream cancelled
- ENHANCE_YOUR_CALM (0xB): Rate limiting / too many requests

Common gRPC failure patterns:
- RST_STREAM after many requests → server-side rate limiting (ENHANCE_YOUR_CALM)
- DEADLINE_EXCEEDED with slow responses → insufficient deadline or server overload
- UNAVAILABLE with connection reset → service crash or network partition
- UNIMPLEMENTED → client/server version mismatch on proto definition
- Request → long delay → CANCELLED → client timeout or context cancellation";

const ZMQ_KNOWLEDGE: &str = "\
PROTOCOL KNOWLEDGE — ZeroMQ (ZMTP 3.x):

Socket Type Patterns (from ZMQ RFC 23/28):
- REQ/REP: Synchronous request-reply (strict alternation)
- PUB/SUB: Publish-subscribe with topic-based filtering
- PUSH/PULL: Pipeline (fan-out/fan-in)
- DEALER/ROUTER: Asynchronous request-reply
- PAIR: Exclusive pair (1:1 bidirectional)

ZMTP Handshake Stages:
1. Greeting: 10+ bytes signature, version, mechanism
2. Handshake: READY command with socket-type and identity
3. Traffic: Message frames (single or multipart)

Common ZMQ issues:
- Slow subscriber: SUB can't keep up → messages dropped at HWM (default 1000)
- Identity collision: Two sockets with same identity → undefined behavior
- Missing subscription: SUB without setsockopt(ZMQ_SUBSCRIBE) → receives nothing
- Multipart boundary: Incomplete multipart message → application-level corruption
- Socket type mismatch: Connecting incompatible socket types → handshake failure";

const DDS_KNOWLEDGE: &str = "\
PROTOCOL KNOWLEDGE — DDS RTPS (OMG DDS-RTPS v2.5):

RTPS Discovery Protocol:
- Simple Participant Discovery Protocol (SPDP): Multicast announcements
- Simple Endpoint Discovery Protocol (SEDP): Unicast endpoint matching
- Domain ID → port mapping: port = 7400 + 250 * domainId + offset

Key RTPS Submessage Types:
- DATA: User data with serialized payload
- DATA_FRAG: Fragmented user data
- HEARTBEAT: Writer → reader reliability signal
- ACKNACK: Reader → writer acknowledgment
- GAP: Writer → reader declaring sequence numbers irrelevant
- INFO_TS: Timestamp for subsequent submessages
- INFO_DST: Destination for subsequent submessages

Common DDS issues:
- Missing discovery: Participant not found → domain ID mismatch or multicast blocked
- QoS mismatch: Incompatible Reliability/Durability → no data delivery
- Fragmentation gaps: Missing DATA_FRAG → incomplete sample, NACK retry
- Deadline missed: Writer not publishing at expected rate
- Liveliness lost: Participant stopped sending keepalives";

const TLS_KNOWLEDGE: &str = "\
PROTOCOL KNOWLEDGE — TLS:

TLS Handshake Stages:
1. ClientHello: Client proposes cipher suites, extensions
2. ServerHello: Server selects cipher suite
3. Certificate: Server sends certificate chain
4. Key Exchange: ECDHE/DHE key agreement
5. Finished: Both sides verify handshake integrity

Common TLS issues:
- Decryption failure: Wrong SSLKEYLOGFILE or key mismatch → raw encrypted bytes
- Certificate errors: Expired, self-signed, wrong hostname
- Cipher suite mismatch: Client/server have no common cipher → handshake failure
- TLS version mismatch: Client requires TLS 1.3, server only supports 1.2";

const TCP_KNOWLEDGE: &str = "\
PROTOCOL KNOWLEDGE — TCP:

TCP Connection Lifecycle:
- SYN → SYN-ACK → ACK: Three-way handshake
- FIN → ACK → FIN → ACK: Graceful close
- RST: Abrupt connection termination

Common TCP issues:
- RST immediately after SYN: Port not open, firewall blocking
- RST during data transfer: Application crash or protocol error
- Retransmissions: Network congestion or packet loss
- Zero window: Receiver buffer full → sender must wait
- Out-of-order segments: Network path changes or load balancer issues";

const TASK_TEMPLATE: &str = "\
Explain what is happening in the TARGET EVENT below. Structure your response as:

1. **What happened**: Plain-English summary of this event
2. **Analysis**: Whether anything looks abnormal or concerning, with specific evidence
3. **Root cause** (if error): Most likely causes ranked by probability
4. **Next steps**: 2-3 concrete debugging actions the developer should take

If surrounding context events are provided, use them to identify patterns \
(repeated errors, request/response pairs, timing anomalies).";

/// Build the complete system prompt with protocol-specific grounding.
#[must_use]
pub fn build_system_prompt(transport: TransportKind, has_tls: bool) -> String {
    let mut prompt = String::with_capacity(4096);

    prompt.push_str(ROLE_PREAMBLE);
    prompt.push_str("\n\n");

    match transport {
        TransportKind::Grpc => prompt.push_str(GRPC_KNOWLEDGE),
        TransportKind::Zmq => prompt.push_str(ZMQ_KNOWLEDGE),
        TransportKind::DdsRtps => prompt.push_str(DDS_KNOWLEDGE),
        TransportKind::RawTcp => prompt.push_str(TCP_KNOWLEDGE),
        _ => {}
    }

    if has_tls {
        prompt.push_str("\n\n");
        prompt.push_str(TLS_KNOWLEDGE);
    }

    prompt.push_str("\n\n");
    prompt.push_str(GROUNDING_RULES);

    prompt
}

/// Build the user message from the explain context.
#[must_use]
pub fn build_user_message(context: &ExplainContext) -> String {
    let mut msg = String::with_capacity(2048);

    msg.push_str(TASK_TEMPLATE);
    msg.push_str("\n\n");

    msg.push_str("═══ TARGET EVENT ═══\n");
    msg.push_str(&context.target_summary);
    msg.push('\n');

    if !context.surrounding_summaries.is_empty() {
        msg.push_str("\n═══ SURROUNDING CONTEXT ═══\n");
        for (i, summary) in context.surrounding_summaries.iter().enumerate() {
            msg.push_str(&format!(
                "--- Context event {}/{} ---\n",
                i + 1,
                context.surrounding_summaries.len()
            ));
            msg.push_str(summary);
            msg.push('\n');
        }
    }

    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_template_grpc() {
        let prompt = build_system_prompt(TransportKind::Grpc, false);
        assert!(prompt.contains("gRPC Status Codes"));
        assert!(prompt.contains("HTTP/2 Error Codes"));
        assert!(prompt.contains("ENHANCE_YOUR_CALM"));
        assert!(prompt.contains("CRITICAL RULES"));
        assert!(!prompt.contains("ZeroMQ"));
        assert!(!prompt.contains("DDS RTPS"));
    }

    #[test]
    fn test_prompt_template_zmq() {
        let prompt = build_system_prompt(TransportKind::Zmq, false);
        assert!(prompt.contains("ZeroMQ"));
        assert!(prompt.contains("PUB/SUB"));
        assert!(prompt.contains("Slow subscriber"));
        assert!(!prompt.contains("gRPC Status Codes"));
    }

    #[test]
    fn test_prompt_template_dds() {
        let prompt = build_system_prompt(TransportKind::DdsRtps, false);
        assert!(prompt.contains("DDS RTPS"));
        assert!(prompt.contains("SEDP"));
        assert!(prompt.contains("HEARTBEAT"));
        assert!(!prompt.contains("gRPC Status Codes"));
    }

    #[test]
    fn test_prompt_template_tls() {
        let prompt = build_system_prompt(TransportKind::Grpc, true);
        assert!(prompt.contains("gRPC Status Codes"));
        assert!(prompt.contains("TLS Handshake Stages"));
        assert!(prompt.contains("SSLKEYLOGFILE"));
    }

    #[test]
    fn test_prompt_template_tcp() {
        let prompt = build_system_prompt(TransportKind::RawTcp, false);
        assert!(prompt.contains("TCP Connection Lifecycle"));
        assert!(prompt.contains("RST"));
    }

    #[test]
    fn test_user_message_structure() {
        let ctx = ExplainContext {
            target_summary: "test target event".into(),
            surrounding_summaries: vec!["context event 1".into(), "context event 2".into()],
            transport: TransportKind::Grpc,
            has_errors: false,
            has_warnings: false,
        };
        let msg = build_user_message(&ctx);
        assert!(msg.contains("TARGET EVENT"));
        assert!(msg.contains("test target event"));
        assert!(msg.contains("SURROUNDING CONTEXT"));
        assert!(msg.contains("context event 1"));
        assert!(msg.contains("Context event 1/2"));
    }

    #[test]
    fn test_user_message_no_context() {
        let ctx = ExplainContext {
            target_summary: "single event".into(),
            surrounding_summaries: vec![],
            transport: TransportKind::Zmq,
            has_errors: false,
            has_warnings: false,
        };
        let msg = build_user_message(&ctx);
        assert!(msg.contains("single event"));
        assert!(!msg.contains("SURROUNDING CONTEXT"));
    }
}
