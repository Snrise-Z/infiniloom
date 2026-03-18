---
name: infiniloom-diff
version: 1.0.0
description: "Get LLM-optimized context for a diff: changed files, their dependents, and related tests."
metadata:
  category: "development"
  requires:
    bins: ["infiniloom"]
    skills: ["infiniloom-shared"]
---

# infiniloom diff

> **PREREQUISITE:** Read `../infiniloom-shared/SKILL.md` for installation and global flags.

## Overview

Generate context-aware diff output for LLMs. Expands changed files to include their dependents and tests, so the LLM has enough context to reason about the impact of changes.

## Usage

```bash
infiniloom diff [OPTIONS] [PATH] [REFERENCE]
```

## Key Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--staged` | | off | Use staged changes (instead of unstaged) |
| `--depth` | `-d` | `2` | Context depth: 1=containing, 2=direct deps, 3=transitive |
| `--budget` | `-b` | `50000` | Token budget for context |
| `--format` | `-f` | `xml` | Output format: xml, markdown, yaml, json, toon, plain |
| `--model` | `-m` | `claude` | Target model for token counting |
| `--output` | `-o` | stdout | Output file path |
| `--include-diff` | | off | Include actual +/- diff lines in output |
| `--include-history` | | off | Include recent commit history per file |
| `--history-count` | | `3` | Number of recent commits to include |
| `--verbose` | `-v` | off | Show detailed output |
| `--include` | `-i` | all | Include only matching files |
| `--exclude` | `-e` | none | Exclude matching files |
| `--include-tests` | | off | Include test files in context |

## Examples

```bash
# Context for unstaged changes
infiniloom diff .

# Context for staged changes with actual diff lines
infiniloom diff . --staged --include-diff

# Context for last commit with full dependency chain
infiniloom diff . HEAD~1 --depth 3

# Compare branches, include tests and history
infiniloom diff . main..feature --include-tests --include-history

# Markdown output for GPT with 30k token budget
infiniloom diff . --staged -f markdown -m gpt4o -b 30000
```

## Tips

- Run `infiniloom index .` first for faster and more accurate dependency resolution.
- Depth 2 (default) covers direct dependents, which is usually sufficient for code review.
- Use `--include-diff` when you want the LLM to see the exact changes alongside surrounding context.
