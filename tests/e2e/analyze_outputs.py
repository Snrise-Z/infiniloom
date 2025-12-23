#!/usr/bin/env python3
"""
Analyze and compare Infiniloom vs Repomix outputs for LLM effectiveness.

This script performs structural analysis without requiring API calls.
"""

import os
import re
import json
from pathlib import Path
from dataclasses import dataclass, field
from typing import Dict, List, Tuple
from collections import Counter
import xml.etree.ElementTree as ET

from config import OUTPUTS_DIR, REPORTS_DIR, ensure_dirs

@dataclass
class OutputAnalysis:
    """Analysis of a single output file"""
    tool: str
    repo: str
    file_size_bytes: int = 0
    file_count: int = 0
    token_count: int = 0

    # Structure metrics
    has_metadata: bool = False
    has_file_list: bool = False
    has_directory_structure: bool = False
    has_summary: bool = False
    has_symbol_index: bool = False
    has_dependency_info: bool = False

    # Content quality metrics
    files_with_line_numbers: int = 0
    files_with_language_tags: int = 0
    files_with_token_counts: int = 0

    # Organization metrics
    uses_cdata: bool = False
    has_clear_file_boundaries: bool = False
    consistent_formatting: bool = False

    # Languages detected
    languages: Dict[str, int] = field(default_factory=dict)

    # Sections found
    sections: List[str] = field(default_factory=list)


def analyze_infiniloom_output(filepath: Path) -> OutputAnalysis:
    """Analyze a Infiniloom XML output file"""
    analysis = OutputAnalysis(tool="infiniloom", repo=filepath.stem.replace("_infiniloom", "").replace("_eval", ""))

    if not filepath.exists():
        return analysis

    analysis.file_size_bytes = filepath.stat().st_size

    try:
        content = filepath.read_text()

        # Check for CDATA usage
        analysis.uses_cdata = "CDATA" in content

        # Check for metadata section
        analysis.has_metadata = "<metadata>" in content or "<stats>" in content

        # Check for summary
        analysis.has_summary = "<summary>" in content or "<repository_map>" in content

        # Check for symbol index
        analysis.has_symbol_index = "<key_symbols>" in content or "<symbol " in content

        # Check for dependency info (allow attributes in tag)
        analysis.has_dependency_info = "<dependencies" in content or "import" in content.lower()[:5000]

        # Check for directory structure
        analysis.has_directory_structure = "<directory_structure>" in content or "<structure>" in content

        # Count files
        file_matches = re.findall(r'<file\s+path="([^"]+)"', content)
        analysis.file_count = len(set(file_matches))
        analysis.has_file_list = analysis.file_count > 0

        # Count files with line numbers
        analysis.files_with_line_numbers = len(re.findall(r'line_numbers="true"', content))

        # Count files with language tags
        analysis.files_with_language_tags = len(re.findall(r'language="(\w+)"', content))

        # Count files with token counts
        analysis.files_with_token_counts = len(re.findall(r'tokens="(\d+)"', content))

        # Extract token count from metadata
        token_match = re.search(r'<tokens[^>]*>(\d+)</tokens>', content)
        if token_match:
            analysis.token_count = int(token_match.group(1))

        # Extract languages
        for lang_match in re.finditer(r'<language name="(\w+)" files="(\d+)"', content):
            analysis.languages[lang_match.group(1)] = int(lang_match.group(2))

        # Check formatting consistency
        analysis.has_clear_file_boundaries = "</file>" in content
        analysis.consistent_formatting = (
            content.count("<file ") == content.count("</file>")
        )

        # Identify sections
        if "<metadata>" in content: analysis.sections.append("metadata")
        if "<repository_map>" in content: analysis.sections.append("repository_map")
        if "<key_symbols>" in content: analysis.sections.append("symbols")
        if "<files>" in content: analysis.sections.append("files")
        if "<directory_structure>" in content: analysis.sections.append("directory")

    except Exception as e:
        print(f"Error analyzing {filepath}: {e}")

    return analysis


