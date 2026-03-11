"""Configuration loader for orchestrate.toml."""

from __future__ import annotations

import os
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
class OrchestrateConfig:
    preamble_files: list[str] = field(default_factory=list)
    extra_rules: str = ""
    max_parallel: int = 4
    segment_timeout: int = 3600
    max_retries: int = 2
    heartbeat_interval: int = 900
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
        )
