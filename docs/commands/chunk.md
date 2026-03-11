# `infiniloom chunk` Command

## Overview

The `chunk` command splits a repository into multiple context chunks suitable for multi-turn LLM conversations. It intelligently groups related files to maintain semantic coherence while respecting token limits.

## Synopsis

```bash
infiniloom chunk [PATH] [OPTIONS]
```

**Default PATH**: Current directory (`.`)

## Description

The `chunk` command:

1. **Scans Repository**: Identifies all source files
2. **Applies Strategy**: Groups files based on selected chunking strategy
3. **Enforces Budget**: Ensures each chunk fits within token limit
4. **Maintains Coherence**: Keeps related code together
5. **Outputs Chunks**: Multiple files or structured output

## Chunking Strategies

| Strategy | Description | Best For |
|----------|-------------|----------|
| `semantic` (default) | Groups by semantic similarity (char frequency heuristics) | General use |
| `file` | One file per chunk | Simple splitting |
| `module` | Groups by directory/module | Modular codebases |
| `symbol` | Groups by key symbols (AST-based) | API surfaces and core logic |
| `dependency` | Groups by import relationships | Understanding flow |
| `fixed` | Fixed token size chunks | Predictable sizing |

### Strategy Details

#### Semantic Strategy

Uses character frequency analysis to group semantically similar files:

```rust
// Heuristic similarity based on character distribution
fn similarity(file1: &str, file2: &str) -> f64 {
    let freq1 = char_frequency(file1);
    let freq2 = char_frequency(file2);
    cosine_similarity(&freq1, &freq2)
}
```

**Advantages:**
- Groups files with similar code patterns
- Language-agnostic
- No external dependencies

**Limitations:**
- Heuristic-based (not neural embeddings)
- May miss semantic relationships across different coding styles

#### Module Strategy

Groups files by directory structure:

```
src/
├── api/           → Chunk 1: API module
│   ├── routes.rs
│   └── handlers.rs
├── database/      → Chunk 2: Database module
│   ├── connection.rs
│   └── queries.rs
└── utils/         → Chunk 3: Utilities
    ├── helpers.rs
    └── validation.rs
```

#### Dependency Strategy

Groups files that import each other:

```
Chunk 1: Core + its dependents
├── src/core/types.rs
├── src/core/parser.rs
└── src/core/tokenizer.rs

Chunk 2: API + its dependents
├── src/api/routes.rs
├── src/api/handlers.rs
└── src/middleware/auth.rs
```

#### Fixed Strategy

Simple token-based splitting:

```rust
// Split at approximately max_tokens boundary
// Tries to split at file boundaries when possible
fn split_fixed(files: Vec<File>, max_tokens: u32) -> Vec<Chunk> {
    let mut chunks = vec![];
    let mut current_chunk = Chunk::new();

    for file in files {
        if current_chunk.tokens + file.tokens > max_tokens {
            chunks.push(current_chunk);
            current_chunk = Chunk::new();
        }
        current_chunk.add(file);
    }
    chunks.push(current_chunk);
    chunks
}
```

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--strategy <STRATEGY>` | `-s` | Chunking strategy | `semantic` |
| `--max-tokens <N>` | `-t` | Maximum tokens per chunk | `8000` |
| `--overlap <TOKENS>` | | Token overlap between chunks for context continuity | `0` |
| `--model <MODEL>` | `-m` | Target model for token counting | `claude` |
| `--format <FORMAT>` | `-f` | Output format (xml, markdown, json, yaml, toon, plain) | `xml` |
| `--output <DIR>` | `-o` | Output directory (creates multiple files) | stdout |
| `--verbose` | `-v` | Show detailed progress | `false` |
| `--no-chunk-summary` | | Disable auto-generated summary headers | `false` |
| `--priority-first` | | Sort chunks by priority (core first, tests last) | `false` |
| `--include <PATTERN>` | `-i` | Include only files matching glob pattern (repeatable) | all |
| `--exclude <PATTERN>` | `-e` | Exclude files/directories matching pattern (repeatable) | none |
| `--include-tests` | | Include test files in chunks (normally excluded) | `false` |

## Output

### Stdout (Default)

When no output directory is specified, outputs all chunks to stdout:

```xml
<chunks total="5" strategy="semantic" max_tokens="8000">
  <chunk number="1" tokens="7543" files="3">
    <file path="src/core/types.rs">...</file>
    <file path="src/core/parser.rs">...</file>
    <file path="src/core/tokenizer.rs">...</file>
  </chunk>
  <chunk number="2" tokens="6891" files="4">
    ...
  </chunk>
</chunks>
```

### Directory Output (`-o dir/`)

Creates numbered chunk files:

```
output/
├── chunk_001.xml  (7543 tokens)
├── chunk_002.xml  (6891 tokens)
├── chunk_003.xml  (7102 tokens)
├── chunk_004.xml  (5234 tokens)
└── chunk_005.xml  (4521 tokens)
```

Each chunk file is a complete, valid output:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<chunk number="1" of="5" strategy="semantic" tokens="7543">
  <files>
    <file path="src/core/types.rs">
      <content>...</content>
    </file>
  </files>
</chunk>
```

## Examples

### Basic Usage

```bash
# Chunk to stdout
infiniloom chunk

# Chunk to directory
infiniloom chunk -o chunks/
```

