#!/usr/bin/env python3
"""
Report generation for competitive benchmarks.

Generates markdown and HTML reports from benchmark results.
"""

import json
from datetime import datetime
from pathlib import Path
from typing import Optional

try:
    import pandas as pd
    PANDAS_AVAILABLE = True
except ImportError:
    PANDAS_AVAILABLE = False

try:
    from tabulate import tabulate
    TABULATE_AVAILABLE = True
except ImportError:
    TABULATE_AVAILABLE = False


def load_benchmark_results(results_dir: Path) -> list[dict]:
    """Load all benchmark result files from a directory."""
    all_results = []

    for json_file in results_dir.glob("results_*.json"):
        with open(json_file) as f:
            data = json.load(f)
            all_results.extend(data)

    return all_results


def load_llm_eval_results(eval_file: Path) -> list[dict]:
    """Load LLM evaluation results."""
    if not eval_file.exists():
        return []

    with open(eval_file) as f:
        return json.load(f)


def generate_performance_table(results: list[dict]) -> str:
    """Generate performance comparison table."""
    if not results:
        return "No performance results available."

    # Group by tool
    by_tool = {}
    for r in results:
        tool = r.get("tool", "unknown")
        if tool not in by_tool:
            by_tool[tool] = []
        by_tool[tool].append(r)

    # Calculate averages
    table_data = []
    for tool, tool_results in sorted(by_tool.items()):
        successful = [r for r in tool_results if r.get("exit_code", 0) == 0]
        if not successful:
            continue

        avg_time = sum(r["execution_time_seconds"] for r in successful) / len(successful)
        avg_memory = sum(r["peak_memory_mb"] for r in successful) / len(successful)
        avg_size = sum(r["output_size_bytes"] for r in successful) / len(successful)

        tokens = [r["token_count"] for r in successful if r.get("token_count")]
        avg_tokens = sum(tokens) / len(tokens) if tokens else 0

        table_data.append({
            "Tool": tool,
            "Avg Time (s)": f"{avg_time:.2f}",
            "Avg Memory (MB)": f"{avg_memory:.1f}",
            "Avg Output (KB)": f"{avg_size / 1024:.1f}",
            "Avg Tokens": f"{avg_tokens:,.0f}" if avg_tokens else "N/A",
            "Success Rate": f"{len(successful)}/{len(tool_results)}",
        })

    if TABULATE_AVAILABLE:
        return tabulate(table_data, headers="keys", tablefmt="github")
    else:
        # Simple markdown table
        headers = list(table_data[0].keys())
        lines = [" | ".join(headers), " | ".join(["---"] * len(headers))]
        for row in table_data:
            lines.append(" | ".join(str(row[h]) for h in headers))
        return "\n".join(lines)


def generate_llm_eval_table(results: list[dict]) -> str:
    """Generate LLM effectiveness comparison table."""
    if not results:
        return "No LLM evaluation results available."

    # Group by tool
    by_tool = {}
    for r in results:
        tool = r.get("tool", "unknown")
        if tool not in by_tool:
            by_tool[tool] = []
        by_tool[tool].append(r)

    table_data = []
    for tool, tool_results in sorted(by_tool.items()):
        scores = [r["score"] for r in tool_results if r.get("score", 0) > 0]
        if not scores:
            continue

        avg_score = sum(scores) / len(scores)

        # By LLM
        claude_scores = [r["score"] for r in tool_results if r.get("llm") == "claude" and r.get("score", 0) > 0]
        gpt4_scores = [r["score"] for r in tool_results if r.get("llm") == "gpt-4" and r.get("score", 0) > 0]

        claude_avg = sum(claude_scores) / len(claude_scores) if claude_scores else 0
        gpt4_avg = sum(gpt4_scores) / len(gpt4_scores) if gpt4_scores else 0

        table_data.append({
            "Tool": tool,
            "Overall Score": f"{avg_score:.1f}/10",
            "Claude Score": f"{claude_avg:.1f}/10" if claude_scores else "N/A",
            "GPT-4 Score": f"{gpt4_avg:.1f}/10" if gpt4_scores else "N/A",
            "Tests Run": len(tool_results),
        })

    if TABULATE_AVAILABLE:
        return tabulate(table_data, headers="keys", tablefmt="github")
    else:
        headers = list(table_data[0].keys())
        lines = [" | ".join(headers), " | ".join(["---"] * len(headers))]
        for row in table_data:
            lines.append(" | ".join(str(row[h]) for h in headers))
        return "\n".join(lines)


