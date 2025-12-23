#!/usr/bin/env python3
"""
LLM-as-Judge Evaluation Framework for Infiniloom vs Repomix

Evaluates which tool produces better context for LLM code understanding tasks.
"""

import subprocess
import json
import time
import os
import re
import hashlib
from pathlib import Path
from dataclasses import dataclass, asdict, field
from typing import Optional, List, Dict, Tuple
from datetime import datetime
import random

# Anthropic API for Claude evaluation
try:
    import anthropic
    HAS_ANTHROPIC = True
except ImportError:
    HAS_ANTHROPIC = False

# Check for Bedrock configuration
USE_BEDROCK = os.environ.get('CLAUDE_CODE_USE_BEDROCK') == '1'
BEDROCK_BEARER_TOKEN = os.environ.get('AWS_BEARER_TOKEN_BEDROCK')

def call_bedrock_api(prompt: str, max_tokens: int = 2000, max_retries: int = 3) -> dict:
    """Call Bedrock API directly with bearer token, with retry logic for rate limits"""
    import httpx

    region = os.environ.get('AWS_REGION', 'us-east-1')
    model_id = 'us.anthropic.claude-sonnet-4-20250514-v1:0'
    url = f'https://bedrock-runtime.{region}.amazonaws.com/model/{model_id}/invoke'

    headers = {
        'Authorization': f'Bearer {BEDROCK_BEARER_TOKEN}',
        'Content-Type': 'application/json',
    }

    payload = {
        'anthropic_version': 'bedrock-2023-05-31',
        'max_tokens': max_tokens,
        'messages': [{'role': 'user', 'content': prompt}]
    }

    for attempt in range(max_retries):
        try:
            response = httpx.post(url, headers=headers, json=payload, timeout=120)
            response.raise_for_status()
            return response.json()
        except httpx.HTTPStatusError as e:
            if e.response.status_code == 429 and attempt < max_retries - 1:
                # Rate limit - wait and retry with exponential backoff
                wait_time = (2 ** attempt) * 5  # 5s, 10s, 20s
                print(f"      Rate limited, waiting {wait_time}s...")
                time.sleep(wait_time)
            else:
                raise

    # Should never reach here
    raise Exception("Max retries exceeded")

from config import (
    PROJECT_ROOT,
    REPOS_DIR,
    OUTPUTS_DIR,
    REPORTS_DIR,
    INFINILOOM_BIN,
    REPOMIX_BIN,
    ensure_dirs,
)

