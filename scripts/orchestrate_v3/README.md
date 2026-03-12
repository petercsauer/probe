# Orchestrate v3

Refactored version of orchestrate_v2 with improved code quality, 70% test coverage, and modular architecture.

## Usage

```bash
python -m scripts.orchestrate_v3 <plan_dir>
```

## Changes from v2

- Improved code organization and modularity
- Enhanced test coverage (target: 70%)
- Refactored for better maintainability

## Development

Run tests:
```bash
python scripts/orchestrate_v3/test_recovery.py
python scripts/orchestrate_v3/test_worktree_pool.py
```

Verify installation:
```bash
python -m scripts.orchestrate_v3 --help
```
