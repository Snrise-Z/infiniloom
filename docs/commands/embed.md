# `infiniloom embed` Command

## Overview

The `embed` command generates deterministic, content-addressable code chunks optimized for vector databases. It transforms a codebase into semantic units (functions, classes, methods) with stable IDs that enable efficient incremental updates and cross-repository deduplication.

## Synopsis

```bash
infiniloom embed [PATH] [OPTIONS]
```

**Default PATH**: Current directory (`.`)

## Quick Start

Get embedding chunks in 3 steps:

```bash
# 1. Generate chunks for your repository
infiniloom embed ./my-repo -o chunks.json

# 2. Check what changed (if manifest exists)
infiniloom embed ./my-repo --diff-only -o changes.json

# 3. Use chunks in your RAG pipeline
# chunks.json contains content-addressable chunks ready for embedding
```

**First run output:**
```json
{
  "chunks": [...],
  "diff": { "added": 150, "modified": 0, "removed": 0, "unchanged": 0 }
}
```

**Common use cases:**
- `infiniloom embed` - Generate chunks for current directory
- `infiniloom embed --max-tokens 1500` - Optimize for voyage-code-2/3
- `infiniloom embed --security-scan --fail-on-secrets` - CI/CD mode
- `infiniloom embed --include "*.py" -o python-chunks.json` - Python only

## Description

The `embed` command performs the following operations:

1. **Repository Scanning**: Walks the directory tree respecting `.gitignore` patterns
2. **Symbol Extraction**: Parses files with Tree-sitter to extract semantic units
3. **Chunk Generation**: Creates chunks for functions, classes, methods, and other symbols
4. **Content Hashing**: Computes BLAKE3 hashes for content-addressable IDs
5. **Security Scanning**: Optionally detects and redacts secrets
6. **Manifest Diffing**: Compares against previous runs to detect added/modified/removed chunks
7. **Context Extraction**: Extracts docstrings, signatures, call graphs, and semantic tags

## Key Features

### Content-Addressable Chunks

Each chunk has a stable ID derived from its normalized content:

```
ec_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6
```

- **Same code anywhere = same ID** (enables deduplication)
- **ID changes only when content changes** (enables incremental updates)
- **Format**: `ec_` prefix + 32 hex characters (128-bit BLAKE3 hash)

### Incremental Updates

The command maintains a manifest file (`.infiniloom-embed.bin`) that tracks all generated chunks:

```bash
# First run: generates all chunks, creates manifest
infiniloom embed

# Subsequent runs: only processes changed chunks
infiniloom embed
# Output: Added: 5, Modified: 12, Removed: 3, Unchanged: 450
```

### AST-Aware Chunking

Chunks respect code structure:
- Never splits mid-function or mid-class
- Large symbols are split at logical boundaries with part numbering
- Each part includes the parent signature for context

## Options

### Output Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--output <PATH>` | `-o` | Write JSON output to file | stdout |
| `--manifest-path <PATH>` | | Custom manifest file location | `.infiniloom-embed.bin` |
| `--diff-only` | | Only output changed chunks (added + modified) | `false` |

### Token Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--max-tokens <N>` | `-t` | Maximum tokens per chunk | `1000` |
| `--min-tokens <N>` | | Minimum tokens per chunk (smaller merged) | `50` |
| `--context-lines <N>` | | Lines of context around symbols | `5` |
| `--model <MODEL>` | `-m` | Token counting model | `claude` |

### Content Options

| Option | Description | Default |
|--------|-------------|---------|
| `--include <PATTERN>` | Include only files matching glob pattern (repeatable) | all |
| `--exclude <PATTERN>` | Exclude files matching glob pattern (repeatable) | none |
| `--include-imports` | Include import statements as chunks | `true` |
| `--include-top-level` | Include top-level code outside symbols | `true` |
| `--include-tests` | Include test files | `false` |

### Security Options

| Option | Description | Default |
|--------|-------------|---------|
| `--security-scan` | Enable secret detection | `true` |
| `--fail-on-secrets` | Exit with error if secrets detected (CI mode) | `false` |
| `--redact-secrets` | Replace detected secrets with `[REDACTED]` | `true` |

### Output Control

| Option | Description | Default |
|--------|-------------|---------|
| `--json` | Output in JSON format | `true` |
| `--verbose` | Show progress and statistics | `false` |

## Output Format

### Chunk Structure

Each chunk contains:

