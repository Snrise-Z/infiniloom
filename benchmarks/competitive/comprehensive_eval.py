#!/usr/bin/env python3
"""Comprehensive evaluation of Infiniloom across all commands, languages, and parameters."""

import json
import os
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

# Test repositories by language
REPOS_BY_LANGUAGE = {
    "rust": "bat",
    "go": "cobra",
    "javascript": "lodash",
    "python": "fastapi",
    "typescript": "zod",
}

# CLI commands to test
COMMANDS = ["pack", "scan", "map", "index", "diff", "impact", "info"]

# Output formats (all 6 supported formats)
FORMATS = ["xml", "markdown", "json", "yaml", "toon", "plain"]

# Compression levels
COMPRESSIONS = ["none", "minimal", "balanced", "aggressive"]

# Target models
MODELS = ["claude", "gpt4o", "gpt4", "gemini", "llama"]


@dataclass
class TestResult:
    """Result of a single test."""
    command: str
    repo: str
    language: str
    params: dict
    success: bool
    output_size: int
    execution_time: float
    error: Optional[str] = None
    output_preview: str = ""


@dataclass
class EvaluationReport:
    """Full evaluation report."""
    results: list = field(default_factory=list)
    total_tests: int = 0
    passed_tests: int = 0
    failed_tests: int = 0


def get_infiniloom_bin() -> str:
    """Get path to infiniloom binary."""
    release_bin = Path(__file__).parent.parent.parent / "target" / "release" / "infiniloom"
    debug_bin = Path(__file__).parent.parent.parent / "target" / "debug" / "infiniloom"
    if release_bin.exists():
        return str(release_bin)
    if debug_bin.exists():
        return str(debug_bin)
    return "infiniloom"


def run_command(cmd: list[str], timeout: int = 120) -> tuple[bool, str, float]:
    """Run a command and return success, output, and execution time."""
    start = time.time()
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            timeout=timeout,
            text=True,
        )
        elapsed = time.time() - start
        if result.returncode == 0:
            return True, result.stdout, elapsed
        else:
            return False, result.stderr, elapsed
    except subprocess.TimeoutExpired:
        return False, "Command timed out", time.time() - start
    except Exception as e:
        return False, str(e), time.time() - start


def test_pack_command(repo_path: Path, params: dict) -> TestResult:
    """Test the pack command with various parameters."""
    bin_path = get_infiniloom_bin()
    cmd = [bin_path, "pack", str(repo_path)]

    if params.get("format"):
        cmd.extend(["--format", params["format"]])
    if params.get("model"):
        cmd.extend(["--model", params["model"]])
    if params.get("compression"):
        cmd.extend(["--compression", params["compression"]])
    if params.get("full"):
        cmd.append("--full")
    if params.get("no_content"):
        cmd.append("--no-content")
    if params.get("include_tests"):
        cmd.append("--include-tests")
    if params.get("max_tokens"):
        cmd.extend(["--max-tokens", str(params["max_tokens"])])

    success, output, elapsed = run_command(cmd)

    return TestResult(
        command="pack",
        repo=repo_path.name,
        language=params.get("language", "unknown"),
        params=params,
        success=success,
        output_size=len(output) if success else 0,
        execution_time=elapsed,
        error=output if not success else None,
        output_preview=output[:500] if success else "",
    )


def test_scan_command(repo_path: Path, params: dict) -> TestResult:
    """Test the scan command."""
    bin_path = get_infiniloom_bin()
    cmd = [bin_path, "scan", str(repo_path)]

    if params.get("model"):
        cmd.extend(["--model", params["model"]])
    if params.get("verbose"):
        cmd.append("--verbose")
    if params.get("json"):
        cmd.append("--json")

    success, output, elapsed = run_command(cmd)

    return TestResult(
        command="scan",
        repo=repo_path.name,
        language=params.get("language", "unknown"),
        params=params,
        success=success,
        output_size=len(output) if success else 0,
        execution_time=elapsed,
        error=output if not success else None,
        output_preview=output[:500] if success else "",
    )


def test_map_command(repo_path: Path, params: dict) -> TestResult:
    """Test the map command."""
    bin_path = get_infiniloom_bin()
    cmd = [bin_path, "map", str(repo_path)]

    if params.get("budget"):
        cmd.extend(["--budget", str(params["budget"])])

    success, output, elapsed = run_command(cmd)

    return TestResult(
        command="map",
        repo=repo_path.name,
        language=params.get("language", "unknown"),
        params=params,
        success=success,
        output_size=len(output) if success else 0,
        execution_time=elapsed,
        error=output if not success else None,
        output_preview=output[:500] if success else "",
    )


