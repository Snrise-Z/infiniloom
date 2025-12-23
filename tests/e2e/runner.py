#!/usr/bin/env python3
"""E2E test runner for Infiniloom vs Repomix comparison."""

import subprocess
import time
import json
import re
import os
import sys
from pathlib import Path
from dataclasses import dataclass, asdict
from typing import Optional, Dict, List, Tuple
import xml.etree.ElementTree as ET
from datetime import datetime

from config import (
    TEST_REPOSITORIES,
    TestRepo,
    ComparisonMetrics,
    REPOS_DIR,
    OUTPUTS_DIR,
    REPORTS_DIR,
    INFINILOOM_BIN,
    REPOMIX_BIN,
    ensure_dirs,
    get_repo_path,
)


@dataclass
class TestResult:
    """Result of a single test run."""
    repo_name: str
    tool: str
    success: bool
    error: Optional[str] = None
    metrics: Optional[ComparisonMetrics] = None
    output_file: Optional[str] = None


def clone_repo(repo: TestRepo, shallow: bool = True) -> Tuple[bool, str]:
    """Clone a repository if not already present."""
    repo_path = get_repo_path(repo)

    if repo_path.exists():
        print(f"  Repository {repo.name} already exists, pulling latest...")
        try:
            subprocess.run(
                ["git", "-C", str(repo_path), "pull", "--ff-only"],
                capture_output=True,
                timeout=120,
            )
            return True, str(repo_path)
        except Exception as e:
            return True, str(repo_path)  # Use existing even if pull fails

    print(f"  Cloning {repo.name} from {repo.url}...")
    cmd = ["git", "clone"]
    if shallow:
        cmd.extend(["--depth", "1"])
    cmd.extend(["--branch", repo.branch, repo.url, str(repo_path)])

    try:
        result = subprocess.run(cmd, capture_output=True, timeout=300, text=True)
        if result.returncode != 0:
            return False, result.stderr
        return True, str(repo_path)
    except subprocess.TimeoutExpired:
        return False, "Clone timed out"
    except Exception as e:
        return False, str(e)


def run_repomix(repo_path: Path, output_file: Path) -> Tuple[ComparisonMetrics, Optional[str]]:
    """Run repomix on a repository and collect metrics."""
    metrics = ComparisonMetrics()

    start_time = time.time()

    try:
        result = subprocess.run(
            [
                REPOMIX_BIN,
                str(repo_path),
                "-o", str(output_file),
                "--style", "xml",
                "--verbose",
            ],
            capture_output=True,
            timeout=600,
            text=True,
            cwd=str(repo_path),
        )

        metrics.scan_time_ms = (time.time() - start_time) * 1000

        if result.returncode != 0:
            return metrics, f"Repomix failed: {result.stderr}"

        # Parse output for metrics
        output = result.stdout + result.stderr

        # Extract file count
        file_match = re.search(r"(\d+)\s*files?\s*(?:processed|found|packed)", output, re.I)
        if file_match:
            metrics.files_included = int(file_match.group(1))

        # Extract token count
        token_match = re.search(r"(\d+(?:,\d+)*)\s*tokens?", output, re.I)
        if token_match:
            metrics.token_count = int(token_match.group(1).replace(",", ""))

        # Get output file size
        if output_file.exists():
            metrics.output_size_bytes = output_file.stat().st_size

        return metrics, None

    except subprocess.TimeoutExpired:
        return metrics, "Repomix timed out after 10 minutes"
    except Exception as e:
        return metrics, str(e)


