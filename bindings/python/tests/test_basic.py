#!/usr/bin/env python3
"""
Basic tests for Infiniloom Python bindings.
"""

import pytest
import infiniloom
from infiniloom import Infiniloom, InfiniloomError, GitRepo, semantic_compress, is_git_repo
import tempfile
import os
import subprocess
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


def test_semantic_compress():
    """Test semantic compression."""
    # Create a long text with repetitive content
    paragraphs = [f"Paragraph {i}\n" + "x" * 140 for i in range(12)]
    text = "\n\n".join(paragraphs)

    # Compress the text
    compressed = semantic_compress(text, similarity_threshold=0.7, budget_ratio=0.5)

    # Should return non-empty result that's smaller than original
    assert len(compressed) > 0
    assert len(compressed) < len(text)


def test_semantic_compress_short_text():
    """Test semantic compression with short text."""
    text = "Hello, world!"
    compressed = semantic_compress(text, similarity_threshold=0.7, budget_ratio=0.5)
    # Short text may not compress much, but should still work
    assert len(compressed) > 0


def test_is_git_repo():
    """Test is_git_repo function."""
    # Test on the actual infiniloom repository root (parent of bindings/python)
    repo_root = Path(__file__).parent.parent.parent.parent
    assert is_git_repo(str(repo_root)) is True

    # Test on a non-git directory
    with tempfile.TemporaryDirectory() as tmpdir:
        assert is_git_repo(tmpdir) is False


def test_is_git_repo_nonexistent():
    """Test is_git_repo with nonexistent path."""
    # Should return False for nonexistent paths, not raise
    assert is_git_repo("/nonexistent/path/xyz123") is False


def create_git_repo():
    """Helper to create a temporary git repository."""
    tmpdir = tempfile.mkdtemp()
    subprocess.run(["git", "init"], cwd=tmpdir, capture_output=True)
    subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=tmpdir, capture_output=True)
    subprocess.run(["git", "config", "user.name", "Test User"], cwd=tmpdir, capture_output=True)

    # Create a test file and commit it
    test_file = Path(tmpdir) / "test.py"
    test_file.write_text("def hello():\n    return 'world'\n")
    subprocess.run(["git", "add", "test.py"], cwd=tmpdir, capture_output=True)
    subprocess.run(["git", "commit", "-m", "Initial commit"], cwd=tmpdir, capture_output=True)

    return tmpdir


def test_git_repo_class():
    """Test GitRepo class."""
    tmpdir = create_git_repo()
    try:
        # Create GitRepo instance
        repo = GitRepo(tmpdir)

        # Test current_branch
        branch = repo.current_branch()
        assert isinstance(branch, str)
        assert len(branch) > 0

        # Test current_commit
        commit = repo.current_commit()
        assert isinstance(commit, str)
        assert len(commit) == 40  # Full SHA-1 hash

        # Test status (should be clean after commit)
        status = repo.status()
        assert isinstance(status, list)

        # Test log
        log = repo.log(count=5)
        assert isinstance(log, list)
        assert len(log) >= 1
        assert "hash" in log[0]
        assert "author" in log[0]
        assert "message" in log[0]

        # Test ls_files
        files = repo.ls_files()
        assert isinstance(files, list)
        assert "test.py" in files

        # Test file_log
        file_log = repo.file_log("test.py", count=5)
        assert isinstance(file_log, list)
        assert len(file_log) >= 1

        # Test last_modified_commit
        last_commit = repo.last_modified_commit("test.py")
        assert isinstance(last_commit, dict)
        assert "hash" in last_commit

        # Test file_change_frequency
        freq = repo.file_change_frequency("test.py", days=30)
        assert isinstance(freq, int)
        assert freq >= 1

        # Test has_changes (should be False after clean commit)
        assert repo.has_changes("test.py") is False

        # Modify the file
        (Path(tmpdir) / "test.py").write_text("def hello():\n    return 'modified'\n")
        assert repo.has_changes("test.py") is True

        # Test uncommitted_diff
        diff = repo.uncommitted_diff("test.py")
        assert isinstance(diff, str)
        assert "modified" in diff

        # Test all_uncommitted_diffs
        all_diff = repo.all_uncommitted_diffs()
        assert isinstance(all_diff, str)

    finally:
        import shutil
        shutil.rmtree(tmpdir)


def test_git_repo_blame():
    """Test GitRepo blame functionality."""
    tmpdir = create_git_repo()
    try:
        repo = GitRepo(tmpdir)

        # Test blame
        blame = repo.blame("test.py")
        assert isinstance(blame, list)
        assert len(blame) >= 1
        assert "commit" in blame[0]
        assert "author" in blame[0]
        assert "line_number" in blame[0]

    finally:
        import shutil
        shutil.rmtree(tmpdir)


def test_git_repo_diff_files():
    """Test GitRepo diff_files functionality."""
    tmpdir = create_git_repo()
    try:
        # Create another commit
        test_file2 = Path(tmpdir) / "test2.py"
        test_file2.write_text("def goodbye():\n    return 'goodbye'\n")
        subprocess.run(["git", "add", "test2.py"], cwd=tmpdir, capture_output=True)
        subprocess.run(["git", "commit", "-m", "Add test2.py"], cwd=tmpdir, capture_output=True)

        repo = GitRepo(tmpdir)

        # Get diff between HEAD~1 and HEAD
        diff_files = repo.diff_files("HEAD~1", "HEAD")
        assert isinstance(diff_files, list)
        assert len(diff_files) >= 1

        found_test2 = False
        for f in diff_files:
            assert "path" in f
            assert "status" in f
            assert "additions" in f
            assert "deletions" in f
            if f["path"] == "test2.py":
                found_test2 = True
                assert f["status"] == "Added"

        assert found_test2

    finally:
        import shutil
        shutil.rmtree(tmpdir)


def test_git_repo_nonexistent():
    """Test that GitRepo raises error for nonexistent path."""
    with pytest.raises(InfiniloomError):
        GitRepo("/nonexistent/path/xyz123")


def test_git_repo_not_a_repo():
    """Test that GitRepo raises error for non-git directory."""
    with tempfile.TemporaryDirectory() as tmpdir:
        with pytest.raises(InfiniloomError):
            GitRepo(tmpdir)


def test_all_exports_available():
    """Test that all expected exports are available."""
    # Functions
    assert callable(infiniloom.pack)
    assert callable(infiniloom.scan)
    assert callable(infiniloom.count_tokens)
    assert callable(infiniloom.scan_security)
    assert callable(infiniloom.semantic_compress)
    assert callable(infiniloom.is_git_repo)

    # Classes
    assert callable(infiniloom.Infiniloom)
    assert callable(infiniloom.GitRepo)

    # Exception
    assert issubclass(infiniloom.InfiniloomError, Exception)

    # Version
    assert isinstance(infiniloom.__version__, str)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
