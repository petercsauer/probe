"""Tests for FileOps utility."""

import os
import tempfile
from pathlib import Path

import pytest

from .fileops import FileOps, FileOpsError


class TestFileOpsReadText:
    """Tests for FileOps.read_text()."""

    def test_read_text_basic(self, tmp_path: Path):
        """Test basic text file reading."""
        test_file = tmp_path / "test.txt"
        content = "Hello, World!"
        test_file.write_text(content, encoding="utf-8")

        result = FileOps.read_text(test_file)
        assert result == content

    def test_read_text_unicode(self, tmp_path: Path):
        """Test reading file with Unicode characters."""
        test_file = tmp_path / "unicode.txt"
        content = "Hello 世界 🌍"
        test_file.write_text(content, encoding="utf-8")

        result = FileOps.read_text(test_file)
        assert result == content

    def test_read_text_nonexistent(self, tmp_path: Path):
        """Test reading nonexistent file raises FileOpsError."""
        test_file = tmp_path / "nonexistent.txt"

        with pytest.raises(FileOpsError) as exc_info:
            FileOps.read_text(test_file)
        assert "Failed to read" in str(exc_info.value)
        assert isinstance(exc_info.value.__cause__, FileNotFoundError)

    def test_read_text_encoding_errors_replace(self, tmp_path: Path):
        """Test reading file with encoding errors using 'replace' strategy."""
        test_file = tmp_path / "invalid.txt"
        # Write invalid UTF-8 bytes
        test_file.write_bytes(b"Hello \xFF World")

        result = FileOps.read_text(test_file, errors="replace")
        assert "Hello" in result
        assert "World" in result

    def test_read_text_custom_encoding(self, tmp_path: Path):
        """Test reading file with custom encoding."""
        test_file = tmp_path / "latin1.txt"
        content = "Café"
        test_file.write_text(content, encoding="latin-1")

        result = FileOps.read_text(test_file, encoding="latin-1")
        assert result == content


class TestFileOpsReadBytes:
    """Tests for FileOps.read_bytes()."""

    def test_read_bytes_basic(self, tmp_path: Path):
        """Test basic binary file reading."""
        test_file = tmp_path / "test.bin"
        content = b"\x00\x01\x02\x03"
        test_file.write_bytes(content)

        result = FileOps.read_bytes(test_file)
        assert result == content

    def test_read_bytes_nonexistent(self, tmp_path: Path):
        """Test reading nonexistent binary file raises FileOpsError."""
        test_file = tmp_path / "nonexistent.bin"

        with pytest.raises(FileOpsError) as exc_info:
            FileOps.read_bytes(test_file)
        assert "Failed to read" in str(exc_info.value)


