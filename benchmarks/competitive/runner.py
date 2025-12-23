#!/usr/bin/env python3
"""
Main competitive benchmark runner.

Compares Infiniloom, Repomix, and Gitingest across multiple dimensions:
- Performance (speed, memory)
- Output quality (size, token efficiency)
- Feature completeness
"""

import argparse
import json
import sys
import tempfile
from datetime import datetime
from pathlib import Path
from typing import Optional

from install_tools import TOOLS, check_tool_installed, get_pack_command
from measure import (
    MeasurementResult,
    calculate_statistics,
    count_tokens_tiktoken,
    measure_command,
    measure_with_warmup,
    save_results,
)
from repos import TEST_REPOS, clone_repo, get_repo


class BenchmarkRunner:
    """Main benchmark orchestrator."""

    def __init__(
        self,
        tools: list[str],
        repos: list[str],
        output_dir: Path,
        warmup_runs: int = 1,
        measurement_runs: int = 3,
    ):
        self.tools = tools
        self.repos = repos
        self.output_dir = output_dir
        self.warmup_runs = warmup_runs
        self.measurement_runs = measurement_runs
        self.results: list[MeasurementResult] = []

        output_dir.mkdir(parents=True, exist_ok=True)

    def verify_tools(self) -> bool:
        """Verify all tools are installed."""
        print("Verifying tools...")
        all_ok = True
        for tool_name in self.tools:
            if tool_name not in TOOLS:
                print(f"  Unknown tool: {tool_name}")
                all_ok = False
                continue

            installed, version = check_tool_installed(TOOLS[tool_name])
            if installed:
                print(f"  {tool_name}: OK ({version})")
            else:
                print(f"  {tool_name}: NOT INSTALLED")
                all_ok = False

        return all_ok

    def prepare_repos(self) -> dict[str, Path]:
        """Clone/prepare all test repositories."""
        print("\nPreparing repositories...")
        repo_paths = {}

        for repo_name in self.repos:
            repo = get_repo(repo_name)
            if not repo:
                print(f"  Unknown repo: {repo_name}")
                continue

            if repo.local_path.exists():
                print(f"  {repo_name}: Already cloned")
                repo_paths[repo_name] = repo.local_path
            else:
                try:
                    path = clone_repo(repo, shallow=True)
                    repo_paths[repo_name] = path
                    print(f"  {repo_name}: Cloned to {path}")
                except Exception as e:
                    print(f"  {repo_name}: Clone failed - {e}")

        return repo_paths

    def run_benchmark(self, tool_name: str, repo_path: Path, repo_name: str) -> list[MeasurementResult]:
        """Run benchmark for a single tool on a single repo."""
        print(f"\n  Benchmarking {tool_name} on {repo_name}...")

        cmd = get_pack_command(tool_name, repo_path)
        results = measure_with_warmup(
            cmd,
            tool_name=tool_name,
            repo_name=repo_name,
            warmup_runs=self.warmup_runs,
            measurement_runs=self.measurement_runs,
        )

        # Add token counts
        for result in results:
            if result.output_size_bytes > 0:
                # Read output and count tokens
                try:
                    output = subprocess.run(cmd, capture_output=True, timeout=300)
                    if output.returncode == 0:
                        text = output.stdout.decode("utf-8", errors="replace")
                        result.token_count = count_tokens_tiktoken(text)
                except Exception:
                    pass

        return results

    def run_all_benchmarks(self, repo_paths: dict[str, Path]) -> list[MeasurementResult]:
        """Run all benchmarks."""
        all_results = []

        for repo_name, repo_path in repo_paths.items():
            print(f"\n{'='*60}")
            print(f"Repository: {repo_name}")
            print(f"{'='*60}")

            for tool_name in self.tools:
                try:
                    results = self.run_benchmark(tool_name, repo_path, repo_name)
                    all_results.extend(results)

                    # Print summary
                    stats = calculate_statistics(results)
                    if stats:
                        print(f"    Time: {stats['execution_time']['mean']:.2f}s")
                        print(f"    Memory: {stats['peak_memory_mb']['mean']:.1f}MB")
                        print(f"    Output: {stats['output_size_bytes']['mean']:,.0f} bytes")

                except Exception as e:
                    print(f"    Error: {e}")
                    all_results.append(
                        MeasurementResult(
                            tool=tool_name,
                            repo=repo_name,
                            execution_time_seconds=0,
                            peak_memory_mb=0,
                            output_size_bytes=0,
                            exit_code=-1,
                            error=str(e),
                        )
                    )

        return all_results

    def save_results(self, results: list[MeasurementResult]):
        """Save all results to files."""
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

        # Save raw results
        raw_path = self.output_dir / f"results_{timestamp}.json"
        save_results(results, raw_path)
        print(f"\nRaw results saved to: {raw_path}")

        # Save summary
        summary = self.generate_summary(results)
        summary_path = self.output_dir / f"summary_{timestamp}.json"
        with open(summary_path, "w") as f:
            json.dump(summary, f, indent=2)
        print(f"Summary saved to: {summary_path}")

    def generate_summary(self, results: list[MeasurementResult]) -> dict:
        """Generate summary statistics from all results."""
        summary = {
            "timestamp": datetime.now().isoformat(),
            "tools": self.tools,
            "repos": self.repos,
            "by_tool": {},
            "by_repo": {},
            "rankings": {},
        }

        # Group by tool
        for tool in self.tools:
            tool_results = [r for r in results if r.tool == tool]
            if tool_results:
                summary["by_tool"][tool] = calculate_statistics(tool_results)

        # Group by repo
        for repo in self.repos:
            repo_results = [r for r in results if r.repo == repo]
            if repo_results:
                summary["by_repo"][repo] = {}
                for tool in self.tools:
                    tool_repo_results = [r for r in repo_results if r.tool == tool]
                    if tool_repo_results:
                        summary["by_repo"][repo][tool] = calculate_statistics(tool_repo_results)

        # Calculate rankings
        summary["rankings"] = self.calculate_rankings(results)

        return summary

    def calculate_rankings(self, results: list[MeasurementResult]) -> dict:
        """Calculate tool rankings by different metrics."""
        rankings = {}

        # Average execution time (lower is better)
        times = {}
        for tool in self.tools:
            tool_results = [r for r in results if r.tool == tool and r.exit_code == 0]
            if tool_results:
                times[tool] = sum(r.execution_time_seconds for r in tool_results) / len(tool_results)

        if times:
            rankings["speed"] = sorted(times.items(), key=lambda x: x[1])

        # Average memory (lower is better)
        memories = {}
        for tool in self.tools:
            tool_results = [r for r in results if r.tool == tool and r.exit_code == 0]
            if tool_results:
                memories[tool] = sum(r.peak_memory_mb for r in tool_results) / len(tool_results)

        if memories:
            rankings["memory_efficiency"] = sorted(memories.items(), key=lambda x: x[1])

        # Token efficiency (higher is better - more content per token)
        token_efficiency = {}
        for tool in self.tools:
            tool_results = [r for r in results if r.tool == tool and r.token_count]
            if tool_results:
                # Bytes per token - higher means more efficient
                ratios = [r.output_size_bytes / r.token_count for r in tool_results if r.token_count > 0]
                if ratios:
                    token_efficiency[tool] = sum(ratios) / len(ratios)

        if token_efficiency:
            rankings["token_efficiency"] = sorted(token_efficiency.items(), key=lambda x: x[1], reverse=True)

        return rankings

    def run(self):
        """Execute full benchmark suite."""
        print("=" * 60)
        print("Competitive Benchmark: Infiniloom vs Repomix vs Gitingest")
        print("=" * 60)

        # Verify tools
        if not self.verify_tools():
            print("\nSome tools are not installed. Run: python install_tools.py install")
            return False

        # Prepare repos
        repo_paths = self.prepare_repos()
        if not repo_paths:
            print("\nNo repositories available for testing")
            return False

        # Run benchmarks
        results = self.run_all_benchmarks(repo_paths)

        # Save results
        self.save_results(results)
        self.results = results

        # Print final rankings
        self.print_rankings()

        return True

    def print_rankings(self):
        """Print final rankings."""
        if not self.results:
            return

        summary = self.generate_summary(self.results)
        rankings = summary.get("rankings", {})

        print("\n" + "=" * 60)
        print("FINAL RANKINGS")
        print("=" * 60)

        if "speed" in rankings:
            print("\nSpeed (fastest to slowest):")
            for i, (tool, time) in enumerate(rankings["speed"], 1):
                print(f"  {i}. {tool}: {time:.2f}s average")

        if "memory_efficiency" in rankings:
            print("\nMemory Efficiency (lowest to highest):")
            for i, (tool, mem) in enumerate(rankings["memory_efficiency"], 1):
                print(f"  {i}. {tool}: {mem:.1f}MB average")

        if "token_efficiency" in rankings:
            print("\nToken Efficiency (most efficient to least):")
            for i, (tool, ratio) in enumerate(rankings["token_efficiency"], 1):
                print(f"  {i}. {tool}: {ratio:.2f} bytes/token")


