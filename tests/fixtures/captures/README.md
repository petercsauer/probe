# Test Capture Fixtures

Real-world network captures for integration testing of protocols.

## HTTP/2 Captures

### http2-h2c.pcap
- **Source:** Wireshark Wiki
- **URL:** https://wiki.wireshark.org/uploads/__moin_import__/attachments/HTTP2/http2-h2c.pcap
- **Description:** HTTP/2 cleartext (h2c) via HTTP/1.1 Upgrade mechanism
- **License:** Public domain / Wireshark sample captures
- **Use:** End-to-end gRPC/HTTP2 pipeline testing

## TCP Captures

### dns-remoteshell.pcap
- **Source:** Wireshark sample captures
- **Description:** TCP stream with DNS and remote shell traffic
- **Use:** TCP reassembly robustness testing

### tcp-ecn-sample.pcap
- **Source:** Wireshark sample captures
- **Description:** TCP with Explicit Congestion Notification
- **Use:** TCP edge case handling

### 200722_tcp_anon.pcapng
- **Source:** Wireshark sample captures
- **Description:** Anonymized TCP capture (pcapng format)
- **Use:** pcapng format compatibility testing

## TLS Captures

### tls12.pcapng + tls12.keylog
- **Source:** Existing test fixtures (from S11)
- **Description:** TLS 1.2 encrypted traffic with SSLKEYLOGFILE
- **Use:** TLS decryption without keylog (encrypted event handling)

### tls13.pcapng + tls13.keylog
- **Source:** Existing test fixtures (from S11)
- **Description:** TLS 1.3 encrypted traffic with SSLKEYLOGFILE
- **Use:** TLS 1.3 robustness testing

## IP Layer Captures

### v6.pcap
- **Source:** Wireshark sample captures
- **Description:** IPv6 packet capture
- **Use:** IPv6 protocol support verification

## Attribution

These captures are publicly available test data from the Wireshark project and community contributors. They are used here under fair use for software testing purposes.

## HTTP/1.x Captures

### http.cap
- **Source:** Wireshark Wiki
- **URL:** https://wiki.wireshark.org/uploads/__moin_import__/attachments/SampleCaptures/http.cap
- **Description:** Basic HTTP/1.1 request/response traffic
- **License:** Public domain / Wireshark sample captures
- **Use:** HTTP/1.1 decode testing

### http-chunked-gzip.pcap
- **Source:** Wireshark Wiki
- **URL:** https://wiki.wireshark.org/uploads/__moin_import__/attachments/SampleCaptures/http-chunked-gzip.pcap
- **Description:** HTTP with chunked transfer encoding and gzip compression
- **Use:** HTTP chunked transfer and compression testing

### http_with_jpegs.cap
- **Source:** Wireshark Wiki
- **URL:** https://wiki.wireshark.org/uploads/__moin_import__/attachments/SampleCaptures/http_with_jpegs.cap.gz
- **Description:** HTTP transferring JPEG images (large payloads)
- **Use:** HTTP large payload robustness testing

## Note on gRPC and WebSocket Captures

Many publicly available gRPC and WebSocket pcap files from Wireshark wiki URLs are no longer accessible (404 or moved to GitLab with different URLs). The HTTP/2 cleartext capture (h2c) provides sufficient coverage for testing gRPC/HTTP2 protocol decoding, as gRPC uses HTTP/2 as its transport. HTTP/1.x captures provide foundation for WebSocket handshake testing.
