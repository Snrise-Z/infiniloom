#!/usr/bin/env python3
"""Comprehensive comparison between Infiniloom and Repomix."""

import subprocess
import time
import re
import os
import json
from pathlib import Path
from dataclasses import dataclass, field
from typing import Dict, List, Optional
import tempfile

# Paths
PROJECT_ROOT = Path(__file__).parent.parent.parent
INFINILOOM_BIN = PROJECT_ROOT / "target" / "release" / "infiniloom"
REPOS_DIR = PROJECT_ROOT / "tests" / "e2e" / "repos"
OUTPUTS_DIR = PROJECT_ROOT / "tests" / "e2e" / "outputs"

OUTPUTS_DIR.mkdir(parents=True, exist_ok=True)

@dataclass
class ToolMetrics:
    """Metrics for a single tool run."""
    tool: str
    repo: str
    time_seconds: float = 0.0
    files_detected: int = 0
    output_size_bytes: int = 0
    token_estimate: int = 0
    languages: Dict[str, int] = field(default_factory=dict)
    error: Optional[str] = None


def run_infiniloom_scan(repo_path: Path) -> ToolMetrics:
    """Run infiniloom scan and collect metrics."""
    metrics = ToolMetrics(tool="infiniloom", repo=repo_path.name)

    start = time.time()
    result = subprocess.run(
        [str(INFINILOOM_BIN), "scan", str(repo_path), "--verbose"],
        capture_output=True,
        timeout=600,
        text=True,
    )
    metrics.time_seconds = time.time() - start

    if result.returncode != 0:
        metrics.error = result.stderr[:500]
        return metrics

    output = result.stdout

    # Parse file count
    file_match = re.search(r"Files:\s*(\d+)", output)
    if file_match:
        metrics.files_detected = int(file_match.group(1))

    # Parse total size
    size_match = re.search(r"Total Size:\s*([\d.]+)\s*(\w+)", output)
    if size_match:
        size_val = float(size_match.group(1))
        unit = size_match.group(2).upper()
        multipliers = {"B": 1, "KB": 1024, "KIB": 1024, "MB": 1024**2, "MIB": 1024**2, "GB": 1024**3}
        metrics.output_size_bytes = int(size_val * multipliers.get(unit, 1))

    # Parse languages
    lang_section = re.search(r"Languages:(.*?)(?:\n\n|\Z)", output, re.DOTALL)
    if lang_section:
        for match in re.finditer(r"(\w+):\s*(\d+)\s*files?", lang_section.group(1)):
            metrics.languages[match.group(1).lower()] = int(match.group(2))

    return metrics


def run_infiniloom_pack(repo_path: Path, format: str = "xml") -> ToolMetrics:
    """Run infiniloom pack and collect metrics."""
    metrics = ToolMetrics(tool=f"infiniloom-pack-{format}", repo=repo_path.name)
    output_file = OUTPUTS_DIR / f"{repo_path.name}_infiniloom_{format}.{format}"

    start = time.time()
    result = subprocess.run(
        [str(INFINILOOM_BIN), "pack", str(repo_path), "--format", format, "-o", str(output_file)],
        capture_output=True,
        timeout=600,
        text=True,
    )
    metrics.time_seconds = time.time() - start

    if result.returncode != 0:
        metrics.error = result.stderr[:500]
        return metrics

    if output_file.exists():
        metrics.output_size_bytes = output_file.stat().st_size
        # Estimate tokens (roughly 4 chars per token)
        metrics.token_estimate = metrics.output_size_bytes // 4

    return metrics


def run_repomix(repo_path: Path, style: str = "xml") -> ToolMetrics:
    """Run repomix and collect metrics."""
    metrics = ToolMetrics(tool=f"repomix-{style}", repo=repo_path.name)
    output_file = OUTPUTS_DIR / f"{repo_path.name}_repomix_{style}.txt"

    start = time.time()
    result = subprocess.run(
        ["repomix", str(repo_path), "-o", str(output_file), "--style", style],
        capture_output=True,
        timeout=600,
        text=True,
        cwd=str(repo_path),
    )
    metrics.time_seconds = time.time() - start

    if result.returncode != 0:
        metrics.error = result.stderr[:500]
        return metrics

    # Parse file count from output
    combined = result.stdout + result.stderr
    file_match = re.search(r"(\d+)\s*files?", combined, re.I)
    if file_match:
        metrics.files_detected = int(file_match.group(1))

    if output_file.exists():
        metrics.output_size_bytes = output_file.stat().st_size
        metrics.token_estimate = metrics.output_size_bytes // 4

    return metrics


def format_size(size_bytes: int) -> str:
    """Format bytes to human readable."""
    for unit in ['B', 'KB', 'MB', 'GB']:
        if size_bytes < 1024:
            return f"{size_bytes:.1f} {unit}"
        size_bytes /= 1024
    return f"{size_bytes:.1f} TB"


