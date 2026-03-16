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
infiniloom embed ./my-repo --diff -o changes.json

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
- `infiniloom embed --include "*.py" -o python-chunks.json` - Python only
- `infiniloom embed --diff -o changes.json` - Only output changed chunks

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

### Why AST-Aware vs Character-Based Splitting?

| Aspect | AST-Aware (Infiniloom) | Character-Based (typical) |
|--------|------------------------|---------------------------|
| **Boundary** | Function/class boundaries | Fixed character count |
| **Context** | Complete semantic units | May split mid-statement |
| **Search quality** | Higher precision | Noisy partial matches |
| **Deduplication** | Content-addressable IDs | Offset-dependent IDs |
| **Updates** | Stable IDs across versions | IDs change with any edit |

**Example - Character-based (problematic):**
```
Chunk 1: "function authenticate(user, pass) { if (!user) { throw new"
Chunk 2: "Error('User required'); } return validateCredentials(user, pa"
Chunk 3: "ss); }"
```

**Example - AST-aware (Infiniloom):**
```
Chunk 1: "function authenticate(user, pass) {
  if (!user) {
    throw new Error('User required');
  }
  return validateCredentials(user, pass);
}"
```

AST-aware chunking ensures:
- Complete functions are searchable as units
- Semantic tags (`async`, `security`) apply to whole functions
- Call graph extraction works correctly
- Embeddings capture full context

## Options

### Output Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--output <PATH>` | `-o` | Write JSON output to file | stdout |
| `--manifest <PATH>` | `-m` | Custom manifest file location | `.infiniloom-embed.bin` |
| `--diff` | | Only output changed chunks (added + modified) | `false` |

### Streaming & Storage Options (v0.7.0)

| Option | Description | Default |
|--------|-------------|---------|
| `--streaming` | Enable streaming JSONL output (lower memory usage for large repos) | `false` |
| `--batch-size <N>` | Files per batch in streaming mode | `50` |
| `--sqlite-manifest` | Use SQLite for manifest storage (WAL mode, concurrent reads) | `false` |
| `--since <COMMIT>` | Only process files changed since this git commit | none |
| `--since-manifest` | Use commit hash from manifest for `--since` | `false` |

### Enrichment Options (v0.7.0)

| Option | Description | Default |
|--------|-------------|---------|
| `--include-signatures` | Generate signature-only chunks alongside code chunks | `false` |
| `--enable-hierarchy` | Enable parent/children chunk linking | `false` |
| `--hierarchy-min-children <N>` | Minimum children for hierarchy summary | `2` |
| `--git-metadata` | Enrich chunks with git commit metadata | `false` |
| `--repo-namespace <NS>` | Repository namespace for cross-repo identity | none |
| `--repo-name <NAME>` | Repository name override | auto-detected |

### Export Options (v0.7.0)

| Option | Description | Default |
|--------|-------------|---------|
| `--graph-export` | Generate Neptune-compatible vertices/edges JSONL | `false` |
| `--graph-dir <DIR>` | Output directory for graph files | `.` |
| `--generate-schema <TYPE>` | Generate database schema and exit (e.g., `pgvector`) | none |
| `--embedding-dims <N>` | Embedding vector dimensions for schema generation | `1536` |

### Token Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--max-tokens <N>` | | Maximum tokens per chunk | `1000` |
| `--min-tokens <N>` | | Minimum tokens per chunk (smaller merged) | `50` |
| `--context-lines <N>` | | Lines of context around symbols | `5` |
| `--token-model <MODEL>` | | Token counting model | `claude` |

### Content Options

| Option | Description | Default |
|--------|-------------|---------|
| `--include <PATTERN>`, `-i` | Include only files matching glob pattern (repeatable) | all |
| `--exclude <PATTERN>`, `-e` | Exclude files matching glob pattern (repeatable) | none |
| `--no-imports` | Exclude import statements from chunks | `false` |
| `--no-top-level` | Exclude top-level code outside symbols | `false` |
| `--include-tests` | Include test files | `false` |

### Security Options

| Option | Description | Default |
|--------|-------------|---------|
| `--no-security-scan` | Disable secret detection (enabled by default) | `false` |

Note: Secret redaction is automatic when security scanning is enabled.

### Output Control