# Extended test repositories for comprehensive evaluation
EVAL_REPOSITORIES = [
    # Small repos (< 100 files) - Quick understanding tasks
    {
        "name": "httpie",
        "url": "https://github.com/httpie/cli",
        "branch": "master",
        "category": "cli-tool",
        "languages": ["python"],
        "complexity": "simple",
    },
    {
        "name": "ripgrep",
        "url": "https://github.com/BurntSushi/ripgrep",
        "branch": "master",
        "category": "cli-tool",
        "languages": ["rust"],
        "complexity": "medium",
    },
    {
        "name": "jq",
        "url": "https://github.com/jqlang/jq",
        "branch": "master",
        "category": "cli-tool",
        "languages": ["c"],
        "complexity": "medium",
    },
    # Medium repos (100-500 files) - Architecture understanding
    {
        "name": "fastapi",
        "url": "https://github.com/tiangolo/fastapi",
        "branch": "master",
        "category": "web-framework",
        "languages": ["python"],
        "complexity": "medium",
    },
    {
        "name": "express",
        "url": "https://github.com/expressjs/express",
        "branch": "master",
        "category": "web-framework",
        "languages": ["javascript"],
        "complexity": "medium",
    },
    {
        "name": "gin",
        "url": "https://github.com/gin-gonic/gin",
        "branch": "master",
        "category": "web-framework",
        "languages": ["go"],
        "complexity": "medium",
    },
    {
        "name": "actix-web",
        "url": "https://github.com/actix/actix-web",
        "branch": "master",
        "category": "web-framework",
        "languages": ["rust"],
        "complexity": "medium",
    },
    # Libraries - API understanding
    {
        "name": "lodash",
        "url": "https://github.com/lodash/lodash",
        "branch": "main",
        "category": "utility-library",
        "languages": ["javascript"],
        "complexity": "medium",
    },
    {
        "name": "requests",
        "url": "https://github.com/psf/requests",
        "branch": "main",
        "category": "http-library",
        "languages": ["python"],
        "complexity": "simple",
    },
    {
        "name": "axios",
        "url": "https://github.com/axios/axios",
        "branch": "v1.x",
        "category": "http-library",
        "languages": ["javascript", "typescript"],
        "complexity": "medium",
    },
    # UI/Frontend - Component understanding
    {
        "name": "excalidraw",
        "url": "https://github.com/excalidraw/excalidraw",
        "branch": "master",
        "category": "frontend-app",
        "languages": ["typescript", "react"],
        "complexity": "complex",
    },
    {
        "name": "shadcn-ui",
        "url": "https://github.com/shadcn-ui/ui",
        "branch": "main",
        "category": "ui-components",
        "languages": ["typescript", "react"],
        "complexity": "medium",
    },
    # Complex multi-language repos
    {
        "name": "deno",
        "url": "https://github.com/denoland/deno",
        "branch": "main",
        "category": "runtime",
        "languages": ["rust", "typescript"],
        "complexity": "complex",
    },
    {
        "name": "lazygit",
        "url": "https://github.com/jesseduffield/lazygit",
        "branch": "master",
        "category": "tui-app",
        "languages": ["go"],
        "complexity": "medium",
    },
    # Database/Storage
    {
        "name": "redis-py",
        "url": "https://github.com/redis/redis-py",
        "branch": "master",
        "category": "database-client",
        "languages": ["python"],
        "complexity": "medium",
    },
    # DevOps/Infrastructure
    {
        "name": "localstack",
        "url": "https://github.com/localstack/localstack",
        "branch": "master",
        "category": "devops",
        "languages": ["python"],
        "complexity": "complex",
    },
]


@dataclass
class EvalTask:
    """A single evaluation task for the LLM"""
    task_type: str  # "architecture", "function_find", "bug_hunt", "api_usage", "dependency"
    question: str
    context_needed: str  # What the LLM needs to answer correctly
    difficulty: str  # "easy", "medium", "hard"


@dataclass
class EvalResult:
    """Result of a single evaluation"""
    repo_name: str
    tool: str
    task_type: str
    question: str
    answer: str
    score: float  # 0-10
    reasoning: str
    tokens_used: int
    time_ms: float


@dataclass
class RepoEvalResults:
    """All evaluation results for a repository"""
    repo_name: str
    infiniloom_results: List[EvalResult] = field(default_factory=list)
    repomix_results: List[EvalResult] = field(default_factory=list)
    infiniloom_avg_score: float = 0.0
    repomix_avg_score: float = 0.0
    winner: str = ""


# Evaluation tasks for different aspects
EVAL_TASKS = {
    "architecture": [
        EvalTask(
            task_type="architecture",
            question="Describe the high-level architecture of this codebase. What are the main modules/components and how do they interact?",
            context_needed="Module structure, entry points, data flow",
            difficulty="medium",
        ),
        EvalTask(
            task_type="architecture",
            question="What design patterns are used in this codebase? Give specific examples with file references.",
            context_needed="Code patterns, class hierarchies, abstractions",
            difficulty="hard",
        ),
    ],
    "function_find": [
        EvalTask(
            task_type="function_find",
            question="Find the main entry point function(s) of this application. What do they do?",
            context_needed="Entry points, main functions, initialization",
            difficulty="easy",
        ),
        EvalTask(
            task_type="function_find",
            question="Find where error handling is implemented. How are errors propagated through the system?",
            context_needed="Error types, exception handling, error propagation",
            difficulty="medium",
        ),
    ],
    "api_usage": [
        EvalTask(
            task_type="api_usage",
            question="How would I use the main API/functionality of this library? Provide a code example.",
            context_needed="Public API, usage patterns, examples",
            difficulty="easy",
        ),
        EvalTask(
            task_type="api_usage",
            question="What configuration options are available? How do I customize the behavior?",
            context_needed="Configuration, options, defaults",
            difficulty="medium",
        ),
    ],
    "dependency": [
        EvalTask(
            task_type="dependency",
            question="What are the key external dependencies of this project? What do they provide?",
            context_needed="Dependencies, imports, external libraries",
            difficulty="easy",
        ),
        EvalTask(
            task_type="dependency",
            question="Trace the data flow from user input to output. What modules does data pass through?",
            context_needed="Data flow, function calls, module interactions",
            difficulty="hard",
        ),
    ],
    "code_quality": [
        EvalTask(
            task_type="code_quality",
            question="Identify potential code quality issues or areas for improvement in this codebase.",
            context_needed="Code patterns, complexity, maintainability",
            difficulty="hard",
        ),
    ],
}


