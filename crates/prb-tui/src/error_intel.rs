//! Error intelligence — static lookup tables for protocol error codes, TCP states, and TLS alerts.
//!
//! Provides human-readable explanations for common protocol errors without requiring LLMs
//! or external services. All lookups are O(1) match statements on static data.

/// Returns the canonical gRPC status code name for a numeric code.
///
/// Maps codes 0-16 to their official gRPC status names.
/// Returns `None` for unknown or invalid codes.
pub fn grpc_status_name(code: u32) -> Option<&'static str> {
    match code {
        0 => Some("OK"),
        1 => Some("CANCELLED"),
        2 => Some("UNKNOWN"),
        3 => Some("INVALID_ARGUMENT"),
        4 => Some("DEADLINE_EXCEEDED"),
        5 => Some("NOT_FOUND"),
        6 => Some("ALREADY_EXISTS"),
        7 => Some("PERMISSION_DENIED"),
        8 => Some("RESOURCE_EXHAUSTED"),
        9 => Some("FAILED_PRECONDITION"),
        10 => Some("ABORTED"),
        11 => Some("OUT_OF_RANGE"),
        12 => Some("UNIMPLEMENTED"),
        13 => Some("INTERNAL"),
        14 => Some("UNAVAILABLE"),
        15 => Some("DATA_LOSS"),
        16 => Some("UNAUTHENTICATED"),
        _ => None,
    }
}

/// Returns a human-readable explanation for common gRPC error codes.
///
/// Only provides explanations for error codes (non-zero) that commonly need
/// human interpretation. Returns `None` for success codes and uncommon errors.
pub fn grpc_status_explanation(code: u32) -> Option<&'static str> {
    match code {
        4 => Some("The deadline expired before the operation completed"),
        7 => Some("Caller lacks permission for this operation"),
        8 => Some("Server resource limit reached (quota, memory, connections)"),
        13 => Some("Internal server error — check server logs"),
        14 => Some("Server unavailable — may be starting up or overloaded"),
        _ => None,
    }
}

/// Returns a human-readable explanation for TCP flags.
///
/// Provides diagnostic context for common TCP control flags that indicate
/// connection state changes or errors.
pub fn tcp_flag_explanation(flags: &str) -> Option<&'static str> {
    match flags {
        "RST" | "R" => Some("Connection forcefully terminated. Causes: crashed server, firewall, port not listening"),
        "FIN" | "F" => Some("Graceful connection close initiated"),
        "SYN" | "S" => Some("Connection establishment request"),
        "SYN-ACK" | "SA" => Some("Connection establishment acknowledgment"),
        _ => None,
    }
}

/// Returns a human-readable description for TLS alert codes.
///
/// Maps TLS alert values (0-255) to their RFC 8446 / RFC 5246 names and
/// diagnostic descriptions. Covers all common alerts seen in production.
pub fn tls_alert_description(code: u8) -> Option<&'static str> {
    match code {
        0 => Some("close_notify — Connection closing normally"),
        10 => Some("unexpected_message — Inappropriate message received"),
        20 => Some("bad_record_mac — Record failed integrity check"),
        21 => Some("decryption_failed — Decryption error (deprecated, use bad_record_mac)"),
        22 => Some("record_overflow — Record exceeded maximum size"),
        30 => Some("decompression_failure — Decompression failed (deprecated)"),
        40 => Some("handshake_failure — No common cipher suite or parameters"),
        41 => Some("no_certificate — No certificate provided (SSL 3.0 only)"),
        42 => Some("bad_certificate — Certificate is corrupt"),
        43 => Some("unsupported_certificate — Certificate type not supported"),
        44 => Some("certificate_revoked — Certificate was revoked by CA"),
        45 => Some("certificate_expired — Certificate has expired"),
        46 => Some("certificate_unknown — Unknown certificate issue"),
        47 => Some("illegal_parameter — Handshake field out of range"),
        48 => Some("unknown_ca — CA certificate not recognized"),
        49 => Some("access_denied — Valid cert but access denied by policy"),
        50 => Some("decode_error — Message could not be decoded"),
        51 => Some("decrypt_error — Decryption or signature verification failed"),
        60 => Some("export_restriction — Export restrictions violated (deprecated)"),
        70 => Some("protocol_version — Protocol version not supported"),
        71 => Some("insufficient_security — Cipher suite too weak"),
        80 => Some("internal_error — Internal error unrelated to peer"),
        86 => Some("inappropriate_fallback — Downgrade attack detected"),
        90 => Some("user_canceled — Handshake canceled by user"),
        100 => Some("no_renegotiation — Renegotiation not supported"),
        109 => Some("missing_extension — Required extension missing"),
        110 => Some("unsupported_extension — Extension not supported"),
        111 => Some("certificate_unobtainable — Could not obtain certificate"),
        112 => Some("unrecognized_name — Server name (SNI) not recognized"),
        113 => Some("bad_certificate_status_response — OCSP response invalid"),
        114 => Some("bad_certificate_hash_value — Certificate hash mismatch"),
        115 => Some("unknown_psk_identity — PSK identity unknown"),
        116 => Some("certificate_required — Certificate required but not provided"),
        120 => Some("no_application_protocol — No common ALPN protocol"),
        _ => None,
    }
}

