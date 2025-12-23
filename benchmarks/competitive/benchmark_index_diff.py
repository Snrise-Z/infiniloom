#!/usr/bin/env python3
"""Performance benchmarks for index and diff commands."""

import json
import os
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

# Test repositories with metadata
REPOS = {
    "cobra": {"lang": "Go", "files": 97, "size_mb": 1.3},
    "lodash": {"lang": "JavaScript", "files": 190, "size_mb": 6.0},
    "zod": {"lang": "TypeScript", "files": 575, "size_mb": 22},
    "bat": {"lang": "Rust", "files": 915, "size_mb": 13},
    "fastapi": {"lang": "Python", "files": 2590, "size_mb": 49},
}


@dataclass
class BenchmarkResult:
    """Result of a benchmark run."""
    command: str
    repo: str
    params: str
    runs: int
    mean_ms: float
    min_ms: float
    max_ms: float
    std_ms: float
    output_size: int


def get_infiniloom_bin() -> str:
    """Get path to infiniloom binary."""
    release_bin = Path(__file__).parent.parent.parent / "target" / "release" / "infiniloom"
    if release_bin.exists():
        return str(release_bin)
    return "infiniloom"


def run_benchmark(cmd: list[str], runs: int = 5) -> tuple[list[float], int]:
    """Run a command multiple times and return timing results."""
    times = []
    output_size = 0

    for i in range(runs):
        start = time.perf_counter()
        result = subprocess.run(cmd, capture_output=True, text=True)
        elapsed = (time.perf_counter() - start) * 1000  # ms
        times.append(elapsed)
        if i == 0:
            output_size = len(result.stdout) if result.returncode == 0 else 0

    return times, output_size


def benchmark_index(repo_path: Path, repo_name: str, runs: int = 5) -> list[BenchmarkResult]:
    """Benchmark index command with various options."""
    bin_path = get_infiniloom_bin()
    results = []

    # Clear existing index first
    index_dir = repo_path / ".infiniloom"
    if index_dir.exists():
        shutil.rmtree(index_dir)

    # Test 1: Fresh index (cold start)
    cmd = [bin_path, "index", str(repo_path)]
    times, output_size = run_benchmark(cmd, runs=1)  # Only 1 run for cold start
    results.append(BenchmarkResult(
        command="index",
        repo=repo_name,
        params="(cold start)",
        runs=1,
        mean_ms=times[0],
        min_ms=times[0],
        max_ms=times[0],
        std_ms=0,
        output_size=output_size,
    ))

    # Test 2: Incremental index (warm, no changes)
    times, output_size = run_benchmark(cmd, runs=runs)
    import statistics
    results.append(BenchmarkResult(
        command="index",
        repo=repo_name,
        params="(incremental)",
        runs=runs,
        mean_ms=statistics.mean(times),
        min_ms=min(times),
        max_ms=max(times),
        std_ms=statistics.stdev(times) if len(times) > 1 else 0,
        output_size=output_size,
    ))

    # Test 3: Force rebuild
    cmd = [bin_path, "index", str(repo_path), "--force"]
    times, output_size = run_benchmark(cmd, runs=runs)
    results.append(BenchmarkResult(
        command="index",
        repo=repo_name,
        params="--force",
        runs=runs,
        mean_ms=statistics.mean(times),
        min_ms=min(times),
        max_ms=max(times),
        std_ms=statistics.stdev(times) if len(times) > 1 else 0,
        output_size=output_size,
    ))

    # Test 4: Status check (fastest)
    cmd = [bin_path, "index", str(repo_path), "--status"]
    times, output_size = run_benchmark(cmd, runs=runs)
    results.append(BenchmarkResult(
        command="index",
        repo=repo_name,
        params="--status",
        runs=runs,
        mean_ms=statistics.mean(times),
        min_ms=min(times),
        max_ms=max(times),
        std_ms=statistics.stdev(times) if len(times) > 1 else 0,
        output_size=output_size,
    ))

    return results


