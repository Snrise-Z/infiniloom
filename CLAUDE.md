# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Infiniloom** is a high-performance repository context generator for Large Language Models. It transforms codebases into optimized formats for Claude, GPT-4o/GPT-5, Gemini, and other LLMs. Built in pure Rust for maximum performance and portability.

Key capabilities:
- AST-based symbol extraction using Tree-sitter (21 languages with full support)
- PageRank-based symbol importance ranking
- Model-specific output formats (XML for Claude, Markdown for GPT, YAML for Gemini)
- Automatic secret detection and redaction with configurable patterns
- Accurate token counting via tiktoken-rs for OpenAI models
- Native language bindings (Python, Node.js)

## Build Commands

```bash
# Build release binary
cargo build --release
# Binary at ./target/release/infiniloom

# Run tests
cargo test --workspace

# Run tests with output
cargo test -- --nocapture

# Run specific crate tests
cargo test -p infiniloom-engine

# Clippy linting (strict)
cargo clippy --workspace --all-targets --all-features

# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check

# Run benchmarks
cargo bench --workspace

# Generate documentation
cargo doc --workspace --all-features --no-deps

# Code coverage (requires cargo-llvm-cov)
cargo llvm-cov --workspace --all-features --html --output-dir target/coverage
```

### Makefile Shortcuts

```bash
make build          # Debug build
make build-release  # Release build
make test           # Run all tests
make lint           # Run strict clippy
make fmt            # Format all code
make coverage       # Generate HTML coverage report
make ci             # Full CI pipeline (format, lint, test, coverage)
make pre-commit     # Quick pre-commit checks
```

## CLI Usage

```bash
# Pack repository into XML (Claude-optimized)
infiniloom pack /path/to/repo --format xml
infiniloom pack . --model gpt4o --compression aggressive
infiniloom pack . -i "src/**/*.rs" -e "tests/*"  # Include/exclude patterns
infiniloom pack . --include-tests               # Include test files
infiniloom pack . --security-check              # Scan for secrets
infiniloom pack . --redact-secrets              # Redact detected secrets

# Scan repository and show statistics
infiniloom scan /path/to/repo
infiniloom scan . --model gpt4o                 # Token counts for specific model
infiniloom scan . -v                            # Verbose file list
infiniloom scan . --json                        # JSON output
infiniloom scan . -i "src/**" -e "vendor/*"     # Include/exclude patterns
infiniloom scan . --include-tests               # Include test files

# Generate repository map with key symbols
infiniloom map /path/to/repo --budget 2000
infiniloom map . -m gpt4o -v                    # Verbose with model
infiniloom map . -i "src/**" --include-tests    # With patterns

# Show version and configuration info
infiniloom info

# Initialize configuration file
infiniloom init

# Build/update symbol index for fast diff context
infiniloom index /path/to/repo
infiniloom index --force                        # Force full rebuild
infiniloom index --status                       # Show index stats
infiniloom index --incremental                  # Only re-index changed files
infiniloom index -i "src/**" -e "vendor/*"      # Include/exclude patterns
infiniloom index --include-tests                # Include test files

# Get context for a diff (changed files, dependents, tests)
infiniloom diff                                 # Unstaged changes
infiniloom diff --staged                        # Staged changes
infiniloom diff HEAD~1                          # Last commit
infiniloom diff main..feature                   # Branch comparison
infiniloom diff --depth 2                       # Context depth (1-3, default: 2)
infiniloom diff --budget 50000                  # Token budget limit
infiniloom diff --include-diff                  # Include actual diff content (+/- lines)
infiniloom diff --format json                   # Output format (xml/json/markdown/yaml)
infiniloom diff -m gpt4o                        # Token counting model
infiniloom diff -i "src/**" -e "vendor/*"       # Include/exclude patterns
infiniloom diff --include-tests                 # Include test files

# Analyze impact of changes
infiniloom impact src/auth.rs                   # What depends on this file?
infiniloom impact --symbol "foo"                # What calls this symbol?
infiniloom impact . src/auth.rs --depth 3       # Full transitive analysis
infiniloom impact . --call-graph                # Show call graph
infiniloom impact . src/auth.rs -m gpt4o        # Token counting model
infiniloom impact . -i "src/**" --include-tests # With patterns

# Chunk repository for multi-turn conversations
infiniloom chunk /path/to/repo
infiniloom chunk . --strategy module            # Group by module/directory
infiniloom chunk . --max-tokens 4000            # Smaller chunks
infiniloom chunk . --overlap 500                # Overlap for context continuity
infiniloom chunk . -i "src/**" --include-tests  # With patterns

# Generate embedding chunks for vector databases (RAG)
infiniloom embed /path/to/repo                  # Generate JSONL output to stdout
infiniloom embed . -o chunks.jsonl              # Output to file
infiniloom embed . --format json                # Single JSON array format
infiniloom embed . --diff-only                  # Only output changed chunks
infiniloom embed . --max-tokens 512             # Token limit per chunk
infiniloom embed . --min-tokens 50              # Minimum tokens per chunk
infiniloom embed . --context-lines 2            # Context lines around symbols
infiniloom embed . --token-model claude         # Token counting model
infiniloom embed . --no-imports                 # Exclude import statements
infiniloom embed . --no-top-level               # Exclude top-level code
infiniloom embed . --no-security-scan           # Disable secret scanning
infiniloom embed . -i "src/**" -e "tests/*"     # Include/exclude patterns
infiniloom embed . --include-tests              # Include test files
infiniloom embed . -v                           # Verbose output with stats
infiniloom embed . --json-stats                 # JSON statistics output

# Ingest documents into LLM-optimized format
infiniloom ingest report.md                     # Markdown → XML (default)
infiniloom ingest page.html -f markdown         # HTML → Markdown
infiniloom ingest data.csv -f json              # CSV → JSON
infiniloom ingest report.docx -o output.xml     # DOCX → XML file
infiniloom ingest doc.md -d aggressive          # Heavy distillation
infiniloom ingest doc.md --pii-scan             # Scan for PII
infiniloom ingest doc.md --redact-pii           # Redact PII in output
infiniloom ingest doc.md --chunk                # Split into chunks
infiniloom ingest doc.md --chunk --max-chunk-tokens 8000
infiniloom ingest doc.md -v                     # Verbose output
```

