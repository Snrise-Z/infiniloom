# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Infiniloom** is a high-performance repository context generator for Large Language Models. It transforms codebases into optimized formats for Claude, GPT-4, Gemini, and other LLMs. Built in pure Rust for maximum performance and portability.

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

# Scan repository and show statistics
infiniloom scan /path/to/repo

# Generate repository map with key symbols
infiniloom map /path/to/repo --budget 2000

# Show version and configuration info
infiniloom info

# Initialize configuration file
infiniloom init

# Build/update symbol index for fast diff context
infiniloom index /path/to/repo
infiniloom index --force              # Force full rebuild
infiniloom index --status             # Show index stats

# Get context for a diff (changed files, dependents, tests)
infiniloom diff                       # Unstaged changes
infiniloom diff --staged              # Staged changes
infiniloom diff HEAD~1                # Last commit
infiniloom diff main..feature         # Branch comparison
infiniloom diff --depth 2             # Context depth (1-3, default: 2)
infiniloom diff --budget 50000        # Token budget limit
infiniloom diff --include-diff        # Include actual diff content (+/- lines)
infiniloom diff --format json         # Output format (xml/json/markdown)

# Analyze impact of changes
infiniloom impact src/auth.rs         # What depends on this file?
infiniloom impact --symbol "foo"      # What calls this symbol?
```

## Code Architecture

### Workspace Structure

```
infiniloom/
├── cli/                    # CLI application (clap-based)
│   └── src/
│       ├── main.rs         # Command handling, argument parsing
│       └── scanner.rs      # Repository scanning with parallel processing
├── engine/                 # Core Rust engine library
│   └── src/
│       ├── lib.rs          # Public API exports
│       ├── types.rs        # Core types: Repository, RepoFile, Symbol
│       ├── parser.rs       # Tree-sitter AST parsing (21 languages)
│       ├── repomap/        # PageRank symbol ranking
│       │   ├── mod.rs      # RepoMapGenerator
│       │   └── graph.rs    # SymbolGraph, PageRank computation
│       ├── output/         # Format generators
│       │   ├── xml.rs      # Claude-optimized XML
│       │   ├── markdown.rs # GPT-optimized Markdown
│       │   └── toon.rs     # Token-efficient TOON format
│       ├── ranking.rs      # File importance ranking
│       ├── security.rs     # Secret detection/redaction
│       ├── tokenizer.rs    # Multi-model token counting (tiktoken-rs)
│       ├── chunking/       # Semantic code chunking
│       ├── config.rs       # Configuration loading (YAML/TOML/JSON)
│       ├── git.rs          # Git operations (log, status, diff)
│       ├── remote.rs       # Remote repository cloning
│       ├── dependencies.rs # Dependency graph resolution
│       ├── mmap_scanner.rs # Memory-mapped file scanning
│       └── index/          # Symbol index for fast diff context
│           ├── mod.rs      # Module exports
│           ├── builder.rs  # Index building with parallel parsing
│           ├── context.rs  # Diff context expansion
│           ├── lazy.rs     # On-the-fly context generation
│           ├── storage.rs  # Bincode serialization
│           ├── types.rs    # Index types (SymbolIndex, DepGraph)
│           ├── query.rs    # Call graph query API (callers, callees, references)
│           ├── convert.rs  # Shared type conversion utilities
│           └── patterns.rs # Pre-compiled regex patterns
└── bindings/               # Language bindings
    ├── python/             # PyO3 bindings (maturin)
    └── node/               # NAPI-RS bindings
```

### Core Types (`engine/src/types.rs`)

- **`Repository`**: Root container with name, path, files, and metadata
- **`RepoFile`**: Single file with path, language, token counts, symbols, importance score
- **`Symbol`**: Extracted code symbol (function, class, etc.) with kind, signature, line numbers
- **`TokenCounts`**: Token counts for multiple models grouped by encoding family:
  - `o200k`: OpenAI modern (GPT-5.x, GPT-4o, O1/O3/O4) - exact via tiktoken
  - `cl100k`: OpenAI legacy (GPT-4, GPT-3.5-turbo) - exact via tiktoken
  - `claude`, `gemini`, `llama`, `mistral`, `deepseek`, `qwen`, `cohere`, `grok`: estimation-based
- **`TokenizerModel`**: Enum with 27 supported LLM tokenizers
- **`CompressionLevel`**: None, Minimal, Balanced, Aggressive, Extreme

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

### Data Flow

1. **Scanning** (`cli/scanner.rs`): Walk directory with `ignore` crate, filter by gitignore, detect languages
2. **Parsing** (`parser.rs`): Tree-sitter AST extraction for symbols (thread-local parsers for parallelism)
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

**Accurate Token Counting** (`tokenizer.rs`):
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
default = []
async = ["tokio", "async-trait"]     # Async operations (placeholder - not yet implemented)
embeddings = ["candle-core", "candle-transformers"]  # Local embeddings (heuristic-based)
watch = ["notify"]                   # File watching (implemented for pack --watch)
full = ["async", "embeddings", "watch"]
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
