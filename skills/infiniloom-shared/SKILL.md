---
name: infiniloom-shared
version: 1.0.0
description: "Shared prerequisites, installation, and global flags for all Infiniloom skills."
metadata:
  category: "development"
  requires:
    bins: ["infiniloom"]
---

# infiniloom shared

## Overview

Infiniloom is a high-performance repository context generator for LLMs. It transforms codebases into optimized formats for Claude, GPT-4o/GPT-5, Gemini, and other models. No API keys or authentication required.

## Installation

```bash
# From crates.io
cargo install infiniloom

# From npm (downloads prebuilt binary)
npm install -g infiniloom

# From source
git clone https://github.com/anthropics/infiniloom && cd infiniloom
cargo build --release
# Binary at ./target/release/infiniloom
```

Verify: `infiniloom info`

## Global Flags

These flags are shared across most commands:

| Flag | Short | Description |
|------|-------|-------------|
| `--include <GLOB>` | `-i` | Include only files matching pattern (repeatable) |
| `--exclude <GLOB>` | `-e` | Exclude files matching pattern (repeatable) |
| `--include-tests` | | Include test files (excluded by default) |
| `--model <MODEL>` | `-m` | Target model for token counting |
| `--verbose` | `-v` | Show detailed output |
| `--output <PATH>` | `-o` | Write to file instead of stdout |

## Supported Models

`claude` (default), `gpt52`, `gpt51`, `gpt5`, `o4-mini`, `o3`, `o1`, `gpt4o`, `gpt4o-mini`, `gpt4`, `gpt35-turbo`, `gemini`, `llama`, `codellama`, `mistral`, `deepseek`, `qwen`, `cohere`, `grok`

## Output Formats

`xml` (Claude-optimized, default), `markdown` (GPT-optimized), `yaml` (Gemini-compatible), `json`, `toon` (most token-efficient), `plain`

## Configuration

Run `infiniloom init` to generate `.infiniloom.yaml`. Settings can also be set via environment variables with the `INFINILOOM_` prefix (e.g., `INFINILOOM_OUTPUT__FORMAT=markdown`).

## Tips

- Infiniloom respects `.gitignore` by default; use `--no-gitignore` to override.
- Test files and docs are excluded by default for smaller context; add `--include-tests` or `--include-docs` when needed.
- Use `--include` / `--exclude` globs to focus on specific subdirectories or file types.