## Code Architecture

### Workspace Structure

```
infiniloom/
├── cli/                        # CLI application (clap-based)
│   └── src/
│       ├── main.rs             # CLI entry point, argument parsing
│       ├── config.rs           # Configuration loading utilities
│       ├── scanner.rs          # Repository scanning with parallel processing
│       └── commands/           # Individual command implementations
│           ├── mod.rs          # Command module exports
│           ├── pack.rs         # Pack command (main output generation)
│           ├── scan.rs         # Scan command (statistics)
│           ├── map.rs          # Map command (symbol ranking)
│           ├── chunk.rs        # Chunk command (repo splitting)
│           ├── diff.rs         # Diff command (context-aware diffs)
│           ├── index.rs        # Index command (build symbol index)
│           ├── impact.rs       # Impact command (change analysis)
│           ├── embed.rs        # Embed command (vector DB chunks)
│           ├── ingest.rs       # Ingest command (document ingestion)
│           ├── init.rs         # Init command (config file creation)
│           └── info.rs         # Info command (version/config display)
├── engine/                     # Core Rust engine library
│   └── src/
│       ├── lib.rs              # Public API exports
│       ├── types.rs            # Core types: Repository, RepoFile, Symbol
│       ├── constants.rs        # Shared constants and magic numbers
│       ├── newtypes.rs         # Type-safe wrappers (SymbolId, FileId, etc.)
│       ├── error.rs            # Error types
│       ├── parser/             # Tree-sitter AST parsing (21 languages)
│       │   ├── mod.rs          # Parser module exports
│       │   ├── core.rs         # Core Parser struct and methods
│       │   ├── language.rs     # Language enum and detection
│       │   ├── extraction.rs   # Symbol extraction from AST
│       │   ├── init.rs         # Tree-sitter initialization
│       │   ├── queries.rs      # Tree-sitter query definitions
│       │   └── query_builder.rs # Dynamic query construction
│       ├── tokenizer/          # Multi-model token counting
│       │   ├── mod.rs          # Tokenizer module exports
│       │   ├── core.rs         # Tokenizer struct and counting
│       │   ├── models.rs       # TokenizerModel enum (27 models)
│       │   └── counts.rs       # TokenCounts struct
│       ├── repomap/            # PageRank symbol ranking
│       │   ├── mod.rs          # RepoMapGenerator
│       │   └── graph.rs        # SymbolGraph, PageRank computation
│       ├── output/             # Format generators
│       │   ├── mod.rs          # OutputFormatter trait
│       │   ├── xml.rs          # Claude-optimized XML
│       │   ├── markdown.rs     # GPT-optimized Markdown
│       │   └── toon.rs         # Token-efficient TOON format
│       ├── chunking/           # Semantic code chunking
│       │   ├── mod.rs          # Chunker struct
│       │   ├── strategies.rs   # ChunkStrategy implementations
│       │   └── types.rs        # Chunk types
│       ├── embedding/          # Embedding chunks for vector DBs
│       │   ├── mod.rs          # Module exports
│       │   ├── chunker.rs      # EmbedChunker with parallel processing
│       │   ├── types.rs        # EmbedChunk, EmbedSettings, ChunkKind
│       │   ├── manifest.rs     # Manifest for incremental updates
│       │   ├── hasher.rs       # BLAKE3 content-addressable hashing
│       │   ├── normalizer.rs   # Cross-platform content normalization
│       │   ├── limits.rs       # Resource limits (DoS protection)
│       │   ├── progress.rs     # Progress reporting
│       │   └── error.rs        # Embedding-specific errors
│       ├── document/           # Document ingestion (feature-gated)
│       │   ├── mod.rs          # parse_document() entry point
│       │   ├── types.rs        # Document, Section, ContentBlock types
│       │   ├── parsers/        # Format-specific parsers
│       │   │   ├── markdown.rs # CommonMark + GFM tables
│       │   │   ├── html.rs     # HTML tag stripping
│       │   │   ├── csv.rs      # CSV/TSV with auto-delimiter
│       │   │   ├── docx.rs     # DOCX via ZIP + XML
│       │   │   └── xlsx.rs     # XLSX via calamine (optional)
│       │   ├── distillation/   # Content compression pipeline
│       │   ├── pii.rs          # PII detection and redaction
│       │   ├── chunking.rs     # Document chunking
│       │   └── output.rs       # XML, Markdown, JSON formatters
│       ├── index/              # Symbol index for fast diff context
│       │   ├── mod.rs          # Module exports
│       │   ├── builder/        # Index building
│       │   │   ├── mod.rs      # IndexBuilder struct
│       │   │   ├── core.rs     # Build logic with parallel parsing
│       │   │   ├── graph.rs    # Dependency graph construction
│       │   │   └── types.rs    # Builder-specific types
│       │   ├── context/        # Diff context expansion
│       │   │   ├── mod.rs      # Context module exports
│       │   │   ├── expander.rs # ContextExpander implementation
│       │   │   └── types.rs    # Context types (DiffChange, etc.)
│       │   ├── lazy.rs         # On-the-fly context generation
│       │   ├── storage.rs      # Bincode serialization
│       │   ├── types.rs        # Index types (SymbolIndex, DepGraph)
│       │   ├── query.rs        # Call graph query API
│       │   ├── convert.rs      # Type conversion utilities
│       │   └── patterns.rs     # Pre-compiled regex patterns
│       ├── ranking.rs          # File importance ranking
│       ├── security.rs         # Secret detection/redaction
│       ├── budget.rs           # Token budget management
│       ├── semantic.rs         # Semantic compression
│       ├── config.rs           # Configuration loading (YAML/TOML/JSON)
│       ├── git.rs              # Git operations (log, status, diff, hunks)
│       ├── remote.rs           # Remote repository cloning
│       ├── dependencies.rs     # Dependency graph resolution
│       ├── mmap_scanner.rs     # Memory-mapped file scanning
│       ├── incremental.rs      # Incremental caching with change detection
│       └── default_ignores.rs  # Default ignore patterns
├── bindings/                   # Language bindings
│   ├── common/                 # Shared bindings code
│   │   └── src/
│   │       ├── lib.rs          # Common utilities (parse_format, parse_model, etc.)
│   │       ├── scanner.rs      # Shared repository scanning
│   │       └── repo_ops.rs     # Shared repository operations
│   ├── python/                 # PyO3 bindings (maturin)
│   │   └── src/lib.rs          # Python module: pack, scan, GitRepo, etc.
│   └── node/                   # NAPI-RS bindings
│       └── src/lib.rs          # Node module: pack, scan, GitRepo, etc.
└── packages/                   # Distribution packages
    └── infiniloom/             # npm CLI wrapper (downloads binary)
```

