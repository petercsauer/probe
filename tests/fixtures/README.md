# Test Fixtures

This directory contains real-world network captures and test data for integration testing across the prb workspace.

## Capture Files

All captures are located in `captures/` organized by protocol category.

| File | Protocol | Source | License | Size | Notes |
|------|----------|--------|---------|------|-------|
| **TLS & Encryption** |
| captures/tls/tls12.pcapng | TLS 1.2 | Internal test fixtures (S11) | MIT | 294KB | With keylog file |
| captures/tls/tls12.keylog | - | Internal test fixtures | MIT | - | SSLKEYLOGFILE for tls12.pcapng |
| captures/tls/tls13.pcapng | TLS 1.3 | Internal test fixtures (S11) | MIT | 9.5KB | With keylog file |
| captures/tls/tls13.keylog | - | Internal test fixtures | MIT | - | SSLKEYLOGFILE for tls13.pcapng |
| **HTTP/1.x & HTTP/2** |
| captures/http/http-chunked-gzip.pcap | HTTP/1.1 | Wireshark Wiki | Public domain | 29KB | Chunked transfer + gzip |
| captures/http2/http2-h2c.pcap | HTTP/2 | Wireshark Wiki | Public domain | 1.6KB | HTTP/2 cleartext (h2c) |
| **TCP Layer** |
| captures/tcp/dns-remoteshell.pcap | TCP | Wireshark samples | Public domain | 24KB | TCP reassembly testing |
| captures/tcp/tcp-ecn-sample.pcap | TCP | Wireshark samples | Public domain | 116KB | TCP ECN flags |
| captures/tcp/200722_tcp_anon.pcapng | TCP | Wireshark samples | Public domain | 13KB | Anonymized TCP, pcapng format |
| **DNS & DHCP** |
| captures/dns/dns-query.pcap | DNS | Wireshark samples | Public domain | 111B | Single DNS query |
| captures/dns/dns-response.pcap | DNS | Wireshark samples | Public domain | 225B | Single DNS response |
| captures/dns/dns.pcap | DNS | Wireshark samples | Public domain | 24KB | DNS traffic mix |
| captures/dhcp/dhcp.pcap | DHCP | Wireshark samples | Public domain | 640B | DHCP DORA sequence |
| captures/dhcp/dhcp-release.pcap | DHCP | Wireshark samples | Public domain | 332B | DHCP release message |
| **Enterprise Protocols** |
| captures/enterprise/krb.pcap | Kerberos | Wireshark samples | Public domain | 2.6KB | Kerberos authentication |
| captures/enterprise/krb-816.pcap | Kerberos | Wireshark samples | Public domain | 33KB | Extended Kerberos trace |
| captures/enterprise/ldap.pcap | LDAP | Wireshark samples | Public domain | 3.0KB | LDAP queries |
| captures/enterprise/sip-rtp.pcap | SIP/RTP | Wireshark samples | Public domain | 2.6KB | VoIP signaling |
| captures/enterprise/snmp_usm.pcap | SNMP v3 | Wireshark samples | Public domain | 34KB | SNMPv3 with USM |
| **SMB & RDP** |
| captures/smb/smb2-peter.pcap | SMB2 | Wireshark samples | Public domain | 513KB | Large SMB2 session |
| captures/smb/smb-on-windows-10.pcapng | SMB | Wireshark samples | Public domain | 139KB | Windows 10 SMB traffic |
| captures/rdp/rdp.pcap | RDP | Wireshark samples | Public domain | 2.6KB | Remote Desktop Protocol |
| captures/rdp/rdp-ssl.pcap | RDP/TLS | Wireshark samples | Public domain | 2.6KB | RDP over TLS |
| **IoT & Messaging** |
| captures/iot/mqtt.pcap | MQTT | Wireshark samples | Public domain | 186B | MQTT pub/sub |
| captures/iot/coap.pcap | CoAP | Wireshark samples | Public domain | 164B | Constrained Application Protocol |
| captures/iot/amqp.pcap | AMQP | Wireshark samples | Public domain | 102B | AMQP messaging |
| captures/mqtt/mqtt.pcap | MQTT | Wireshark samples | Public domain | 186B | MQTT telemetry |
| captures/rtps/rtps_sample.pcap | RTPS/DDS | Wireshark samples | Public domain | 126B | DDS pub/sub protocol |
| **Modern Transports** |
| captures/quic/quic_initial.pcap | QUIC | Wireshark samples | Public domain | 124B | QUIC initial packet |
| captures/ssh/ssh_banner.pcap | SSH | Wireshark samples | Public domain | 206B | SSH version exchange |
| captures/modern/sctp_test.pcap | SCTP | Wireshark samples | Public domain | 428B | Stream Control Protocol |
| captures/modern/wireguard.pcap | WireGuard | Wireshark samples | Public domain | 5.0KB | WireGuard VPN |
| **IP Layer** |
| captures/ip/v6.pcap | IPv6 | Wireshark samples | Public domain | 28KB | IPv6 traffic |
| captures/ip/ipv6-ripng.pcap | IPv6/RIPng | Wireshark samples | Public domain | 33KB | IPv6 routing protocol |
| **Adversarial Cases** |
| captures/adversarial/empty.pcap | - | Synthetic | MIT | 24B | Minimal valid pcap (header only) |
| captures/adversarial/dhcp-nanosecond.pcap | DHCP | Synthetic | MIT | 1.4KB | Nanosecond timestamps |
| captures/adversarial/invalid-checksum.pcap | TCP | Synthetic | MIT | 2.6KB | Invalid TCP checksum |
| captures/adversarial/malformed-ip.pcap | IP | Synthetic | MIT | 2.6KB | Truncated IP header |
| captures/adversarial/tcp-truncated.pcap | TCP | Synthetic | MIT | 2.6KB | Truncated TCP segment |

## OpenTelemetry Test Data

Located in `captures/otel/`:

| File | Type | Description | Use |
|------|------|-------------|-----|
| synthetic-trace.spans.json | OTLP JSON | Single-service trace with HTTP/DB spans | Basic correlation testing |
| multi-service-trace.spans.json | OTLP JSON | Multi-service distributed trace | Cross-service correlation |

## Attribution

### Wireshark Sample Captures
Most captures are from the [Wireshark Sample Captures](https://wiki.wireshark.org/SampleCaptures) collection, maintained by the Wireshark community. These are public domain test data used under fair use for software testing purposes.

### Internal Test Fixtures
TLS captures with keylog files and adversarial test cases were created specifically for this project and are licensed under MIT.

## Usage

The machine-readable fixture manifest is available at `captures/manifest.json` for programmatic access to fixture metadata including expected event counts and protocol detection targets.

Tests reference fixtures using workspace-relative paths:
```rust
PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../tests/fixtures/captures/<category>/<file>")
```

## Adding New Fixtures

When adding new fixtures:
1. Place in appropriate protocol subdirectory under `captures/`
2. Update this README with source, license, and description
3. Update `captures/manifest.json` with metadata
4. Add test coverage in relevant `real_data_*_tests.rs` file
5. Verify fixture processes without panic in regression suite
