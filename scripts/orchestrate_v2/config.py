"""Configuration loader for orchestrate.toml."""

from __future__ import annotations

import os
import random
import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path


_ENV_REF_RE = re.compile(r"\$\{([^}]+)\}")


def _resolve_env_refs(value: str) -> str:
    """Resolve ${VAR} and ${VAR:-default} references in a string."""
    def _replace(m: re.Match) -> str:
        expr = m.group(1)
        if ":-" in expr:
            var, default = expr.split(":-", 1)
            return os.environ.get(var.strip(), default.strip())
        return os.environ.get(expr.strip(), "")
    return _ENV_REF_RE.sub(_replace, value)


@dataclass
class RetryPolicy:
    """Configurable retry policy with exponential backoff and jitter.

    Implements industry-standard exponential backoff with jitter to prevent
    thundering herd problems when retrying failed segments.
    """

    max_retries: int = 3
    initial_delay: int = 30  # 30 seconds
    max_delay: int = 600     # 10 minutes cap
    exponential_base: float = 2.0
    jitter: bool = True

    # Which statuses trigger retry
    retry_on: set[str] = field(default_factory=lambda: {"timeout", "failed", "unknown"})

    # Which statuses never retry
    no_retry_on: set[str] = field(default_factory=lambda: {"blocked"})

    def get_delay(self, attempt: int) -> int:
        """Calculate retry delay for attempt N with exponential backoff and jitter.

        Args:
            attempt: 0-indexed attempt number (0 = first retry)

        Returns:
            Delay in seconds

        Examples:
            >>> policy = RetryPolicy()
            >>> policy.get_delay(0)  # First retry
            30  # (or 15-45 with jitter)
            >>> policy.get_delay(1)  # Second retry
            60  # (or 30-90 with jitter)
            >>> policy.get_delay(2)  # Third retry
            120  # (or 60-180 with jitter)
        """
        # Exponential backoff: delay * (base ^ attempt)
        delay = self.initial_delay * (self.exponential_base ** attempt)

        # Cap at max_delay
        delay = min(delay, self.max_delay)

        # Add jitter to prevent thundering herd
        if self.jitter:
            # Random factor between 0.5 and 1.5
            jitter_factor = 0.5 + random.random()
            delay = delay * jitter_factor

        return int(delay)

    def should_retry(self, status: str) -> bool:
        """Check if status is retryable per policy configuration.

        Args:
            status: Segment status string

        Returns:
            True if this status should be retried, False otherwise
        """
        if status in self.no_retry_on:
            return False
        if status in self.retry_on:
            return True
        # Unknown status - don't retry to be safe
        return False


@dataclass
class OrchestrateConfig:
    """Configuration for orchestration.

    isolation_strategy values:
        - "none": No isolation (default) - segments run in main worktree
        - "env": Per-segment environment variables only
        - "worktree": Git worktree pool isolation - each segment gets isolated worktree
    """
    preamble_files: list[str] = field(default_factory=list)
    extra_rules: str = ""
    max_parallel: int = 4
    segment_timeout: int = 3600
    max_retries: int = 2
    heartbeat_interval: int = 300
    isolation_strategy: str = "none"
    isolation_env: dict[str, str] = field(default_factory=dict)
    gate_command: str = ""
    auth_env: dict[str, str] = field(default_factory=dict)
    notify_enabled: bool = False
    ntfy_topic: str = ""
    notify_verbosity: str = "all"
    notify_max_attempts: int = 3
    notify_retry_delays: list[int] = field(default_factory=lambda: [10, 60, 300])
    monitor_port: int = 0
    stall_threshold: int = 1800
    network_retry_max: int = 600
    recovery_enabled: bool = True
    recovery_max_attempts: int = 1
    recovery_health_check_timeout: int = 120
    retry_policy: RetryPolicy = field(default_factory=RetryPolicy)

    @classmethod
    def load(cls, plan_dir: Path) -> OrchestrateConfig:
        """Load config from orchestrate.toml in plan_dir, falling back to defaults."""
        toml_path = plan_dir / "orchestrate.toml"
        if not toml_path.exists():
            return cls()

        with open(toml_path, "rb") as f:
            raw = tomllib.load(f)

        plan = raw.get("plan", {})
        execution = raw.get("execution", {})
        isolation = raw.get("isolation", {})
        gate = raw.get("gate", {})
        auth = raw.get("auth", {})
        notifications = raw.get("notifications", {})
        monitor = raw.get("monitor", {})
        recovery = raw.get("recovery", {})
        retry_config = raw.get("retry_policy", {})

        # Isolation env vars: nested table under [isolation]
        iso_env: dict[str, str] = {}
        if isinstance(isolation.get("env"), dict):
            iso_env = {k: str(v) for k, v in isolation["env"].items()}

        # Auth env vars: everything under [auth]
        # Supports ${ENV_VAR} and ${ENV_VAR:-default} syntax
        auth_env: dict[str, str] = {}
        for k, v in auth.items():
            val = _resolve_env_refs(str(v)) if v else os.environ.get(k, "")
            if val:
                auth_env[k] = val

        # Retry policy configuration
        retry_policy = RetryPolicy(
            max_retries=retry_config.get("max_retries", 3),
            initial_delay=retry_config.get("initial_delay", 30),
            max_delay=retry_config.get("max_delay", 600),
            exponential_base=retry_config.get("exponential_base", 2.0),
            jitter=retry_config.get("jitter", True),
            retry_on=set(retry_config.get("retry_on", ["timeout", "failed", "unknown"])),
            no_retry_on=set(retry_config.get("no_retry_on", ["blocked"])),
        )

        return cls(
            preamble_files=plan.get("preamble", []),
            extra_rules=plan.get("extra_rules", ""),
            max_parallel=execution.get("max_parallel", 4),
            segment_timeout=execution.get("segment_timeout", 3600),
            max_retries=execution.get("max_retries", 2),
            heartbeat_interval=execution.get("heartbeat_interval", 900),
            isolation_strategy=isolation.get("strategy", "none"),
            isolation_env=iso_env,
            gate_command=gate.get("command", ""),
            auth_env=auth_env,
            notify_enabled=notifications.get("enabled", False),
            ntfy_topic=notifications.get("ntfy_topic", ""),
            notify_verbosity=notifications.get("verbosity", "all"),
            notify_max_attempts=notifications.get("max_attempts", 3),
            notify_retry_delays=notifications.get("retry_delays", [10, 60, 300]),
            monitor_port=monitor.get("port", 0) if monitor.get("enabled", False) else 0,
            stall_threshold=execution.get("stall_threshold", 1800),
            network_retry_max=execution.get("network_retry_max", 600),
            recovery_enabled=recovery.get("enabled", True),
            recovery_max_attempts=recovery.get("max_attempts", 1),
            recovery_health_check_timeout=recovery.get("health_check_timeout", 120),
            retry_policy=retry_policy,
        )
