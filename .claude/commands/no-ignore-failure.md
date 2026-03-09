Apply the no-ignore-failure discipline to any code being written or reviewed. Errors during development are valuable signals — never mask them.

## Rules

- **Never catch blanket `Exception`** — catch specific exception types only
- **Never skip tests** — fix the underlying issue instead
- **Never continue processing after a failure with a silent fallback**
- **Never suppress errors without explicit user permission**

## Permissible exceptions

- Network retries that eventually raise (retries are fine; swallowing the final error is not)
- Catching specific expected conditions (e.g., `FileNotFoundError` for optional files)
- Explicitly optional features confirmed by the user

## Review checklist

When reviewing code for this discipline, flag any:
- Bare `except:` or `except Exception:` clauses
- Error handlers that log-and-continue without re-raising
- `try/catch` blocks that return a default value on failure without alerting the caller
- Tests marked as `skip`, `xfail`, or similar without a documented, time-bound reason
- Silent `None` returns where the caller cannot distinguish success from failure

For each flagged issue, propose the minimal fix: either catch the specific exception type, let the error propagate, or add explicit error reporting.