def clone_repo(repo: dict) -> Tuple[bool, Path]:
    """Clone a repository if not present"""
    repo_path = REPOS_DIR / repo["name"]

    if repo_path.exists():
        return True, repo_path

    print(f"  Cloning {repo['name']}...")
    cmd = ["git", "clone", "--depth", "1", "--branch", repo["branch"], repo["url"], str(repo_path)]

    try:
        result = subprocess.run(cmd, capture_output=True, timeout=300, text=True)
        return result.returncode == 0, repo_path
    except Exception as e:
        print(f"  Error cloning: {e}")
        return False, repo_path


def generate_infiniloom_output(repo_path: Path, output_path: Path, max_tokens: int = 100000) -> Tuple[bool, float]:
    """Generate Infiniloom output for a repository"""
    start = time.time()

    try:
        result = subprocess.run(
            [
                str(INFINILOOM_BIN),
                "pack",
                str(repo_path),
                "-o", str(output_path),
                "-f", "xml",
                "-t", str(max_tokens),
                "-c", "balanced",  # Use balanced compression for fair comparison
            ],
            capture_output=True,
            timeout=300,
            text=True,
        )
        elapsed = (time.time() - start) * 1000
        return result.returncode == 0, elapsed
    except Exception as e:
        return False, 0


def generate_repomix_output(repo_path: Path, output_path: Path) -> Tuple[bool, float]:
    """Generate Repomix output for a repository"""
    start = time.time()

    try:
        result = subprocess.run(
            [
                REPOMIX_BIN,
                str(repo_path),
                "-o", str(output_path),
                "--style", "xml",
            ],
            capture_output=True,
            timeout=600,
            text=True,
            cwd=str(repo_path),
        )
        elapsed = (time.time() - start) * 1000
        return result.returncode == 0, elapsed
    except Exception as e:
        return False, 0


def truncate_content(content: str, max_chars: int = 180000) -> str:
    """Truncate content to fit within context limits"""
    if len(content) <= max_chars:
        return content

    # Try to truncate at a reasonable boundary
    truncated = content[:max_chars]
    last_file_end = truncated.rfind("</file>")
    if last_file_end > max_chars * 0.8:
        return truncated[:last_file_end + 7] + "\n<!-- Content truncated -->"
    return truncated + "\n<!-- Content truncated -->"