### Core Types (`engine/src/types.rs`)

- **`Repository`**: Root container with name, path, files, and metadata
- **`RepoFile`**: Single file with path, language, token counts, symbols, importance score
- **`Symbol`**: Extracted code symbol (function, class, etc.) with kind, signature, line numbers
- **`SymbolKind`**: Function, Class, Method, Struct, Enum, Trait, Interface, Constant, Variable, Import, Type
- **`Visibility`**: Public, Private, Protected, Internal
- **`CompressionLevel`**: None, Minimal, Balanced, Aggressive, Extreme, Focused, Semantic

### Tokenizer Types (`engine/src/tokenizer/`)

- **`TokenCounts`**: Token counts for multiple models grouped by encoding family:
  - `o200k`: OpenAI modern (GPT-5.x, GPT-4o, O1/O3/O4) - exact via tiktoken
  - `cl100k`: OpenAI legacy (GPT-4, GPT-3.5-turbo) - exact via tiktoken
  - `claude`, `gemini`, `llama`, `mistral`, `deepseek`, `qwen`, `cohere`, `grok`: estimation-based
- **`TokenizerModel`**: Enum with 27 supported LLM tokenizers
- **`Tokenizer`**: Thread-safe tokenizer with lazy tiktoken initialization

### Newtype Wrappers (`engine/src/newtypes.rs`)

