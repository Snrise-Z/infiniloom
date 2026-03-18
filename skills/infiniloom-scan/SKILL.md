---
name: infiniloom-scan
version: 1.0.0
description: "Scan a repository and show statistics: file counts, languages, token counts, and security issues."
metadata:
  category: "development"
  requires:
    bins: ["infiniloom"]
    skills: ["infiniloom-shared"]
---

# infiniloom scan

> **PREREQUISITE:** Read `../infiniloom-shared/SKILL.md` for installation and global flags.

## Overview

Scan a repository and report statistics including file counts, language breakdown, and token counts per model. Useful for estimating context window usage before packing.

## Usage

```bash
infiniloom scan [OPTIONS] [PATH]
```

## Key Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--model` | `-m` | `claude` | Target model for token counting |
| `--verbose` | `-v` | off | Show detailed per-file list |
| `--json` | | off | Output as JSON (machine-readable) |
| `--security-check` | | off | Scan for secrets and API keys |
| `--sample <N>` | | all | Sample N random files (for large repos) |
| `--sample-percent <P>` | | all | Sample P% of files |
| `--include` | `-i` | all | Include only matching files |
| `--exclude` | `-e` | none | Exclude matching files |
| `--include-tests` | | off | Include test files in scan |

## Examples

```bash
# Quick scan of current directory
infiniloom scan .

# Scan with GPT-4o token counts, verbose file list
infiniloom scan . -m gpt4o -v

# JSON output for CI pipelines
infiniloom scan . --json

# Scan only Python files, check for secrets
infiniloom scan . -i "**/*.py" --security-check

# Sample 100 files from a large monorepo
infiniloom scan /path/to/monorepo --sample 100
```

## Tips

- Run `scan` before `pack` to check if your repo fits within a model's context window.
- Use `--json` to integrate token counts into CI/CD budget checks.
- The `--sample` flag gives a fast estimate for repos with 10k+ files.
