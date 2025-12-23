"""Performance measurement utilities for competitive benchmarks."""

import json
import os
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

import psutil


@dataclass
class MeasurementResult:
    """Result of a single benchmark measurement."""

    tool: str
    repo: str
    execution_time_seconds: float
    peak_memory_mb: float
    output_size_bytes: int
    token_count: Optional[int] = None
    exit_code: int = 0
    error: Optional[str] = None
    extra_metrics: dict = field(default_factory=dict)

    def tokens_per_second(self) -> Optional[float]:
        """Calculate token processing rate."""
        if self.token_count and self.execution_time_seconds > 0:
            return self.token_count / self.execution_time_seconds
        return None

    def to_dict(self) -> dict:
        """Convert to dictionary for serialization."""
        return {
            "tool": self.tool,
            "repo": self.repo,
            "execution_time_seconds": self.execution_time_seconds,
            "peak_memory_mb": self.peak_memory_mb,
            "output_size_bytes": self.output_size_bytes,
            "token_count": self.token_count,
            "tokens_per_second": self.tokens_per_second(),
            "exit_code": self.exit_code,
            "error": self.error,
            **self.extra_metrics,
        }


def measure_command(
    cmd: list[str],
    tool_name: str,
    repo_name: str,
    capture_output: bool = True,
    timeout_seconds: int = 300,
) -> MeasurementResult:
    """
    Measure execution time, memory usage, and output size of a command.

    Args:
        cmd: Command and arguments to execute
        tool_name: Name of the tool being measured
        repo_name: Name of the repository being processed
        capture_output: Whether to capture stdout
        timeout_seconds: Maximum execution time

    Returns:
        MeasurementResult with all measurements
    """
    output_data = b""
    peak_memory_mb = 0.0
    error = None

    start_time = time.perf_counter()

    try:
        process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE if capture_output else subprocess.DEVNULL,
            stderr=subprocess.PIPE,
        )

        # Monitor memory usage during execution
        ps_process = psutil.Process(process.pid)

        while process.poll() is None:
            try:
                mem_info = ps_process.memory_info()
                current_memory = mem_info.rss / (1024 * 1024)  # Convert to MB
                peak_memory_mb = max(peak_memory_mb, current_memory)
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass
            time.sleep(0.01)  # Sample every 10ms

        # Get final output
        if capture_output:
            output_data, stderr_data = process.communicate(timeout=timeout_seconds)
            if process.returncode != 0:
                error = stderr_data.decode("utf-8", errors="replace")
        else:
            _, stderr_data = process.communicate(timeout=timeout_seconds)
            if process.returncode != 0:
                error = stderr_data.decode("utf-8", errors="replace")

        exit_code = process.returncode

    except subprocess.TimeoutExpired:
        process.kill()
        output_data, _ = process.communicate()
        exit_code = -1
        error = f"Timeout after {timeout_seconds} seconds"

    except Exception as e:
        exit_code = -1
        error = str(e)

    end_time = time.perf_counter()
    execution_time = end_time - start_time

    return MeasurementResult(
        tool=tool_name,
        repo=repo_name,
        execution_time_seconds=execution_time,
        peak_memory_mb=peak_memory_mb,
        output_size_bytes=len(output_data),
        exit_code=exit_code,
        error=error,
    )


def measure_with_warmup(
    cmd: list[str],
    tool_name: str,
    repo_name: str,
    warmup_runs: int = 1,
    measurement_runs: int = 3,
) -> list[MeasurementResult]:
    """
    Run benchmark with warmup and multiple measurements.

    Args:
        cmd: Command to execute
        tool_name: Name of the tool
        repo_name: Name of the repository
        warmup_runs: Number of warmup runs (not measured)
        measurement_runs: Number of measured runs

    Returns:
        List of measurement results
    """
    # Warmup runs
    print(f"  Warming up {tool_name}...")
    for _ in range(warmup_runs):
        subprocess.run(cmd, capture_output=True, timeout=300)

    # Measurement runs
    results = []
    for i in range(measurement_runs):
        print(f"  Run {i + 1}/{measurement_runs}...")
        result = measure_command(cmd, tool_name, repo_name)
        results.append(result)

    return results


def calculate_statistics(results: list[MeasurementResult]) -> dict:
    """
    Calculate statistics from multiple measurements.

    Returns:
        Dictionary with min, max, mean, median for each metric
    """
    if not results:
        return {}

    times = [r.execution_time_seconds for r in results]
    memories = [r.peak_memory_mb for r in results]
    sizes = [r.output_size_bytes for r in results]

    def stats(values):
        sorted_vals = sorted(values)
        n = len(sorted_vals)
        return {
            "min": min(sorted_vals),
            "max": max(sorted_vals),
            "mean": sum(sorted_vals) / n,
            "median": sorted_vals[n // 2],
        }

    return {
        "execution_time": stats(times),
        "peak_memory_mb": stats(memories),
        "output_size_bytes": stats(sizes),
        "successful_runs": sum(1 for r in results if r.exit_code == 0),
        "total_runs": len(results),
    }


def count_tokens_tiktoken(text: str, model: str = "gpt-4") -> int:
    """
    Count tokens using tiktoken (OpenAI tokenizer).

    Args:
        text: Text to tokenize
        model: Model name for encoding selection

    Returns:
        Token count
    """
    try:
        import tiktoken

        encoding = tiktoken.encoding_for_model(model)
        return len(encoding.encode(text))
    except ImportError:
        # Fallback: rough estimate
        return len(text) // 4


def save_results(results: list[MeasurementResult], output_path: Path):
    """Save benchmark results to JSON file."""
    data = [r.to_dict() for r in results]
    with open(output_path, "w") as f:
        json.dump(data, f, indent=2)


def load_results(input_path: Path) -> list[dict]:
    """Load benchmark results from JSON file."""
    with open(input_path) as f:
        return json.load(f)


if __name__ == "__main__":
    # Quick test
    result = measure_command(
        ["echo", "hello world"],
        tool_name="echo",
        repo_name="test",
    )
    print(f"Execution time: {result.execution_time_seconds:.3f}s")
    print(f"Peak memory: {result.peak_memory_mb:.1f}MB")
    print(f"Output size: {result.output_size_bytes} bytes")
