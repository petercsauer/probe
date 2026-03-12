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
