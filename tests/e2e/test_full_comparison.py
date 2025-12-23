#!/usr/bin/env python3
"""
Comprehensive E2E comparison tests for Infiniloom vs Repomix vs GitIngest.

This test suite compares:
1. Performance (speed)
2. Output size and token efficiency
3. File coverage
4. Feature parity
5. Output quality
"""

import subprocess
import time
import json
import re
import os
import sys
import tempfile
import shutil
from pathlib import Path
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple
from datetime import datetime

# Add parent to path for config import
sys.path.insert(0, str(Path(__file__).parent))
from config import TEST_REPOSITORIES, TestRepo, REPOS_DIR, OUTPUTS_DIR, PROJECT_ROOT

# Tool paths
INFINILOOM_BIN = PROJECT_ROOT / "target" / "release" / "infiniloom"
REPOMIX_BIN = "repomix"
GITINGEST_BIN = "gitingest"


@dataclass
class ToolResult:
    """Results from running a tool on a repository."""
    tool_name: str
    repo_name: str
    success: bool
    error: Optional[str] = None

    # Timing
    time_seconds: float = 0.0

    # Output metrics
    output_size_bytes: int = 0
    file_count: int = 0
    token_count: int = 0
    line_count: int = 0

    # Features detected
    has_directory_structure: bool = False
    has_file_contents: bool = False
    has_metadata: bool = False
    has_security_scan: bool = False

    # Output path
    output_path: Optional[Path] = None


@dataclass
class ComparisonReport:
    """Full comparison report across all tools and repos."""
    timestamp: str = field(default_factory=lambda: datetime.now().isoformat())
    results: Dict[str, Dict[str, ToolResult]] = field(default_factory=dict)

    def add_result(self, result: ToolResult):
        if result.repo_name not in self.results:
            self.results[result.repo_name] = {}
        self.results[result.repo_name][result.tool_name] = result


def check_tool_available(tool: str) -> bool:
    """Check if a tool is available."""
    if tool == "infiniloom":
        return INFINILOOM_BIN.exists()
    try:
        result = subprocess.run([tool, "--help"], capture_output=True, timeout=10)
        return result.returncode == 0
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return False


def clone_repo(repo: TestRepo) -> Optional[Path]:
    """Clone repository if needed."""
    repo_path = REPOS_DIR / repo.name

    if repo_path.exists():
        return repo_path

    print(f"  Cloning {repo.name}...")
    try:
        result = subprocess.run(
            ["git", "clone", "--depth", "1", "--branch", repo.branch, repo.url, str(repo_path)],
            capture_output=True,
            timeout=300,
        )
        if result.returncode == 0:
            return repo_path
    except subprocess.TimeoutExpired:
        pass

    return None


def run_infiniloom(repo_path: Path, output_path: Path, extra_args: List[str] = None) -> ToolResult:
    """Run Infiniloom on a repository."""
    result = ToolResult(tool_name="infiniloom", repo_name=repo_path.name, success=False)

    cmd = [str(INFINILOOM_BIN), "pack", str(repo_path), "-o", str(output_path), "-v"]
    if extra_args:
        cmd.extend(extra_args)

    try:
        start = time.time()
        proc = subprocess.run(cmd, capture_output=True, timeout=600, text=True)
        result.time_seconds = time.time() - start

        if proc.returncode == 0:
            result.success = True
            result.output_path = output_path

            # Parse output metrics
            output = proc.stderr + proc.stdout

            # File count
            match = re.search(r"(\d+)\s*files", output, re.I)
            if match:
                result.file_count = int(match.group(1))

            # Token count
            match = re.search(r"~?(\d+)\s*tokens", output, re.I)
            if match:
                result.token_count = int(match.group(1))

            # Line count
            match = re.search(r"(\d+)\s*lines", output, re.I)
            if match:
                result.line_count = int(match.group(1))

            # Output size
            if output_path.exists():
                result.output_size_bytes = output_path.stat().st_size
                content = output_path.read_text(errors='ignore')
                result.has_directory_structure = "directory" in content.lower() or "<structure>" in content.lower()
                result.has_file_contents = "<content>" in content or "```" in content
                result.has_metadata = "<metadata>" in content or "repository" in content.lower()
        else:
            result.error = proc.stderr[:500]

    except subprocess.TimeoutExpired:
        result.error = "Timeout after 600s"
    except Exception as e:
        result.error = str(e)

    return result