def test_index_command(repo_path: Path, params: dict) -> TestResult:
    """Test the index command."""
    bin_path = get_infiniloom_bin()
    cmd = [bin_path, "index", str(repo_path)]

    if params.get("force"):
        cmd.append("--force")
    if params.get("status"):
        cmd.append("--status")
    if params.get("verbose"):
        cmd.append("--verbose")

    success, output, elapsed = run_command(cmd)

    return TestResult(
        command="index",
        repo=repo_path.name,
        language=params.get("language", "unknown"),
        params=params,
        success=success,
        output_size=len(output) if success else 0,
        execution_time=elapsed,
        error=output if not success else None,
        output_preview=output[:500] if success else "",
    )


def test_diff_command(repo_path: Path, params: dict) -> TestResult:
    """Test the diff command."""
    bin_path = get_infiniloom_bin()
    cmd = [bin_path, "diff"]

    if params.get("staged"):
        cmd.append("--staged")
    if params.get("depth"):
        cmd.extend(["--depth", str(params["depth"])])
    if params.get("budget"):
        cmd.extend(["--budget", str(params["budget"])])
    if params.get("include_diff"):
        cmd.append("--include-diff")
    if params.get("format"):
        cmd.extend(["--format", params["format"]])

    # Run from repo directory
    success, output, elapsed = run_command(cmd, timeout=60)

    return TestResult(
        command="diff",
        repo=repo_path.name,
        language=params.get("language", "unknown"),
        params=params,
        success=success,
        output_size=len(output) if success else 0,
        execution_time=elapsed,
        error=output if not success else None,
        output_preview=output[:500] if success else "",
    )


def test_impact_command(repo_path: Path, params: dict) -> TestResult:
    """Test the impact command."""
    bin_path = get_infiniloom_bin()
    lang = params.get("language", "unknown")

    # Map language to file extension
    lang_ext_map = {
        "rust": ".rs",
        "go": ".go",
        "javascript": ".js",
        "python": ".py",
        "typescript": ".ts",
    }
    ext = lang_ext_map.get(lang, ".rs")

    # Find first source file in repo
    target_file = None
    files = list(repo_path.rglob(f"*{ext}"))
    if files:
        # Get relative path from repo root
        target_file = str(files[0].relative_to(repo_path))

    if not target_file:
        return TestResult(
            command="impact",
            repo=repo_path.name,
            language=lang,
            params=params,
            success=False,
            output_size=0,
            execution_time=0,
            error=f"No {ext} files found for impact test",
        )

    # Run impact command: infiniloom impact [PATH] <TARGET>
    cmd = [bin_path, "impact", str(repo_path), target_file]
    if params.get("symbol"):
        cmd.append("--symbol")
    if params.get("json"):
        cmd.append("--json")

    success, output, elapsed = run_command(cmd, timeout=60)

    return TestResult(
        command="impact",
        repo=repo_path.name,
        language=lang,
        params=params,
        success=success,
        output_size=len(output) if success else 0,
        execution_time=elapsed,
        error=output if not success else None,
        output_preview=output[:500] if success else "",
    )


def test_info_command() -> TestResult:
    """Test the info command."""
    bin_path = get_infiniloom_bin()
    cmd = [bin_path, "info"]

    success, output, elapsed = run_command(cmd)

    return TestResult(
        command="info",
        repo="N/A",
        language="N/A",
        params={},
        success=success,
        output_size=len(output) if success else 0,
        execution_time=elapsed,
        error=output if not success else None,
        output_preview=output[:500] if success else "",
    )


