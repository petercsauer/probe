"""Configuration loader for orchestrate.toml."""

from __future__ import annotations

import os
import tomllib
from dataclasses import dataclass, field
from pathlib import Path


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
    notify_contact: str = ""
    monitor_port: int = 0

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
        auth_env: dict[str, str] = {}
        for k, v in auth.items():
            val = str(v) if v else os.environ.get(k, "")
            if val:
                auth_env[k] = val

        # Notification contact: fall back to env var
        contact = notifications.get("contact", "")
        if not contact:
            contact = os.environ.get("PRB_NOTIFY_CONTACT", "")

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
            notify_contact=contact,
            monitor_port=monitor.get("port", 0) if monitor.get("enabled", False) else 0,
        )
