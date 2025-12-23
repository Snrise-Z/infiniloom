#!/usr/bin/env python3
"""
LLM Effectiveness Evaluation.

Tests how well each tool's output works with Claude and GPT-4.
Measures: code understanding, bug finding, code generation, symbol location.
"""

import json
import os
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

# API clients - imported conditionally
try:
    import anthropic

    ANTHROPIC_AVAILABLE = True
except ImportError:
    ANTHROPIC_AVAILABLE = False

try:
    import openai

    OPENAI_AVAILABLE = True
except ImportError:
    OPENAI_AVAILABLE = False


@dataclass
class EvalResult:
    """Result of a single LLM evaluation."""

    tool: str
    repo: str
    llm: str  # "claude" or "gpt-4"
    test_type: str  # "understanding", "bug_finding", etc.
    prompt: str
    response: str
    score: float  # 0-10
    latency_seconds: float
    input_tokens: int
    output_tokens: int
    notes: str = ""

    def to_dict(self) -> dict:
        return {
            "tool": self.tool,
            "repo": self.repo,
            "llm": self.llm,
            "test_type": self.test_type,
            "prompt": self.prompt[:200] + "..." if len(self.prompt) > 200 else self.prompt,
            "response": self.response[:500] + "..." if len(self.response) > 500 else self.response,
            "score": self.score,
            "latency_seconds": self.latency_seconds,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "notes": self.notes,
        }


class LLMEvaluator:
    """Evaluates tool outputs with LLMs."""

    def __init__(self):
        self.anthropic_client = None
        self.openai_client = None

        # Initialize clients if API keys available
        if ANTHROPIC_AVAILABLE and os.environ.get("ANTHROPIC_API_KEY"):
            self.anthropic_client = anthropic.Anthropic()

        if OPENAI_AVAILABLE and os.environ.get("OPENAI_API_KEY"):
            self.openai_client = openai.OpenAI()

    def available_llms(self) -> list[str]:
        """Get list of available LLMs."""
        available = []
        if self.anthropic_client:
            available.append("claude")
        if self.openai_client:
            available.append("gpt-4")
        return available

    def query_claude(self, system_prompt: str, user_prompt: str) -> tuple[str, float, int, int]:
        """
        Query Claude API.

        Returns:
            Tuple of (response, latency, input_tokens, output_tokens)
        """
        if not self.anthropic_client:
            raise RuntimeError("Anthropic client not initialized")

        start = time.time()
        response = self.anthropic_client.messages.create(
            model="claude-sonnet-4-20250514",
            max_tokens=2048,
            system=system_prompt,
            messages=[{"role": "user", "content": user_prompt}],
        )
        latency = time.time() - start

        return (
            response.content[0].text,
            latency,
            response.usage.input_tokens,
            response.usage.output_tokens,
        )

    def query_gpt4(self, system_prompt: str, user_prompt: str) -> tuple[str, float, int, int]:
        """
        Query GPT-4 API.

        Returns:
            Tuple of (response, latency, input_tokens, output_tokens)
        """
        if not self.openai_client:
            raise RuntimeError("OpenAI client not initialized")

        start = time.time()
        response = self.openai_client.chat.completions.create(
            model="gpt-4-turbo-preview",
            max_tokens=2048,
            messages=[
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt},
            ],
        )
        latency = time.time() - start

        return (
            response.choices[0].message.content,
            latency,
            response.usage.prompt_tokens,
            response.usage.completion_tokens,
        )

    def query(self, llm: str, system_prompt: str, user_prompt: str) -> tuple[str, float, int, int]:
        """Query specified LLM."""
        if llm == "claude":
            return self.query_claude(system_prompt, user_prompt)
        elif llm == "gpt-4":
            return self.query_gpt4(system_prompt, user_prompt)
        else:
            raise ValueError(f"Unknown LLM: {llm}")