Type-safe wrappers to prevent ID confusion:
- **`SymbolId`**: Unique identifier for symbols within an index
- **`FileId`**: Unique identifier for files within an index
- **`LineNumber`**: 1-indexed line number

### Index and Call Graph Types (`engine/src/index/`)

- **`SymbolIndex`**: Stores all symbols with fast lookup by name
- **`DepGraph`**: Dependency graph storing call edges as `(caller_id, callee_id)` pairs
- **`SymbolInfo`**: Query result with id, name, kind, file, line numbers, signature, visibility
- **`CallGraph`**: Complete graph with nodes (symbols), edges (calls), and statistics
- **`CallGraphEdge`**: Single edge with caller/callee IDs, names, file, and line

### Call Graph Query API (`engine/src/index/query.rs`)

High-level functions for querying symbol relationships:
- `find_symbol(index, name)` - Find symbols by name
- `get_callers_by_name(index, graph, name)` - Get all callers of a symbol
- `get_callees_by_name(index, graph, name)` - Get all callees of a symbol
- `get_references_by_name(index, graph, name)` - Get all references (calls + imports)
- `get_call_graph(index, graph)` - Get complete call graph
- `get_call_graph_filtered(index, graph, max_nodes, max_edges)` - Get filtered graph

### Embedding Types (`engine/src/embedding/`)

Types for generating deterministic, content-addressable chunks for vector databases:

- **`EmbedChunk`**: Single chunk with content-addressable ID (BLAKE3 hash), content, tokens, kind, source location, and context
- **`EmbedSettings`**: Configuration for chunk generation (max_tokens, min_tokens, context_lines, security options)
- **`EmbedManifest`**: Tracks all chunks for incremental updates (bincode serialized with integrity checksum)
- **`EmbedDiff`**: Result of diffing current chunks against manifest (added, modified, removed, unchanged)
- **`ChunkKind`**: Function, Method, Class, Struct, Enum, Interface, Trait, Module, etc.
- **`ChunkSource`**: Source location metadata (file, lines, symbol name, language, parent, visibility)
- **`ChunkContext`**: Semantic context (docstring, signature, calls, imports, auto-generated tags)
- **`ResourceLimits`**: DoS protection limits (max files, file size, chunks, recursion depth)

**Key Features**:
- Deterministic output (same input = same output) via sorted processing and BTreeMap
- Content-addressable IDs: `ec_` + 32 hex chars (128-bit BLAKE3 truncation)
- Cross-platform normalization (Unicode NFC, line endings, whitespace)
- Secret scanning with redaction support
- Incremental updates via manifest diffing

### Data Flow

1. **Scanning** (`cli/scanner.rs`): Walk directory with `ignore` crate, filter by gitignore, detect languages
2. **Parsing** (`engine/src/parser/`): Tree-sitter AST extraction for symbols (thread-local parsers for parallelism)
3. **Ranking** (`ranking.rs`, `repomap/`): PageRank-based importance scoring
4. **Formatting** (`output/`): Model-specific output generation
5. **Security** (`security.rs`): Secret detection before output

### Key Patterns

