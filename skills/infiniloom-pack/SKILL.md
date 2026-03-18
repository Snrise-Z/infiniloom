---
name: infiniloom-pack
version: 1.0.0
description: "Pack a repository into an LLM-optimized context format (XML, Markdown, YAML, JSON, TOON)."
metadata:
  category: "development"
  requires:
    bins: ["infiniloom"]
    skills: ["infiniloom-shared"]
---

# infiniloom pack

> **PREREQUISITE:** Read `../infiniloom-shared/SKILL.md` for installation and global flags.

## Overview

Pack an entire repository (or selected files) into a single LLM-optimized document. Supports multiple output formats and compression levels, with optional symbol extraction, security scanning, and git history.

## Usage

```bash
infiniloom pack [OPTIONS] [PATH]
```

## Key Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--format` | `-f` | `xml` | Output format: xml, markdown, yaml, json, toon, plain |
| `--model` | `-m` | `claude` | Target model for token counting |
| `--compression` | `-c` | `balanced` | Compression: none, minimal, balanced, aggressive, extreme, focused, semantic |
| `--max-tokens` | `-t` | `0` | Token budget (0 = no limit). Alias: `--budget` / `-b` |
| `--output` | `-o` | stdout | Output file path |
| `--full` | | off | Enable full analysis (symbols + repo map + PageRank) |
| `--security-check` | | off | Scan for secrets and API keys |
| `--redact-secrets` | | off | Replace detected secrets with [REDACTED] |
| `--include-logs` | | off | Include git commit history |
| `--include-diffs` | | off | Include git diffs |
| `--copy-to-clipboard` | | off | Copy output to system clipboard |
| `--watch` | | off | Regenerate on file changes |
| `--no-content` | | off | Metadata only, exclude file contents |
| `--remove-comments` | | off | Strip comments from code |
| `--stdin` | | off | Read file paths from stdin |
| `--config` | | auto | Path to config file |

## Examples

```bash
# Pack current repo as XML (Claude-optimized) to stdout
infiniloom pack .

# Pack for GPT-4o with aggressive compression, save to file
infiniloom pack . -f markdown -m gpt4o -c aggressive -o context.md

# Pack only Rust source files with a 50k token budget
infiniloom pack . -i "src/**/*.rs" -t 50000

# Full analysis with security scan, copy to clipboard
infiniloom pack . --full --security-check --copy-to-clipboard

# Pack a remote repository (sparse checkout for speed)
infiniloom pack https://github.com/org/repo --sparse-path src --sparse-path lib
```

## Tips

- Use `--compression aggressive` to fit large repos within token limits (signatures only).
- Combine `--full` with `--format xml` for the richest context including PageRank-ranked symbols.
- Use `--stdin` to pipe a custom file list from `git diff --name-only` or similar tools.