### Strategy Selection

```bash
# Semantic chunking (default)
infiniloom chunk --strategy semantic

# Module-based chunking
infiniloom chunk --strategy module

# Symbol-based chunking
infiniloom chunk --strategy symbol

# Dependency-based chunking
infiniloom chunk --strategy dependency

# Fixed-size chunking
infiniloom chunk --strategy fixed
```

### Budget Control

```bash
# Small chunks for limited context models
infiniloom chunk --max-tokens 4000

# Larger chunks for Claude/Gemini
infiniloom chunk --max-tokens 30000

# Optimize for specific model
infiniloom chunk --max-tokens 15000 --model gpt4o
```

### Format Selection

```bash
# JSON chunks for programmatic use
infiniloom chunk --format json -o chunks/

# Markdown chunks for human review
infiniloom chunk --format markdown -o chunks/
```

## Best Practices for LLM Context

### Multi-Turn Conversation Flow

```python
import os

# Load chunks
chunks = sorted(os.listdir("chunks/"))

# Send chunks progressively
conversation = []
for chunk_file in chunks:
    with open(f"chunks/{chunk_file}") as f:
        chunk_content = f.read()

    if len(conversation) == 0:
        # First chunk: establish context
        prompt = f"""I'm going to share a codebase with you in parts.
        Here's part 1 of {len(chunks)}:

        {chunk_content}

        Please acknowledge and wait for more parts."""
    else:
        # Subsequent chunks
        prompt = f"""Here's part {len(conversation)+1} of {len(chunks)}:

        {chunk_content}

        Please acknowledge."""

    response = send_to_llm(prompt)
    conversation.append((prompt, response))

# Final analysis request
final_prompt = "Now that you have the full codebase, please analyze..."
```

### Strategy Selection Guide

| Codebase Type | Recommended Strategy | Rationale |
|---------------|---------------------|-----------|
| Monolith | `module` | Clear module boundaries |
| Microservices | `dependency` | Service relationships matter |
| Library | `dependency` | API surface grouped |
| Scripts | `file` | Independent files |
| ML Project | `semantic` | Group by function type |
| Unknown | `semantic` | General purpose |

### Token Budget Selection

| Model | Recommended Chunk Size | Rationale |
|-------|----------------------|-----------|
| Claude 3.5 | 20,000-30,000 | Leave room for response |
| GPT-5/GPT-4o | 20,000-30,000 | 128K context window |
| GPT-4 Turbo | 15,000-20,000 | 128K context |
| GPT-4 (legacy) | 6,000-8,000 | Smaller context window |
| Gemini 3.1 Pro | 50,000-100,000 | Massive context |

### Overlap Strategy

For better continuity between chunks, use the `--overlap` flag:

```bash
# Add 500 tokens of overlap between consecutive chunks
infiniloom chunk --overlap 500
# Last 500 tokens of chunk N are prepended to chunk N+1
# Marked with: <!-- [OVERLAP FROM PREVIOUS CHUNK] -->
```

### Priority-Based Ordering

Ensure important files appear in earlier chunks:

```bash
# Core modules first, tests and utilities last
infiniloom chunk --priority-first

# Priority scores:
# - Entry points (main.rs, index.ts): 100
# - Config files (Cargo.toml, package.json): 90
# - Core modules (lib/, core/): 80
# - API handlers/routes: 75
# - Source files: 60
# - Utilities: 30
# - Tests: 20
# - Examples/docs: 10
```

### Summary Headers

By default, each chunk includes an auto-generated summary header describing its contents:

```xml
<!-- Chunk 1/5: src module | Files: types.rs, parser.rs, tokenizer.rs | ~7543 tokens -->
```

To disable summary headers:

```bash
infiniloom chunk --no-chunk-summary
```

## Performance Characteristics

| Strategy | Time Complexity | Memory Usage |
|----------|----------------|--------------|
| `fixed` | O(n) | Low |
| `file` | O(n) | Low |
| `module` | O(n log n) | Medium |
| `semantic` | O(n²) | High (similarity matrix) |
| `dependency` | O(n + e) | Medium |

## Potential Improvements

### 1. Neural Embeddings for Semantic Chunking

```bash
# Future: use actual embeddings for semantic similarity
infiniloom chunk --strategy semantic-neural
# Requires embedding model, more accurate but slower
```

### 2. Adaptive Sizing

```bash
# Future: variable chunk sizes based on coherence
infiniloom chunk --adaptive
# Creates larger chunks when content is highly related
```

### 3. Test Isolation

```bash
# Future: separate test chunks
infiniloom chunk --separate-tests
# Creates: code_chunk_*.xml and test_chunk_*.xml
```

### 4. Incremental Chunking

```bash
# Future: only re-chunk changed modules
infiniloom chunk --incremental
# Reuses cached chunks for unchanged code
```

### 5. Conversation Flow Generator

```bash
# Future: generate suggested prompts for each chunk
infiniloom chunk --generate-prompts
# Outputs: chunk_001.xml, chunk_001_prompt.txt
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Invalid path or output directory error |

## Related Commands

- [`pack`](pack.md) - Generate single context file
- [`embed`](embed.md) - AST-aware chunks for vector databases (preferred for RAG)
- [`scan`](scan.md) - Check total token counts before chunking
- [`map`](map.md) - Understand symbol importance for priority chunking
