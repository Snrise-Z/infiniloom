#!/usr/bin/env python3
"""
Quality Evaluation for Repository Context Tools.

Measures how well tool outputs help LLMs understand and work with code.
Uses both objective tests (verifiable answers) and LLM-graded tests.
"""

import json
import os
import re
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

# API clients
try:
    import anthropic
    ANTHROPIC_AVAILABLE = True
except ImportError:
    ANTHROPIC_AVAILABLE = False

try:
    from anthropic import AnthropicBedrock
    BEDROCK_AVAILABLE = True
except ImportError:
    BEDROCK_AVAILABLE = False

try:
    from openai import AzureOpenAI
    AZURE_OPENAI_AVAILABLE = True
except ImportError:
    AZURE_OPENAI_AVAILABLE = False

os.chdir(Path(__file__).parent)


@dataclass
class TestResult:
    """Result of a single quality test."""
    test_name: str
    test_type: str  # "objective" or "llm_graded"
    tool: str
    repo: str
    question: str
    expected_answer: str
    llm_answer: str
    is_correct: bool
    score: float  # 0.0 to 1.0
    latency_seconds: float
    input_tokens: int
    output_tokens: int
    notes: str = ""

    def to_dict(self) -> dict:
        return {
            "test_name": self.test_name,
            "test_type": self.test_type,
            "tool": self.tool,
            "repo": self.repo,
            "question": self.question,
            "expected_answer": self.expected_answer,
            "llm_answer": self.llm_answer[:200] if self.llm_answer else "",
            "is_correct": self.is_correct,
            "score": self.score,
            "latency_seconds": self.latency_seconds,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "notes": self.notes,
        }


@dataclass
class EvalConfig:
    """Configuration for quality evaluation."""
    tools: list[str] = field(default_factory=lambda: ["infiniloom", "repomix", "gitingest"])
    repos: list[str] = field(default_factory=lambda: ["lodash", "fastapi"])
    runs_per_test: int = 3
    output_dir: Path = field(default_factory=lambda: Path("./quality_results"))