```json
{
  "id": "ec_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6",
  "full_hash": "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8s9t0u1v2w3x4y5z6a7b8c9d0e1f2",
  "content": "fn calculate(a: i32, b: i32) -> i32 {\n    a + b\n}",
  "tokens": 25,
  "kind": "function",
  "source": {
    "file": "src/math.rs",
    "lines": [10, 12],
    "symbol": "calculate",
    "fqn": "src::math::calculate",
    "language": "Rust",
    "parent": null,
    "visibility": "public",
    "is_test": false
  },
  "context": {
    "docstring": "Adds two numbers together",
    "signature": "fn calculate(a: i32, b: i32) -> i32",
    "calls": ["add", "validate"],
    "called_by": ["main", "process"],
    "tags": ["public-api"],
    "lines_of_code": 3,
    "max_nesting_depth": 1
  }
}
```

### Chunk Kinds

| Kind | Description |
|------|-------------|
| `function` | Standalone function |
| `method` | Class/struct method |
| `class` | Class definition |
| `struct` | Struct definition |
| `enum` | Enum definition |
| `interface` | Interface/protocol definition |
| `trait` | Trait definition |
| `module` | Module declaration |
| `constant` | Constant definition |
| `variable` | Variable declaration |
| `imports` | Import statements |
| `top_level` | Code outside symbols |
| `function_part` | Part of a split function |
| `class_part` | Part of a split class |

### Auto-Generated Tags

The system automatically generates semantic tags for better retrieval:

| Tag | Triggers |
|-----|----------|
| `async` | `async`, `await`, `suspend` (Kotlin) in signature |
| `concurrency` | `thread`, `mutex`, `lock`, `spawn`, `channel`, `goroutine`, Go sync patterns |
| `security` | `password`, `token`, `secret`, `auth`, `crypt`, `hash` |
| `error-handling` | `Error`, `Result`, `exception`, `panic` |
| `database` | `query`, `sql`, `database`, `repository`, `transaction` |
| `http` | `http`, `request`, `response`, `endpoint`, `handler` |
| `cli` | `command`, `cli`, `arg`, `flag`, `subcommand` |
| `config` | `config`, `setting`, `preference`, `env` |
| `logging` | `log`, `trace`, `debug`, `warn`, `metric` |
| `cache` | `cache`, `memoize`, `invalidate` |
| `validation` | `valid`, `check`, `verify`, `assert`, `sanitize` |
| `serialization` | `serialize`, `json`, `yaml`, `encode`, `decode`, `parse` |
| `io` | `file`, `read`, `write`, `path`, `fs` |
| `network` | `socket`, `connect`, `tcp`, `udp`, `client`, `server` |
| `init` | `new`, `init`, `setup`, `create` |
| `cleanup` | `cleanup`, `teardown`, `close`, `dispose`, `drop` |
| `test` | `test_`, `_test`, `Test`, `mock`, `stub`, `fixture` |
| `deprecated` | `deprecated`, `Deprecated` in signature |
| `public-api` | `pub fn`, `pub async fn`, `export` |

### Complexity Metrics

Each chunk includes complexity information:

| Metric | Description |
|--------|-------------|
| `lines_of_code` | Lines excluding blank lines and comments |
| `max_nesting_depth` | Maximum bracket/indentation nesting depth |

## Diff Summary

When a manifest exists, the output includes a diff summary:

```json
{
  "diff": {
    "added": 5,
    "modified": 12,
    "removed": 3,
    "unchanged": 450,
    "total_chunks": 467
  }
}
```

## Embedding Model Presets

Optimal chunk sizes for popular embedding models:

| Model | Max Tokens | Usage |
|-------|------------|-------|
| `voyage-code-2/3` | 1500 | Large code context |
| `openai-text-embedding-3` | 800 | Balanced |
| `cohere-embed-v3` | 400 | Smaller model |
| `sentence-transformers` | 384 | BERT-based |

```bash
# Use preset for Voyage embedding model
infiniloom embed --max-tokens 1500
```

## Resource Limits

The command enforces limits to prevent resource exhaustion:

| Limit | Default | Description |
|-------|---------|-------------|
| Max file size | 10 MB | Files larger than this are skipped |
| Max line length | 10,000 | Lines longer trigger minified file detection |
| Max total chunks | 1,000,000 | Enterprise-scale limit |
| Max files | 500,000 | Large monorepo scale |
| Max recursion depth | 500 | Handles deeply nested code |

## Examples

### Basic Usage

```bash
# Generate chunks for current directory
infiniloom embed

# Generate chunks for specific directory
infiniloom embed /path/to/repo -o chunks.json

# Verbose output with progress
infiniloom embed -v
```

### Incremental Workflow

```bash
# First run: full generation
infiniloom embed -o chunks.json
# Created manifest: .infiniloom-embed.bin

# After code changes: only changed chunks
infiniloom embed --diff-only -o changed-chunks.json
# Output: Added: 2, Modified: 5, Removed: 1
```

### CI/CD Pipeline