/// Returns a human-readable explanation for HTTP status codes.
///
/// Covers common HTTP client and server errors that appear in gRPC-over-HTTP/2 traces.
pub fn http_status_explanation(code: u16) -> Option<&'static str> {
    match code {
        400 => Some("Bad Request — Malformed syntax"),
        401 => Some("Unauthorized — Authentication required"),
        403 => Some("Forbidden — Authentication succeeded but access denied"),
        404 => Some("Not Found — Resource does not exist"),
        408 => Some("Request Timeout — Client took too long to send request"),
        429 => Some("Too Many Requests — Rate limit exceeded"),
        500 => Some("Internal Server Error — Unhandled server exception"),
        502 => Some("Bad Gateway — Upstream server returned invalid response"),
        503 => Some("Service Unavailable — Server overloaded or down"),
        504 => Some("Gateway Timeout — Upstream server did not respond in time"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_status_name_all_standard_codes() {
        assert_eq!(grpc_status_name(0), Some("OK"));
        assert_eq!(grpc_status_name(1), Some("CANCELLED"));
        assert_eq!(grpc_status_name(2), Some("UNKNOWN"));
        assert_eq!(grpc_status_name(3), Some("INVALID_ARGUMENT"));
        assert_eq!(grpc_status_name(4), Some("DEADLINE_EXCEEDED"));
        assert_eq!(grpc_status_name(5), Some("NOT_FOUND"));
        assert_eq!(grpc_status_name(6), Some("ALREADY_EXISTS"));
        assert_eq!(grpc_status_name(7), Some("PERMISSION_DENIED"));
        assert_eq!(grpc_status_name(8), Some("RESOURCE_EXHAUSTED"));
        assert_eq!(grpc_status_name(9), Some("FAILED_PRECONDITION"));
        assert_eq!(grpc_status_name(10), Some("ABORTED"));
        assert_eq!(grpc_status_name(11), Some("OUT_OF_RANGE"));
        assert_eq!(grpc_status_name(12), Some("UNIMPLEMENTED"));
        assert_eq!(grpc_status_name(13), Some("INTERNAL"));
        assert_eq!(grpc_status_name(14), Some("UNAVAILABLE"));
        assert_eq!(grpc_status_name(15), Some("DATA_LOSS"));
        assert_eq!(grpc_status_name(16), Some("UNAUTHENTICATED"));
    }

    #[test]
    fn test_grpc_status_name_unknown_codes() {
        assert_eq!(grpc_status_name(17), None);
        assert_eq!(grpc_status_name(99), None);
        assert_eq!(grpc_status_name(255), None);
    }

    #[test]
    fn test_grpc_status_explanation_common_errors() {
        assert!(grpc_status_explanation(4).is_some());
        assert!(grpc_status_explanation(7).is_some());
        assert!(grpc_status_explanation(8).is_some());
        assert!(grpc_status_explanation(13).is_some());
        assert!(grpc_status_explanation(14).is_some());
    }

    #[test]
    fn test_grpc_status_explanation_no_explanation_for_success() {
        assert_eq!(grpc_status_explanation(0), None);
    }

    #[test]
    fn test_grpc_status_explanation_not_all_errors_have_explanations() {
        // Some error codes don't have custom explanations
        assert_eq!(grpc_status_explanation(1), None); // CANCELLED
        assert_eq!(grpc_status_explanation(2), None); // UNKNOWN
    }

    #[test]
    fn test_tcp_flag_explanation_common_flags() {
        assert!(tcp_flag_explanation("RST").is_some());
        assert!(tcp_flag_explanation("R").is_some());
        assert!(tcp_flag_explanation("FIN").is_some());
        assert!(tcp_flag_explanation("F").is_some());
        assert!(tcp_flag_explanation("SYN").is_some());
        assert!(tcp_flag_explanation("SYN-ACK").is_some());
    }

    #[test]
    fn test_tcp_flag_explanation_unknown_flags() {
        assert_eq!(tcp_flag_explanation("ACK"), None);
        assert_eq!(tcp_flag_explanation("PSH"), None);
        assert_eq!(tcp_flag_explanation("URG"), None);
    }

    #[test]
    fn test_tls_alert_description_common_alerts() {
        assert_eq!(tls_alert_description(0), Some("close_notify — Connection closing normally"));
        assert_eq!(tls_alert_description(40), Some("handshake_failure — No common cipher suite or parameters"));
        assert_eq!(tls_alert_description(42), Some("bad_certificate — Certificate is corrupt"));
        assert_eq!(tls_alert_description(45), Some("certificate_expired — Certificate has expired"));
        assert_eq!(tls_alert_description(48), Some("unknown_ca — CA certificate not recognized"));
        assert_eq!(tls_alert_description(70), Some("protocol_version — Protocol version not supported"));
    }

    #[test]
    fn test_tls_alert_description_all_rfc_alerts() {
        // Test a representative sample of all RFC-defined alerts
        assert!(tls_alert_description(0).is_some());   // close_notify
        assert!(tls_alert_description(10).is_some());  // unexpected_message
        assert!(tls_alert_description(20).is_some());  // bad_record_mac
        assert!(tls_alert_description(40).is_some());  // handshake_failure
        assert!(tls_alert_description(80).is_some());  // internal_error
        assert!(tls_alert_description(86).is_some());  // inappropriate_fallback
        assert!(tls_alert_description(112).is_some()); // unrecognized_name
        assert!(tls_alert_description(120).is_some()); // no_application_protocol
    }

    #[test]
    fn test_tls_alert_description_unknown_codes() {
        assert_eq!(tls_alert_description(5), None);
        assert_eq!(tls_alert_description(99), None);
        assert_eq!(tls_alert_description(255), None);
    }

    #[test]
    fn test_http_status_explanation_client_errors() {
        assert!(http_status_explanation(400).is_some());
        assert!(http_status_explanation(401).is_some());
        assert!(http_status_explanation(403).is_some());
        assert!(http_status_explanation(404).is_some());
    }

    #[test]
    fn test_http_status_explanation_server_errors() {
        assert!(http_status_explanation(500).is_some());
        assert!(http_status_explanation(502).is_some());
        assert!(http_status_explanation(503).is_some());
        assert!(http_status_explanation(504).is_some());
    }

    #[test]
    fn test_http_status_explanation_rate_limiting() {
        assert!(http_status_explanation(429).is_some());
    }

    #[test]
    fn test_http_status_explanation_success_codes_no_explanation() {
        assert_eq!(http_status_explanation(200), None);
        assert_eq!(http_status_explanation(204), None);
    }
}
