---
name: infiniloom-map
version: 1.0.0
description: "Generate a repository map showing the most important symbols ranked by PageRank."
metadata:
  category: "development"
  requires:
    bins: ["infiniloom"]
    skills: ["infiniloom-shared"]
---

# infiniloom map

> **PREREQUISITE:** Read `../infiniloom-shared/SKILL.md` for installation and global flags.

## Overview

Generate a compact repository map that lists the most important symbols (functions, classes, structs, traits) ranked by PageRank. Ideal for giving an LLM a high-level overview of a codebase within a small token budget.

## Usage

```bash
infiniloom map [OPTIONS] [PATH]
```

## Key Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--budget` | `-b` | `2000` | Token budget for the map |
| `--model` | `-m` | `claude` | Target model for token counting |
| `--output` | `-o` | stdout | Output file path |
| `--verbose` | `-v` | off | Show detailed ranking scores |
| `--include` | `-i` | all | Include only matching files |
| `--exclude` | `-e` | none | Exclude matching files |
| `--include-tests` | | off | Include test files in map |

## Examples

```bash
# Generate a 2000-token map of the current repo
infiniloom map .

# Larger map with verbose scores, saved to file
infiniloom map . -b 5000 -v -o map.txt

# Map only the engine crate
infiniloom map . -i "engine/src/**/*.rs"

# Map for GPT-4o token counting
infiniloom map . -m gpt4o -b 3000
```

## Tips

- A 2000-token map typically covers the top 30-50 symbols -- enough for an LLM to understand architecture.
- Increase `--budget` for larger projects or when you need deeper coverage of utility functions.
- Combine with `pack --no-content` to get a metadata-only overview alongside the symbol map.