**Parallel File Processing with Thread-Local Parsers**:
```rust
// cli/scanner.rs - Lock-free parallel parsing
thread_local! {
    static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

files.into_par_iter()
    .filter_map(|file| {
        let content = fs::read_to_string(&file.path).ok()?;
        let symbols = THREAD_PARSER.with(|p| p.borrow_mut().parse(&content, lang));
        Some(RepoFile { content, symbols, ... })
    })
    .collect()
```

**PageRank Ranking** (`repomap/graph.rs`):
- Builds symbol graph from imports/references
- Computes PageRank with damping factor 0.85
- Top symbols returned with importance scores

**Accurate Token Counting** (`engine/src/tokenizer/`):
```rust
// Uses tiktoken-rs for exact OpenAI token counts
let tokenizer = Tokenizer::new();
let gpt4o_tokens = tokenizer.count(content, TokenModel::Gpt4o);   // Exact via tiktoken (o200k_base)
let gpt4_tokens = tokenizer.count(content, TokenModel::Gpt4);     // Exact via tiktoken (cl100k_base)
let claude_tokens = tokenizer.count(content, TokenModel::Claude); // Calibrated estimation
```

**Output Formatting**:
```rust
// OutputFormatter chooses format based on target model
let formatter = OutputFormatter::by_format(OutputFormat::Xml);
let output = formatter.format(&repo, &map);
```

## Feature Flags

```toml
# engine/Cargo.toml features
default = ["document"]
document = ["zip", "quick-xml"]      # Document ingestion (MD, HTML, CSV, DOCX)
document-xlsx = ["document", "calamine"]  # XLSX spreadsheet support
async = ["tokio", "async-trait"]     # Async operations (placeholder - not yet implemented)
embeddings = ["candle-core", "candle-transformers"]  # Local embeddings (heuristic-based)
watch = ["notify"]                   # File watching (implemented for pack --watch)
full = ["async", "embeddings", "watch", "document", "document-xlsx"]
```

**Note**: Git operations use the CLI (`git` command) via `std::process::Command` rather than Rust crates.

## Testing

```bash
# Unit tests
cargo test --workspace

# Specific test
cargo test test_generate_repomap

# Integration tests with verbose output
cargo test --workspace -- --nocapture

# Property-based tests (using proptest)
cargo test proptest
```

Test files are in `engine/src/*/tests` modules and `tests/` directories.

## Linting Configuration

The project uses strict clippy lints defined in `Cargo.toml`:
- `correctness` and `perf` are **deny** (errors)
- `suspicious`, `complexity`, `style` are **warn**
- Print macros (`print_stdout`, `print_stderr`) are warned except in CLI

## Language Bindings Development

### Python (PyO3 + Maturin)
```bash
cd bindings/python
pip install maturin
maturin develop  # Development build
maturin build --release  # Release wheel
```

### Node.js (NAPI-RS)
```bash
cd bindings/node
npm install
npm run build
```

## Configuration Files

- **`.infiniloom.yaml`** / **`.infiniloom.toml`**: Project configuration
- **`.infiniloomignore`**: Additional ignore patterns (like .gitignore)

Run `infiniloom init` to create a configuration file. Both nested and flat formats are supported.

Example `.infiniloom.yaml`:
```yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 100000
  line_numbers: true
  show_file_summary: true

scan:
  include:
    - "*.rs"
    - "*.py"
    - "*.ts"
  exclude:
    - "tests/*"
    - "docs/*"
    - "*.test.*"

security:
  scan_secrets: true           # Enable secret scanning
  fail_on_secrets: false       # Exit with error if secrets found (CI/CD)
  redact_secrets: true         # Replace secrets with [REDACTED]
  allowlist:                   # Patterns to ignore
    - "EXAMPLE"
    - "test_key"
  custom_patterns:             # Additional regex patterns
    - "MY_SECRET_[A-Z0-9]{32}"
```

Flat format (legacy, also supported):
```yaml
include:
  - "*.rs"
exclude:
  - "tests/*"
include_tests: false
include_docs: false
```

## Environment Variables

Environment variables use double underscore (`__`) to separate nested config keys:

