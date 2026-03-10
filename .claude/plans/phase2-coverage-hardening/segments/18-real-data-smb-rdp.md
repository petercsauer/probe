---
segment: 18
title: "Real-Data Tests: SMB, RDP, and Enterprise Protocol Captures"
depends_on: [11]
risk: 3
complexity: Medium
cycle_budget: 4
status: pending
commit_message: "test(prb-pcap,prb-decode): add real-data SMB, RDP, and enterprise protocol tests"
---

# Segment 18: Real-Data Tests — SMB, RDP, and Enterprise Protocol Captures

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Download real-world SMB (v2/v3), RDP, and other enterprise protocol captures. Write tests verifying the decode pipeline handles these correctly.

**Depends on:** Segment 11 (integration tests)

## Data Sources

### From Wireshark Wiki SampleCaptures
1. **smb-on-windows-10.pcapng** — SMB3 traffic
   - URL: `https://wiki.wireshark.org/uploads/__moin_import__/attachments/SMB2/smb-on-windows-10.pcapng`
2. **smb2-peter.pcap** — SMB2 negotiation and file access
3. **smb3-aes-128-ccm.pcap.gz** — SMB3 with AES encryption
4. **rdp.pcap** — RDP session establishment
5. **rdp-ssl.pcap** — RDP over TLS
6. **krb-816.pcap** — Kerberos authentication
7. **snmp_usm.pcap** — SNMP v3 with USM security
8. **sip-rtp-example.pcap** — SIP signaling + RTP media
9. **ldap-controls-dirsync-01.pcap** — LDAP with directory sync

### From NetResec
10. Various enterprise network captures
    - `https://www.netresec.com/?page=PcapFiles`

Store fixtures in `tests/fixtures/captures/smb/`, `tests/fixtures/captures/rdp/`, `tests/fixtures/captures/enterprise/`.

## Scope

- `tests/fixtures/captures/smb/*.pcap` — SMB captures
- `tests/fixtures/captures/rdp/*.pcap` — RDP captures
- `tests/fixtures/captures/enterprise/*.pcap` — Kerberos, LDAP, SNMP, SIP/RTP
- `crates/prb-pcap/tests/real_data_enterprise_tests.rs` — New test file

## Implementation Approach

### SMB decode tests
```rust
#[test]
fn test_smb2_negotiation_real() {
    // Load smb2-peter.pcap
    // Run through pipeline
    // Assert: SMB2 NEGOTIATE, SESSION_SETUP commands detected
    // Assert: dialect version, security mode extracted
}

#[test]
fn test_smb3_file_access_real() {
    // Load smb-on-windows-10.pcapng
    // Assert: TREE_CONNECT, CREATE, READ/WRITE operations found
    // Assert: share paths and file names extractable
}

#[test]
fn test_smb3_encrypted_traffic() {
    // Load smb3-aes-128-ccm.pcap
    // Assert: encrypted SMB3 sessions detected
    // Assert: graceful handling without keys (events still generated)
}
```

### RDP decode tests
```rust
#[test]
fn test_rdp_session_establishment() {
    // Load rdp.pcap
    // Assert: RDP connection sequence detected
    // Assert: X.224, MCS, security exchange phases identified
}

#[test]
fn test_rdp_over_tls() {
    // Load rdp-ssl.pcap
    // Assert: TLS negotiation for RDP detected
    // Assert: protocol type identified as RDP despite TLS wrapper
}
```

### Other enterprise protocols
```rust
#[test]
fn test_kerberos_auth_real() {
    // Load krb-816.pcap
    // Assert: AS-REQ, AS-REP, TGS-REQ, TGS-REP messages detected
    // Assert: principal names and realm extracted
}

#[test]
fn test_ldap_real() {
    // Load ldap-controls-dirsync-01.pcap
    // Assert: LDAP bind, search operations detected
    // Assert: DN paths and filter expressions extracted
}

#[test]
fn test_snmp_real() {
    // Load snmp_usm.pcap
    // Assert: SNMP GET/SET/TRAP messages detected
    // Assert: OID values parsed
}

#[test]
fn test_sip_rtp_real() {
    // Load sip-rtp-example.pcap
    // Assert: SIP INVITE, 200 OK, BYE detected
    // Assert: RTP media streams identified
    // Assert: SDP codec info extracted
}
```

## Pre-Mortem Risks

- SMB3 encryption makes deep decode impossible without session keys — test detection-level only
- RDP uses complex multi-layer protocol stack — focus on connection detection, not deep decode
- Some protocols may not have dedicated decoders in prb — test that pipeline handles them as "unknown" gracefully

## Build and Test Commands

- Build: `cargo check -p prb-pcap -p prb-decode`
- Test (targeted): `cargo nextest run -E 'test(real_data_enterprise)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 8 tests with real enterprise captures, all passing
2. **Fixture files:** At least 6 capture files across SMB, RDP, Kerberos, LDAP, SNMP, SIP
3. **Graceful degradation:** Encrypted protocols produce events even without key material
4. **Regression tests:** `cargo nextest run --workspace` — no regressions
5. **Full build gate:** `cargo build --workspace`
6. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
7. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