def run_comprehensive_evaluation() -> EvaluationReport:
    """Run comprehensive evaluation across all combinations."""
    report = EvaluationReport()
    repos_dir = Path(__file__).parent / "repos"

    print("=" * 80)
    print("COMPREHENSIVE INFINILOOM EVALUATION")
    print("=" * 80)
    print()

    # Test 1: Pack command with all format combinations
    print("## Testing PACK command with all formats")
    print("-" * 40)
    for lang, repo_name in REPOS_BY_LANGUAGE.items():
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            print(f"  SKIP: {repo_name} (not found)")
            continue

        for fmt in FORMATS:
            params = {"format": fmt, "language": lang, "full": True}
            result = test_pack_command(repo_path, params)
            report.results.append(result)
            report.total_tests += 1
            if result.success:
                report.passed_tests += 1
                print(f"  PASS: {repo_name} --format {fmt} ({result.output_size:,} chars, {result.execution_time:.2f}s)")
            else:
                report.failed_tests += 1
                print(f"  FAIL: {repo_name} --format {fmt} - {result.error[:100]}")
    print()

    # Test 2: Pack command with all compression levels
    print("## Testing PACK command with all compression levels")
    print("-" * 40)
    for lang, repo_name in REPOS_BY_LANGUAGE.items():
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            continue

        for comp in COMPRESSIONS:
            params = {"format": "xml", "compression": comp, "language": lang, "full": True}
            result = test_pack_command(repo_path, params)
            report.results.append(result)
            report.total_tests += 1
            if result.success:
                report.passed_tests += 1
                print(f"  PASS: {repo_name} --compression {comp} ({result.output_size:,} chars)")
            else:
                report.failed_tests += 1
                print(f"  FAIL: {repo_name} --compression {comp}")
    print()

    # Test 3: Pack command with all target models
    print("## Testing PACK command with all target models")
    print("-" * 40)
    for lang, repo_name in REPOS_BY_LANGUAGE.items():
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            continue

        for model in MODELS:
            params = {"format": "xml", "model": model, "language": lang}
            result = test_pack_command(repo_path, params)
            report.results.append(result)
            report.total_tests += 1
            if result.success:
                report.passed_tests += 1
                print(f"  PASS: {repo_name} --model {model} ({result.output_size:,} chars)")
            else:
                report.failed_tests += 1
                print(f"  FAIL: {repo_name} --model {model}")
    print()

    # Test 4: Scan command
    print("## Testing SCAN command")
    print("-" * 40)
    for lang, repo_name in REPOS_BY_LANGUAGE.items():
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            continue

        # Basic scan
        params = {"language": lang}
        result = test_scan_command(repo_path, params)
        report.results.append(result)
        report.total_tests += 1
        if result.success:
            report.passed_tests += 1
            print(f"  PASS: {repo_name} scan ({result.execution_time:.2f}s)")
        else:
            report.failed_tests += 1
            print(f"  FAIL: {repo_name} scan")

        # JSON output scan
        params = {"language": lang, "json": True}
        result = test_scan_command(repo_path, params)
        report.results.append(result)
        report.total_tests += 1
        if result.success:
            report.passed_tests += 1
            print(f"  PASS: {repo_name} scan --json ({result.execution_time:.2f}s)")
        else:
            report.failed_tests += 1
            print(f"  FAIL: {repo_name} scan --json")
    print()

    # Test 5: Map command
    print("## Testing MAP command")
    print("-" * 40)
    for lang, repo_name in REPOS_BY_LANGUAGE.items():
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            continue

        for budget in [1000, 5000, 10000]:
            params = {"budget": budget, "language": lang}
            result = test_map_command(repo_path, params)
            report.results.append(result)
            report.total_tests += 1
            if result.success:
                report.passed_tests += 1
                print(f"  PASS: {repo_name} map --budget {budget} ({result.output_size:,} chars)")
            else:
                report.failed_tests += 1
                print(f"  FAIL: {repo_name} map --budget {budget}")
    print()

    # Test 6: Special parameter combinations
    print("## Testing special parameter combinations")
    print("-" * 40)
    for lang, repo_name in REPOS_BY_LANGUAGE.items():
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            continue

        special_params = [
            {"format": "xml", "full": True, "include_tests": True, "language": lang},
            {"format": "xml", "no_content": True, "language": lang},
            {"format": "xml", "max_tokens": 50000, "language": lang},
            {"format": "markdown", "compression": "aggressive", "language": lang},
        ]

        for params in special_params:
            result = test_pack_command(repo_path, params)
            report.results.append(result)
            report.total_tests += 1
            param_str = " ".join(f"--{k.replace('_', '-')}" + (f" {v}" if v is not True else "")
                                 for k, v in params.items() if k != "language" and v)
            if result.success:
                report.passed_tests += 1
                print(f"  PASS: {repo_name} {param_str[:50]}")
            else:
                report.failed_tests += 1
                print(f"  FAIL: {repo_name} {param_str[:50]}")
    print()

    # Test 7: Index command
    print("## Testing INDEX command")
    print("-" * 40)
    for lang, repo_name in REPOS_BY_LANGUAGE.items():
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            continue

        # Basic index
        params = {"language": lang}
        result = test_index_command(repo_path, params)
        report.results.append(result)
        report.total_tests += 1
        if result.success:
            report.passed_tests += 1
            print(f"  PASS: {repo_name} index ({result.execution_time:.2f}s)")
        else:
            report.failed_tests += 1
            print(f"  FAIL: {repo_name} index - {result.error[:80] if result.error else 'Unknown'}")

        # Index with --status
        params = {"language": lang, "status": True}
        result = test_index_command(repo_path, params)
        report.results.append(result)
        report.total_tests += 1
        if result.success:
            report.passed_tests += 1
            print(f"  PASS: {repo_name} index --status ({result.execution_time:.2f}s)")
        else:
            report.failed_tests += 1
            print(f"  FAIL: {repo_name} index --status")

        # Index with --force
        params = {"language": lang, "force": True}
        result = test_index_command(repo_path, params)
        report.results.append(result)
        report.total_tests += 1
        if result.success:
            report.passed_tests += 1
            print(f"  PASS: {repo_name} index --force ({result.execution_time:.2f}s)")
        else:
            report.failed_tests += 1
            print(f"  FAIL: {repo_name} index --force")

        # Index with --verbose
        params = {"language": lang, "verbose": True}
        result = test_index_command(repo_path, params)
        report.results.append(result)
        report.total_tests += 1
        if result.success:
            report.passed_tests += 1
            print(f"  PASS: {repo_name} index --verbose ({result.execution_time:.2f}s)")
        else:
            report.failed_tests += 1
            print(f"  FAIL: {repo_name} index --verbose")
    print()

    # Test 8: Diff command (run from each repo directory)
    print("## Testing DIFF command")
    print("-" * 40)
    for lang, repo_name in REPOS_BY_LANGUAGE.items():
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            continue

        # Save current directory
        old_cwd = os.getcwd()
        os.chdir(repo_path)
        try:
            # Basic diff (unstaged)
            params = {"language": lang}
            result = test_diff_command(repo_path, params)
            report.results.append(result)
            report.total_tests += 1
            if result.success:
                report.passed_tests += 1
                print(f"  PASS: {repo_name} diff ({result.execution_time:.2f}s)")
            else:
                report.failed_tests += 1
                print(f"  FAIL: {repo_name} diff - {result.error[:80] if result.error else 'Unknown'}")

            # Diff with --staged
            params = {"language": lang, "staged": True}
            result = test_diff_command(repo_path, params)
            report.results.append(result)
            report.total_tests += 1
            if result.success:
                report.passed_tests += 1
                print(f"  PASS: {repo_name} diff --staged ({result.execution_time:.2f}s)")
            else:
                report.failed_tests += 1
                print(f"  FAIL: {repo_name} diff --staged")

            # Diff with all format options
            for fmt in FORMATS:
                params = {"language": lang, "format": fmt}
                result = test_diff_command(repo_path, params)
                report.results.append(result)
                report.total_tests += 1
                if result.success:
                    report.passed_tests += 1
                    print(f"  PASS: {repo_name} diff --format {fmt} ({result.execution_time:.2f}s)")
                else:
                    report.failed_tests += 1
                    print(f"  FAIL: {repo_name} diff --format {fmt}")

            # Diff with depth options (1, 2, 3)
            for depth in [1, 2, 3]:
                params = {"language": lang, "depth": depth}
                result = test_diff_command(repo_path, params)
                report.results.append(result)
                report.total_tests += 1
                if result.success:
                    report.passed_tests += 1
                    print(f"  PASS: {repo_name} diff --depth {depth} ({result.execution_time:.2f}s)")
                else:
                    report.failed_tests += 1
                    print(f"  FAIL: {repo_name} diff --depth {depth}")

            # Diff with budget options
            for budget in [10000, 50000, 100000]:
                params = {"language": lang, "budget": budget}
                result = test_diff_command(repo_path, params)
                report.results.append(result)
                report.total_tests += 1
                if result.success:
                    report.passed_tests += 1
                    print(f"  PASS: {repo_name} diff --budget {budget} ({result.execution_time:.2f}s)")
                else:
                    report.failed_tests += 1
                    print(f"  FAIL: {repo_name} diff --budget {budget}")

            # Diff with --include-diff
            params = {"language": lang, "include_diff": True}
            result = test_diff_command(repo_path, params)
            report.results.append(result)
            report.total_tests += 1
            if result.success:
                report.passed_tests += 1
                print(f"  PASS: {repo_name} diff --include-diff ({result.execution_time:.2f}s)")
            else:
                report.failed_tests += 1
                print(f"  FAIL: {repo_name} diff --include-diff")
        finally:
            os.chdir(old_cwd)
    print()

    # Test 9: Impact command
    print("## Testing IMPACT command")
    print("-" * 40)
    for lang, repo_name in REPOS_BY_LANGUAGE.items():
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            continue

        # Basic impact on first file
        params = {"language": lang}
        result = test_impact_command(repo_path, params)
        report.results.append(result)
        report.total_tests += 1
        if result.success:
            report.passed_tests += 1
            print(f"  PASS: {repo_name} impact ({result.execution_time:.2f}s)")
        else:
            report.failed_tests += 1
            print(f"  FAIL: {repo_name} impact - {result.error[:80] if result.error else 'Unknown'}")

        # Impact with --json output
        params = {"language": lang, "json": True}
        result = test_impact_command(repo_path, params)
        report.results.append(result)
        report.total_tests += 1
        if result.success:
            report.passed_tests += 1
            print(f"  PASS: {repo_name} impact --json ({result.execution_time:.2f}s)")
        else:
            report.failed_tests += 1
            print(f"  FAIL: {repo_name} impact --json")
    print()

    # Test 10: Info command (only once, not per-repo)
    print("## Testing INFO command")
    print("-" * 40)
    result = test_info_command()
    report.results.append(result)
    report.total_tests += 1
    if result.success:
        report.passed_tests += 1
        print(f"  PASS: info ({result.execution_time:.2f}s)")
    else:
        report.failed_tests += 1
        print(f"  FAIL: info - {result.error[:80] if result.error else 'Unknown'}")
    print()

    return report