# Test prompts for different evaluation types
TEST_PROMPTS = {
    "code_understanding": {
        "system": "You are a code analysis expert. Analyze the provided codebase context and answer questions about its architecture.",
        "user_template": """Based on the following codebase context, explain:
1. What is the main purpose of this project?
2. What are the key components/modules?
3. How do the components interact with each other?

Codebase context:
{context}

Provide a clear, structured explanation.""",
        "grading_criteria": [
            "Correctly identifies project purpose",
            "Lists major components",
            "Explains component interactions",
            "Provides accurate details",
        ],
    },
    "symbol_location": {
        "system": "You are a code navigation expert. Find specific symbols in the provided codebase context.",
        "user_template": """Based on the following codebase context, answer:
1. Where is the main entry point of the application?
2. Find a function that handles {feature_keyword}
3. What file contains {class_name} class/struct?

Codebase context:
{context}

Provide file paths and line numbers where possible.""",
        "grading_criteria": [
            "Correctly locates entry point",
            "Finds relevant function",
            "Identifies correct file for class",
            "Provides accurate line numbers",
        ],
    },
    "bug_finding": {
        "system": "You are a security and code quality expert. Review the provided code for bugs and issues.",
        "user_template": """Review the following codebase context for potential issues:
1. Are there any obvious bugs?
2. Are there any security vulnerabilities?
3. Are there any code quality issues?

Codebase context:
{context}

List any issues found with file locations and explanations.""",
        "grading_criteria": [
            "Identifies real issues (not false positives)",
            "Provides accurate locations",
            "Gives useful explanations",
            "Prioritizes by severity",
        ],
    },
    "code_generation": {
        "system": "You are a software developer. Use the provided codebase context to generate new code.",
        "user_template": """Based on the following codebase context, generate code to:
{task_description}

Match the existing code style and patterns.

Codebase context:
{context}

Provide the complete implementation.""",
        "grading_criteria": [
            "Code compiles/runs without errors",
            "Follows existing patterns",
            "Correctly implements requirements",
            "Integrates with existing code",
        ],
    },
}


def get_tool_output(tool_name: str, repo_path: Path) -> str:
    """Get packed output from a tool."""
    from install_tools import get_pack_command

    cmd = get_pack_command(tool_name, repo_path)
    result = subprocess.run(cmd, capture_output=True, timeout=300)

    if result.returncode != 0:
        raise RuntimeError(f"{tool_name} failed: {result.stderr.decode()}")

    return result.stdout.decode("utf-8", errors="replace")


def score_response(response: str, criteria: list[str], evaluator: LLMEvaluator, llm: str) -> tuple[float, str]:
    """
    Use LLM to score a response based on criteria.

    Returns:
        Tuple of (score 0-10, notes)
    """
    system = "You are an expert evaluator. Score the following response objectively."
    user = f"""Score this response from 0-10 based on these criteria:
{chr(10).join(f"- {c}" for c in criteria)}

Response to evaluate:
{response[:2000]}

Provide:
1. Overall score (0-10)
2. Brief justification

Format: SCORE: X/10
NOTES: Your justification"""

    try:
        eval_response, _, _, _ = evaluator.query(llm, system, user)

        # Parse score
        score = 5.0  # Default
        notes = ""

        for line in eval_response.split("\n"):
            if "SCORE:" in line.upper():
                try:
                    score_str = line.split(":")[1].strip().split("/")[0]
                    score = float(score_str)
                except (IndexError, ValueError):
                    pass
            elif "NOTES:" in line.upper():
                notes = line.split(":", 1)[1].strip() if ":" in line else ""

        return min(10, max(0, score)), notes

    except Exception as e:
        return 5.0, f"Scoring failed: {e}"


