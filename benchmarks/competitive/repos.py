"""Repository definitions and cloning utilities for competitive benchmarks."""

import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class TestRepo:
    """Definition of a test repository."""

    name: str
    url: str
    description: str
    estimated_files: int
    primary_language: str
    branch: str = "main"

    @property
    def local_path(self) -> Path:
        """Get local path for cloned repo."""
        return Path(__file__).parent / "repos" / self.name


# Real-world test repositories of varying sizes
TEST_REPOS = [
    TestRepo(
        name="fastapi",
        url="https://github.com/fastapi/fastapi",
        description="Popular Python framework",
        estimated_files=500,
        primary_language="Python",
    ),
    TestRepo(
        name="deno",
        url="https://github.com/denoland/deno",
        description="Large mixed codebase (Rust + TS)",
        estimated_files=2000,
        primary_language="Rust",
    ),
    TestRepo(
        name="lodash",
        url="https://github.com/lodash/lodash",
        description="JavaScript utility library",
        estimated_files=600,
        primary_language="JavaScript",
    ),
    TestRepo(
        name="rust-analyzer",
        url="https://github.com/rust-lang/rust-analyzer",
        description="Complex Rust project",
        estimated_files=1500,
        primary_language="Rust",
    ),
    TestRepo(
        name="TypeScript",
        url="https://github.com/microsoft/TypeScript",
        description="Large TypeScript project",
        estimated_files=3000,
        primary_language="TypeScript",
    ),
    TestRepo(
        name="infiniloom",
        url="https://github.com/yourusername/infiniloom",  # Update with actual URL
        description="Self-test baseline (this project)",
        estimated_files=100,
        primary_language="Rust",
    ),
]


def clone_repo(repo: TestRepo, force: bool = False, shallow: bool = True) -> Path:
    """
    Clone a repository for benchmarking.

    Args:
        repo: Repository definition
        force: If True, delete existing clone
        shallow: If True, do shallow clone (faster)

    Returns:
        Path to cloned repository
    """
    local_path = repo.local_path

    if local_path.exists():
        if force:
            shutil.rmtree(local_path)
        else:
            print(f"Repository {repo.name} already exists at {local_path}")
            return local_path

    local_path.parent.mkdir(parents=True, exist_ok=True)

    cmd = ["git", "clone"]
    if shallow:
        cmd.extend(["--depth", "1"])
    cmd.extend(["--branch", repo.branch, repo.url, str(local_path)])

    print(f"Cloning {repo.name} from {repo.url}...")
    subprocess.run(cmd, check=True)

    return local_path


def clone_all_repos(force: bool = False, shallow: bool = True) -> dict[str, Path]:
    """
    Clone all test repositories.

    Returns:
        Dictionary mapping repo name to local path
    """
    results = {}
    for repo in TEST_REPOS:
        try:
            path = clone_repo(repo, force=force, shallow=shallow)
            results[repo.name] = path
        except subprocess.CalledProcessError as e:
            print(f"Failed to clone {repo.name}: {e}")
            results[repo.name] = None
    return results


def get_repo(name: str) -> Optional[TestRepo]:
    """Get repository definition by name."""
    for repo in TEST_REPOS:
        if repo.name == name:
            return repo
    return None


def list_repos() -> list[str]:
    """List all available repository names."""
    return [repo.name for repo in TEST_REPOS]


def count_files(path: Path, extensions: Optional[list[str]] = None) -> int:
    """
    Count source files in a directory.

    Args:
        path: Directory to scan
        extensions: File extensions to count (e.g., ['.py', '.rs'])

    Returns:
        Number of matching files
    """
    count = 0
    for root, _, files in os.walk(path):
        # Skip hidden directories
        if any(part.startswith('.') for part in Path(root).parts):
            continue
        for file in files:
            if extensions:
                if any(file.endswith(ext) for ext in extensions):
                    count += 1
            else:
                count += 1
    return count


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Manage test repositories")
    parser.add_argument("action", choices=["clone", "list", "clean"])
    parser.add_argument("--repo", help="Specific repo to operate on")
    parser.add_argument("--force", action="store_true", help="Force reclone")

    args = parser.parse_args()

    if args.action == "list":
        print("Available test repositories:")
        for repo in TEST_REPOS:
            status = "cloned" if repo.local_path.exists() else "not cloned"
            print(f"  {repo.name}: {repo.description} ({status})")

    elif args.action == "clone":
        if args.repo:
            repo = get_repo(args.repo)
            if repo:
                clone_repo(repo, force=args.force)
            else:
                print(f"Unknown repo: {args.repo}")
        else:
            clone_all_repos(force=args.force)

    elif args.action == "clean":
        repos_dir = Path(__file__).parent / "repos"
        if repos_dir.exists():
            shutil.rmtree(repos_dir)
            print("Cleaned all cloned repositories")