def print_summary(report: EvaluationReport):
    """Print evaluation summary."""
    print("=" * 80)
    print("EVALUATION SUMMARY")
    print("=" * 80)
    print(f"Total tests:  {report.total_tests}")
    print(f"Passed:       {report.passed_tests} ({100*report.passed_tests/report.total_tests:.1f}%)")
    print(f"Failed:       {report.failed_tests} ({100*report.failed_tests/report.total_tests:.1f}%)")
    print()

    # Group results by command
    by_command = {}
    for r in report.results:
        if r.command not in by_command:
            by_command[r.command] = {"passed": 0, "failed": 0}
        if r.success:
            by_command[r.command]["passed"] += 1
        else:
            by_command[r.command]["failed"] += 1

    print("Results by command:")
    for cmd, stats in by_command.items():
        total = stats["passed"] + stats["failed"]
        print(f"  {cmd}: {stats['passed']}/{total} passed ({100*stats['passed']/total:.1f}%)")
    print()

    # Group results by language
    by_language = {}
    for r in report.results:
        if r.language not in by_language:
            by_language[r.language] = {"passed": 0, "failed": 0}
        if r.success:
            by_language[r.language]["passed"] += 1
        else:
            by_language[r.language]["failed"] += 1

    print("Results by language:")
    for lang, stats in by_language.items():
        total = stats["passed"] + stats["failed"]
        print(f"  {lang}: {stats['passed']}/{total} passed ({100*stats['passed']/total:.1f}%)")
    print()

    # Show any failures
    failures = [r for r in report.results if not r.success]
    if failures:
        print("FAILURES:")
        for f in failures[:10]:  # Show first 10 failures
            print(f"  - {f.command} {f.repo}: {f.error[:80] if f.error else 'Unknown error'}")
        if len(failures) > 10:
            print(f"  ... and {len(failures) - 10} more failures")


def save_report(report: EvaluationReport, output_path: Path):
    """Save report to JSON."""
    data = {
        "total_tests": report.total_tests,
        "passed_tests": report.passed_tests,
        "failed_tests": report.failed_tests,
        "pass_rate": report.passed_tests / report.total_tests if report.total_tests > 0 else 0,
        "results": [
            {
                "command": r.command,
                "repo": r.repo,
                "language": r.language,
                "params": r.params,
                "success": r.success,
                "output_size": r.output_size,
                "execution_time": r.execution_time,
                "error": r.error,
            }
            for r in report.results
        ],
    }

    with open(output_path, "w") as f:
        json.dump(data, f, indent=2)
    print(f"Report saved to: {output_path}")


if __name__ == "__main__":
    report = run_comprehensive_evaluation()
    print_summary(report)

    output_dir = Path(__file__).parent / "quality_results"
    output_dir.mkdir(exist_ok=True)
    save_report(report, output_dir / "comprehensive_results.json")