| Option | Description | Default |
|--------|-------------|---------|
| `--format <FORMAT>` | Output format: `jsonl` or `json` | `jsonl` |
| `--verbose`, `-v` | Show progress and statistics | `false` |
| `--quiet`, `-q` | Suppress all output except chunk data | `false` |
| `--json-stats` | Output statistics as JSON to stderr | `false` |

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
    "parent_chunk_id": null,
    "children_chunk_ids": [],
    "visibility": "public",
    "is_test": false,
    "module_path": "src/math"
  },
  "context": {
    "docstring": "Adds two numbers together",
    "signature": "fn calculate(a: i32, b: i32) -> i32",
    "type_signature": "fn(i32, i32) -> i32",
    "parameter_types": ["i32", "i32"],
    "return_type": "i32",
    "calls": ["add", "validate"],
    "qualified_calls": ["math::add", "util::validate"],
    "called_by": ["main", "process"],
    "keywords": ["calculate", "add", "validate"],
    "tags": ["public-api"],
    "summary": "Public function `calculate` in src/math.rs (3 lines, 2 calls)",
    "lines_of_code": 3,
    "max_nesting_depth": 1,
    "cyclomatic_complexity": 1,
    "git_last_commit": "abc1234",
    "git_last_author": "dev@example.com",
    "git_last_date": "2026-03-10"
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
infiniloom embed --diff -o changed-chunks.json
# Output: Added: 2, Modified: 5, Removed: 1
```

### CI/CD Pipeline

```bash
# Security scanning is enabled by default
# To disable it (not recommended):
infiniloom embed --no-security-scan -o chunks.json

# Custom manifest location (shared across builds)
infiniloom embed -m .cache/embed-manifest.bin -o chunks.json
```

### Streaming Mode (Large Repos)

```bash
# Stream chunks for memory-efficient processing of large monorepos
infiniloom embed . --streaming --batch-size 100 -o chunks.jsonl

# Streaming with SQLite manifest for concurrent CI/CD
infiniloom embed . --streaming --sqlite-manifest -o chunks.jsonl
```

### Git-Diff Incremental Updates

```bash
# Only process files changed since last commit
infiniloom embed . --since HEAD~1 -o updates.jsonl

# Use the commit stored in manifest for automatic tracking
infiniloom embed . --since-manifest -o updates.jsonl
```

### Graph Database Export

```bash
# Generate Neptune-compatible graph files
infiniloom embed . --graph-export --graph-dir ./graph-output/

# Generates: vertices.jsonl and edges.jsonl
```

### pgvector Schema Generation

```bash
# Generate PostgreSQL pgvector schema
infiniloom embed . --generate-schema pgvector --embedding-dims 1536
```

### Cross-Repository Identity

```bash
# Set namespace for cross-repo deduplication
infiniloom embed . --repo-namespace "myorg" --repo-name "backend"
```

### Signature-Only Chunks

```bash
# Generate lightweight signature chunks for tiered retrieval
infiniloom embed . --include-signatures -o chunks.jsonl
```

### Hierarchical Chunking

```bash
# Enable parent-child chunk relationships
infiniloom embed . --enable-hierarchy --hierarchy-min-children 3
```

### Vector Database Integration

```bash
# Generate chunks optimized for Pinecone/Weaviate
infiniloom embed --max-tokens 1000 -o chunks.json

# Only get chunks that need to be updated
infiniloom embed --diff -o upsert.json
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

Embedding chunk generation supports all 24 languages with full Tree-sitter AST support:

- **Systems**: Rust, C, C++, Go, Zig
- **Web**: JavaScript, TypeScript, JSX, TSX
- **Enterprise**: Java, C#, Kotlin, Scala
- **Scripting**: Python, Ruby, PHP, Bash
- **Functional**: Haskell, Elixir
- **Mobile**: Swift, Dart
- **Infrastructure**: HCL/Terraform
- **Data**: YAML, TOML

**Deprecated Languages**: Clojure and F# are deprecated as of v0.7.0 (no compatible tree-sitter grammars). Files are detected but receive text-only processing without AST symbols.

### Known Limitations

**OCaml**: Similar ML-family syntax challenges apply. Symbol boundaries may not be perfectly detected for complex pattern matching expressions.

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

## Auto-Generated Chunk Context (v0.7.0)

Each chunk is automatically enriched with semantic metadata:

| Field | Description |
|-------|-------------|
| `type_signature` | Complete type signature (e.g., `fn(i32, i32) -> i32`) |
| `parameter_types` | List of parameter types |
| `return_type` | Return type |
| `keywords` | BM25-friendly identifiers extracted from content |
| `qualified_calls` | Import-resolved call targets with module paths |
| `summary` | Auto-generated natural language description |
| `cyclomatic_complexity` | McCabe complexity score |
| `git_last_commit` | Last commit hash touching this code |
| `git_last_author` | Author of last commit |
| `git_last_date` | Date of last commit |
| `parent_chunk_id` | ID of parent container chunk |
| `children_chunk_ids` | IDs of child member chunks |
| `module_path` | Module/directory path for the symbol |

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