def benchmark_diff(repo_path: Path, repo_name: str, runs: int = 5) -> list[BenchmarkResult]:
    """Benchmark diff command with various options."""
    bin_path = get_infiniloom_bin()
    results = []
    import statistics

    # Ensure index exists first
    subprocess.run([bin_path, "index", str(repo_path)], capture_output=True)

    # Change to repo directory for diff command
    old_cwd = os.getcwd()
    os.chdir(repo_path)

    try:
        # Test 1: Basic diff (unstaged)
        cmd = [bin_path, "diff"]
        times, output_size = run_benchmark(cmd, runs=runs)
        results.append(BenchmarkResult(
            command="diff",
            repo=repo_name,
            params="(basic)",
            runs=runs,
            mean_ms=statistics.mean(times),
            min_ms=min(times),
            max_ms=max(times),
            std_ms=statistics.stdev(times) if len(times) > 1 else 0,
            output_size=output_size,
        ))

        # Test 2: Diff with --staged
        cmd = [bin_path, "diff", "--staged"]
        times, output_size = run_benchmark(cmd, runs=runs)
        results.append(BenchmarkResult(
            command="diff",
            repo=repo_name,
            params="--staged",
            runs=runs,
            mean_ms=statistics.mean(times),
            min_ms=min(times),
            max_ms=max(times),
            std_ms=statistics.stdev(times) if len(times) > 1 else 0,
            output_size=output_size,
        ))

        # Test 3: Diff with different depths
        for depth in [1, 2, 3]:
            cmd = [bin_path, "diff", "--depth", str(depth)]
            times, output_size = run_benchmark(cmd, runs=runs)
            results.append(BenchmarkResult(
                command="diff",
                repo=repo_name,
                params=f"--depth {depth}",
                runs=runs,
                mean_ms=statistics.mean(times),
                min_ms=min(times),
                max_ms=max(times),
                std_ms=statistics.stdev(times) if len(times) > 1 else 0,
                output_size=output_size,
            ))

        # Test 4: Diff with different formats
        for fmt in ["xml", "json", "markdown"]:
            cmd = [bin_path, "diff", "--format", fmt]
            times, output_size = run_benchmark(cmd, runs=runs)
            results.append(BenchmarkResult(
                command="diff",
                repo=repo_name,
                params=f"--format {fmt}",
                runs=runs,
                mean_ms=statistics.mean(times),
                min_ms=min(times),
                max_ms=max(times),
                std_ms=statistics.stdev(times) if len(times) > 1 else 0,
                output_size=output_size,
            ))

        # Test 5: Diff with --include-diff
        cmd = [bin_path, "diff", "--include-diff"]
        times, output_size = run_benchmark(cmd, runs=runs)
        results.append(BenchmarkResult(
            command="diff",
            repo=repo_name,
            params="--include-diff",
            runs=runs,
            mean_ms=statistics.mean(times),
            min_ms=min(times),
            max_ms=max(times),
            std_ms=statistics.stdev(times) if len(times) > 1 else 0,
            output_size=output_size,
        ))

    finally:
        os.chdir(old_cwd)

    return results


def print_results_table(results: list[BenchmarkResult], title: str):
    """Print results as a formatted table."""
    print(f"\n{'=' * 80}")
    print(f"{title}")
    print(f"{'=' * 80}")
    print(f"{'Repo':<12} {'Params':<20} {'Mean (ms)':<12} {'Min':<10} {'Max':<10} {'Std':<10}")
    print("-" * 80)

    for r in results:
        print(f"{r.repo:<12} {r.params:<20} {r.mean_ms:>8.1f}    {r.min_ms:>6.1f}    {r.max_ms:>6.1f}    {r.std_ms:>6.1f}")