| Variable | Description | Default |
|----------|-------------|---------|
| `INFINILOOM_OUTPUT__MODEL` | Default tokenizer model | `claude` |
| `INFINILOOM_OUTPUT__FORMAT` | Default output format | `xml` |
| `INFINILOOM_OUTPUT__COMPRESSION` | Default compression | `balanced` |
| `INFINILOOM_OUTPUT__TOKEN_BUDGET` | Default token budget | `0` (no limit) |
| `INFINILOOM_SCAN__INCLUDE_HIDDEN` | Include hidden files | `false` |
| `INFINILOOM_SCAN__RESPECT_GITIGNORE` | Respect .gitignore | `true` |
| `INFINILOOM_SECURITY__SCAN_SECRETS` | Enable secret scanning | `false` |
| `INFINILOOM_SECURITY__REDACT_SECRETS` | Redact detected secrets | `true` |

**Note**: CLI default for `--max-tokens` is 0 (no limit). Config files can override this.

## CI/CD

GitHub Actions workflow (`.github/workflows/ci.yml`) runs:
1. Format check (`cargo fmt --check`)
2. Clippy linting
3. Build (Ubuntu + macOS)
4. Tests
5. Python/Node.js binding builds
6. Security scan (Trivy)
7. Code coverage (Codecov)

## Performance Architecture

The project is optimized for high performance through careful Rust design patterns.

### Key Performance Features

#### 1. Thread-Local Parsers (`cli/scanner.rs`)
Each Rayon worker thread has its own Tree-sitter parser instance, eliminating mutex contention:
```rust
thread_local! {
    static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}
```

#### 2. Parallel File Processing
Uses Rayon's parallel iterators for concurrent file reading and parsing:
```rust
file_infos
    .into_par_iter()
    .filter_map(process_file_with_content)
    .collect()
```

#### 3. Gitignore-Respecting Walker
Uses the `ignore` crate for fast, gitignore-aware directory traversal:
```rust
WalkBuilder::new(path)
    .hidden(!include_hidden)
    .git_ignore(true)
    .git_global(true)
    .build()
```

#### 4. Accurate Token Counting
Uses `tiktoken-rs` for exact BPE token counts for OpenAI models:
- **Exact (tiktoken)**: GPT-5.2, GPT-5.1, GPT-5, O4-mini, O3, O1, GPT-4o, GPT-4o-mini (o200k_base); GPT-4, GPT-3.5-turbo (cl100k_base)
- **Calibrated estimation (~95% accuracy)**: Claude, Gemini, Llama, CodeLlama, Mistral, DeepSeek, Qwen, Cohere, Grok

#### 5. Memory-Mapped I/O (`mmap_scanner.rs`)
Optional mmap-based scanning for large files using `memmap2` crate.

#### 6. Incremental Caching (`incremental.rs`)
File-level caching with change detection:
- Caches parsed symbols, token counts, and metadata per file
- Fast path: mtime/size comparison for quick invalidation
- Accurate path: content hash comparison catches changes with same mtime/size
- Use `needs_rescan()` for fast check, `needs_rescan_with_content()` for hash-based check

### Performance Tips

1. **Skip symbols for speed**: Use `--skip-symbols` flag for 80x speedup on large repos
2. **Parallel by default**: Rayon auto-scales to available CPU cores
3. **Binary detection**: First 8KB checked, binary files automatically skipped
4. **Gitignore caching**: Patterns compiled once per directory tree

## Embedding Workflow for Vector Databases

The `embed` command generates deterministic, content-addressable code chunks optimized for RAG (Retrieval-Augmented Generation) applications.

### Design Philosophy

**The embed command is a library/CLI tool, not a full-fledged enterprise solution.**

It is designed to be a building block that integrates into your existing data pipelines:

| What it IS | What it is NOT |
|------------|----------------|
| Deterministic chunk generator | Multi-tenant SaaS platform |
| Content-addressable ID system | Access control / RBAC system |
| Incremental diff calculator | Distributed job queue |
| JSONL/JSON output producer | Vector database |
| Secret scanner/redactor | Compliance management system |

**Intended Usage Pattern:**
```bash
# Process repos sequentially in your pipeline
for repo in repos/*; do
    infiniloom embed "$repo" -o "chunks/${repo##*/}.jsonl"
done

# Then ingest into your vector DB of choice
python ingest_to_pinecone.py chunks/*.jsonl
```

**Key Guarantees:**
- **Deterministic**: Same code → same chunk IDs (enables cross-repo deduplication)
- **Semantically correct**: AST-aware chunking respects function/class boundaries
- **Incremental**: Manifest-based diffing for efficient updates
- **Portable**: JSONL output works with any vector DB or pipeline