def run_evaluation(
    tool_name: str,
    repo_path: Path,
    repo_name: str,
    evaluator: LLMEvaluator,
    test_types: Optional[list[str]] = None,
    llms: Optional[list[str]] = None,
) -> list[EvalResult]:
    """
    Run LLM effectiveness evaluation for a tool on a repo.

    Returns:
        List of evaluation results
    """
    results = []

    # Get tool output
    try:
        context = get_tool_output(tool_name, repo_path)
    except Exception as e:
        print(f"  Failed to get {tool_name} output: {e}")
        return results

    # Determine which tests and LLMs to use
    test_types = test_types or ["code_understanding", "symbol_location"]
    llms = llms or evaluator.available_llms()

    for test_type in test_types:
        if test_type not in TEST_PROMPTS:
            continue

        test_config = TEST_PROMPTS[test_type]

        for llm in llms:
            print(f"  Running {test_type} with {llm}...")

            # Format prompt
            user_prompt = test_config["user_template"].format(
                context=context[:50000],  # Limit context size
                feature_keyword="main",
                class_name="Config",
                task_description="Add a new helper function",
            )

            try:
                response, latency, input_tokens, output_tokens = evaluator.query(
                    llm, test_config["system"], user_prompt
                )

                # Score the response
                score, notes = score_response(
                    response, test_config["grading_criteria"], evaluator, llm
                )

                results.append(
                    EvalResult(
                        tool=tool_name,
                        repo=repo_name,
                        llm=llm,
                        test_type=test_type,
                        prompt=user_prompt,
                        response=response,
                        score=score,
                        latency_seconds=latency,
                        input_tokens=input_tokens,
                        output_tokens=output_tokens,
                        notes=notes,
                    )
                )

            except Exception as e:
                print(f"    Error: {e}")
                results.append(
                    EvalResult(
                        tool=tool_name,
                        repo=repo_name,
                        llm=llm,
                        test_type=test_type,
                        prompt=user_prompt,
                        response="",
                        score=0,
                        latency_seconds=0,
                        input_tokens=0,
                        output_tokens=0,
                        notes=f"Error: {e}",
                    )
                )

    return results


def save_eval_results(results: list[EvalResult], output_path: Path):
    """Save evaluation results to JSON."""
    data = [r.to_dict() for r in results]
    with open(output_path, "w") as f:
        json.dump(data, f, indent=2)


def print_eval_summary(results: list[EvalResult]):
    """Print summary of evaluation results."""
    if not results:
        print("No results to summarize")
        return

    # Group by tool
    by_tool = {}
    for r in results:
        if r.tool not in by_tool:
            by_tool[r.tool] = []
        by_tool[r.tool].append(r)

    print("\n" + "=" * 60)
    print("LLM EFFECTIVENESS EVALUATION SUMMARY")
    print("=" * 60)

    for tool, tool_results in sorted(by_tool.items()):
        scores = [r.score for r in tool_results if r.score > 0]
        avg_score = sum(scores) / len(scores) if scores else 0

        print(f"\n{tool}:")
        print(f"  Average Score: {avg_score:.1f}/10")
        print(f"  Tests Run: {len(tool_results)}")

        # By LLM
        for llm in ["claude", "gpt-4"]:
            llm_results = [r for r in tool_results if r.llm == llm]
            if llm_results:
                llm_scores = [r.score for r in llm_results if r.score > 0]
                llm_avg = sum(llm_scores) / len(llm_scores) if llm_scores else 0
                print(f"    {llm}: {llm_avg:.1f}/10")


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Run LLM effectiveness evaluation")
    parser.add_argument(
        "--tools",
        nargs="+",
        default=["infiniloom", "repomix", "gitingest"],
        help="Tools to evaluate",
    )
    parser.add_argument(
        "--repo",
        required=True,
        help="Repository name to test on",
    )
    parser.add_argument(
        "--tests",
        nargs="+",
        choices=list(TEST_PROMPTS.keys()),
        help="Test types to run",
    )
    parser.add_argument(
        "--llms",
        nargs="+",
        choices=["claude", "gpt-4"],
        help="LLMs to use",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("./llm_eval_results.json"),
        help="Output file for results",
    )

    args = parser.parse_args()

    # Initialize evaluator
    evaluator = LLMEvaluator()
    available = evaluator.available_llms()

    if not available:
        print("No LLM APIs available. Set ANTHROPIC_API_KEY and/or OPENAI_API_KEY")
        return 1

    print(f"Available LLMs: {', '.join(available)}")

    # Get repo
    from repos import get_repo

    repo = get_repo(args.repo)
    if not repo:
        print(f"Unknown repo: {args.repo}")
        return 1

    if not repo.local_path.exists():
        print(f"Repo not cloned. Run: python repos.py clone --repo {args.repo}")
        return 1

    # Run evaluation
    all_results = []
    for tool in args.tools:
        print(f"\nEvaluating {tool}...")
        results = run_evaluation(
            tool_name=tool,
            repo_path=repo.local_path,
            repo_name=args.repo,
            evaluator=evaluator,
            test_types=args.tests,
            llms=args.llms or available,
        )
        all_results.extend(results)

    # Save and summarize
    save_eval_results(all_results, args.output)
    print(f"\nResults saved to: {args.output}")

    print_eval_summary(all_results)

    return 0


if __name__ == "__main__":
    exit(main())