def evaluate_with_claude(
    context: str,
    task: EvalTask,
    repo_name: str,
    tool_name: str,
) -> EvalResult:
    """Use Claude to evaluate the context quality for a task"""

    if not HAS_ANTHROPIC:
        # Return mock result if no API
        return EvalResult(
            repo_name=repo_name,
            tool=tool_name,
            task_type=task.task_type,
            question=task.question,
            answer="[API not available]",
            score=5.0,
            reasoning="Mock evaluation - Anthropic API not configured",
            tokens_used=0,
            time_ms=0,
        )

    # Truncate context to fit
    context = truncate_content(context)

    eval_prompt = f"""You are evaluating the quality of repository context provided to an LLM for code understanding tasks.

<repository_context>
{context}
</repository_context>

<task>
{task.question}
</task>

First, answer the task question based on the provided context.

Then, evaluate how well the context helped you answer:
- Score 1-3: Context was insufficient, missing key information
- Score 4-6: Context was adequate but could be better organized
- Score 7-8: Context was good, well-organized with relevant information
- Score 9-10: Context was excellent, perfectly organized for this task

Respond in this JSON format:
{{
    "answer": "Your detailed answer to the task question",
    "score": <number 1-10>,
    "reasoning": "Why you gave this score - what was helpful or missing"
}}"""

    start = time.time()

    try:
        # Use Bedrock bearer token API if configured, otherwise standard Anthropic
        if USE_BEDROCK and BEDROCK_BEARER_TOKEN:
            result = call_bedrock_api(eval_prompt, max_tokens=2000)
            response_text = result['content'][0]['text']
            tokens_used = result.get('usage', {}).get('input_tokens', 0) + result.get('usage', {}).get('output_tokens', 0)
        else:
            client = anthropic.Anthropic()
            response = client.messages.create(
                model="claude-sonnet-4-20250514",
                max_tokens=2000,
                messages=[{"role": "user", "content": eval_prompt}]
            )
            response_text = response.content[0].text
            tokens_used = response.usage.input_tokens + response.usage.output_tokens

        elapsed = (time.time() - start) * 1000

        # Parse JSON response
        try:
            # Find JSON in response
            json_match = re.search(r'\{[^{}]*"answer"[^{}]*"score"[^{}]*"reasoning"[^{}]*\}', response_text, re.DOTALL)
            if json_match:
                result_json = json.loads(json_match.group())
            else:
                # Try parsing entire response as JSON
                result_json = json.loads(response_text)

            return EvalResult(
                repo_name=repo_name,
                tool=tool_name,
                task_type=task.task_type,
                question=task.question,
                answer=result_json.get("answer", ""),
                score=float(result_json.get("score", 5)),
                reasoning=result_json.get("reasoning", ""),
                tokens_used=tokens_used,
                time_ms=elapsed,
            )
        except json.JSONDecodeError:
            return EvalResult(
                repo_name=repo_name,
                tool=tool_name,
                task_type=task.task_type,
                question=task.question,
                answer=response_text,
                score=5.0,
                reasoning="Failed to parse structured response",
                tokens_used=tokens_used,
                time_ms=elapsed,
            )

    except Exception as e:
        return EvalResult(
            repo_name=repo_name,
            tool=tool_name,
            task_type=task.task_type,
            question=task.question,
            answer=f"Error: {e}",
            score=0,
            reasoning=str(e),
            tokens_used=0,
            time_ms=0,
        )


