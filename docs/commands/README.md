# Infiniloom CLI Commands Reference

Complete technical documentation for all Infiniloom CLI commands.

## Command Overview

| Command | Description | Primary Use Case |
|---------|-------------|------------------|
| [`pack`](pack.md) | Transform repository into LLM-optimized context | Generating context for AI assistants |
| [`embed`](embed.md) | Generate content-addressable chunks for RAG | Vector database ingestion, semantic search |
| [`scan`](scan.md) | Analyze repository statistics and security | Pre-pack analysis, security audit |
| [`map`](map.md) | Generate PageRank-ranked symbol map | Understanding codebase architecture |
| [`index`](index.md) | Build persistent symbol index | Enable fast diff/impact analysis |
| [`diff`](diff.md) | Generate context for code changes | Code review, debugging |
| [`impact`](impact.md) | Analyze change impact for file/symbol | Pre-change analysis |
| [`chunk`](chunk.md) | Split repository into multiple contexts | Multi-turn LLM conversations |
| [`info`](info.md) | Show version and configuration | Debugging, option discovery |
| [`init`](init.md) | Create configuration file | Project setup |

## Quick Reference

### Most Common Workflows

#### 1. Basic Repository Context

```bash
# Generate XML context for Claude
infiniloom pack -o context.xml

# Generate Markdown for GPT
infiniloom pack -f markdown -m gpt4o -o context.md

# Generate with token limit
infiniloom pack --max-tokens 100000 -o context.xml
```

#### 2. Code Review Context

```bash
# Build index first (optional, for speed)
infiniloom index

# Generate context for PR changes
infiniloom diff main..feature --depth 2 -o review-context.xml
```

#### 3. Security Audit

```bash
# Scan for secrets
infiniloom scan --security-check

# Pack with redaction
infiniloom pack --redact-secrets -o safe-context.xml
```

#### 4. RAG Pipeline / Vector Database

```bash
# Generate chunks for vector DB ingestion
infiniloom embed -o chunks.json

# Optimized for voyage-code-2/3
infiniloom embed --max-tokens 1500 -o chunks.json

# Incremental updates (only changed chunks)
infiniloom embed --diff -o changes.json
```

#### 5. Large Repository Handling

```bash
# Check size first
infiniloom scan -m claude

# Use chunking for multi-turn
infiniloom chunk --max-tokens 30000 -o chunks/

# Or use aggressive compression
infiniloom pack -c aggressive --max-tokens 150000
```

## Output Formats

| Format | Best For | Token Efficiency |
|--------|----------|------------------|
| `xml` | Claude | Baseline |
| `markdown` | GPT models | ~10% more tokens |
| `json` | Programmatic use | ~15% more tokens |
| `yaml` | Gemini | ~12% more tokens |
| `toon` | Any (maximum efficiency) | ~40% fewer tokens |
| `plain` | Simple use | ~5% fewer tokens |

## Model Support

### Exact Token Counting (via tiktoken)

- **o200k_base encoding**: GPT-5.x, O4-mini, O3, O1, GPT-4o, GPT-4o-mini
- **cl100k_base encoding**: GPT-4, GPT-3.5-turbo (legacy)

### Calibrated Estimation (~95% accuracy)

- Claude (Anthropic)
- Gemini (Google)
- Llama, CodeLlama (Meta)
- Mistral (Mistral AI)
- DeepSeek (DeepSeek)
- Qwen (Alibaba)
- Cohere (Cohere)
- Grok (xAI)

## Configuration

### Config File Locations

1. `.infiniloom.yaml` (recommended)
2. `.infiniloom.toml`
3. `.infiniloom.json`
4. `.infiniloomrc.yaml` (legacy)

### Environment Variables

```bash
INFINILOOM_OUTPUT__FORMAT=xml
INFINILOOM_OUTPUT__MODEL=claude
INFINILOOM_OUTPUT__COMPRESSION=balanced
INFINILOOM_OUTPUT__TOKEN_BUDGET=100000
INFINILOOM_SCAN__INCLUDE_HIDDEN=false
INFINILOOM_SECURITY__SCAN_SECRETS=true
```

## Performance Tips

### Speed Optimization

1. **Skip symbols** (default): 80x faster for most use cases
2. **Use cache**: `infiniloom pack --cache` for repeated runs
3. **Build index once**: `infiniloom index` then use `diff`/`impact`

### Token Optimization

1. **Use TOON format**: 40% smaller than XML
2. **Apply compression**: `--compression aggressive` for 60% reduction
3. **Filter files**: `--include "src/**/*.rs" --exclude "tests/*"`

### Quality Optimization

1. **Enable symbols**: `infiniloom pack --symbols` for better maps
2. **Use full mode**: `infiniloom pack --full` for PageRank ranking
3. **Include context**: `infiniloom diff --depth 3` for comprehensive review

## Exit Codes

All commands use consistent exit codes:

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Error (invalid path, I/O error, validation failure) |

## Getting Help

```bash
# General help
infiniloom --help

# Command-specific help
infiniloom pack --help
infiniloom diff --help

# Version info
infiniloom --version
infiniloom info
```

## See Also

- [Main README](../../README.md) - Project overview
- [CLAUDE.md](../../CLAUDE.md) - Development guidelines
- [Design Document](../INFINILOOM_DESIGN.md) - Architecture details
- [Output Formats](../INFINILOOM_OUTPUT_FORMATS.md) - Format specifications
