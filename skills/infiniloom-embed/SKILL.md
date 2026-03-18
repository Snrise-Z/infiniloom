---
name: infiniloom-embed
version: 1.0.0
description: "Generate deterministic, content-addressable code chunks for vector databases and RAG pipelines."
metadata:
  category: "development"
  requires:
    bins: ["infiniloom"]
    skills: ["infiniloom-shared"]
---

# infiniloom embed

> **PREREQUISITE:** Read `../infiniloom-shared/SKILL.md` for installation and global flags.

## Overview

Generate AST-aware, content-addressable code chunks optimized for vector database ingestion (RAG). Each chunk has a stable BLAKE3-based ID, enabling incremental updates and cross-repo deduplication.

## Usage

```bash
infiniloom embed [OPTIONS] [PATH]
```

## Key Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--format` | `-f` | `jsonl` | Output format: jsonl, json |
| `--output` | `-o` | stdout | Output file path |
| `--max-tokens` | | `1000` | Maximum tokens per chunk |
| `--min-tokens` | | `50` | Minimum tokens per chunk |
| `--context-lines` | | `5` | Context lines around symbols |
| `--token-model` | | `claude` | Model for token counting |
| `--diff` | | off | Only output changed chunks (incremental) |
| `--manifest` | `-m` | `.infiniloom-embed.bin` | Manifest file for incremental tracking |
| `--since <REF>` | | none | Only process files changed since git ref |
| `--since-manifest` | | off | Use manifest's stored commit for `--since` |
| `--hierarchy` | | off | Generate summary chunks for classes/structs |
| `--include-signatures` | | off | Generate signature-only chunks for tiered retrieval |
| `--git-metadata` | | off | Enrich with change frequency and authors |
| `--streaming` | | off | Stream output in batches (lower memory) |
| `--batch-size` | | `500` | Files per batch in streaming mode |
| `--no-imports` | | off | Exclude import statements |
| `--no-top-level` | | off | Exclude top-level code |
| `--no-security-scan` | | off | Disable secret scanning |
| `--json-stats` | | off | Output statistics as JSON to stderr |
| `--quiet` | `-q` | off | Suppress non-error output except chunk data |

## Examples

```bash
# Generate chunks for current repo
infiniloom embed . -o chunks.jsonl

# Incremental update (only changed chunks since last run)
infiniloom embed . --diff -o updates.jsonl

# Optimized for Voyage Code embedding model (1500 tokens)
infiniloom embed . --max-tokens 1500 -o chunks.jsonl

# With hierarchy and git metadata for richer retrieval
infiniloom embed . --hierarchy --git-metadata -o chunks.jsonl

# Streaming mode for large repos (lower memory)
infiniloom embed . --streaming --batch-size 200 -o chunks.jsonl
```

## Tips

- Use `--diff` with `--since-manifest` for efficient CI/CD incremental updates.
- Chunk IDs are deterministic: same code anywhere produces the same ID, enabling cross-repo dedup.
- Set `--max-tokens` to match your embedding model's context window (e.g., 512 for sentence-transformers, 1500 for Voyage).
