# Handling Large Repositories

Strategies for working with codebases that exceed LLM context limits.

## Assessing Your Repository

First, understand what you're working with:

```bash
infiniloom scan .
```

This shows:
- Total files and token count
- Language breakdown
- File size distribution

### Size Categories

| Tokens | Category | Strategy |
|--------|----------|----------|
| <50K | Small | Pack everything |
| 50-200K | Medium | Filter or compress |
| 200K-1M | Large | Chunk or focus |
| >1M | Very Large | Sample or map-first |

## Strategy 1: Smart Filtering

### Exclude Non-Essential Files

```bash
infiniloom pack . \
  --exclude "tests/*" \
  --exclude "**/*.test.*" \
  --exclude "**/*.spec.*" \
  --exclude "docs/*" \
  --exclude "examples/*" \
  --exclude "fixtures/*" \
  --exclude "*.md"
```

### Focus on Core Code

```bash
# Only source files
infiniloom pack . --include "src/**/*"

# Specific languages
infiniloom pack . --include "*.rs" --include "*.py"
```

### Top Files by Importance

```bash
# Most important 50 files
infiniloom pack . --top-files 50
```

Infiniloom ranks files by:
1. PageRank (how connected they are)
2. Symbol count
3. Entry point detection
4. Recent modifications

## Strategy 2: Compression

### Compression Levels

```bash
# Balanced (30-40% reduction)
infiniloom pack . --compression balanced

# Aggressive (50-60% reduction)
infiniloom pack . --compression aggressive

# Extreme (70-80% reduction, signatures only)
infiniloom pack . --compression extreme
```

### What Gets Removed

| Level | Removes |
|-------|---------|
| `minimal` | Empty lines, trailing whitespace |
| `balanced` | + Comments, redundant whitespace |
| `aggressive` | + Docstrings, inline comments |
| `extreme` | + Function bodies (signatures only) |

### Combine with Filtering

```bash
infiniloom pack . \
  --exclude "tests/*" \
  --compression aggressive \
  --max-tokens 80000
```

## Strategy 3: Chunking

Split the repository into digestible pieces:

```bash
# Split into 50K token chunks
infiniloom chunk . --max-tokens 50000
```

### Chunking Strategies

**Directory-based (default):**
```bash
infiniloom chunk . --strategy directory
```
Groups files by directory.

**Semantic:**
```bash
infiniloom chunk . --strategy semantic
```
Groups related files (imports/dependencies).

**File-based:**
```bash
infiniloom chunk . --strategy file
```
One file per chunk (for very large files).

### Overlap for Continuity

```bash
infiniloom chunk . --max-tokens 50000 --overlap 2000
```

Overlap ensures context isn't lost between chunks.

### Priority Ordering

```bash
infiniloom chunk . --priority-first
```

Most important chunks come first.

## Strategy 4: Map-First Approach

For very large codebases, start with a map:

```bash
# Generate high-level overview (3K tokens)
infiniloom map . --budget 3000
```

This produces:
- Key symbols ranked by importance
- Module/directory structure
- File index with importance levels

### Workflow

1. **Generate map**: `infiniloom map . --budget 3000`
2. **Send to LLM**: "Based on this map, which files should I examine for [task]?"
3. **Pack specific files**: `infiniloom pack . --include "suggested/files/*"`

## Strategy 5: Diff-Based Context

For code reviews and modifications, only send relevant context:

```bash
# Build index (once)
infiniloom index .

# Context for changes
infiniloom diff . --staged --include-diff
```

### Depth Control

```bash
# Depth 1: Changed files only
infiniloom diff . --depth 1

# Depth 2: + Direct imports/importers (default)
infiniloom diff . --depth 2

# Depth 3: + Second-degree connections
infiniloom diff . --depth 3
```

### Impact Analysis

```bash
# What depends on this file?
infiniloom impact . src/core/auth.rs

# What calls this function?
infiniloom impact . --symbol "authenticate"
```

## Strategy 6: Sampling

For initial exploration of very large repos:

```bash
# Sample 500 files
infiniloom scan . --sample 500

# Sample 1% of files
infiniloom scan . --sample-percent 1
```

This gives quick statistics without processing everything.

## Performance Tips

### Skip Symbol Extraction

For maximum speed, skip AST parsing:

```bash
infiniloom pack . --skip-symbols
```

This is 50-80x faster but loses symbol information.

### Use Watch Mode

For repeated packing:

```bash
infiniloom pack . --output context.xml --watch
```

Regenerates only when files change.

### Build Index Once

```bash
# Build comprehensive index
infiniloom index .

# Keep it updated in background
infiniloom index . --watch
```

Then diff/impact commands are instant.

## Configuration for Large Repos

```yaml
# .infiniloom.yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 80000

scan:
  include:
    - "src/**/*"
    - "lib/**/*"
  exclude:
    - "tests/*"
    - "docs/*"
    - "examples/*"
    - "vendor/*"
    - "node_modules/*"
    - "target/*"
    - "dist/*"
    - "build/*"
    - "*.min.js"
    - "*.bundle.js"
    - "package-lock.json"
    - "yarn.lock"
  max_file_size: 524288  # Skip files > 512KB
```

## Example: Linux Kernel (78K files)

```bash
# This would timeout
infiniloom pack /path/to/linux  # Don't do this

# Better: Focus on specific subsystem
infiniloom pack /path/to/linux --include "kernel/sched/*"

# Or: Get map first
infiniloom map /path/to/linux --budget 5000

# Or: Sample for stats
infiniloom scan /path/to/linux --sample-percent 0.5
```

## Example: Large Monorepo

```bash
# Scan to understand structure
infiniloom scan .

# Pack specific package
infiniloom pack . --include "packages/core/*"

# Or use workspace-aware chunking
infiniloom chunk . --strategy semantic --max-tokens 80000
```

## Troubleshooting

### "Processing taking too long"

1. Use `--skip-symbols` for faster processing
2. Reduce scope with `--include`
3. Use sampling: `--sample 500`

### "Out of memory"

1. Process in chunks: `infiniloom chunk .`
2. Exclude large generated files
3. Set max file size in config

### "Output too large"

1. Reduce `--max-tokens`
2. Increase compression
3. Use `--top-files` limit
4. Exclude more directories
