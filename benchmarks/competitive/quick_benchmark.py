#!/usr/bin/env python3
"""Quick benchmark comparison without memory monitoring (simpler, more reliable)."""
import subprocess
import time
import tempfile
from pathlib import Path
import os

os.chdir(Path(__file__).parent)

# Use absolute paths
LODASH = (Path(__file__).parent / "repos/lodash").resolve()
INFINILOOM = (Path(__file__).parent.parent.parent / "target" / "debug" / "infiniloom").resolve()

def benchmark_tool(name, cmd, output_file=None):
    """Run a tool and measure time + output size."""
    start = time.perf_counter()
    result = subprocess.run(cmd, capture_output=True, timeout=120)
    elapsed = time.perf_counter() - start
    
    # Get output size
    if output_file and Path(output_file).exists():
        size = Path(output_file).stat().st_size
        Path(output_file).unlink()  # Clean up
    else:
        size = len(result.stdout)
    
    return {
        "name": name,
        "time_sec": elapsed,
        "output_bytes": size,
        "exit_code": result.returncode,
        "stderr": result.stderr.decode()[:200] if result.returncode != 0 else "",
    }

print("=" * 70)
print("COMPETITIVE BENCHMARK: Infiniloom vs Repomix vs Gitingest")
print(f"Repository: lodash ({LODASH})")
print("=" * 70)

results = []

# Infiniloom
r = benchmark_tool("Infiniloom", [str(INFINILOOM), "pack", str(LODASH), "--format", "xml"])
results.append(r)
print(f"\n{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>10,} bytes | Exit: {r['exit_code']}")
if r['stderr']: print(f"  Error: {r['stderr']}")

# Repomix (outputs to file by default)
output_file = "repomix-output.xml"
r = benchmark_tool("Repomix", ["repomix", str(LODASH), "-o", output_file], output_file)
results.append(r)
print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>10,} bytes | Exit: {r['exit_code']}")
if r['stderr']: print(f"  Error: {r['stderr']}")

# Gitingest (outputs to file by default)
output_file = "digest.txt"
r = benchmark_tool("Gitingest", ["gitingest", str(LODASH)], output_file)
results.append(r)
print(f"{r['name']:15} | Time: {r['time_sec']:6.2f}s | Output: {r['output_bytes']:>10,} bytes | Exit: {r['exit_code']}")
if r['stderr']: print(f"  Error: {r['stderr']}")

print("\n" + "=" * 70)
print("RANKINGS")
print("=" * 70)

# Only consider successful runs
successful = [r for r in results if r["exit_code"] == 0]

if successful:
    # Speed ranking
    by_speed = sorted(successful, key=lambda x: x["time_sec"])
    print("\nBy Speed (fastest first):")
    for i, r in enumerate(by_speed, 1):
        print(f"  {i}. {r['name']:15} - {r['time_sec']:.2f}s")

    # Size ranking (smaller can be better for token efficiency)
    by_size = sorted(successful, key=lambda x: x["output_bytes"])
    print("\nBy Output Size (smallest first):")
    for i, r in enumerate(by_size, 1):
        print(f"  {i}. {r['name']:15} - {r['output_bytes']:,} bytes ({r['output_bytes']/1024:.1f} KB)")

print("\n" + "=" * 70)