def analyze_repomix_output(filepath: Path) -> OutputAnalysis:
    """Analyze a Repomix XML output file"""
    analysis = OutputAnalysis(tool="repomix", repo=filepath.stem.replace("_repomix", "").replace("_eval", ""))

    if not filepath.exists():
        return analysis

    analysis.file_size_bytes = filepath.stat().st_size

    try:
        content = filepath.read_text()

        # Check for CDATA usage
        analysis.uses_cdata = "CDATA" in content

        # Check for file summary section
        analysis.has_summary = "<file_summary>" in content
        analysis.has_metadata = "<file_summary>" in content

        # Check for directory structure
        analysis.has_directory_structure = "<directory_structure>" in content

        # Check for file list (repomix uses simple <file path="..."> tags)
        file_matches = re.findall(r'<file path="([^"]+)"', content)
        analysis.file_count = len(set(file_matches))
        analysis.has_file_list = analysis.file_count > 0

        # Repomix doesn't typically include line numbers or per-file tokens
        analysis.files_with_line_numbers = 0
        analysis.files_with_language_tags = 0
        analysis.files_with_token_counts = 0

        # Check formatting
        analysis.has_clear_file_boundaries = "</file>" in content
        analysis.consistent_formatting = True  # Repomix is generally consistent

        # Identify sections
        if "<file_summary>" in content: analysis.sections.append("summary")
        if "<directory_structure>" in content: analysis.sections.append("directory")
        if "<files>" in content or '<file path=' in content: analysis.sections.append("files")

    except Exception as e:
        print(f"Error analyzing {filepath}: {e}")

    return analysis


def calculate_llm_effectiveness_score(analysis: OutputAnalysis) -> Dict[str, float]:
    """
    Calculate an LLM effectiveness score based on best practices.

    Scoring criteria based on LLM context best practices:
    1. Structure & Navigation (30 points)
       - Clear metadata/summary for quick understanding
       - Directory structure for navigation
       - Symbol index for code navigation

    2. Content Quality (30 points)
       - Line numbers for precise references
       - Language tags for syntax understanding
       - Token counts for budget awareness

    3. Organization (20 points)
       - CDATA for proper escaping
       - Clear file boundaries
       - Consistent formatting

    4. Completeness (20 points)
       - File coverage
       - Dependency information
    """

    scores = {}

    # Structure & Navigation (30 points)
    structure_score = 0
    if analysis.has_metadata: structure_score += 8
    if analysis.has_summary: structure_score += 8
    if analysis.has_directory_structure: structure_score += 7
    if analysis.has_symbol_index: structure_score += 7
    scores["structure"] = structure_score

    # Content Quality (30 points)
    content_score = 0
    if analysis.files_with_line_numbers > 0:
        content_score += min(10, analysis.files_with_line_numbers / max(analysis.file_count, 1) * 10)
    if analysis.files_with_language_tags > 0:
        content_score += min(10, analysis.files_with_language_tags / max(analysis.file_count, 1) * 10)
    if analysis.files_with_token_counts > 0:
        content_score += min(10, analysis.files_with_token_counts / max(analysis.file_count, 1) * 10)
    scores["content_quality"] = content_score

    # Organization (20 points)
    org_score = 0
    if analysis.uses_cdata: org_score += 7
    if analysis.has_clear_file_boundaries: org_score += 7
    if analysis.consistent_formatting: org_score += 6
    scores["organization"] = org_score

    # Completeness (20 points)
    completeness_score = 0
    if analysis.file_count > 0: completeness_score += 10
    if analysis.has_dependency_info: completeness_score += 5
    if len(analysis.languages) > 0: completeness_score += 5
    scores["completeness"] = completeness_score

    scores["total"] = sum(scores.values())

    return scores