class QualityEvaluator:
    """Evaluates output quality using LLM-based tests."""

    def __init__(self):
        self.client = None
        self.model = "claude-sonnet-4-20250514"
        self.client_type = None  # "anthropic", "bedrock", or "azure"

        # Try Azure OpenAI first
        if AZURE_OPENAI_AVAILABLE and os.environ.get("AZURE_OPENAI_API_KEY"):
            try:
                self.client = AzureOpenAI(
                    api_version=os.environ.get("AZURE_OPENAI_API_VERSION", "2024-12-01-preview"),
                    azure_endpoint=os.environ.get("AZURE_OPENAI_ENDPOINT", "https://cst-eastus2.cognitiveservices.azure.com/"),
                    api_key=os.environ.get("AZURE_OPENAI_API_KEY"),
                )
                self.model = os.environ.get("AZURE_OPENAI_DEPLOYMENT", "gpt-5.1")
                self.client_type = "azure"
                print(f"Using Azure OpenAI ({self.model})")
            except Exception as e:
                print(f"Azure OpenAI init failed: {e}")

        # Try Bedrock (AWS)
        if self.client is None and BEDROCK_AVAILABLE and os.environ.get("CLAUDE_CODE_USE_BEDROCK"):
            try:
                self.client = AnthropicBedrock(
                    aws_region=os.environ.get("AWS_REGION", "us-east-1"),
                )
                self.model = "us.anthropic.claude-sonnet-4-20250514-v1:0"
                self.client_type = "bedrock"
                print("Using AWS Bedrock for Claude API")
            except Exception as e:
                print(f"Bedrock init failed: {e}")

        # Fall back to direct Anthropic API
        if self.client is None and ANTHROPIC_AVAILABLE and os.environ.get("ANTHROPIC_API_KEY"):
            self.client = anthropic.Anthropic()
            self.client_type = "anthropic"
            print("Using direct Anthropic API")

        self.ground_truth_dir = Path("./ground_truth")
        self.repos_dir = Path("./repos")

    def is_available(self) -> bool:
        return self.client is not None

    def load_ground_truth(self, repo_name: str) -> dict:
        """Load ground truth for a repository."""
        gt_file = self.ground_truth_dir / f"{repo_name}.json"
        if not gt_file.exists():
            raise FileNotFoundError(f"Ground truth not found: {gt_file}")
        return json.loads(gt_file.read_text())

    def get_tool_output(self, tool_name: str, repo_path: Path) -> str:
        """Get packed output from a tool."""
        from install_tools import get_pack_command

        # Use absolute path to avoid tool confusion
        abs_path = repo_path.resolve()
        cmd = get_pack_command(tool_name, abs_path)
        result = subprocess.run(cmd, capture_output=True, timeout=300)

        if result.returncode != 0:
            raise RuntimeError(f"{tool_name} failed: {result.stderr.decode()[:200]}")

        return result.stdout.decode("utf-8", errors="replace")

    def query_llm(self, system: str, user: str, max_tokens: int = 500) -> tuple[str, float, int, int]:
        """Query LLM API. Returns (response, latency, input_tokens, output_tokens)."""
        if not self.client:
            raise RuntimeError("LLM client not available")

        start = time.time()

        if self.client_type == "azure":
            # Azure OpenAI API
            response = self.client.chat.completions.create(
                model=self.model,
                max_completion_tokens=max_tokens,
                messages=[
                    {"role": "system", "content": system},
                    {"role": "user", "content": user},
                ],
            )
            latency = time.time() - start
            return (
                response.choices[0].message.content,
                latency,
                response.usage.prompt_tokens,
                response.usage.completion_tokens,
            )
        else:
            # Anthropic API (direct or Bedrock)
            response = self.client.messages.create(
                model=self.model,
                max_tokens=max_tokens,
                system=system,
                messages=[{"role": "user", "content": user}],
            )
            latency = time.time() - start
            return (
                response.content[0].text,
                latency,
                response.usage.input_tokens,
                response.usage.output_tokens,
            )

    def test_symbol_location(
        self,
        context: str,
        symbol_name: str,
        expected_file: str,
        expected_line: int,
        tool: str,
        repo: str,
    ) -> TestResult:
        """Test if LLM can locate a symbol given the context."""

        system = "You are a code analysis assistant. Answer precisely with file path and line number."
        user = f"""Based on the following codebase context, where is the '{symbol_name}' function/class defined?

Answer in the format: filename:line_number (e.g., src/utils.py:42)
Only provide the location, nothing else.

Codebase context:
{context[:100000]}"""

        try:
            answer, latency, in_tok, out_tok = self.query_llm(system, user)
            answer = answer.strip()

            # Check if answer matches expected
            expected = f"{expected_file}:{expected_line}"

            # Allow some flexibility in matching
            is_correct = False
            if expected_file in answer:
                # Check if line number is within 5 lines
                match = re.search(r":(\d+)", answer)
                if match:
                    found_line = int(match.group(1))
                    is_correct = abs(found_line - expected_line) <= 5

            return TestResult(
                test_name="symbol_location",
                test_type="objective",
                tool=tool,
                repo=repo,
                question=f"Where is {symbol_name} defined?",
                expected_answer=expected,
                llm_answer=answer,
                is_correct=is_correct,
                score=1.0 if is_correct else 0.0,
                latency_seconds=latency,
                input_tokens=in_tok,
                output_tokens=out_tok,
            )
        except Exception as e:
            return TestResult(
                test_name="symbol_location",
                test_type="objective",
                tool=tool,
                repo=repo,
                question=f"Where is {symbol_name} defined?",
                expected_answer=f"{expected_file}:{expected_line}",
                llm_answer="",
                is_correct=False,
                score=0.0,
                latency_seconds=0,
                input_tokens=0,
                output_tokens=0,
                notes=f"Error: {e}",
            )

    def test_file_count(
        self,
        context: str,
        extension: str,
        expected_count: int,
        tool: str,
        repo: str,
    ) -> TestResult:
        """Test if LLM can count files of a type from context."""

        system = "You are a code analysis assistant. Answer with a number only."
        user = f"""Based on the following codebase context, how many .{extension} files are there?

Answer with just the number, nothing else.

Codebase context:
{context[:100000]}"""

        try:
            answer, latency, in_tok, out_tok = self.query_llm(system, user, max_tokens=50)
            answer = answer.strip()

            # Extract number from answer
            match = re.search(r"(\d+)", answer)
            found_count = int(match.group(1)) if match else -1

            # Allow 20% tolerance
            tolerance = max(2, int(expected_count * 0.2))
            is_correct = abs(found_count - expected_count) <= tolerance

            return TestResult(
                test_name="file_count",
                test_type="objective",
                tool=tool,
                repo=repo,
                question=f"How many .{extension} files?",
                expected_answer=str(expected_count),
                llm_answer=answer,
                is_correct=is_correct,
                score=1.0 if is_correct else 0.0,
                latency_seconds=latency,
                input_tokens=in_tok,
                output_tokens=out_tok,
            )
        except Exception as e:
            return TestResult(
                test_name="file_count",
                test_type="objective",
                tool=tool,
                repo=repo,
                question=f"How many .{extension} files?",
                expected_answer=str(expected_count),
                llm_answer="",
                is_correct=False,
                score=0.0,
                latency_seconds=0,
                input_tokens=0,
                output_tokens=0,
                notes=f"Error: {e}",
            )

    def test_architecture_understanding(
        self,
        context: str,
        tool: str,
        repo: str,
        expected_structure: str,
    ) -> TestResult:
        """Test if LLM understands the project architecture (LLM-graded)."""

        system = "You are a code analysis expert."
        user = f"""Based on the following codebase context, describe the project structure in 2-3 sentences.
Focus on: main entry point, key modules, and how they relate.

Codebase context:
{context[:100000]}"""

        try:
            answer, latency, in_tok, out_tok = self.query_llm(system, user, max_tokens=300)

            # Grade the response using LLM
            grade_system = "You are an evaluator. Score responses 0-10."
            grade_user = f"""Score this architecture description from 0-10.

Expected key points: {expected_structure}

Response to evaluate:
{answer}

Respond with just a number 0-10."""

            grade_response, _, _, _ = self.query_llm(grade_system, grade_user, max_tokens=10)
            match = re.search(r"(\d+)", grade_response)
            score = int(match.group(1)) / 10.0 if match else 0.5

            return TestResult(
                test_name="architecture_understanding",
                test_type="llm_graded",
                tool=tool,
                repo=repo,
                question="Describe the project structure",
                expected_answer=expected_structure,
                llm_answer=answer,
                is_correct=score >= 0.7,
                score=score,
                latency_seconds=latency,
                input_tokens=in_tok,
                output_tokens=out_tok,
            )
        except Exception as e:
            return TestResult(
                test_name="architecture_understanding",
                test_type="llm_graded",
                tool=tool,
                repo=repo,
                question="Describe the project structure",
                expected_answer=expected_structure,
                llm_answer="",
                is_correct=False,
                score=0.0,
                latency_seconds=0,
                input_tokens=0,
                output_tokens=0,
                notes=f"Error: {e}",
            )

    def run_all_tests(self, tool: str, repo: str, context: str) -> list[TestResult]:
        """Run all quality tests for a tool/repo combination."""
        results = []

        # Load ground truth
        gt = self.load_ground_truth(repo)

        # Test 1: Symbol location tests
        for symbol_name, info in list(gt.get("symbol_locations", {}).items())[:3]:
            result = self.test_symbol_location(
                context=context,
                symbol_name=symbol_name,
                expected_file=info["file"],
                expected_line=info["line"],
                tool=tool,
                repo=repo,
            )
            results.append(result)
            print(f"    {symbol_name}: {'✓' if result.is_correct else '✗'} ({result.llm_answer})")

        # Test 2: File count test
        for ext, count in list(gt.get("file_counts", {}).items())[:2]:
            result = self.test_file_count(
                context=context,
                extension=ext,
                expected_count=count,
                tool=tool,
                repo=repo,
            )
            results.append(result)
            print(f"    .{ext} files: {'✓' if result.is_correct else '✗'} (expected {count}, got {result.llm_answer})")

        # Test 3: Architecture understanding
        arch = gt.get("architecture", {})
        if arch:
            result = self.test_architecture_understanding(
                context=context,
                tool=tool,
                repo=repo,
                expected_structure=arch.get("structure", ""),
            )
            results.append(result)
            print(f"    Architecture: score={result.score:.2f}")

        return results


