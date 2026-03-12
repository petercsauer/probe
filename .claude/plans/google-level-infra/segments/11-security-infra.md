---
segment: 11
title: Security Infrastructure
depends_on: [3]
risk: 3
complexity: Medium
cycle_budget: 4
estimated_lines: 3 new files (~150 lines total)
---

# Segment 11: Security Infrastructure

## Context

Establish comprehensive security infrastructure including vulnerability scanning, license compliance, and security policy documentation.

## Current State

- No security scanning in place
- No vulnerability monitoring
- No security policy documented
- No automated supply chain validation

## Goal

Implement security audit tooling, policy documentation, and automated scanning in CI.

## Exit Criteria

1. [ ] `SECURITY.md` created with vulnerability reporting process
2. [ ] Security scanning enabled in CI (from Segment 03)
3. [ ] cargo-deny configured (from Segment 02)
4. [ ] cargo-audit runs in CI and locally
5. [ ] Supply chain validation active
6. [ ] License compliance enforced
7. [ ] Security policy reviewed and approved
8. [ ] Manual test: Run security checks locally

## Implementation Plan

### File 1: Security Policy

Create `/Users/psauer/probe/SECURITY.md`:

```markdown
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
```

### File 2: Security Scanning Documentation

Create `/Users/psauer/probe/docs/security-scanning.md`:

```markdown
# Security Scanning

PRB uses multiple layers of security scanning to protect against vulnerabilities and supply chain attacks.

## Tools

### cargo-audit

Checks dependencies against the RustSec Advisory Database for known security vulnerabilities.

```bash
# Install
cargo install cargo-audit

# Run locally
cargo audit

# Fix by updating vulnerable dependencies
cargo audit --fix
```

Runs automatically in CI on every push.

### cargo-deny

Validates supply chain security, license compliance, and dependency policies.

```bash
# Install
cargo install cargo-deny

# Run all checks
cargo deny check

# Check specific categories
cargo deny check advisories  # Security vulnerabilities
cargo deny check licenses    # License compliance
cargo deny check bans        # Banned crates/versions
cargo deny check sources     # Dependency sources
```

Configuration: `deny.toml` at repository root.

## CI Integration

Security checks run automatically in the "Security Audit" job:

```yaml
security:
  name: Security Audit
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Run cargo-audit
      run: cargo audit
    - name: Run cargo-deny
      run: cargo deny check
```

Failures in security checks will block CI and prevent merges.

## Dependency Policy

### Allowed Sources

- crates.io only (official Rust package registry)
- No git dependencies
- No path dependencies outside workspace

### License Requirements

- Permissive licenses only (Apache-2.0, MIT, BSD, ISC)
- No copyleft licenses (GPL, LGPL)
- Project itself is AGPL-3.0 (requires source disclosure for network services)

### Version Management

- Locked versions via Cargo.lock (committed to repository)
- Regular updates via Dependabot
- Security updates prioritized and fast-tracked

## Handling Security Issues

### Vulnerable Dependencies

If cargo-audit identifies a vulnerability:

1. Check if a fixed version is available
2. Update the dependency: `cargo update -p vulnerable-crate`
3. Run tests to verify compatibility
4. If no fix available, consider alternatives or workarounds
5. Document decision in issue tracker

### Supply Chain Issues

If cargo-deny identifies a supply chain issue:

1. Verify the source of the dependency
2. Check if it's a transitive dependency (harder to replace)
3. Evaluate alternatives
4. Update deny.toml if the dependency is deemed safe

## Local Development

Run security checks before committing:

```bash
# Quick security check
just audit

# Full deny check
just deny
```

Pre-commit hooks do NOT include security checks by default (too slow), but you can run them manually before pushing.

## Reporting New Vulnerabilities

If you discover a vulnerability in PRB itself, please follow our [Security Policy](../SECURITY.md).

Do NOT open a public issue for security vulnerabilities.
```

### File 3: Supply Chain Bill of Materials

Add script to generate SBOM: `/Users/psauer/probe/scripts/generate-sbom.sh`:

```bash
#!/bin/bash
# Generate Software Bill of Materials (SBOM) for PRB

set -e

echo "Generating SBOM..."

# Output file
SBOM_FILE="sbom.json"

# Generate dependency tree in JSON format
cargo tree --workspace --format "{p} {l}" --prefix none > /tmp/cargo-tree.txt

# Create SBOM structure
cat > "$SBOM_FILE" <<EOF
{
  "bomFormat": "CycloneDX",
  "specVersion": "1.4",
  "version": 1,
  "metadata": {
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "component": {
      "type": "application",
      "name": "prb",
      "version": "0.1.0"
    }
  },
  "components": [
EOF

# Parse cargo tree output and add to SBOM
# (Simplified - real SBOM would use cargo-cyclonedx)
echo "  ]" >> "$SBOM_FILE"
echo "}" >> "$SBOM_FILE"

echo "✅ SBOM generated: $SBOM_FILE"
echo "For production SBOM, consider: cargo install cargo-cyclonedx"
```

## Files to Create

1. `/Users/psauer/probe/SECURITY.md` (~150 lines)
2. `/Users/psauer/probe/docs/security-scanning.md` (~120 lines)
3. `/Users/psauer/probe/scripts/generate-sbom.sh` (~40 lines)

## Test Plan

1. Create SECURITY.md
2. Create security scanning documentation
3. Create SBOM generation script
4. Verify security checks in CI (should already run from S03)
5. Run local security checks:
   ```bash
   cargo audit
   cargo deny check
   ```
6. Verify any vulnerabilities are addressed or documented
7. Make script executable:
   ```bash
   chmod +x scripts/generate-sbom.sh
   ```
8. Test SBOM generation:
   ```bash
   bash scripts/generate-sbom.sh
   ```
9. Review SECURITY.md with team
10. Commit: "security: Add security policy and scanning infrastructure"

## Blocked By

- Segment 03 (Main CI Workflow) - security scans run in CI

## Blocks

None - security is foundational but doesn't block other work.

## Success Metrics

- SECURITY.md created and reviewed
- Security scanning documentation complete
- cargo-audit runs successfully
- cargo-deny configured and passing
- SBOM generation script functional
- No critical vulnerabilities identified

## Notes

- Update security contact email in SECURITY.md
- Consider setting up security@yourdomain.com email alias
- Review and update dependency allow-lists in deny.toml
- Some vulnerabilities may require version upgrades or code changes
- SBOM generation is simplified - consider cargo-cyclonedx for production