def run_repomix(repo_path: Path, output_path: Path, extra_args: List[str] = None) -> ToolResult:
    """Run Repomix on a repository."""
    result = ToolResult(tool_name="repomix", repo_name=repo_path.name, success=False)

    cmd = ["repomix", str(repo_path), "-o", str(output_path), "--style", "xml"]
    if extra_args:
        cmd.extend(extra_args)

    try:
        start = time.time()
        proc = subprocess.run(cmd, capture_output=True, timeout=600, text=True, cwd=str(repo_path))
        result.time_seconds = time.time() - start

        if proc.returncode == 0:
            result.success = True
            result.output_path = output_path

            # Parse output metrics from verbose output
            output = proc.stderr + proc.stdout

            # File count
            match = re.search(r"(\d+)\s*files?", output, re.I)
            if match:
                result.file_count = int(match.group(1))

            # Token count
            match = re.search(r"(\d+)\s*tokens?", output, re.I)
            if match:
                result.token_count = int(match.group(1))

            # Output size
            if output_path.exists():
                result.output_size_bytes = output_path.stat().st_size
                content = output_path.read_text(errors='ignore')
                result.has_directory_structure = "directory" in content.lower() or "structure" in content.lower()
                result.has_file_contents = "<file" in content or "```" in content
                result.has_metadata = "summary" in content.lower()
        else:
            result.error = proc.stderr[:500]

    except subprocess.TimeoutExpired:
        result.error = "Timeout after 600s"
    except Exception as e:
        result.error = str(e)

    return result


def run_gitingest(repo_path: Path, output_path: Path, extra_args: List[str] = None) -> ToolResult:
    """Run GitIngest on a repository."""
    result = ToolResult(tool_name="gitingest", repo_name=repo_path.name, success=False)

    cmd = ["gitingest", str(repo_path), "-o", str(output_path)]
    if extra_args:
        cmd.extend(extra_args)

    try:
        start = time.time()
        proc = subprocess.run(cmd, capture_output=True, timeout=600, text=True)
        result.time_seconds = time.time() - start

        if proc.returncode == 0:
            result.success = True
            result.output_path = output_path

            # Parse output metrics
            output = proc.stderr + proc.stdout

            # File count
            match = re.search(r"(\d+)\s*files?", output, re.I)
            if match:
                result.file_count = int(match.group(1))

            # Token count
            match = re.search(r"(\d+)\s*tokens?", output, re.I)
            if match:
                result.token_count = int(match.group(1))

            # Output size
            if output_path.exists():
                result.output_size_bytes = output_path.stat().st_size
                content = output_path.read_text(errors='ignore')
                result.has_directory_structure = "directory" in content.lower() or "tree" in content.lower()
                result.has_file_contents = "```" in content or "content" in content.lower()
                result.has_metadata = "repository" in content.lower() or "summary" in content.lower()
        else:
            result.error = proc.stderr[:500]

    except subprocess.TimeoutExpired:
        result.error = "Timeout after 600s"
    except Exception as e:
        result.error = str(e)

    return result


def count_tokens_tiktoken(text: str) -> int:
    """Count tokens using tiktoken (cl100k_base for GPT-4/Claude)."""
    try:
        import tiktoken
        enc = tiktoken.get_encoding("cl100k_base")
        return len(enc.encode(text))
    except ImportError:
        # Fallback: estimate ~4 chars per token
        return len(text) // 4


def analyze_output_quality(output_path: Path) -> Dict:
    """Analyze output quality metrics."""
    if not output_path or not output_path.exists():
        return {}

    content = output_path.read_text(errors='ignore')

    return {
        "total_chars": len(content),
        "total_lines": content.count('\n'),
        "estimated_tokens": count_tokens_tiktoken(content),
        "has_xml_structure": content.startswith("<?xml") or "<repository" in content,
        "has_markdown": "```" in content,
        "code_blocks": content.count("```") // 2,
        "file_sections": content.count("<file") + content.count("### "),
    }