def run_evaluation(config: EvalConfig) -> list[TestResult]:
    """Run the full quality evaluation."""
    evaluator = QualityEvaluator()

    if not evaluator.is_available():
        print("ERROR: ANTHROPIC_API_KEY not set. Cannot run LLM-based evaluation.")
        return []

    all_results = []
    config.output_dir.mkdir(exist_ok=True)

    for repo in config.repos:
        repo_path = evaluator.repos_dir / repo
        if not repo_path.exists():
            print(f"Skipping {repo} - not found")
            continue

        print(f"\n{'='*60}")
        print(f"Evaluating: {repo}")
        print(f"{'='*60}")

        for tool in config.tools:
            print(f"\n  Tool: {tool}")

            try:
                # Get tool output
                context = evaluator.get_tool_output(tool, repo_path)
                print(f"    Context size: {len(context):,} chars")

                # Run multiple times for statistical significance
                for run in range(config.runs_per_test):
                    print(f"    Run {run + 1}/{config.runs_per_test}:")
                    results = evaluator.run_all_tests(tool, repo, context)
                    all_results.extend(results)

            except Exception as e:
                print(f"    ERROR: {e}")

    return all_results


def generate_summary(results: list[TestResult]) -> str:
    """Generate markdown summary of results."""
    if not results:
        return "No results to summarize"

    # Group by tool
    by_tool = {}
    for r in results:
        if r.tool not in by_tool:
            by_tool[r.tool] = []
        by_tool[r.tool].append(r)

    lines = [
        "# Quality Evaluation Results\n",
        f"**Total tests**: {len(results)}\n",
        "\n## Summary by Tool\n",
        "| Tool | Avg Score | Correct % | Tests |",
        "|------|-----------|-----------|-------|",
    ]

    for tool, tool_results in sorted(by_tool.items()):
        avg_score = sum(r.score for r in tool_results) / len(tool_results)
        correct_pct = sum(1 for r in tool_results if r.is_correct) / len(tool_results) * 100
        lines.append(f"| {tool} | {avg_score:.2f} | {correct_pct:.1f}% | {len(tool_results)} |")

    # Detailed results by test type
    lines.append("\n## Objective Tests (Symbol Location, File Count)\n")
    lines.append("| Tool | Test | Repo | Correct | Answer |")
    lines.append("|------|------|------|---------|--------|")

    for r in results:
        if r.test_type == "objective":
            status = "✓" if r.is_correct else "✗"
            lines.append(f"| {r.tool} | {r.test_name} | {r.repo} | {status} | {r.llm_answer[:30]} |")

    lines.append("\n## LLM-Graded Tests (Architecture Understanding)\n")
    lines.append("| Tool | Repo | Score |")
    lines.append("|------|------|-------|")

    for r in results:
        if r.test_type == "llm_graded":
            lines.append(f"| {r.tool} | {r.repo} | {r.score:.2f} |")

    return "\n".join(lines)


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Run quality evaluation")
    parser.add_argument("--tools", nargs="+", default=["infiniloom", "repomix", "gitingest"])
    parser.add_argument("--repos", nargs="+", default=["lodash", "fastapi"])
    parser.add_argument("--runs", type=int, default=1, help="Runs per test for statistical significance")
    parser.add_argument("--output", type=Path, default=Path("./quality_results"))

    args = parser.parse_args()

    config = EvalConfig(
        tools=args.tools,
        repos=args.repos,
        runs_per_test=args.runs,
        output_dir=args.output,
    )

    print("Quality Evaluation")
    print("=" * 60)
    print(f"Tools: {', '.join(config.tools)}")
    print(f"Repos: {', '.join(config.repos)}")
    print(f"Runs per test: {config.runs_per_test}")

    results = run_evaluation(config)

    if results:
        # Save raw results
        config.output_dir.mkdir(exist_ok=True)
        results_file = config.output_dir / "results.json"
        with open(results_file, "w") as f:
            json.dump([r.to_dict() for r in results], f, indent=2)
        print(f"\nResults saved to: {results_file}")

        # Generate and save summary
        summary = generate_summary(results)
        summary_file = config.output_dir / "QUALITY_RESULTS.md"
        summary_file.write_text(summary)
        print(f"Summary saved to: {summary_file}")

        print("\n" + summary)

    return 0 if results else 1


if __name__ == "__main__":
    exit(main())
