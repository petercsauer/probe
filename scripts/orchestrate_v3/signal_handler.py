"""Signal handling for graceful shutdown."""

from __future__ import annotations

import asyncio
import logging
import signal

log = logging.getLogger("orchestrate")


class SignalHandler:
    """Manages graceful shutdown via SIGINT/SIGTERM."""

    def __init__(self):
        self.shutting_down = asyncio.Event()
        self.worker_stop = asyncio.Event()

    def register_handlers(self, loop: asyncio.AbstractEventLoop) -> None:
        """Register signal handlers for current event loop."""
        for sig in (signal.SIGINT, signal.SIGTERM):
            loop.add_signal_handler(sig, self.shutdown)

    def shutdown(self) -> None:
        """Trigger graceful shutdown."""
        log.warning("Received shutdown signal")
        self.shutting_down.set()
        self.worker_stop.set()

    def is_shutting_down(self) -> bool:
        """Check if shutdown has been initiated."""
        return self.shutting_down.is_set()
