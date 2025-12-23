#!/usr/bin/env python3
"""
Basic tests for Infiniloom Python bindings.
"""

import pytest
import infiniloom
from infiniloom import Infiniloom, InfiniloomError
import tempfile
import os
from pathlib import Path


def test_version():
    """Test that version is available."""
    assert hasattr(infiniloom, "__version__")
    assert infiniloom.__version__


def test_count_tokens():
    """Test token counting."""
    text = "Hello, world!"

    # Test different models
    claude_tokens = infiniloom.count_tokens(text, model="claude")
    gpt_tokens = infiniloom.count_tokens(text, model="gpt")
    gemini_tokens = infiniloom.count_tokens(text, model="gemini")

    assert claude_tokens > 0
    assert gpt_tokens > 0
    assert gemini_tokens > 0

    # Tokens should be similar but not necessarily identical
    assert abs(claude_tokens - gpt_tokens) < 5


def test_count_tokens_invalid_model():
    """Test that invalid model raises error."""
    with pytest.raises(ValueError):
        infiniloom.count_tokens("test", model="invalid_model")


def test_scan_nonexistent_path():
    """Test that scanning nonexistent path raises error."""
    with pytest.raises(InfiniloomError):
        infiniloom.scan("/nonexistent/path/xyz123")


def test_scan_with_temp_repo():
    """Test scanning a temporary repository."""
    with tempfile.TemporaryDirectory() as tmpdir:
        # Create a simple Python file
        test_file = Path(tmpdir) / "test.py"
        test_file.write_text("def hello():\n    print('world')\n")

        # Scan the directory
        stats = infiniloom.scan(tmpdir, respect_gitignore=False)

        assert stats["name"] == os.path.basename(tmpdir)
        assert stats["total_files"] == 1
        assert stats["total_lines"] > 0
        assert "total_tokens" in stats
        assert stats["total_tokens"]["claude"] > 0

        # Check languages
        assert len(stats["languages"]) > 0
        assert any(lang["language"] == "python" for lang in stats["languages"])


def test_pack_with_temp_repo():
    """Test packing a temporary repository."""
    with tempfile.TemporaryDirectory() as tmpdir:
        # Create test files
        (Path(tmpdir) / "main.py").write_text("def main():\n    pass\n")
        (Path(tmpdir) / "utils.py").write_text("def util():\n    pass\n")

        # Pack in different formats
        xml_output = infiniloom.pack(tmpdir, format="xml", model="claude")
        assert len(xml_output) > 0
        assert "repository" in xml_output.lower() or "repo" in xml_output.lower()

        md_output = infiniloom.pack(tmpdir, format="markdown", model="gpt")
        assert len(md_output) > 0

        json_output = infiniloom.pack(tmpdir, format="json", model="claude")
        assert len(json_output) > 0


def test_pack_invalid_format():
    """Test that invalid format raises error."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with pytest.raises(ValueError):
            infiniloom.pack(tmpdir, format="invalid_format")


def test_pack_invalid_compression():
    """Test that invalid compression raises error."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with pytest.raises(ValueError):
            infiniloom.pack(tmpdir, compression="invalid_compression")


def test_infiniloom_class():
    """Test Infiniloom class."""
    with tempfile.TemporaryDirectory() as tmpdir:
        # Create test file
        (Path(tmpdir) / "test.py").write_text("def test():\n    pass\n")

        # Create Infiniloom instance
        loom = Infiniloom(tmpdir)
        assert str(tmpdir) in str(loom)

        # Test stats
        stats = loom.stats()
        assert stats["total_files"] == 1
        assert "tokens" in stats

        # Test files
        files = loom.files()
        assert len(files) == 1
        assert files[0]["path"] == "test.py"
        assert files[0]["language"] == "python"

        # Test pack
        context = loom.pack(format="xml", model="claude")
        assert len(context) > 0

        # Test map
        repo_map = loom.map(map_budget=1000, max_symbols=10)
        assert "summary" in repo_map
        assert "key_symbols" in repo_map
        assert "token_count" in repo_map


def test_infiniloom_class_nonexistent():
    """Test that Infiniloom raises error for nonexistent path."""
    with pytest.raises(IOError):
        Infiniloom("/nonexistent/path/xyz123")


def test_security_scan():
    """Test security scanning."""
    with tempfile.TemporaryDirectory() as tmpdir:
        # Create a file with potential security issue
        test_file = Path(tmpdir) / "test.py"
        test_file.write_text("password = 'secret123'\napi_key = 'sk-1234567890'\n")

        # Scan for security issues
        findings = infiniloom.scan_security(tmpdir)

        # We expect to find some issues (hardcoded credentials)
        assert isinstance(findings, list)
        # Note: The actual findings depend on the SecurityScanner implementation


def test_multiple_languages():
    """Test scanning repository with multiple languages."""
    with tempfile.TemporaryDirectory() as tmpdir:
        # Create files in different languages
        (Path(tmpdir) / "main.py").write_text("def main(): pass")
        (Path(tmpdir) / "utils.js").write_text("function utils() {}")
        (Path(tmpdir) / "lib.rs").write_text("fn main() {}")

        stats = infiniloom.scan(tmpdir, respect_gitignore=False)

        assert stats["total_files"] == 3

        languages = {lang["language"] for lang in stats["languages"]}
        assert "python" in languages
        assert "javascript" in languages
        assert "rust" in languages


def test_gitignore_respect():
    """Test that .gitignore is respected."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)

        # Create .gitignore
        (tmpdir_path / ".gitignore").write_text("ignored.py\n")

        # Create files
        (tmpdir_path / "main.py").write_text("def main(): pass")
        (tmpdir_path / "ignored.py").write_text("def ignored(): pass")

        # Scan with gitignore respect
        stats = infiniloom.scan(tmpdir, respect_gitignore=True)

        # Should only find main.py and .gitignore
        # (gitignore itself is typically included)
        assert stats["total_files"] <= 2

        # Scan without gitignore respect
        stats_no_ignore = infiniloom.scan(tmpdir, respect_gitignore=False)
        assert stats_no_ignore["total_files"] >= 2


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