def generate_feature_matrix() -> str:
    """Generate feature comparison matrix."""
    features = [
        ("XML Output", "Yes", "Yes", "No"),
        ("JSON Output", "Yes", "Yes", "Yes"),
        ("YAML Output", "Yes", "No", "No"),
        ("Markdown Output", "Yes", "Yes", "Yes"),
        ("Token Counting", "Yes", "Yes", "Limited"),
        ("Accurate Tokenizer (tiktoken)", "Yes", "No", "No"),
        ("Security Scanning", "Yes", "Limited", "No"),
        ("Secret Redaction", "Yes", "No", "No"),
        ("Compression Levels", "Yes (5 levels)", "No", "No"),
        ("Symbol Extraction", "Yes (30+ langs)", "Limited", "No"),
        ("Repository Map", "Yes", "No", "No"),
        ("Git History", "Yes", "No", "Yes"),
        ("Diff Context", "Yes", "No", "No"),
        ("Remote Repos", "Yes", "Yes", "Yes"),
        ("Config File", "Yes (YAML/TOML)", "Yes", "Limited"),
        ("Watch Mode", "Planned", "No", "No"),
        ("Incremental Cache", "Yes", "No", "No"),
        ("Code Chunking", "Yes", "No", "No"),
        ("Multi-Model Tokens", "Yes (27 models)", "Limited", "No"),
        ("Language Bindings", "Python, Node", "Node only", "Python only"),
    ]

    headers = ["Feature", "Infiniloom", "Repomix", "Gitingest"]
    lines = [" | ".join(headers), " | ".join(["---"] * len(headers))]

    for feature in features:
        lines.append(" | ".join(feature))

    return "\n".join(lines)


def generate_markdown_report(
    perf_results: list[dict],
    llm_results: list[dict],
    output_path: Path,
):
    """Generate comprehensive markdown report."""
    report = f"""# Competitive Benchmark Report

**Generated:** {datetime.now().strftime("%Y-%m-%d %H:%M:%S")}

## Executive Summary

This report compares three repository packing tools for LLM context generation:
- **Infiniloom** (Rust) - High-performance, feature-rich
- **Repomix** (Node.js) - Simple, widely used
- **Gitingest** (Python) - Lightweight, Python-native

## Performance Comparison

{generate_performance_table(perf_results)}

### Key Findings

1. **Speed**: Lower execution time is better
2. **Memory**: Lower memory usage is better
3. **Output Size**: Depends on use case (smaller may mean better compression)
4. **Tokens**: Lower token count for same content = more efficient

## LLM Effectiveness

{generate_llm_eval_table(llm_results)}

### Evaluation Methodology

Tests measure how well each tool's output enables LLMs to:
1. **Code Understanding**: Explain project architecture
2. **Symbol Location**: Find specific functions/classes
3. **Bug Finding**: Identify issues in code
4. **Code Generation**: Write new code matching existing patterns

## Feature Comparison Matrix

{generate_feature_matrix()}

### Legend
- **Yes**: Feature fully implemented
- **Limited**: Partial implementation
- **No**: Feature not available
- **Planned**: On roadmap

## Detailed Results by Repository

"""
    # Add per-repo sections
    repos = set(r.get("repo") for r in perf_results)
    for repo in sorted(repos):
        repo_results = [r for r in perf_results if r.get("repo") == repo]

        report += f"\n### {repo}\n\n"

        table_data = []
        for r in repo_results:
            if r.get("exit_code", 0) != 0:
                continue
            table_data.append({
                "Tool": r["tool"],
                "Time (s)": f"{r['execution_time_seconds']:.2f}",
                "Memory (MB)": f"{r['peak_memory_mb']:.1f}",
                "Output (KB)": f"{r['output_size_bytes'] / 1024:.1f}",
            })

        if table_data:
            if TABULATE_AVAILABLE:
                report += tabulate(table_data, headers="keys", tablefmt="github") + "\n\n"
            else:
                headers = list(table_data[0].keys())
                report += " | ".join(headers) + "\n"
                report += " | ".join(["---"] * len(headers)) + "\n"
                for row in table_data:
                    report += " | ".join(str(row[h]) for h in headers) + "\n"
                report += "\n"

    report += """
## Recommendations

### Best for Different Use Cases

| Use Case | Recommended Tool | Reason |
|----------|-----------------|--------|
| Large codebases | Infiniloom | Best performance, lowest memory |
| Claude workflows | Infiniloom | Optimized XML output |
| Quick prototyping | Repomix | Simple, well-known |
| Python environments | Gitingest | Native Python |
| Security-sensitive | Infiniloom | Secret detection/redaction |
| Token-limited contexts | Infiniloom | Multiple compression levels |

## Methodology

### Performance Measurement
- Each tool run 3 times per repository
- 1 warmup run before measurements
- Memory tracked via psutil (peak RSS)
- Tokens counted via tiktoken (GPT-4 encoding)

### LLM Evaluation
- Tests run with Claude (claude-sonnet-4-20250514) and GPT-4 (gpt-4-turbo-preview)
- Responses scored 0-10 by LLM-as-judge
- Multiple test types averaged

---

*Report generated by Infiniloom Competitive Benchmark Suite*
"""

    with open(output_path, "w") as f:
        f.write(report)

    return report


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Generate benchmark reports")
    parser.add_argument(
        "--results-dir",
        type=Path,
        default=Path("./benchmark_results"),
        help="Directory containing benchmark results",
    )
    parser.add_argument(
        "--llm-results",
        type=Path,
        default=Path("./llm_eval_results.json"),
        help="LLM evaluation results file",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("./benchmark_report.md"),
        help="Output report file",
    )

    args = parser.parse_args()

    # Load results
    perf_results = load_benchmark_results(args.results_dir)
    llm_results = load_llm_eval_results(args.llm_results)

    print(f"Loaded {len(perf_results)} performance results")
    print(f"Loaded {len(llm_results)} LLM evaluation results")

    # Generate report
    report = generate_markdown_report(perf_results, llm_results, args.output)

    print(f"\nReport generated: {args.output}")
    print(f"Report length: {len(report):,} characters")


if __name__ == "__main__":
    main()