def run_evaluation(repos: List[dict], tasks_per_repo: int = 3) -> List[RepoEvalResults]:
    """Run full evaluation across repositories"""

    ensure_dirs()
    all_results = []

    # Flatten all tasks
    all_tasks = []
    for task_list in EVAL_TASKS.values():
        all_tasks.extend(task_list)

    for repo in repos:
        print(f"\n{'='*60}")
        print(f"Evaluating: {repo['name']}")
        print(f"{'='*60}")

        # Clone repo
        success, repo_path = clone_repo(repo)
        if not success:
            print(f"  Failed to clone {repo['name']}, skipping")
            continue

        # Generate outputs
        infiniloom_output = OUTPUTS_DIR / f"{repo['name']}_infiniloom_eval.xml"
        repomix_output = OUTPUTS_DIR / f"{repo['name']}_repomix_eval.xml"

        print(f"  Generating Infiniloom output...")
        cl_success, cl_time = generate_infiniloom_output(repo_path, infiniloom_output)
        if cl_success:
            print(f"    Done in {cl_time:.0f}ms")
        else:
            print(f"    Failed!")

        print(f"  Generating Repomix output...")
        rm_success, rm_time = generate_repomix_output(repo_path, repomix_output)
        if rm_success:
            print(f"    Done in {rm_time:.0f}ms")
        else:
            print(f"    Failed!")

        if not cl_success or not rm_success:
            continue

        # Load outputs
        try:
            with open(infiniloom_output, "r") as f:
                infiniloom_context = f.read()
            with open(repomix_output, "r") as f:
                repomix_context = f.read()
        except Exception as e:
            print(f"  Error loading outputs: {e}")
            continue

        # Select random tasks for this repo
        selected_tasks = random.sample(all_tasks, min(tasks_per_repo, len(all_tasks)))

        repo_results = RepoEvalResults(repo_name=repo["name"])

        for task in selected_tasks:
            print(f"\n  Task: {task.task_type} ({task.difficulty})")
            print(f"  Question: {task.question[:80]}...")

            # Evaluate with Infiniloom context
            print(f"    Evaluating Infiniloom...")
            cl_result = evaluate_with_claude(infiniloom_context, task, repo["name"], "infiniloom")
            repo_results.infiniloom_results.append(cl_result)
            print(f"    Score: {cl_result.score}/10")

            # Small delay to avoid rate limiting
            time.sleep(2)

            # Evaluate with Repomix context
            print(f"    Evaluating Repomix...")
            rm_result = evaluate_with_claude(repomix_context, task, repo["name"], "repomix")
            repo_results.repomix_results.append(rm_result)
            print(f"    Score: {rm_result.score}/10")

            # Small delay between tasks
            time.sleep(2)

        # Calculate averages
        if repo_results.infiniloom_results:
            repo_results.infiniloom_avg_score = sum(r.score for r in repo_results.infiniloom_results) / len(repo_results.infiniloom_results)
        if repo_results.repomix_results:
            repo_results.repomix_avg_score = sum(r.score for r in repo_results.repomix_results) / len(repo_results.repomix_results)

        if repo_results.infiniloom_avg_score > repo_results.repomix_avg_score:
            repo_results.winner = "infiniloom"
        elif repo_results.repomix_avg_score > repo_results.infiniloom_avg_score:
            repo_results.winner = "repomix"
        else:
            repo_results.winner = "tie"

        print(f"\n  Results for {repo['name']}:")
        print(f"    Infiniloom avg: {repo_results.infiniloom_avg_score:.1f}/10")
        print(f"    Repomix avg: {repo_results.repomix_avg_score:.1f}/10")
        print(f"    Winner: {repo_results.winner}")

        all_results.append(repo_results)

    return all_results