```bash
# Fail if secrets detected
infiniloom embed --fail-on-secrets -o chunks.json

# Custom manifest location (shared across builds)
infiniloom embed --manifest-path .cache/embed-manifest.bin
```

### Vector Database Integration

```bash
# Generate chunks optimized for Pinecone/Weaviate
infiniloom embed --max-tokens 1000 -o chunks.json

# Only get chunks that need to be updated
infiniloom embed --diff-only -o upsert.json
```

### Filtering

```bash
# Only Rust files
infiniloom embed --include "*.rs" -o rust-chunks.json

# Exclude test and generated files
infiniloom embed --exclude "tests/*" --exclude "*.generated.*"

# Include test files
infiniloom embed --include-tests
```

## Manifest File

The manifest (`.infiniloom-embed.bin`) stores:

- All chunk IDs and hashes
- Settings used for generation
- Integrity checksum
- Last update timestamp

### Manifest Operations

```bash
# Force rebuild (ignore existing manifest)
rm .infiniloom-embed.bin
infiniloom embed

# Use custom manifest location
infiniloom embed --manifest-path my-manifest.bin
```

## Security Features

### Secret Detection

The embed command scans for:
- AWS keys (`AKIA...`)
- GitHub tokens (`ghp_...`, `github_pat_...`)
- Private keys (RSA, EC, DSA, OpenSSH)
- API keys (OpenAI `sk-...`, Anthropic `sk-ant-...`, Stripe)
- Database connection strings
- JWT tokens
- Generic secrets and passwords

### Redaction

When `--redact-secrets` is enabled (default), detected secrets are replaced:

```
Original: const API_KEY = "sk-proj-abc123xyz789"
Redacted: const API_KEY = "sk-p************z789"
```

The redaction preserves the first 4 and last 4 characters, replacing the middle with asterisks.

### Homoglyph Attack Prevention

The scanner uses NFKC Unicode normalization to detect obfuscated secrets using lookalike characters (e.g., fullwidth characters like `ＡＫＩＡ` → `AKIA`).

## Language Support

Embedding chunk generation supports all 21 languages from the Tree-sitter parser:

- **Systems**: Rust, C, C++, Go
- **Web**: JavaScript, TypeScript, JSX, TSX
- **Enterprise**: Java, C#, Kotlin, Scala
- **Scripting**: Python, Ruby, PHP, Bash
- **Functional**: Haskell, Elixir
- **Mobile**: Swift
- **Data**: YAML, TOML

## API Usage

### Rust

```rust
use infiniloom_engine::embedding::{EmbedChunker, EmbedSettings, ResourceLimits};

let settings = EmbedSettings::default();
let limits = ResourceLimits::default();
let chunker = EmbedChunker::new(settings, limits);

let chunks = chunker.chunk_repository(Path::new("/path/to/repo"))?;

for chunk in &chunks {
    println!("Chunk {}: {} tokens", chunk.id, chunk.tokens);
}
```

### Node.js

```javascript
const { embed } = require('infiniloom-node');

const result = embed('./my-repo', {
  maxTokens: 1000,
  securityScan: true,
  diffOnly: false
});

console.log(`Generated ${result.chunks.length} chunks`);

for (const chunk of result.chunks) {
  console.log(`${chunk.id}: ${chunk.source.symbol} (${chunk.tokens} tokens)`);
}
```

### Python

```python
from infiniloom import embed

result = embed("./my-repo", max_tokens=1000, security_scan=True)

print(f"Generated {len(result.chunks)} chunks")

for chunk in result.chunks:
    print(f"{chunk.id}: {chunk.source.symbol} ({chunk.tokens} tokens)")
```

## Determinism Guarantees

The embed command provides strong determinism:

1. **Files processed in sorted lexicographic order**
2. **Symbols within files sorted by (line, name)**
3. **Output chunks sorted by (file, line, id)**
4. **All hash computations use integer-only math** (no floats)
5. **Cross-platform identical output** (Windows/Linux/macOS)
6. **BLAKE3 for cryptographically strong, fast hashing**

## Performance

- **Parallel processing**: Uses all available CPU cores via Rayon
- **Thread-local parsers**: Eliminates mutex contention
- **ASCII fast path**: Skips NFKC normalization for ASCII-only content (~99% of code)
- **Incremental updates**: Only re-processes changed files
- **Memory-efficient**: Streams chunks instead of loading all into memory

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Error (invalid path, I/O error, etc.) |
| 1 | Secrets detected with `--fail-on-secrets` |

## See Also

- [`infiniloom pack`](pack.md) - Generate LLM context from repository
- [`infiniloom index`](index.md) - Build symbol index for fast diff context
- [`infiniloom scan`](scan.md) - Scan repository statistics