class TestFileOpsWriteTextAtomic:
    """Tests for FileOps.write_text_atomic()."""

    def test_write_text_atomic_basic(self, tmp_path: Path):
        """Test basic atomic text file writing."""
        test_file = tmp_path / "test.txt"
        content = "Hello, World!"

        FileOps.write_text_atomic(test_file, content)

        assert test_file.exists()
        assert test_file.read_text(encoding="utf-8") == content

    def test_write_text_atomic_creates_parent_dir(self, tmp_path: Path):
        """Test that atomic write creates parent directories."""
        test_file = tmp_path / "subdir" / "nested" / "test.txt"
        content = "Hello!"

        FileOps.write_text_atomic(test_file, content)

        assert test_file.exists()
        assert test_file.read_text(encoding="utf-8") == content

    def test_write_text_atomic_overwrites(self, tmp_path: Path):
        """Test that atomic write overwrites existing file."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("old content", encoding="utf-8")

        new_content = "new content"
        FileOps.write_text_atomic(test_file, new_content)

        assert test_file.read_text(encoding="utf-8") == new_content

    def test_write_text_atomic_no_temp_file_remains(self, tmp_path: Path):
        """Test that temporary file is cleaned up after successful write."""
        test_file = tmp_path / "test.txt"
        content = "Hello!"

        FileOps.write_text_atomic(test_file, content)

        # Check that no .tmp files remain
        tmp_files = list(tmp_path.glob("*.tmp"))
        assert len(tmp_files) == 0

    def test_write_text_atomic_unicode(self, tmp_path: Path):
        """Test atomic write with Unicode content."""
        test_file = tmp_path / "unicode.txt"
        content = "Hello 世界 🌍"

        FileOps.write_text_atomic(test_file, content)

        assert test_file.read_text(encoding="utf-8") == content

    def test_write_text_atomic_empty_content(self, tmp_path: Path):
        """Test atomic write with empty content."""
        test_file = tmp_path / "empty.txt"

        FileOps.write_text_atomic(test_file, "")

        assert test_file.exists()
        assert test_file.read_text(encoding="utf-8") == ""

    def test_write_text_atomic_large_content(self, tmp_path: Path):
        """Test atomic write with large content."""
        test_file = tmp_path / "large.txt"
        content = "x" * 1_000_000  # 1MB of data

        FileOps.write_text_atomic(test_file, content)

        assert test_file.read_text(encoding="utf-8") == content

    def test_write_text_atomic_permission_denied(self, tmp_path: Path):
        """Test that permission denied raises FileOpsError."""
        if os.name == 'nt':
            pytest.skip("Permission test not reliable on Windows")

        # Create a read-only directory
        readonly_dir = tmp_path / "readonly"
        readonly_dir.mkdir()
        readonly_dir.chmod(0o444)

        test_file = readonly_dir / "test.txt"

        try:
            with pytest.raises(FileOpsError) as exc_info:
                FileOps.write_text_atomic(test_file, "content")
            assert "Failed to write" in str(exc_info.value)
        finally:
            # Clean up: restore write permission
            readonly_dir.chmod(0o755)


class TestFileOpsWriteBytesAtomic:
    """Tests for FileOps.write_bytes_atomic()."""

    def test_write_bytes_atomic_basic(self, tmp_path: Path):
        """Test basic atomic binary file writing."""
        test_file = tmp_path / "test.bin"
        content = b"\x00\x01\x02\x03"

        FileOps.write_bytes_atomic(test_file, content)

        assert test_file.exists()
        assert test_file.read_bytes() == content

    def test_write_bytes_atomic_overwrites(self, tmp_path: Path):
        """Test that atomic binary write overwrites existing file."""
        test_file = tmp_path / "test.bin"
        test_file.write_bytes(b"old")

        new_content = b"new"
        FileOps.write_bytes_atomic(test_file, new_content)

        assert test_file.read_bytes() == new_content


class TestFileOpsRemoveSafe:
    """Tests for FileOps.remove_safe()."""

    def test_remove_safe_existing_file(self, tmp_path: Path):
        """Test safe removal of existing file."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("content", encoding="utf-8")

        result = FileOps.remove_safe(test_file)

        assert result is True
        assert not test_file.exists()

    def test_remove_safe_nonexistent_file(self, tmp_path: Path):
        """Test safe removal of nonexistent file returns True."""
        test_file = tmp_path / "nonexistent.txt"

        result = FileOps.remove_safe(test_file)

        assert result is True

    def test_remove_safe_directory_returns_false(self, tmp_path: Path):
        """Test that removing a directory returns False."""
        test_dir = tmp_path / "testdir"
        test_dir.mkdir()

        result = FileOps.remove_safe(test_dir)

        # Should fail because it's a directory, not a file
        assert result is False
        assert test_dir.exists()


class TestFileOpsEnsureDir:
    """Tests for FileOps.ensure_dir()."""

    def test_ensure_dir_creates_directory(self, tmp_path: Path):
        """Test that ensure_dir creates a directory."""
        test_dir = tmp_path / "testdir"

        FileOps.ensure_dir(test_dir)

        assert test_dir.exists()
        assert test_dir.is_dir()

    def test_ensure_dir_creates_nested_directories(self, tmp_path: Path):
        """Test that ensure_dir creates nested directories."""
        test_dir = tmp_path / "parent" / "child" / "grandchild"

        FileOps.ensure_dir(test_dir)

        assert test_dir.exists()
        assert test_dir.is_dir()

    def test_ensure_dir_idempotent(self, tmp_path: Path):
        """Test that ensure_dir is idempotent (calling twice is safe)."""
        test_dir = tmp_path / "testdir"

        FileOps.ensure_dir(test_dir)
        FileOps.ensure_dir(test_dir)  # Should not raise

        assert test_dir.exists()
        assert test_dir.is_dir()

    def test_ensure_dir_existing_file_raises_error(self, tmp_path: Path):
        """Test that ensure_dir raises error if path is an existing file."""
        test_file = tmp_path / "testfile"
        test_file.write_text("content", encoding="utf-8")

        with pytest.raises(FileOpsError) as exc_info:
            FileOps.ensure_dir(test_file)
        assert "Failed to create directory" in str(exc_info.value)


class TestFileOpsErrorChaining:
    """Tests for error chain preservation."""

    def test_error_chain_preserved_on_read(self, tmp_path: Path):
        """Test that exception chain is preserved for read errors."""
        test_file = tmp_path / "nonexistent.txt"

        with pytest.raises(FileOpsError) as exc_info:
            FileOps.read_text(test_file)

        # Verify the exception chain is preserved
        assert exc_info.value.__cause__ is not None
        assert isinstance(exc_info.value.__cause__, FileNotFoundError)

    def test_error_chain_preserved_on_write(self, tmp_path: Path):
        """Test that exception chain is preserved for write errors."""
        if os.name == 'nt':
            pytest.skip("Permission test not reliable on Windows")

        # Create a read-only directory
        readonly_dir = tmp_path / "readonly"
        readonly_dir.mkdir()
        readonly_dir.chmod(0o444)

        test_file = readonly_dir / "test.txt"

        try:
            with pytest.raises(FileOpsError) as exc_info:
                FileOps.write_text_atomic(test_file, "content")

            # Verify the exception chain is preserved
            assert exc_info.value.__cause__ is not None
        finally:
            # Clean up
            readonly_dir.chmod(0o755)