def print_comparison_table(report: ComparisonReport):
    """Print a formatted comparison table."""
    print("\n" + "=" * 100)
    print("COMPARISON RESULTS")
    print("=" * 100)

    tools = ["infiniloom", "repomix", "gitingest"]

    for repo_name, tool_results in report.results.items():
        print(f"\n📁 {repo_name}")
        print("-" * 80)

        # Header
        print(f"{'Metric':<25} | {'Infiniloom':<20} | {'Repomix':<20} | {'GitIngest':<20}")
        print("-" * 80)

        # Success
        row = f"{'Success':<25}"
        for tool in tools:
            r = tool_results.get(tool)
            val = "✓" if (r and r.success) else "✗" if r else "N/A"
            row += f" | {val:<20}"
        print(row)

        # Time
        row = f"{'Time (seconds)':<25}"
        for tool in tools:
            r = tool_results.get(tool)
            val = f"{r.time_seconds:.2f}s" if (r and r.success) else "-"
            row += f" | {val:<20}"
        print(row)

        # Files
        row = f"{'Files':<25}"
        for tool in tools:
            r = tool_results.get(tool)
            val = str(r.file_count) if (r and r.success) else "-"
            row += f" | {val:<20}"
        print(row)

        # Tokens
        row = f"{'Tokens':<25}"
        for tool in tools:
            r = tool_results.get(tool)
            val = f"{r.token_count:,}" if (r and r.success and r.token_count) else "-"
            row += f" | {val:<20}"
        print(row)

        # Output size
        row = f"{'Output Size':<25}"
        for tool in tools:
            r = tool_results.get(tool)
            if r and r.success and r.output_size_bytes:
                if r.output_size_bytes > 1024 * 1024:
                    val = f"{r.output_size_bytes / 1024 / 1024:.1f} MB"
                else:
                    val = f"{r.output_size_bytes / 1024:.1f} KB"
            else:
                val = "-"
            row += f" | {val:<20}"
        print(row)

        # Features
        row = f"{'Has Dir Structure':<25}"
        for tool in tools:
            r = tool_results.get(tool)
            val = "✓" if (r and r.has_directory_structure) else "✗"
            row += f" | {val:<20}"
        print(row)

        row = f"{'Has File Contents':<25}"
        for tool in tools:
            r = tool_results.get(tool)
            val = "✓" if (r and r.has_file_contents) else "✗"
            row += f" | {val:<20}"
        print(row)

    # Summary
    print("\n" + "=" * 100)
    print("SUMMARY")
    print("=" * 100)

    # Calculate averages
    for tool in tools:
        successes = 0
        total_time = 0
        total_size = 0
        count = 0

        for repo_results in report.results.values():
            r = repo_results.get(tool)
            if r:
                if r.success:
                    successes += 1
                    total_time += r.time_seconds
                    total_size += r.output_size_bytes
                count += 1

        if count > 0:
            print(f"\n{tool.upper()}:")
            print(f"  Success rate: {successes}/{count} ({100*successes/count:.0f}%)")
            if successes > 0:
                print(f"  Avg time: {total_time/successes:.2f}s")
                print(f"  Avg output: {total_size/successes/1024:.1f} KB")


