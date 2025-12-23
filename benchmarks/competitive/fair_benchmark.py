#!/usr/bin/env python3
"""Fair benchmark comparison with equivalent configurations."""
import subprocess
import time
from pathlib import Path
import os
import re

os.chdir(Path(__file__).parent)

REPOS = {
    "lodash": Path("repos/lodash").resolve(),
    "fastapi": Path("repos/fastapi").resolve(),
}

INFINILOOM = (Path(__file__).parent.parent.parent / "target" / "release" / "infiniloom").resolve()

def benchmark_tool(name, cmd, output_file=None):
    """Run a tool and measure time + output size."""
    start = time.perf_counter()
    result = subprocess.run(cmd, capture_output=True, timeout=300)
    elapsed = time.perf_counter() - start

    if output_file and Path(output_file).exists():
        size = Path(output_file).stat().st_size
        content = Path(output_file).read_text(errors='ignore')
        Path(output_file).unlink()
    else:
        size = len(result.stdout)
        content = result.stdout.decode(errors='ignore')

    # Count files mentioned in output (rough estimate)
    file_count = len(re.findall(r'<file|File:|path="|── ', content))

    return {
        "name": name,
        "time_sec": elapsed,
        "output_bytes": size,
        "exit_code": result.returncode,
        "file_count": file_count,
        "stderr": result.stderr.decode()[:200] if result.returncode != 0 else "",
    }

def count_source_files(repo_path):
    """Count source files excluding .git"""
    count = 0
    for f in repo_path.rglob("*"):
        if f.is_file() and ".git" not in str(f):
            count += 1
    return count

print("=" * 80)
print("FAIR BENCHMARK COMPARISON")
print("=" * 80)
print()
print("This benchmark ensures apples-to-apples comparison by:")
print("1. Test A: All tools include ALL files (Infiniloom with --include-tests --include-docs)")
print("2. Test B: All tools exclude tests/docs (competitors with exclusion patterns)")
print()

# Test exclusion patterns for competitors (to match Infiniloom defaults)
EXCLUDE_PATTERNS = "test/**,tests/**,doc/**,docs/**,.github/**,*_test.*,*.test.*,*.spec.*"

for repo_name, repo_path in REPOS.items():
    if not repo_path.exists():
        print(f"\nSkipping {repo_name} - not found")
        continue

    total_files = count_source_files(repo_path)

    print(f"\n{'='*80}")
    print(f"Repository: {repo_name} ({total_files} total files)")
    print(f"Path: {repo_path}")
    print("=" * 80)

    # =========================================================================
    # TEST A: All files included (match competitors' defaults)
    # =========================================================================
    print(f"\n--- TEST A: All Files Included ---")
    print("(Infiniloom with --include-tests --include-docs --no-default-ignores)")
    print("-" * 70)

    results_a = []

    # Infiniloom - include everything
    r = benchmark_tool(
        "Infiniloom",
        [str(INFINILOOM), "pack", str(repo_path), "--format", "xml",
         "--include-tests", "--include-docs", "--no-default-ignores"]
    )
    results_a.append(r)
    print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>12,} bytes | Exit: {r['exit_code']}")

    # Repomix - default (includes everything)
    output_file = "repomix-output.xml"
    r = benchmark_tool("Repomix", ["repomix", str(repo_path), "-o", output_file, "--style", "xml"], output_file)
    results_a.append(r)
    print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>12,} bytes | Exit: {r['exit_code']}")

    # Gitingest - default (includes everything)
    output_file = "digest.txt"
    r = benchmark_tool("Gitingest", ["gitingest", str(repo_path), "-o", output_file], output_file)
    results_a.append(r)
    print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>12,} bytes | Exit: {r['exit_code']}")

    # =========================================================================
    # TEST B: Smart filtering (match Infiniloom defaults)
    # =========================================================================
    print(f"\n--- TEST B: Smart Filtering (exclude tests/docs) ---")
    print("(All tools with test/doc exclusions applied)")
    print("-" * 70)

    results_b = []

    # Infiniloom - defaults (excludes tests/docs)
    r = benchmark_tool(
        "Infiniloom",
        [str(INFINILOOM), "pack", str(repo_path), "--format", "xml"]
    )
    results_b.append(r)
    print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>12,} bytes | Exit: {r['exit_code']}")

    # Repomix - with exclusions
    output_file = "repomix-output.xml"
    r = benchmark_tool(
        "Repomix",
        ["repomix", str(repo_path), "-o", output_file, "--style", "xml", "-i", EXCLUDE_PATTERNS],
        output_file
    )
    results_b.append(r)
    print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>12,} bytes | Exit: {r['exit_code']}")

    # Gitingest - with exclusions (multiple -e flags)
    output_file = "digest.txt"
    r = benchmark_tool(
        "Gitingest",
        ["gitingest", str(repo_path), "-o", output_file,
         "-e", "test/**", "-e", "tests/**", "-e", "doc/**", "-e", "docs/**",
         "-e", ".github/**", "-e", "*_test.*", "-e", "*.test.*", "-e", "*.spec.*"],
        output_file
    )
    results_b.append(r)
    print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>12,} bytes | Exit: {r['exit_code']}")

    # =========================================================================
    # Summary for this repo
    # =========================================================================
    print(f"\n--- Summary for {repo_name} ---")

    # Test A comparison
    successful_a = [r for r in results_a if r["exit_code"] == 0]
    if len(successful_a) >= 2:
        by_size_a = sorted(successful_a, key=lambda x: x["output_bytes"])
        print("\nTest A (all files) - By Output Size:")
        for i, r in enumerate(by_size_a, 1):
            print(f"  {i}. {r['name']:15} - {r['output_bytes']:>12,} bytes ({r['output_bytes']/1024:.1f} KB)")

    # Test B comparison
    successful_b = [r for r in results_b if r["exit_code"] == 0]
    if len(successful_b) >= 2:
        by_size_b = sorted(successful_b, key=lambda x: x["output_bytes"])
        print("\nTest B (filtered) - By Output Size:")
        for i, r in enumerate(by_size_b, 1):
            print(f"  {i}. {r['name']:15} - {r['output_bytes']:>12,} bytes ({r['output_bytes']/1024:.1f} KB)")

print("\n" + "=" * 80)
print("CONCLUSIONS")
print("=" * 80)
print("""
Test A shows: How tools compare with SAME file content
Test B shows: How tools compare with SAME filtering rules

Key metrics to evaluate:
1. Output size ratio (smaller = better for LLM context)
2. Speed (faster = better for developer experience)
3. Format overhead (structure bytes vs content bytes)
""")
