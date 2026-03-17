# Using Infiniloom with OpenAI Codex CLI

This guide covers practical workflows for combining Infiniloom with OpenAI's Codex CLI and other GPT-based coding assistants. Infiniloom generates Markdown-formatted, token-budgeted context optimized for GPT models.

## Why Use Infiniloom with Codex?

Codex CLI reads files from your repository, but Infiniloom helps when:

- **Your repo exceeds the context window** - Infiniloom ranks and prioritizes what fits
- **You need precise token counts** - Exact tiktoken counts for all OpenAI models
- **Security matters** - Automatic secret redaction before any code reaches OpenAI
- **You want structured context** - Not just raw files, but ranked symbols and dependency-aware diffs

## Format and Model Selection

Use **Markdown format** for GPT models - it's their native structured format:

```bash
# GPT-4o (128K context)
infiniloom pack . -f markdown -m gpt4o --max-tokens 80000 -o context.md

# GPT-5.1 Codex (128K context, code-specialized)
infiniloom pack . -f markdown -m gpt51-codex --max-tokens 100000 -o context.md

# GPT-5.2 (latest flagship)
infiniloom pack . -f markdown -m gpt52 --max-tokens 100000 -o context.md

# For tight budgets, use TOON format (~40% smaller)
infiniloom pack . -f toon -m gpt4 --max-tokens 6000 -o context.toon
```

## Token Budget Recommendations

Infiniloom uses tiktoken for **exact** token counts on all OpenAI models:

| Model | Context Window | Recommended Budget | Command |
|-------|---------------|-------------------|---------|
| GPT-5.2 / GPT-5.1 | 128K | 100,000 | `-m gpt52 --max-tokens 100000` |
| GPT-5.1 Codex | 128K | 100,000 | `-m gpt51-codex --max-tokens 100000` |
| O3 / O4 Mini | 200K | 150,000 | `-m o3 --max-tokens 150000` |
| GPT-4o | 128K | 80,000 | `-m gpt4o --max-tokens 80000` |
| GPT-4o Mini | 128K | 80,000 | `-m gpt4o-mini --max-tokens 80000` |
| GPT-4 (legacy) | 128K | 80,000 | `-m gpt4 --max-tokens 80000` |
| GPT-3.5 Turbo | 16K | 12,000 | `-m gpt35-turbo --max-tokens 12000` |

**Why leave headroom?** The budget covers input context only. You need remaining tokens for the model's response. 75-80% of context window is a good rule of thumb.

## Workflow 1: Code Review

```bash
# Build symbol index
infiniloom index .

# Generate review context in Markdown
infiniloom diff main..feature-branch \
  --include-diff \
  --depth 2 \
  --budget 80000 \
  --redact-secrets \
  -f markdown \
  -o review.md
```

## Workflow 2: Full Codebase Context

```bash
# For GPT-4o
infiniloom pack . \
  -f markdown \
  -m gpt4o \
  --compression balanced \
  --max-tokens 80000 \
  --redact-secrets \
  -o context.md

# For O3 (larger context)
infiniloom pack . \
  -f markdown \
  -m o3 \
  --compression balanced \
  --max-tokens 150000 \
  -o context.md
```

## Workflow 3: RAG Pipeline with OpenAI Embeddings

```bash
# Generate chunks sized for text-embedding-3-large
infiniloom embed . --max-tokens 800 --token-model gpt4o -o chunks.jsonl

# For voyage-code (if using OpenAI-compatible API)
infiniloom embed . --max-tokens 1500 -o chunks.jsonl

# Incremental updates after code changes
infiniloom embed . --diff -o updates.jsonl
```

## Workflow 4: Maximizing Token Efficiency

When working with smaller context windows:

```bash
# TOON format saves ~40% tokens vs Markdown
infiniloom pack . -f toon -m gpt4 --max-tokens 6000 -o context.toon

# Aggressive compression for tight budgets
infiniloom pack . -f markdown --compression aggressive -m gpt4o-mini --max-tokens 50000

# Only include specific directories
infiniloom pack . -f markdown -i "src/**" -e "tests/*" -m gpt4o --max-tokens 60000
```

## Workflow 5: Impact Analysis Before Refactoring

Check what depends on a file before asking Codex to make changes:

```bash
# Build index if not already done
infiniloom index .

# Check blast radius
infiniloom impact . src/auth.rs --depth 2

# JSON output for programmatic use
infiniloom impact . src/auth.rs --json
```

## Workflow 6: Onboarding to a New Codebase

Generate a comprehensive overview for Codex to explain a new project:

```bash
# Get project stats
infiniloom scan . -m gpt4o

# Generate ranked symbol map
infiniloom map . --budget 5000

# Full context for understanding
infiniloom pack . -f markdown -m gpt4o --compression balanced --max-tokens 80000 -o overview.md
```

## Workflow 7: Using the Symbol Index

The symbol index powers `diff` and `impact` commands:

```bash
# Build index (stored in .infiniloom/)
infiniloom index .

# Context for staged changes
infiniloom diff --staged --include-diff -f markdown -m gpt4o -o review.md

# Context for a branch comparison
infiniloom diff main..feature -f markdown --depth 2 --budget 80000 -o pr-context.md
```

## Workflow 8: Document Ingestion

Feed non-code documents (API specs, design docs) alongside code context:

```bash
# Convert an API spec to Markdown for Codex
infiniloom ingest api-spec.md -f markdown -o api-context.md

# Convert with PII redaction
infiniloom ingest report.docx --redact-pii -f markdown -o safe-report.md
```

## Configuration for GPT Workflows

Create a config file for consistent team usage:

```bash
infiniloom init --template typescript
```

Example `.infiniloom.yaml`:

```yaml
output:
  format: markdown       # GPT-optimized
  model: gpt4o           # Exact tiktoken counting
  compression: balanced
  token_budget: 80000

scan:
  exclude:
    - "tests/*"
    - "node_modules/*"
    - "*.test.*"

security:
  scan_secrets: true
  redact_secrets: true
```

Configuration precedence (later overrides earlier):
1. Default values
2. Config file (`.infiniloom.yaml` / `.infiniloom.toml` / `.infiniloom.json`)
3. Environment variables (e.g., `INFINILOOM_OUTPUT__MODEL=gpt4o`)
4. CLI flags

## See Also

- [Claude Code Integration Guide](claude-code-integration.md) - Using Infiniloom with Claude Code
- [LLM Optimization Guide](llm-optimization.md) - Model-specific optimization tips
- [Tokenizer Reference](../TOKENIZERS.md) - All 27 supported tokenizer models
- [Output Formats](../INFINILOOM_OUTPUT_FORMATS.md) - Format specifications and comparisons
- [Configuration Guide](../CONFIGURATION.md) - Full config reference
- [index command](../commands/index.md) - Symbol index documentation
- [impact command](../commands/impact.md) - Impact analysis documentation
