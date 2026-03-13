# Security Policy

## Supported Versions

We provide security updates for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via email to: [security@yourdomain.com](mailto:security@yourdomain.com)

You should receive a response within 48 hours. If for some reason you do not, please follow up via email to ensure we received your original message.

Please include the following information in your report:

- Type of vulnerability
- Full paths of source file(s) related to the vulnerability
- Location of the affected source code (tag/branch/commit)
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if possible)
- Impact of the vulnerability

## Security Measures

PRB implements the following security measures:

### Automated Scanning

- **Vulnerability scanning**: `cargo audit` runs on every CI build
- **Supply chain validation**: `cargo deny` checks dependency sources
- **License compliance**: Automated checks for license compatibility

### Dependencies

- All dependencies sourced exclusively from crates.io (no git dependencies in production)
- Regular updates via automated Dependabot pull requests
- Security advisories monitored via RustSec Advisory Database

### Code Quality

- Strict linting with Clippy to catch common security anti-patterns
- Minimal use of `unsafe` code (40 occurrences, all justified and documented)
- Memory safety guarantees from Rust's type system

### Build Security

- Reproducible builds with locked dependency versions (Cargo.lock committed)
- Multi-platform testing (Linux, macOS, Windows)
- Code review required for all changes

## Known Security Considerations

### Packet Capture Privileges

PRB requires elevated privileges to capture live network traffic:

- On Unix: Root access or `CAP_NET_RAW` capability
- On Windows: Administrator privileges

**Recommendation**: Use PRB with the principle of least privilege. Capture packets with elevated privileges, then analyze the resulting PCAP files as a regular user.

### TLS Decryption

PRB can decrypt TLS traffic using SSLKEYLOGFILE. This is a powerful debugging feature that should be used responsibly:

- Never enable SSLKEYLOGFILE in production environments
- Protect key log files with appropriate filesystem permissions
- Delete key log files after debugging sessions

### Protocol Parsing

PRB parses untrusted network data. While Rust's memory safety prevents many vulnerabilities, malformed packets could potentially:

- Cause excessive memory usage
- Trigger panic conditions
- Expose parsing logic bugs

**Recommendation**: Use PRB on trusted networks or sanitized packet captures when possible.

## Known Accepted Risks

### WASM Plugin Sandbox (wasmtime CVEs)

**Status**: Accepted risk pending upstream fix

PRB uses the wasmtime runtime (via extism) to safely execute WASM plugins in a sandboxed environment. Currently, wasmtime 37.0.3 has three known CVEs:

- **RUSTSEC-2026-0020** (severity 6.9/10): Guest-controlled resource exhaustion
- **RUSTSEC-2026-0021** (severity 6.9/10): Panic adding excessive HTTP fields
- **RUSTSEC-2026-0006** (severity 4.1/10): Segfault with f64.copysign on x86-64

**Why this risk is acceptable:**

1. **User-controlled plugins**: These vulnerabilities only affect malicious plugins. PRB does not auto-load plugins from untrusted sources.
2. **Local-only tool**: PRB is not a network service. No remote exploitation vector exists.
3. **Explicit plugin loading**: Users must explicitly load a `.wasm` file. No drive-by exploitation.
4. **Upstream dependency constraint**: extism (our plugin framework) requires wasmtime ^37, and no patched 37.x version exists. The fixed versions (36.0.6, 40.0.4+) are incompatible with extism's semver constraints.

**Threat model:**

An attacker would need to:
- Convince you to download a malicious WASM plugin
- Have that plugin loaded into PRB
- Exploit the vulnerability from within the sandbox

**Risk mitigation:**

- Only load plugins from trusted sources
- Review plugin source code before compilation
- Run PRB in a restricted environment when analyzing untrusted plugins
- Monitor upstream tracking: https://github.com/extism/extism/issues/889

**Resolution plan:**

- This risk will be resolved when extism upgrades to a patched wasmtime version
- We will remove this section and upgrade immediately when extism releases a fix
- Last checked: 2026-03-13

### Unmaintained Dependencies

**backoff 0.4.0**: Used transitively via async-openai for retry logic. No known vulnerabilities. Low-risk dependency with simple, stable functionality. Will be resolved when async-openai migrates to maintained alternatives.

**fxhash 0.2.1**: Used transitively via wasmtime for hash operations. No known vulnerabilities. Will be resolved when wasmtime upgrades dependencies.

## Disclosure Policy

When we receive a security bug report, we will:

1. Confirm receipt within 48 hours
2. Provide an initial assessment within 1 week
3. Work with the reporter to understand and reproduce the issue
4. Develop a fix and release timeline
5. Coordinate public disclosure with the reporter

We ask that reporters:

- Allow us reasonable time to address the issue before public disclosure
- Make a good faith effort to avoid privacy violations, data destruction, and service interruption

## Attribution

We believe in recognizing security researchers who help us improve PRB's security. If you report a valid security issue, we will:

- Credit you in the security advisory (if you wish)
- Thank you in the release notes
- Consider you for our security hall of fame (if we create one)

## Updates

This security policy may be updated over time. Check back periodically for changes.

Last updated: 2026-03-12