**Not Provided (build your own or use existing tools):**
- User authentication / authorization
- Multi-tenant isolation enforcement
- Distributed processing coordination
- Vector embedding generation (use OpenAI, Voyage, Cohere, etc.)
- Vector database storage (use Pinecone, Weaviate, Qdrant, etc.)
- Monitoring dashboards

### Quick Start

```bash
# Generate chunks for current repository
infiniloom embed -o chunks.json

# Incremental update (only changed chunks)
infiniloom embed --diff-only -o updates.json

# CI/CD mode (fail on secrets)
infiniloom embed --fail-on-secrets -o chunks.json
```

### Content-Addressable IDs

Each chunk has a stable ID based on BLAKE3 hash of normalized content:

```
ec_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6
```

**Key Property**: Same code anywhere = same ID. This enables:
- Cross-repository deduplication
- Incremental vector DB updates
- Stable references for retrieval

### Incremental Update Pattern

```bash
# First run: full generation, creates manifest
infiniloom embed -o chunks.json
# Creates .infiniloom-embed.bin manifest

# After code changes: only changed chunks
infiniloom embed --diff-only -o updates.json
# Output includes: added, modified, removed, unchanged counts

# Vector DB workflow:
# 1. Upsert chunks from updates.json
# 2. Delete IDs from diff.removed
```

### Chunk Structure

```json
{
  "id": "ec_...",
  "content": "fn calculate(a: i32, b: i32) -> i32 { ... }",
  "tokens": 25,
  "kind": "function",
  "source": {
    "file": "src/math.rs",
    "lines": [10, 15],
    "symbol": "calculate",
    "language": "Rust"
  },
  "context": {
    "signature": "fn calculate(a: i32, b: i32) -> i32",
    "docstring": "Adds two numbers",
    "calls": ["add", "validate"],
    "called_by": ["main", "process"],
    "tags": ["public-api"],
    "lines_of_code": 5,
    "max_nesting_depth": 2
  }
}
```

### Embedding Model Presets

```bash
# For Voyage Code (1500 token context)
infiniloom embed --max-tokens 1500

# For Cohere (400 token context)
infiniloom embed --max-tokens 400

# For sentence-transformers (384 token context)
infiniloom embed --max-tokens 384
```

### Security Integration

The embed command integrates with the security scanner:

```bash
# Default: scan and redact secrets
infiniloom embed -o chunks.json

# CI mode: fail if secrets detected
infiniloom embed --fail-on-secrets

# Skip scanning (trusted input only)
infiniloom embed --no-security-scan
```

### Auto-Generated Semantic Tags

Chunks are automatically tagged for better retrieval:

| Tag | Triggers |
|-----|----------|
| `async` | async/await keywords, Kotlin suspend |
| `concurrency` | threads, mutex, channels, Go goroutines |
| `security` | auth, password, token, crypto |
| `database` | query, sql, transaction |
| `http` | request, response, endpoint |
| `error-handling` | Error, Result, exception |
| `test` | test_, _test, mock, stub |

### Determinism Guarantees

The embedding system provides strong determinism for CI/CD:

1. Files processed in sorted lexicographic order
2. Symbols sorted by (line, name) within each file
3. Output chunks sorted by (file, line, id)
4. All hash computations use integer-only math
5. Cross-platform identical output (Windows/Linux/macOS)

### API Usage

**Rust:**
```rust
use infiniloom_engine::embedding::{EmbedChunker, EmbedSettings, ResourceLimits};

let chunker = EmbedChunker::new(EmbedSettings::default(), ResourceLimits::default());
let chunks = chunker.chunk_repository(Path::new("./repo"))?;
```

**Node.js:**
```javascript
const { embed } = require('infiniloom-node');
const result = embed('./repo', { maxTokens: 1000, securityScan: true });
console.log(`Generated ${result.chunks.length} chunks`);
```

**Python:**
```python
from infiniloom import embed
result = embed("./repo", max_tokens=1000)
print(f"Generated {len(result.chunks)} chunks")
```

### Resource Limits

Default limits protect against DoS:

| Limit | Default | Description |
|-------|---------|-------------|
| `max_file_size` | 10 MB | Files larger are skipped |
| `max_line_length` | 10,000 | Detects minified files |
| `max_total_chunks` | 1,000,000 | Enterprise scale |
| `max_files` | 500,000 | Large monorepo scale |

See [docs/commands/embed.md](docs/commands/embed.md) for complete documentation.