def main():
    repos_dir = Path(__file__).parent / "repos"
    all_index_results = []
    all_diff_results = []

    print("=" * 80)
    print("INFINILOOM INDEX & DIFF PERFORMANCE BENCHMARKS")
    print("=" * 80)
    print()

    # Print repo info
    print("Test Repositories (ordered by size):")
    print("-" * 60)
    print(f"{'Repo':<12} {'Language':<12} {'Files':<10} {'Size':<10}")
    print("-" * 60)
    for repo_name, info in REPOS.items():
        print(f"{repo_name:<12} {info['lang']:<12} {info['files']:<10} {info['size_mb']:.1f} MB")
    print()

    # Run benchmarks for each repo (sorted by size)
    sorted_repos = sorted(REPOS.items(), key=lambda x: x[1]['files'])

    for repo_name, info in sorted_repos:
        repo_path = repos_dir / repo_name
        if not repo_path.exists():
            print(f"SKIP: {repo_name} (not found)")
            continue

        print(f"\n--- Benchmarking {repo_name} ({info['files']} files, {info['size_mb']} MB) ---")

        # Index benchmarks
        print("  Running index benchmarks...")
        index_results = benchmark_index(repo_path, repo_name, runs=5)
        all_index_results.extend(index_results)
        for r in index_results:
            print(f"    {r.params:<20} {r.mean_ms:>8.1f} ms")

        # Diff benchmarks
        print("  Running diff benchmarks...")
        diff_results = benchmark_diff(repo_path, repo_name, runs=5)
        all_diff_results.extend(diff_results)
        for r in diff_results:
            print(f"    {r.params:<20} {r.mean_ms:>8.1f} ms")

    # Print summary tables
    print_results_table(all_index_results, "INDEX COMMAND BENCHMARKS")
    print_results_table(all_diff_results, "DIFF COMMAND BENCHMARKS")

    # Print scaling analysis
    print(f"\n{'=' * 80}")
    print("SCALING ANALYSIS")
    print(f"{'=' * 80}")

    # Index cold start scaling
    print("\nIndex (cold start) vs Repository Size:")
    print("-" * 50)
    cold_starts = [r for r in all_index_results if "cold" in r.params]
    for r in cold_starts:
        files = REPOS[r.repo]['files']
        ms_per_file = r.mean_ms / files
        print(f"  {r.repo:<12} {files:>5} files -> {r.mean_ms:>8.1f} ms ({ms_per_file:.2f} ms/file)")

    # Index force rebuild scaling
    print("\nIndex --force vs Repository Size:")
    print("-" * 50)
    force_rebuilds = [r for r in all_index_results if "--force" in r.params]
    for r in force_rebuilds:
        files = REPOS[r.repo]['files']
        ms_per_file = r.mean_ms / files
        print(f"  {r.repo:<12} {files:>5} files -> {r.mean_ms:>8.1f} ms ({ms_per_file:.2f} ms/file)")

    # Diff basic scaling
    print("\nDiff (basic) vs Repository Size:")
    print("-" * 50)
    basic_diffs = [r for r in all_diff_results if "basic" in r.params]
    for r in basic_diffs:
        files = REPOS[r.repo]['files']
        print(f"  {r.repo:<12} {files:>5} files -> {r.mean_ms:>8.1f} ms")

    # Save results to JSON
    output_dir = Path(__file__).parent / "quality_results"
    output_dir.mkdir(exist_ok=True)

    results_data = {
        "index_benchmarks": [
            {
                "repo": r.repo,
                "params": r.params,
                "mean_ms": r.mean_ms,
                "min_ms": r.min_ms,
                "max_ms": r.max_ms,
                "std_ms": r.std_ms,
                "runs": r.runs,
            }
            for r in all_index_results
        ],
        "diff_benchmarks": [
            {
                "repo": r.repo,
                "params": r.params,
                "mean_ms": r.mean_ms,
                "min_ms": r.min_ms,
                "max_ms": r.max_ms,
                "std_ms": r.std_ms,
                "runs": r.runs,
            }
            for r in all_diff_results
        ],
        "repos": REPOS,
    }

    with open(output_dir / "benchmark_index_diff.json", "w") as f:
        json.dump(results_data, f, indent=2)

    print(f"\nResults saved to: {output_dir / 'benchmark_index_diff.json'}")


if __name__ == "__main__":
    main()
