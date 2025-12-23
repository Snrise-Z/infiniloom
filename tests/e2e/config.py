"""Configuration for E2E comparison tests between Infiniloom and Repomix."""

from pathlib import Path
from dataclasses import dataclass
from typing import Optional
import os

# Base paths
PROJECT_ROOT = Path(__file__).parent.parent.parent
TEST_ROOT = PROJECT_ROOT / "tests" / "e2e"
REPOS_DIR = TEST_ROOT / "repos"
OUTPUTS_DIR = TEST_ROOT / "outputs"
REPORTS_DIR = TEST_ROOT / "reports"

# Tool paths
INFINILOOM_BIN = PROJECT_ROOT / "target" / "release" / "infiniloom"
REPOMIX_BIN = "repomix"  # Assumes globally installed


@dataclass
class TestRepo:
    """Configuration for a test repository."""
    name: str
    url: str
    description: str
    branch: str = "main"
    expected_files_min: int = 10
    expected_files_max: int = 10000
    primary_languages: tuple = ()
    complexity: str = "medium"  # simple, medium, complex


# Target repositories for testing - diverse set of real-world projects
TEST_REPOSITORIES = [
    # Small Python CLI tool
    TestRepo(
        name="httpie",
        url="https://github.com/httpie/cli",
        description="Modern HTTP client for the terminal",
        branch="master",
        expected_files_min=50,
        expected_files_max=500,
        primary_languages=("python",),
        complexity="simple",
    ),
    # Medium TypeScript/React project
    TestRepo(
        name="excalidraw",
        url="https://github.com/excalidraw/excalidraw",
        description="Virtual whiteboard for sketching hand-drawn like diagrams",
        branch="master",
        expected_files_min=200,
        expected_files_max=2000,
        primary_languages=("typescript", "javascript"),
        complexity="medium",
    ),
    # Complex multi-language project
    TestRepo(
        name="deno",
        url="https://github.com/denoland/deno",
        description="JavaScript/TypeScript runtime with Rust core",
        branch="main",
        expected_files_min=500,
        expected_files_max=15000,  # Deno has grown significantly
        primary_languages=("rust", "typescript", "javascript"),
        complexity="complex",
    ),
    # Go project with moderate size
    TestRepo(
        name="lazygit",
        url="https://github.com/jesseduffield/lazygit",
        description="Simple terminal UI for git commands",
        branch="master",
        expected_files_min=100,
        expected_files_max=1000,
        primary_languages=("go",),
        complexity="medium",
    ),
    # Rust project
    TestRepo(
        name="ripgrep",
        url="https://github.com/BurntSushi/ripgrep",
        description="Fast line-oriented search tool",
        branch="master",
        expected_files_min=50,
        expected_files_max=500,
        primary_languages=("rust",),
        complexity="medium",
    ),
    # Large monorepo with many file types
    TestRepo(
        name="material-ui",
        url="https://github.com/mui/material-ui",
        description="React UI component library",
        branch="master",
        expected_files_min=1000,
        expected_files_max=10000,
        primary_languages=("typescript", "javascript"),
        complexity="complex",
    ),
]


@dataclass
class ComparisonMetrics:
    """Metrics for comparing tool outputs."""
    # Performance
    scan_time_ms: float = 0
    token_count: int = 0

    # Coverage
    files_included: int = 0
    files_excluded: int = 0
    total_bytes: int = 0

    # Quality
    output_size_bytes: int = 0
    compression_ratio: float = 0

    # Language breakdown
    languages: dict = None

    def __post_init__(self):
        if self.languages is None:
            self.languages = {}


def ensure_dirs():
    """Ensure all required directories exist."""
    REPOS_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUTS_DIR.mkdir(parents=True, exist_ok=True)
    REPORTS_DIR.mkdir(parents=True, exist_ok=True)


def get_repo_path(repo: TestRepo) -> Path:
    """Get local path for a repository."""
    return REPOS_DIR / repo.name