def generate_report(results: List[RepoEvalResults]) -> str:
    """Generate a comprehensive evaluation report"""

    report = []
    report.append("# LLM Context Quality Evaluation: Infiniloom vs Repomix")
    report.append(f"\nGenerated: {datetime.now().isoformat()}\n")

    # Overall summary
    report.append("## Overall Summary\n")

    total_cl_score = sum(r.infiniloom_avg_score for r in results)
    total_rm_score = sum(r.repomix_avg_score for r in results)
    cl_wins = sum(1 for r in results if r.winner == "infiniloom")
    rm_wins = sum(1 for r in results if r.winner == "repomix")
    ties = sum(1 for r in results if r.winner == "tie")

    report.append(f"- **Repositories evaluated:** {len(results)}")
    report.append(f"- **Infiniloom wins:** {cl_wins}")
    report.append(f"- **Repomix wins:** {rm_wins}")
    report.append(f"- **Ties:** {ties}")
    report.append(f"- **Infiniloom average score:** {total_cl_score/len(results):.2f}/10")
    report.append(f"- **Repomix average score:** {total_rm_score/len(results):.2f}/10")

    # Summary table
    report.append("\n## Results by Repository\n")
    report.append("| Repository | Infiniloom | Repomix | Winner |")
    report.append("|------------|----------|---------|--------|")

    for r in results:
        winner_mark = "**" if r.winner != "tie" else ""
        cl_score = f"{winner_mark}{r.infiniloom_avg_score:.1f}{winner_mark}" if r.winner == "infiniloom" else f"{r.infiniloom_avg_score:.1f}"
        rm_score = f"{winner_mark}{r.repomix_avg_score:.1f}{winner_mark}" if r.winner == "repomix" else f"{r.repomix_avg_score:.1f}"
        report.append(f"| {r.repo_name} | {cl_score} | {rm_score} | {r.winner} |")

    # Results by task type
    report.append("\n## Results by Task Type\n")

    task_scores = {}
    for r in results:
        for cl_res in r.infiniloom_results:
            if cl_res.task_type not in task_scores:
                task_scores[cl_res.task_type] = {"infiniloom": [], "repomix": []}
            task_scores[cl_res.task_type]["infiniloom"].append(cl_res.score)
        for rm_res in r.repomix_results:
            if rm_res.task_type not in task_scores:
                task_scores[rm_res.task_type] = {"infiniloom": [], "repomix": []}
            task_scores[rm_res.task_type]["repomix"].append(rm_res.score)

    report.append("| Task Type | Infiniloom Avg | Repomix Avg | Better |")
    report.append("|-----------|--------------|-------------|--------|")

    for task_type, scores in task_scores.items():
        cl_avg = sum(scores["infiniloom"]) / len(scores["infiniloom"]) if scores["infiniloom"] else 0
        rm_avg = sum(scores["repomix"]) / len(scores["repomix"]) if scores["repomix"] else 0
        better = "Infiniloom" if cl_avg > rm_avg else ("Repomix" if rm_avg > cl_avg else "Tie")
        report.append(f"| {task_type} | {cl_avg:.1f} | {rm_avg:.1f} | {better} |")

    # Detailed results
    report.append("\n## Detailed Results\n")

    for r in results:
        report.append(f"### {r.repo_name}\n")

        for i, (cl_res, rm_res) in enumerate(zip(r.infiniloom_results, r.repomix_results)):
            report.append(f"#### Task {i+1}: {cl_res.task_type}")
            report.append(f"**Question:** {cl_res.question}\n")

            report.append("**Infiniloom Response:**")
            report.append(f"- Score: {cl_res.score}/10")
            report.append(f"- Reasoning: {cl_res.reasoning}")
            report.append(f"- Answer excerpt: {cl_res.answer[:500]}...\n")

            report.append("**Repomix Response:**")
            report.append(f"- Score: {rm_res.score}/10")
            report.append(f"- Reasoning: {rm_res.reasoning}")
            report.append(f"- Answer excerpt: {rm_res.answer[:500]}...\n")

    return "\n".join(report)


def main():
    """Main evaluation runner"""
    print("=" * 60)
    print("LLM Context Quality Evaluation")
    print("Infiniloom vs Repomix")
    print("=" * 60)

    if not HAS_ANTHROPIC:
        print("\nWARNING: anthropic package not installed.")
        print("Install with: pip install anthropic")
        print("Set ANTHROPIC_API_KEY environment variable")
        print("\nRunning in mock mode...\n")

    # Run evaluation on subset of repos for testing
    test_repos = EVAL_REPOSITORIES[:6]  # Start with 6 repos

    results = run_evaluation(test_repos, tasks_per_repo=3)

    # Generate report
    report = generate_report(results)

    report_path = REPORTS_DIR / f"llm_eval_{datetime.now().strftime('%Y%m%d_%H%M%S')}.md"
    with open(report_path, "w") as f:
        f.write(report)

    print(f"\n{'='*60}")
    print(f"Report saved to: {report_path}")
    print(f"{'='*60}")

    # Print summary
    print("\n" + "=" * 60)
    print("FINAL SUMMARY")
    print("=" * 60)

    cl_wins = sum(1 for r in results if r.winner == "infiniloom")
    rm_wins = sum(1 for r in results if r.winner == "repomix")

    print(f"\nInfiniloom wins: {cl_wins}/{len(results)}")
    print(f"Repomix wins: {rm_wins}/{len(results)}")

    if results:
        avg_cl = sum(r.infiniloom_avg_score for r in results) / len(results)
        avg_rm = sum(r.repomix_avg_score for r in results) / len(results)
        print(f"\nOverall Infiniloom avg: {avg_cl:.2f}/10")
        print(f"Overall Repomix avg: {avg_rm:.2f}/10")


if __name__ == "__main__":
    main()
