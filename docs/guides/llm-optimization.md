# LLM Optimization Guide

Get the best results from Infiniloom with different LLMs.

## Model-Specific Formats

| Model | Format | Why |
|-------|--------|-----|
| Claude | XML | Prompt caching, CDATA for code, structured tags |
| GPT-4/GPT-4o | Markdown | Native markdown understanding, tables, headers |
| Gemini | YAML | Hierarchical structure, query at end |
| Generic | TOON | ~40% smaller than JSON, works everywhere |

## Claude Optimization

### Use XML Format

```bash
infiniloom pack . --format xml --model claude
```

XML format includes:
- `<repository>` wrapper for prompt caching
- `<file>` tags with path and language attributes
- `<symbol>` tags for functions/classes
- CDATA sections to preserve code formatting

### Prompt Caching

Place the repository context at the **start** of your conversation:

```
[Repository context from Infiniloom]

Now, based on this codebase, please...
```

This allows Claude to cache the context across multiple turns.

### Token Budget for Claude

- Claude 3.5 Sonnet: 200K context, aim for 100-150K
- Claude 3 Opus: 200K context, aim for 100-150K

```bash
infiniloom pack . --max-tokens 100000
```

## GPT-4 / GPT-4o Optimization

### Use Markdown Format

```bash
infiniloom pack . --format markdown --model gpt4o
```

Markdown format includes:
- Hierarchical headers for structure
- Code fences with language hints
- Tables for metadata
- Clean formatting GPT understands well

### Token Budget for GPT

- GPT-4o: 128K context, aim for 80-100K
- GPT-4 Turbo: 128K context, aim for 80-100K
- GPT-4: 8K/32K context

```bash
# GPT-4o
infiniloom pack . --max-tokens 80000 --model gpt4o

# GPT-4 (8K)
infiniloom pack . --max-tokens 6000 --model gpt4
```

### Exact Token Counting

Infiniloom uses tiktoken for exact GPT token counts:

```bash
infiniloom scan . --model gpt4o
```

## Gemini Optimization

### Use YAML Format

```bash
infiniloom pack . --format yaml --model gemini
```

YAML format includes:
- Structured hierarchy Gemini parses well
- Query/instruction section at end
- Clean key-value structure

### Token Budget for Gemini

- Gemini 1.5 Pro: 1M+ context (but quality degrades)
- Practical limit: 100-200K for best results

```bash
infiniloom pack . --max-tokens 150000 --model gemini
```

## Compression Strategies

### When to Use Compression

| Situation | Level | Reduction |
|-----------|-------|-----------|
| Context fits easily | `none` | 0% |
| Slightly over budget | `minimal` | 10-20% |
| Need more room | `balanced` | 30-40% |
| Very large codebase | `aggressive` | 50-60% |
| Extreme budget constraints | `extreme` | 70-80% |
| Key symbols with context | `focused` | ~75% |
| Semantic understanding | `semantic` | 60-70% |

### Compression Examples

```bash
# Preserve everything (default)
infiniloom pack . --compression none

# Remove empty lines, trailing whitespace
infiniloom pack . --compression minimal

# Remove comments (good default)
infiniloom pack . --compression balanced

# Remove docstrings too
infiniloom pack . --compression aggressive

# Signatures only
infiniloom pack . --compression extreme

# Key symbols with surrounding context
infiniloom pack . --compression focused

# Semantic compression
infiniloom pack . --compression semantic
```

### Semantic Compression

For large codebases, use the repository map instead of full content:

```bash
# Generate map of key symbols
infiniloom map . --budget 5000

# Include only most important files
infiniloom pack . --top-files 50
```

## Context Window Management

### Prioritization

Infiniloom ranks files by importance:

1. Entry points (main.rs, index.ts, app.py)
2. Config files (Cargo.toml, package.json)
3. Files with many symbols
4. Files referenced by many other files
5. Recently modified files

Use `--top-files` to limit to most important:

```bash
infiniloom pack . --top-files 30
```

### Chunking for Multi-Turn

For codebases that don't fit in one context:

```bash
# Split into 50K token chunks
infiniloom chunk . --max-tokens 50000

# Semantic chunking (keeps related files together)
infiniloom chunk . --strategy semantic --max-tokens 50000

# With overlap for context continuity
infiniloom chunk . --max-tokens 50000 --overlap 2000
```

## Best Practices

### 1. Start with Repository Map

For architecture questions, start with the map:

```bash
infiniloom map . --budget 3000
```

Then ask the LLM which files to examine in detail.

### 2. Filter Aggressively

Exclude what's not needed:

```bash
infiniloom pack . \
  --exclude "tests/*" \
  --exclude "docs/*" \
  --exclude "*.test.*" \
  --exclude "node_modules/*"
```

### 3. Use Diff Context for Reviews

For code reviews, don't send the whole repo:

```bash
infiniloom diff . --staged --include-diff --depth 2
```

This sends only:
- Changed files
- Files that import changed files
- Files that changed files import
- The actual diff content

### 4. Create Project-Specific Config

```yaml
# .infiniloom.yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 80000

scan:
  include:
    - "src/**/*.ts"
    - "lib/**/*.ts"
  exclude:
    - "**/*.test.ts"
    - "**/*.spec.ts"
    - "dist/*"
```

### 5. Iterate on Token Budget

Start with a reasonable budget and adjust:

```bash
# Check current token count
infiniloom scan .

# If 200K tokens, and target is 80K:
# Option 1: Compression
infiniloom pack . --compression aggressive

# Option 2: Fewer files
infiniloom pack . --top-files 40

# Option 3: Focus on specific directories
infiniloom pack . --include "src/*"
```

## Troubleshooting

### "Context too long"

1. Reduce token budget: `--max-tokens 50000`
2. Increase compression: `--compression aggressive`
3. Exclude more files: `--exclude "tests/*"`
4. Use top files: `--top-files 30`

### "Missing important context"

1. Use repository map first to identify key files
2. Use semantic chunking with overlap
3. Increase depth for diff context: `--depth 3`

### "LLM ignoring parts of context"

1. Put most important content first
2. Use model-appropriate format
3. Reduce total context size
4. Consider multi-turn with chunking