def main():
    parser = argparse.ArgumentParser(
        description="Run competitive benchmarks between repository packing tools"
    )
    parser.add_argument(
        "--tools",
        nargs="+",
        default=["infiniloom", "repomix", "gitingest"],
        choices=list(TOOLS.keys()),
        help="Tools to benchmark",
    )
    parser.add_argument(
        "--repos",
        nargs="+",
        default=["fastapi", "lodash"],
        help="Repositories to test on",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("./benchmark_results"),
        help="Directory for output files",
    )
    parser.add_argument(
        "--warmup",
        type=int,
        default=1,
        help="Number of warmup runs",
    )
    parser.add_argument(
        "--runs",
        type=int,
        default=3,
        help="Number of measurement runs",
    )
    parser.add_argument(
        "--quick",
        action="store_true",
        help="Quick mode: 0 warmup, 1 run",
    )

    args = parser.parse_args()

    warmup = 0 if args.quick else args.warmup
    runs = 1 if args.quick else args.runs

    runner = BenchmarkRunner(
        tools=args.tools,
        repos=args.repos,
        output_dir=args.output_dir,
        warmup_runs=warmup,
        measurement_runs=runs,
    )

    success = runner.run()
    sys.exit(0 if success else 1)


# Import subprocess here for use in run_benchmark
import subprocess

if __name__ == "__main__":
    main()
