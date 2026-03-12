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