def run_infiniloom(repo_path: Path, output_file: Path) -> Tuple[ComparisonMetrics, Optional[str]]:
    """Run Infiniloom on a repository and collect metrics."""
    metrics = ComparisonMetrics()

    if not INFINILOOM_BIN.exists():
        return metrics, f"Infiniloom binary not found at {INFINILOOM_BIN}"

    start_time = time.time()

    try:
        # Use the 'pack' subcommand to generate full output like repomix
        result = subprocess.run(
            [
                str(INFINILOOM_BIN),
                "pack",
                str(repo_path),
                "-o", str(output_file),
                "-f", "xml",
                "-t", "0",  # No token limit
                "-c", "none",  # No compression for fair comparison
            ],
            capture_output=True,
            timeout=600,
            text=True,
        )

        metrics.scan_time_ms = (time.time() - start_time) * 1000

        if result.returncode != 0:
            return metrics, f"Infiniloom failed: {result.stderr}"

        # Parse metrics from the generated XML file
        if output_file.exists():
            metrics.output_size_bytes = output_file.stat().st_size

            try:
                with open(output_file, "r") as f:
                    xml_content = f.read()

                # Parse file count from <files>N</files>
                file_match = re.search(r"<files>(\d+)</files>", xml_content)
                if file_match:
                    metrics.files_included = int(file_match.group(1))

                # Parse token count from <tokens model="claude">N</tokens>
                token_match = re.search(r'<tokens[^>]*>(\d+)</tokens>', xml_content)
                if token_match:
                    metrics.token_count = int(token_match.group(1))

                # Parse language breakdown from <language name="X" files="N" .../>
                for lang_match in re.finditer(r'<language name="(\w+)" files="(\d+)"', xml_content):
                    metrics.languages[lang_match.group(1)] = int(lang_match.group(2))

            except Exception as e:
                pass  # Continue even if XML parsing fails

        return metrics, None

    except subprocess.TimeoutExpired:
        return metrics, "Infiniloom timed out after 10 minutes"
    except Exception as e:
        return metrics, str(e)


def run_comparison_test(repo: TestRepo) -> Dict[str, TestResult]:
    """Run both tools on a repository and compare results."""
    results = {}

    print(f"\n{'='*60}")
    print(f"Testing: {repo.name}")
    print(f"Description: {repo.description}")
    print(f"Complexity: {repo.complexity}")
    print(f"{'='*60}")

    # Clone repository
    success, result = clone_repo(repo)
    if not success:
        error_result = TestResult(
            repo_name=repo.name,
            tool="clone",
            success=False,
            error=result,
        )
        results["repomix"] = error_result
        results["infiniloom"] = error_result
        return results

    repo_path = Path(result)

    # Run Repomix
    print(f"\n  Running Repomix...")
    repomix_output = OUTPUTS_DIR / f"{repo.name}_repomix.xml"
    repomix_metrics, repomix_error = run_repomix(repo_path, repomix_output)

    results["repomix"] = TestResult(
        repo_name=repo.name,
        tool="repomix",
        success=repomix_error is None,
        error=repomix_error,
        metrics=repomix_metrics,
        output_file=str(repomix_output) if repomix_output.exists() else None,
    )

    print(f"    Time: {repomix_metrics.scan_time_ms:.0f}ms")
    print(f"    Files: {repomix_metrics.files_included}")
    print(f"    Tokens: {repomix_metrics.token_count}")
    if repomix_error:
        print(f"    Error: {repomix_error}")

    # Run Infiniloom
    print(f"\n  Running Infiniloom...")
    infiniloom_output = OUTPUTS_DIR / f"{repo.name}_infiniloom.xml"
    infiniloom_metrics, infiniloom_error = run_infiniloom(repo_path, infiniloom_output)

    results["infiniloom"] = TestResult(
        repo_name=repo.name,
        tool="infiniloom",
        success=infiniloom_error is None,
        error=infiniloom_error,
        metrics=infiniloom_metrics,
        output_file=str(infiniloom_output) if infiniloom_output.exists() else None,
    )

    print(f"    Time: {infiniloom_metrics.scan_time_ms:.0f}ms")
    print(f"    Files: {infiniloom_metrics.files_included}")
    print(f"    Tokens: {infiniloom_metrics.token_count}")
    if infiniloom_error:
        print(f"    Error: {infiniloom_error}")

    # Print comparison
    if not repomix_error and not infiniloom_error:
        print(f"\n  Comparison:")
        speedup = repomix_metrics.scan_time_ms / max(infiniloom_metrics.scan_time_ms, 1)
        print(f"    Speed: Infiniloom is {speedup:.1f}x {'faster' if speedup > 1 else 'slower'}")

        if repomix_metrics.files_included and infiniloom_metrics.files_included:
            file_diff = infiniloom_metrics.files_included - repomix_metrics.files_included
            print(f"    Files: Infiniloom found {file_diff:+d} more files")

    return results


