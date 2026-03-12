# Orchestrate v3

Refactored version of orchestrate_v2 with improved code quality, 70% test coverage, and modular architecture.

## Usage

```bash
python -m scripts.orchestrate_v3 <plan_dir>
```

## Features

- Wave-based parallel dispatch
- State persistence and recovery
- Real-time monitoring dashboard
- Configurable gates and circuit breakers
- Worktree pool management

## Commands

```bash
# Run orchestrator
python -m scripts.orchestrate_v3 <plan_dir>

# Show help
python -m scripts.orchestrate_v3 --help
```