def run_feature_tests(repo_path: Path, output_dir: Path) -> Dict[str, Dict]:
    """Run feature-specific tests."""
    results = {}

    # Test 1: Basic pack
    print("  Testing basic pack...")
    results["basic"] = {}

    for tool_name, run_func in [("infiniloom", run_infiniloom), ("repomix", run_repomix), ("gitingest", run_gitingest)]:
        if not check_tool_available(tool_name):
            continue
        output = output_dir / f"{repo_path.name}_{tool_name}_basic.xml"
        r = run_func(repo_path, output)
        results["basic"][tool_name] = {"success": r.success, "time": r.time_seconds, "size": r.output_size_bytes}

    # Test 2: With compression (Infiniloom specific)
    if check_tool_available("infiniloom"):
        print("  Testing compression levels...")
        for level in ["none", "minimal", "balanced", "aggressive"]:
            output = output_dir / f"{repo_path.name}_infiniloom_{level}.xml"
            r = run_infiniloom(repo_path, output, ["-c", level])
            results[f"compression_{level}"] = {"success": r.success, "size": r.output_size_bytes, "tokens": r.token_count}

    # Test 3: Different output formats (Infiniloom)
    if check_tool_available("infiniloom"):
        print("  Testing output formats...")
        for fmt in ["xml", "markdown", "json", "plain", "toon"]:
            output = output_dir / f"{repo_path.name}_infiniloom_{fmt}.{'json' if fmt == 'json' else 'txt'}"
            r = run_infiniloom(repo_path, output, ["-f", fmt])
            results[f"format_{fmt}"] = {"success": r.success, "size": r.output_size_bytes}

    return results


def main():
    """Run the full E2E comparison test suite."""
    print("=" * 100)
    print("Infiniloom vs Repomix vs GitIngest - Full E2E Comparison")
    print("=" * 100)
    print(f"Timestamp: {datetime.now().isoformat()}")
    print()

    # Check tool availability
    print("Checking tool availability...")
    tools_status = {
        "infiniloom": check_tool_available("infiniloom"),
        "repomix": check_tool_available("repomix"),
        "gitingest": check_tool_available("gitingest"),
    }

    for tool, available in tools_status.items():
        status = "✓ Available" if available else "✗ Not found"
        print(f"  {tool}: {status}")

    if not any(tools_status.values()):
        print("\nNo tools available! Exiting.")
        return 1

    # Ensure directories exist
    OUTPUTS_DIR.mkdir(parents=True, exist_ok=True)
    REPOS_DIR.mkdir(parents=True, exist_ok=True)

    # Run tests
    report = ComparisonReport()

    # Test on a subset of repositories for speed
    test_repos = TEST_REPOSITORIES[:4]  # httpie, excalidraw, deno, lazygit

    for repo in test_repos:
        print(f"\n{'=' * 80}")
        print(f"Testing: {repo.name} ({repo.complexity})")
        print(f"{'=' * 80}")

        # Clone if needed
        repo_path = clone_repo(repo)
        if not repo_path:
            print(f"  ✗ Failed to clone {repo.name}")
            continue

        print(f"  Repository ready at {repo_path}")

        # Run each tool
        for tool_name, run_func in [
            ("infiniloom", run_infiniloom),
            ("repomix", run_repomix),
            ("gitingest", run_gitingest),
        ]:
            if not tools_status[tool_name]:
                print(f"  Skipping {tool_name} (not available)")
                continue

            print(f"  Running {tool_name}...")
            output_path = OUTPUTS_DIR / f"{repo.name}_{tool_name}.xml"
            result = run_func(repo_path, output_path)
            report.add_result(result)

            if result.success:
                print(f"    ✓ Success in {result.time_seconds:.2f}s ({result.file_count} files, {result.output_size_bytes//1024}KB)")
            else:
                print(f"    ✗ Failed: {result.error[:100] if result.error else 'Unknown error'}")

        # Run feature-specific tests
        print("  Running feature tests...")
        feature_results = run_feature_tests(repo_path, OUTPUTS_DIR)

    # Print comparison table
    print_comparison_table(report)

    # Save report
    report_path = OUTPUTS_DIR / "comparison_report.json"
    report_data = {
        "timestamp": report.timestamp,
        "tools_available": tools_status,
        "results": {
            repo_name: {
                tool_name: {
                    "success": r.success,
                    "time_seconds": r.time_seconds,
                    "file_count": r.file_count,
                    "token_count": r.token_count,
                    "output_size_bytes": r.output_size_bytes,
                    "error": r.error,
                }
                for tool_name, r in tools.items()
            }
            for repo_name, tools in report.results.items()
        }
    }

    with open(report_path, 'w') as f:
        json.dump(report_data, f, indent=2)

    print(f"\n📊 Report saved to: {report_path}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
