"""File operations utility with atomic writes and consistent error handling.

Provides centralized file I/O operations to prevent TOCTOU bugs and ensure
consistent error handling across the orchestrator codebase.
"""

from __future__ import annotations

import os
import tempfile
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    pass


class FileOpsError(Exception):
    """File operation failed.

    This exception wraps underlying I/O errors and preserves the exception chain
    via `raise ... from e` to maintain stack traces for debugging.
    """
    pass


class FileOps:
    """Utility class for atomic file operations with consistent error handling.

    All methods are static and provide:
    - Atomic writes via temp file + os.replace()
    - Consistent error wrapping in FileOpsError
    - Safe operations that handle common edge cases (missing files, etc.)
    """

    @staticmethod
    def read_text(path: Path, encoding: str = "utf-8", errors: str = "replace") -> str:
        """Read text file with error handling.

        Args:
            path: Path to file to read
            encoding: Text encoding (default: utf-8)
            errors: Error handling strategy (default: replace)

        Returns:
            File contents as string

        Raises:
            FileOpsError: If read fails (wraps underlying exception)
        """
        try:
            return path.read_text(encoding=encoding, errors=errors)
        except Exception as e:
            raise FileOpsError(f"Failed to read {path}: {e}") from e

    @staticmethod
    def read_bytes(path: Path) -> bytes:
        """Read binary file with error handling.

        Args:
            path: Path to file to read

        Returns:
            File contents as bytes

        Raises:
            FileOpsError: If read fails (wraps underlying exception)
        """
        try:
            return path.read_bytes()
        except Exception as e:
            raise FileOpsError(f"Failed to read {path}: {e}") from e

    @staticmethod
    def write_text_atomic(
        path: Path,
        content: str,
        encoding: str = "utf-8",
    ) -> None:
        """Write text file atomically via temp file + os.replace().

        Creates a temporary file in the same directory, writes content, then
        atomically replaces the target file. This prevents partial writes and
        ensures readers never see incomplete content.

        Args:
            path: Destination file path
            content: Text content to write
            encoding: Text encoding (default: utf-8)

        Raises:
            FileOpsError: If write fails (wraps underlying exception)
        """
        try:
            # Ensure parent directory exists
            path.parent.mkdir(parents=True, exist_ok=True)

            # Write to temp file in same directory (ensures same filesystem for atomic replace)
            fd, tmp_path = tempfile.mkstemp(
                dir=path.parent,
                prefix=f".{path.name}.",
                suffix=".tmp"
            )
            try:
                # Write content to temp file
                with os.fdopen(fd, 'w', encoding=encoding) as f:
                    f.write(content)
                    f.flush()
                    os.fsync(f.fileno())

                # Atomic replace (POSIX guarantees atomicity)
                os.replace(tmp_path, path)
            except Exception:
                # Clean up temp file on failure
                try:
                    os.unlink(tmp_path)
                except Exception:
                    pass
                raise
        except Exception as e:
            raise FileOpsError(f"Failed to write {path}: {e}") from e

    @staticmethod
    def write_bytes_atomic(path: Path, content: bytes) -> None:
        """Write binary file atomically via temp file + os.replace().

        Args:
            path: Destination file path
            content: Binary content to write

        Raises:
            FileOpsError: If write fails (wraps underlying exception)
        """
        try:
            # Ensure parent directory exists
            path.parent.mkdir(parents=True, exist_ok=True)

            # Write to temp file in same directory
            fd, tmp_path = tempfile.mkstemp(
                dir=path.parent,
                prefix=f".{path.name}.",
                suffix=".tmp"
            )
            try:
                # Write content to temp file
                with os.fdopen(fd, 'wb') as f:
                    f.write(content)
                    f.flush()
                    os.fsync(f.fileno())

                # Atomic replace
                os.replace(tmp_path, path)
            except Exception:
                # Clean up temp file on failure
                try:
                    os.unlink(tmp_path)
                except Exception:
                    pass
                raise
        except Exception as e:
            raise FileOpsError(f"Failed to write {path}: {e}") from e

    @staticmethod
    def remove_safe(path: Path) -> bool:
        """Remove file safely, returning success status.

        Args:
            path: Path to file to remove

        Returns:
            True if file was removed or didn't exist, False on error
        """
        try:
            path.unlink(missing_ok=True)
            return True
        except Exception:
            return False

    @staticmethod
    def ensure_dir(path: Path) -> None:
        """Ensure directory exists, creating parents as needed.

        Args:
            path: Directory path to ensure exists

        Raises:
            FileOpsError: If directory creation fails (wraps underlying exception)
        """
        try:
            path.mkdir(parents=True, exist_ok=True)
        except Exception as e:
            raise FileOpsError(f"Failed to create directory {path}: {e}") from e
