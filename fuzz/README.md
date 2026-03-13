# Fuzzing Infrastructure for PRB

This directory contains fuzzing targets for the PRB project using `cargo-fuzz` (libFuzzer).

## Prerequisites

Install cargo-fuzz:
```bash
cargo install cargo-fuzz
```

## Running Fuzz Tests

### Keylog Parser Fuzzing

Fuzz the TLS keylog parser for 60 seconds:
```bash
cargo fuzz run keylog_parser -- -max_total_time=60
```

Run indefinitely (Ctrl+C to stop):
```bash
cargo fuzz run keylog_parser
```

Run with a specific number of iterations:
```bash
cargo fuzz run keylog_parser -- -runs=1000000
```

### Seed Corpus

The `corpus/keylog_parser/` directory contains seed inputs that provide good starting points for fuzzing:
- `valid_tls12.txt` - Valid TLS 1.2 keylog entry
- `valid_tls13.txt` - Valid TLS 1.3 keylog entries
- `mixed.txt` - Mixed content with comments

New interesting inputs discovered during fuzzing are automatically added to the corpus.

### Crash Artifacts

If the fuzzer discovers a crash, it will save the input to `artifacts/keylog_parser/`.

## CI Integration

Fuzzing is intended to run in nightly CI jobs (not per-commit) to avoid increasing build times.
A suggested CI configuration:

```yaml
fuzz:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: cargo install cargo-fuzz
    - run: cargo fuzz run keylog_parser -- -max_total_time=300
```

## Adding New Fuzz Targets

1. Create a new file in `fuzz_targets/`
2. Add a `[[bin]]` entry in `fuzz/Cargo.toml`
3. Create a seed corpus directory in `corpus/<target_name>/`
4. Update this README