def format_time(seconds: float) -> str:
    """Format seconds to human readable."""
    if seconds < 1:
        return f"{seconds*1000:.0f}ms"
    return f"{seconds:.2f}s"


def print_comparison_table(metrics_list: List[ToolMetrics], title: str):
    """Print a comparison table."""
    print(f"\n{'='*80}")
    print(f"  {title}")
    print(f"{'='*80}")

    # Group by repo
    repos = {}
    for m in metrics_list:
        if m.repo not in repos:
            repos[m.repo] = {}
        repos[m.repo][m.tool] = m

    for repo, tools in repos.items():
        print(f"\n📁 {repo}")
        print("-" * 60)

        for tool, m in sorted(tools.items()):
            if m.error:
                print(f"  {tool:25} ❌ Error: {m.error[:50]}...")
            else:
                print(f"  {tool:25} ⏱️  {format_time(m.time_seconds):>8}  "
                      f"📄 {m.files_detected:>5} files  "
                      f"📦 {format_size(m.output_size_bytes):>10}  "
                      f"🔢 ~{m.token_estimate:>7} tokens")


def run_comprehensive_comparison():
    """Run comprehensive comparison on all test repos."""
    print("\n" + "="*80)
    print("  COMPREHENSIVE INFINILOOM vs REPOMIX COMPARISON")
    print("="*80)

    # Get all available repos
    repos = [d for d in REPOS_DIR.iterdir() if d.is_dir() and not d.name.startswith('.')]

    if not repos:
        print("❌ No test repositories found. Please run pytest first to clone repos.")
        return

    print(f"\n📊 Testing {len(repos)} repositories: {', '.join(r.name for r in repos)}")

    all_metrics = []

    # Test 1: Scan Speed Comparison
    print("\n" + "="*80)
    print("  TEST 1: SCAN SPEED COMPARISON")
    print("="*80)

    scan_metrics = []
    for repo in repos:
        print(f"\n⏳ Scanning {repo.name}...")

        # Infiniloom scan
        m = run_infiniloom_scan(repo)
        scan_metrics.append(m)
        print(f"  Infiniloom: {format_time(m.time_seconds)} ({m.files_detected} files)")

        # Repomix (scan equivalent - just run with output to /dev/null)
        start = time.time()
        result = subprocess.run(
            ["repomix", str(repo), "-o", "/dev/null"],
            capture_output=True,
            timeout=600,
            cwd=str(repo),
        )
        repomix_time = time.time() - start
        rm = ToolMetrics(tool="repomix-scan", repo=repo.name, time_seconds=repomix_time)

        # Get file count from verbose output
        result = subprocess.run(
            ["repomix", str(repo), "-o", "/dev/null", "--verbose"],
            capture_output=True,
            timeout=600,
            text=True,
            cwd=str(repo),
        )
        file_match = re.search(r"(\d+)\s*files?", result.stdout + result.stderr, re.I)
        if file_match:
            rm.files_detected = int(file_match.group(1))
        scan_metrics.append(rm)
        print(f"  Repomix:    {format_time(rm.time_seconds)} ({rm.files_detected} files)")

        # Calculate speedup
        if rm.time_seconds > 0:
            speedup = rm.time_seconds / max(m.time_seconds, 0.001)
            if speedup > 1:
                print(f"  📈 Infiniloom is {speedup:.1f}x faster")
            else:
                print(f"  📉 Repomix is {1/speedup:.1f}x faster")

    all_metrics.extend(scan_metrics)

    # Test 2: Pack/Output Comparison (XML)
    print("\n" + "="*80)
    print("  TEST 2: XML OUTPUT COMPARISON")
    print("="*80)

    xml_metrics = []
    for repo in repos:
        print(f"\n⏳ Packing {repo.name} (XML)...")

        # Infiniloom pack XML
        m = run_infiniloom_pack(repo, "xml")
        xml_metrics.append(m)
        print(f"  Infiniloom: {format_time(m.time_seconds)} -> {format_size(m.output_size_bytes)}")

        # Repomix XML
        rm = run_repomix(repo, "xml")
        xml_metrics.append(rm)
        print(f"  Repomix:    {format_time(rm.time_seconds)} -> {format_size(rm.output_size_bytes)}")

        # Compare
        if m.output_size_bytes > 0 and rm.output_size_bytes > 0:
            ratio = m.output_size_bytes / rm.output_size_bytes
            if ratio < 1:
                print(f"  📉 Infiniloom output is {(1-ratio)*100:.1f}% smaller")
            else:
                print(f"  📈 Repomix output is {(ratio-1)*100:.1f}% smaller")

    all_metrics.extend(xml_metrics)

    # Test 3: Markdown Output Comparison
    print("\n" + "="*80)
    print("  TEST 3: MARKDOWN OUTPUT COMPARISON")
    print("="*80)

    md_metrics = []
    for repo in repos:
        print(f"\n⏳ Packing {repo.name} (Markdown)...")

        # Infiniloom pack markdown
        m = run_infiniloom_pack(repo, "markdown")
        md_metrics.append(m)
        print(f"  Infiniloom: {format_time(m.time_seconds)} -> {format_size(m.output_size_bytes)}")

        # Repomix markdown
        rm = run_repomix(repo, "markdown")
        md_metrics.append(rm)
        print(f"  Repomix:    {format_time(rm.time_seconds)} -> {format_size(rm.output_size_bytes)}")

    all_metrics.extend(md_metrics)

    # Test 4: Plain Text Comparison
    print("\n" + "="*80)
    print("  TEST 4: PLAIN TEXT OUTPUT COMPARISON")
    print("="*80)

    plain_metrics = []
    for repo in repos:
        print(f"\n⏳ Packing {repo.name} (Plain)...")

        # Infiniloom pack plain
        m = run_infiniloom_pack(repo, "plain")
        plain_metrics.append(m)
        print(f"  Infiniloom: {format_time(m.time_seconds)} -> {format_size(m.output_size_bytes)}")

        # Repomix plain
        rm = run_repomix(repo, "plain")
        plain_metrics.append(rm)
        print(f"  Repomix:    {format_time(rm.time_seconds)} -> {format_size(rm.output_size_bytes)}")

    all_metrics.extend(plain_metrics)

    # Summary
    print("\n" + "="*80)
    print("  SUMMARY")
    print("="*80)

    # Calculate aggregates
    infiniloom_times = [m.time_seconds for m in all_metrics if "infiniloom" in m.tool and not m.error]
    repomix_times = [m.time_seconds for m in all_metrics if "repomix" in m.tool and not m.error]

    infiniloom_sizes = [m.output_size_bytes for m in all_metrics if "infiniloom-pack" in m.tool and not m.error and m.output_size_bytes > 0]
    repomix_sizes = [m.output_size_bytes for m in all_metrics if "repomix" in m.tool and "scan" not in m.tool and not m.error and m.output_size_bytes > 0]

    print(f"\n📊 Aggregate Results ({len(repos)} repos tested)")
    print("-" * 60)

    if infiniloom_times and repomix_times:
        avg_infiniloom = sum(infiniloom_times) / len(infiniloom_times)
        avg_repomix = sum(repomix_times) / len(repomix_times)
        print(f"  Average Time:")
        print(f"    Infiniloom: {format_time(avg_infiniloom)}")
        print(f"    Repomix:    {format_time(avg_repomix)}")
        if avg_repomix > 0:
            speedup = avg_repomix / avg_infiniloom
            print(f"    → Infiniloom is {speedup:.2f}x {'faster' if speedup > 1 else 'slower'}")

    if infiniloom_sizes and repomix_sizes:
        avg_inf_size = sum(infiniloom_sizes) / len(infiniloom_sizes)
        avg_rep_size = sum(repomix_sizes) / len(repomix_sizes)
        print(f"\n  Average Output Size:")
        print(f"    Infiniloom: {format_size(int(avg_inf_size))}")
        print(f"    Repomix:    {format_size(int(avg_rep_size))}")
        if avg_rep_size > 0:
            ratio = avg_inf_size / avg_rep_size
            if ratio < 1:
                print(f"    → Infiniloom output is {(1-ratio)*100:.1f}% smaller")
            else:
                print(f"    → Repomix output is {(ratio-1)*100:.1f}% smaller")

    # Token efficiency
    infiniloom_tokens = [m.token_estimate for m in all_metrics if "infiniloom-pack" in m.tool and not m.error and m.token_estimate > 0]
    repomix_tokens = [m.token_estimate for m in all_metrics if "repomix" in m.tool and "scan" not in m.tool and not m.error and m.token_estimate > 0]

    if infiniloom_tokens and repomix_tokens:
        avg_inf_tokens = sum(infiniloom_tokens) / len(infiniloom_tokens)
        avg_rep_tokens = sum(repomix_tokens) / len(repomix_tokens)
        print(f"\n  Average Token Estimate:")
        print(f"    Infiniloom: ~{int(avg_inf_tokens):,} tokens")
        print(f"    Repomix:    ~{int(avg_rep_tokens):,} tokens")
        if avg_rep_tokens > 0:
            ratio = avg_inf_tokens / avg_rep_tokens
            if ratio < 1:
                print(f"    → Infiniloom uses {(1-ratio)*100:.1f}% fewer tokens")
            else:
                print(f"    → Repomix uses {(ratio-1)*100:.1f}% fewer tokens")

    print("\n" + "="*80)
    print("  COMPARISON COMPLETE")
    print("="*80)

    return all_metrics


if __name__ == "__main__":
    run_comprehensive_comparison()