def compare_outputs() -> str:
    """Compare all Infiniloom and Repomix outputs"""

    ensure_dirs()

    report = []
    report.append("# Infiniloom vs Repomix: LLM Effectiveness Analysis")
    report.append(f"\nThis analysis evaluates output files based on LLM context best practices.\n")

    # Find all output pairs
    infiniloom_files = list(OUTPUTS_DIR.glob("*_infiniloom*.xml"))
    repomix_files = list(OUTPUTS_DIR.glob("*_repomix*.xml"))

    # Match pairs by repo name
    repos = set()
    for f in infiniloom_files:
        repo = f.stem.replace("_infiniloom", "").replace("_eval", "")
        repos.add(repo)
    for f in repomix_files:
        repo = f.stem.replace("_repomix", "").replace("_eval", "")
        repos.add(repo)

    all_results = []

    for repo in sorted(repos):
        # Find matching files
        cl_file = None
        rm_file = None

        for f in infiniloom_files:
            if repo in f.stem:
                cl_file = f
                break
        for f in repomix_files:
            if repo in f.stem:
                rm_file = f
                break

        if not cl_file or not rm_file:
            continue

        cl_analysis = analyze_infiniloom_output(cl_file)
        rm_analysis = analyze_repomix_output(rm_file)

        cl_scores = calculate_llm_effectiveness_score(cl_analysis)
        rm_scores = calculate_llm_effectiveness_score(rm_analysis)

        all_results.append({
            "repo": repo,
            "infiniloom": {"analysis": cl_analysis, "scores": cl_scores},
            "repomix": {"analysis": rm_analysis, "scores": rm_scores},
        })

    # Summary table
    report.append("## Summary Scores\n")
    report.append("| Repository | Infiniloom | Repomix | Winner | Margin |")
    report.append("|------------|----------|---------|--------|--------|")

    cl_total = 0
    rm_total = 0
    cl_wins = 0
    rm_wins = 0

    for r in all_results:
        cl_score = r["infiniloom"]["scores"]["total"]
        rm_score = r["repomix"]["scores"]["total"]
        cl_total += cl_score
        rm_total += rm_score

        if cl_score > rm_score:
            winner = "**Infiniloom**"
            cl_wins += 1
        elif rm_score > cl_score:
            winner = "**Repomix**"
            rm_wins += 1
        else:
            winner = "Tie"

        margin = abs(cl_score - rm_score)
        report.append(f"| {r['repo']} | {cl_score:.1f}/100 | {rm_score:.1f}/100 | {winner} | +{margin:.1f} |")

    # Overall summary
    report.append(f"\n**Overall Results:**")
    report.append(f"- Infiniloom wins: {cl_wins}/{len(all_results)}")
    report.append(f"- Repomix wins: {rm_wins}/{len(all_results)}")
    report.append(f"- Infiniloom average: {cl_total/max(len(all_results),1):.1f}/100")
    report.append(f"- Repomix average: {rm_total/max(len(all_results),1):.1f}/100")

    # Score breakdown
    report.append("\n## Score Breakdown by Category\n")
    report.append("| Category | Infiniloom Avg | Repomix Avg | Better |")
    report.append("|----------|--------------|-------------|--------|")

    categories = ["structure", "content_quality", "organization", "completeness"]
    for cat in categories:
        cl_avg = sum(r["infiniloom"]["scores"][cat] for r in all_results) / max(len(all_results), 1)
        rm_avg = sum(r["repomix"]["scores"][cat] for r in all_results) / max(len(all_results), 1)
        better = "Infiniloom" if cl_avg > rm_avg else ("Repomix" if rm_avg > cl_avg else "Tie")
        report.append(f"| {cat.replace('_', ' ').title()} | {cl_avg:.1f} | {rm_avg:.1f} | {better} |")

    # Feature comparison
    report.append("\n## Feature Comparison\n")
    report.append("| Feature | Infiniloom | Repomix |")
    report.append("|---------|----------|---------|")

    features = [
        ("Metadata section", "has_metadata"),
        ("Summary/overview", "has_summary"),
        ("Directory structure", "has_directory_structure"),
        ("Symbol index", "has_symbol_index"),
        ("Dependency info", "has_dependency_info"),
        ("Line numbers", "files_with_line_numbers"),
        ("Language tags", "files_with_language_tags"),
        ("Per-file tokens", "files_with_token_counts"),
        ("CDATA escaping", "uses_cdata"),
        ("Clear file boundaries", "has_clear_file_boundaries"),
    ]

    for feature_name, attr in features:
        cl_count = sum(1 for r in all_results if getattr(r["infiniloom"]["analysis"], attr, False) or
                       (isinstance(getattr(r["infiniloom"]["analysis"], attr, 0), int) and getattr(r["infiniloom"]["analysis"], attr, 0) > 0))
        rm_count = sum(1 for r in all_results if getattr(r["repomix"]["analysis"], attr, False) or
                       (isinstance(getattr(r["repomix"]["analysis"], attr, 0), int) and getattr(r["repomix"]["analysis"], attr, 0) > 0))

        cl_pct = cl_count / max(len(all_results), 1) * 100
        rm_pct = rm_count / max(len(all_results), 1) * 100

        report.append(f"| {feature_name} | {cl_pct:.0f}% | {rm_pct:.0f}% |")

    # Detailed analysis per repo
    report.append("\n## Detailed Analysis\n")

    for r in all_results:
        cl = r["infiniloom"]["analysis"]
        rm = r["repomix"]["analysis"]
        cl_s = r["infiniloom"]["scores"]
        rm_s = r["repomix"]["scores"]

        report.append(f"### {r['repo']}\n")
        report.append(f"| Metric | Infiniloom | Repomix |")
        report.append(f"|--------|----------|---------|")
        report.append(f"| File size | {cl.file_size_bytes/1024:.1f} KB | {rm.file_size_bytes/1024:.1f} KB |")
        report.append(f"| Files included | {cl.file_count} | {rm.file_count} |")
        report.append(f"| Token count | {cl.token_count:,} | N/A |")
        report.append(f"| Files with line numbers | {cl.files_with_line_numbers} | {rm.files_with_line_numbers} |")
        report.append(f"| Files with language tags | {cl.files_with_language_tags} | {rm.files_with_language_tags} |")
        report.append(f"| Sections | {', '.join(cl.sections)} | {', '.join(rm.sections)} |")
        report.append(f"| **Total Score** | **{cl_s['total']:.1f}/100** | **{rm_s['total']:.1f}/100** |")
        report.append("")

    # Best practices recommendations
    report.append("\n## LLM Context Best Practices\n")
    report.append("""
Based on this analysis and LLM documentation, here are the key best practices for repository context:

### Essential Features (High Impact)
1. **Structured Metadata** - Token counts, file counts, language breakdown
2. **Repository Map/Summary** - High-level overview of architecture
3. **Symbol Index** - Quick navigation to key functions/classes
4. **Line Numbers** - Precise code references in responses
5. **Language Tags** - Proper syntax highlighting context

### Important Features (Medium Impact)
6. **Directory Structure** - Understand project organization
7. **Dependency Information** - Understand relationships
8. **CDATA Escaping** - Prevent XML parsing issues
9. **Per-file Token Counts** - Budget awareness

### Optimization Features
10. **Compression Options** - Reduce context size while preserving meaning
11. **Importance Ranking** - Put most relevant files first
12. **Caching Headers** - Enable prompt caching for repeated use
""")

    return "\n".join(report)


def main():
    """Run the analysis"""
    print("Analyzing Infiniloom vs Repomix outputs...")

    report = compare_outputs()

    report_path = REPORTS_DIR / f"output_analysis_{__import__('datetime').datetime.now().strftime('%Y%m%d_%H%M%S')}.md"
    with open(report_path, "w") as f:
        f.write(report)

    print(f"\nReport saved to: {report_path}")
    print("\n" + "="*60)
    print(report)


if __name__ == "__main__":
    main()
