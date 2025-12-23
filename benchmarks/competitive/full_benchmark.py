#!/usr/bin/env python3
"""Full benchmark comparison on multiple repositories."""
import subprocess
import time
from pathlib import Path
import os

os.chdir(Path(__file__).parent)

REPOS = {
    "lodash": Path("repos/lodash").resolve(),
    "fastapi": Path("repos/fastapi").resolve(),
}

INFINILOOM = (Path(__file__).parent.parent.parent / "target" / "debug" / "infiniloom").resolve()

def benchmark_tool(name, cmd, output_file=None):
    start = time.perf_counter()
    result = subprocess.run(cmd, capture_output=True, timeout=300)
    elapsed = time.perf_counter() - start
    
    if output_file and Path(output_file).exists():
        size = Path(output_file).stat().st_size
        Path(output_file).unlink()
    else:
        size = len(result.stdout)
    
    return {
        "name": name,
        "time_sec": elapsed,
        "output_bytes": size,
        "exit_code": result.returncode,
    }

print("=" * 80)
print("COMPETITIVE BENCHMARK: Infiniloom vs Repomix vs Gitingest")
print("=" * 80)

all_results = {tool: [] for tool in ["Infiniloom", "Repomix", "Gitingest"]}

for repo_name, repo_path in REPOS.items():
    if not repo_path.exists():
        print(f"\nSkipping {repo_name} - not found")
        continue
        
    print(f"\n{'='*80}")
    print(f"Repository: {repo_name}")
    print(f"Path: {repo_path}")
    print("-" * 80)
    
    # Infiniloom
    r = benchmark_tool("Infiniloom", [str(INFINILOOM), "pack", str(repo_path), "--format", "xml"])
    all_results["Infiniloom"].append({**r, "repo": repo_name})
    print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>10,} bytes | Exit: {r['exit_code']}")
    
    # Repomix
    output_file = "repomix-output.xml"
    r = benchmark_tool("Repomix", ["repomix", str(repo_path), "-o", output_file], output_file)
    all_results["Repomix"].append({**r, "repo": repo_name})
    print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>10,} bytes | Exit: {r['exit_code']}")
    
    # Gitingest
    output_file = "digest.txt"
    r = benchmark_tool("Gitingest", ["gitingest", str(repo_path)], output_file)
    all_results["Gitingest"].append({**r, "repo": repo_name})
    print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>10,} bytes | Exit: {r['exit_code']}")

print("\n" + "=" * 80)
print("AGGREGATE RESULTS")
print("=" * 80)

for tool, results in all_results.items():
    successful = [r for r in results if r["exit_code"] == 0]
    if not successful:
        continue
    avg_time = sum(r["time_sec"] for r in successful) / len(successful)
    avg_size = sum(r["output_bytes"] for r in successful) / len(successful)
    print(f"\n{tool}:")
    print(f"  Average Time:   {avg_time:.2f}s")
    print(f"  Average Output: {avg_size/1024:.1f} KB")
    print(f"  Success Rate:   {len(successful)}/{len(results)}")

print("\n" + "=" * 80)
print("FINAL RANKINGS")
print("=" * 80)

# Calculate averages for ranking
tool_avgs = {}
for tool, results in all_results.items():
    successful = [r for r in results if r["exit_code"] == 0]
    if successful:
        tool_avgs[tool] = {
            "time": sum(r["time_sec"] for r in successful) / len(successful),
            "size": sum(r["output_bytes"] for r in successful) / len(successful),
        }

print("\nBy Average Speed (fastest first):")
for i, (tool, avgs) in enumerate(sorted(tool_avgs.items(), key=lambda x: x[1]["time"]), 1):
    print(f"  {i}. {tool:15} - {avgs['time']:.2f}s avg")

print("\nBy Average Output Size (smallest first - better for LLM context efficiency):")
for i, (tool, avgs) in enumerate(sorted(tool_avgs.items(), key=lambda x: x[1]["size"]), 1):
    print(f"  {i}. {tool:15} - {avgs['size']/1024:.1f} KB avg")

print("\n" + "=" * 80)