def generate_report(all_results: Dict[str, Dict[str, TestResult]]) -> str:
    """Generate a markdown report of all test results."""
    report = []
    report.append("# Infiniloom vs Repomix E2E Comparison Report")
    report.append(f"\nGenerated: {datetime.now().isoformat()}\n")

    # Summary table
    report.append("## Summary\n")
    report.append("| Repository | Complexity | Repomix Time | Infiniloom Time | Speedup | Files (R) | Files (C) |")
    report.append("|------------|------------|--------------|---------------|---------|-----------|-----------|")

    for repo in TEST_REPOSITORIES:
        if repo.name not in all_results:
            continue

        results = all_results[repo.name]
        r = results.get("repomix")
        c = results.get("infiniloom")

        r_time = f"{r.metrics.scan_time_ms:.0f}ms" if r and r.metrics else "N/A"
        c_time = f"{c.metrics.scan_time_ms:.0f}ms" if c and c.metrics else "N/A"

        speedup = "N/A"
        if r and c and r.metrics and c.metrics and c.metrics.scan_time_ms > 0:
            s = r.metrics.scan_time_ms / c.metrics.scan_time_ms
            speedup = f"{s:.1f}x"

        r_files = r.metrics.files_included if r and r.metrics else 0
        c_files = c.metrics.files_included if c and c.metrics else 0

        report.append(f"| {repo.name} | {repo.complexity} | {r_time} | {c_time} | {speedup} | {r_files} | {c_files} |")

    # Detailed results
    report.append("\n## Detailed Results\n")

    for repo in TEST_REPOSITORIES:
        if repo.name not in all_results:
            continue

        report.append(f"### {repo.name}\n")
        report.append(f"**URL:** {repo.url}\n")
        report.append(f"**Description:** {repo.description}\n")
        report.append(f"**Primary Languages:** {', '.join(repo.primary_languages)}\n")

        results = all_results[repo.name]

        for tool in ["repomix", "infiniloom"]:
            result = results.get(tool)
            if not result:
                continue

            report.append(f"\n#### {tool.capitalize()}\n")
            report.append(f"- **Success:** {result.success}")
            if result.error:
                report.append(f"- **Error:** {result.error}")
            if result.metrics:
                m = result.metrics
                report.append(f"- **Scan Time:** {m.scan_time_ms:.0f}ms")
                report.append(f"- **Files Included:** {m.files_included}")
                report.append(f"- **Token Count:** {m.token_count}")
                report.append(f"- **Output Size:** {m.output_size_bytes:,} bytes")
                if m.languages:
                    report.append(f"- **Languages:** {m.languages}")

        report.append("")

    return "\n".join(report)


def main():
    """Main test runner."""
    print("=" * 60)
    print("Infiniloom vs Repomix E2E Comparison Tests")
    print("=" * 60)

    ensure_dirs()

    # Check for Infiniloom binary
    if not INFINILOOM_BIN.exists():
        print(f"\nWARNING: Infiniloom binary not found at {INFINILOOM_BIN}")
        print("Run 'cargo build --release' first.")

    # Check for Repomix
    try:
        subprocess.run(["repomix", "--version"], capture_output=True, check=True)
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("\nWARNING: Repomix not found. Install with: npm install -g repomix")

    # Select repos to test (can filter via command line)
    repos_to_test = TEST_REPOSITORIES
    if len(sys.argv) > 1:
        names = sys.argv[1:]
        repos_to_test = [r for r in TEST_REPOSITORIES if r.name in names]
        if not repos_to_test:
            print(f"No matching repos found. Available: {[r.name for r in TEST_REPOSITORIES]}")
            return 1

    print(f"\nTesting {len(repos_to_test)} repositories...")

    all_results = {}

    for repo in repos_to_test:
        try:
            results = run_comparison_test(repo)
            all_results[repo.name] = results
        except Exception as e:
            print(f"\nERROR testing {repo.name}: {e}")
            all_results[repo.name] = {
                "repomix": TestResult(repo.name, "repomix", False, str(e)),
                "infiniloom": TestResult(repo.name, "infiniloom", False, str(e)),
            }

    # Generate report
    print("\n" + "=" * 60)
    print("Generating Report...")
    print("=" * 60)

    report = generate_report(all_results)
    report_file = REPORTS_DIR / f"comparison_{datetime.now().strftime('%Y%m%d_%H%M%S')}.md"
    with open(report_file, "w") as f:
        f.write(report)

    print(f"\nReport saved to: {report_file}")

    # Print summary
    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)

    total_repos = len(all_results)
    repomix_success = sum(1 for r in all_results.values() if r.get("repomix", TestResult("", "", False)).success)
    infiniloom_success = sum(1 for r in all_results.values() if r.get("infiniloom", TestResult("", "", False)).success)

    print(f"\nRepositories tested: {total_repos}")
    print(f"Repomix successes: {repomix_success}/{total_repos}")
    print(f"Infiniloom successes: {infiniloom_success}/{total_repos}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
