# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in PRB, please report it responsibly.

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, please email the maintainers directly at the email address listed in the repository's package metadata, or use GitHub's private vulnerability reporting feature if available.

### What to include

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Response timeline

- **Acknowledgment**: within 48 hours
- **Assessment**: within 1 week
- **Fix or mitigation**: depends on severity, targeting within 30 days for critical issues

## Scope

PRB is a network packet analysis tool. The following are considered security-relevant:

### In scope

- **Arbitrary code execution** via maliciously crafted PCAP files, fixture files, or network input
- **Memory safety violations** (buffer overflows, use-after-free) triggered by malformed input
- **Path traversal** in file operations (plugin loading, schema loading, export output)
- **Plugin sandbox escapes** (WASM plugins accessing resources outside the sandbox)
- **Sensitive data exposure** (TLS keys, decoded payloads) written to unintended locations
- **Denial of service** via crafted input causing excessive memory or CPU usage

### Out of scope

- **Traffic content** -- PRB decodes whatever protocols are in the capture; the content of decoded messages is not a PRB vulnerability
- **libpcap vulnerabilities** -- report these to the libpcap project
- **Intentional functionality** -- PRB is designed to read packet captures and decrypt TLS with provided keys; this is expected behavior

## Supported Versions

Security fixes are applied to the latest release only. There are no long-term support branches at this time.

## Security Considerations for Users

- **TLS keylog files** contain sensitive cryptographic material. Protect them with appropriate file permissions (`chmod 600`) and delete them after use.
- **PCAP files** may contain sensitive network traffic. Handle them according to your organization's data handling policies.
- **Plugins** run with the same privileges as PRB. Only install plugins from trusted sources. WASM plugins run in a sandbox, but native plugins have full process access.
- **Live capture** requires elevated privileges (root or `CAP_NET_RAW`). Use the principle of least privilege.
