# Infiniloom Engine Architecture

> Last updated: 2025-12-24

## Overview

The infiniloom-engine is a high-performance Rust library for generating optimized repository context for Large Language Models. This document describes the module structure, data flow, and key design patterns.

## Module Dependency Graph

```
                                    ┌─────────────┐
                                    │   lib.rs    │
                                    │  (exports)  │
                                    └──────┬──────┘
                                           │
           ┌───────────────────────────────┼───────────────────────────────┐
           │                               │                               │
           ▼                               ▼                               ▼
    ┌─────────────┐               ┌─────────────┐               ┌─────────────┐
    │   parser/   │               │  tokenizer/ │               │  security   │
    │  (21 langs) │               │  (tiktoken) │               │  (redact)   │
    └──────┬──────┘               └──────┬──────┘               └─────────────┘
           │                             │
           ▼                             ▼
    ┌─────────────┐               ┌─────────────┐
    │  repomap/   │◄──────────────│   budget    │
    │ (PageRank)  │               │ (truncate)  │
    └──────┬──────┘               └─────────────┘
           │
           ▼
    ┌─────────────┐               ┌─────────────┐
    │   output/   │               │   index/    │
    │ (formatters)│               │ (diff ctx)  │
    └─────────────┘               └─────────────┘
```

## Module Descriptions

### Core Parsing & Analysis

| Module | Path | Description |
|--------|------|-------------|
| `parser/` | `src/parser/` | Tree-sitter based AST parsing for 21 languages |
| `tokenizer/` | `src/tokenizer/` | Multi-model token counting (tiktoken + estimation) |
| `repomap/` | `src/repomap/` | PageRank-based symbol importance ranking |
| `ranking` | `src/ranking.rs` | File importance scoring algorithms |
| `semantic` | `src/semantic.rs` | Heuristic-based code compression |

### Output & Formatting

| Module | Path | Description |
|--------|------|-------------|
| `output/` | `src/output/` | Model-specific formatters (XML, Markdown, YAML, JSON, TOON) |
| `chunking/` | `src/chunking/` | Semantic code chunking strategies |
| `budget` | `src/budget.rs` | Token budget enforcement and truncation |

### Git & Index

| Module | Path | Description |
|--------|------|-------------|
| `git` | `src/git.rs` | Git operations via CLI commands |
| `index/` | `src/index/` | Symbol index for fast diff context generation |
| `remote` | `src/remote.rs` | Remote repository cloning (GitHub, GitLab, Bitbucket) |
| `dependencies` | `src/dependencies.rs` | Dependency graph resolution |

### Infrastructure

| Module | Path | Description |
|--------|------|-------------|
| `types` | `src/types.rs` | Core types: Repository, RepoFile, Symbol |
| `newtypes` | `src/newtypes.rs` | Type-safe wrappers (TokenCount, LineNumber, etc.) |
| `constants` | `src/constants.rs` | Centralized configuration constants |
| `config` | `src/config.rs` | Configuration loading (YAML/TOML/JSON) |
| `security` | `src/security.rs` | Secret detection and redaction |
| `error` | `src/error.rs` | Unified error types |
| `incremental` | `src/incremental.rs` | Content-addressed caching |
| `mmap_scanner` | `src/mmap_scanner.rs` | Memory-mapped file scanning |

## Key Data Flow

### 1. Repository Scanning

```
Files on disk
     │
     ▼ (WalkBuilder with gitignore)
FileEntry list
     │
     ▼ (parallel processing)
[parser/] Tree-sitter AST parsing
     │
     ▼
Vec<Symbol> per file
     │
     ▼
RepoFile (content, symbols, token counts)
     │
     ▼
Repository (all files)
```

### 2. Symbol Importance Ranking

```
Repository
     │
     ▼ [repomap/graph.rs]
SymbolGraph (directed graph of symbol references)
     │
     ▼ (PageRank algorithm, damping=0.85)
HashMap<symbol_key, f64> ranks
     │
     ▼ (filter imports, boost entry points)
RepoMap (top N symbols)
```

### 3. Output Generation

```
Repository + RepoMap
     │
     ▼ [output/]
OutputFormatter (by format or model)
     │
     ├─▶ xml.rs    (Claude)
     ├─▶ markdown.rs (GPT)
     ├─▶ yaml.rs   (Gemini)
     ├─▶ json.rs   (APIs)
     └─▶ toon.rs   (Compact)
     │
     ▼
String output
```

### 4. Diff Context Generation (Index)

```
Git diff (changed files)
     │
     ▼ [index/]
SymbolIndex.lookup(file_path)
     │
     ▼ [index/context/]
ContextExpander.expand(changed_symbols)
     │
     ▼ (callers, callees, tests)
DiffContext (files + symbols + tests)
```

## Module Deep Dives

### parser/ Module Structure

```
parser/
├── mod.rs           # Public API, re-exports
├── core.rs          # Parser struct, main entry point
├── language.rs      # Language enum (21 variants)
├── extraction.rs    # Symbol extraction from AST
├── queries.rs       # Tree-sitter query strings
├── query_builder.rs # Super-query construction
└── init.rs          # Tree-sitter parser initialization
```

**Key Design**: Uses "super-queries" that combine symbol extraction and import detection in a single AST traversal for performance.

### index/ Module Structure

```
index/
├── mod.rs          # Public API, re-exports
├── types.rs        # SymbolIndex, DepGraph, FileEntry
├── builder/        # Index construction
│   ├── mod.rs      # Builder module exports
│   ├── core.rs     # IndexBuilder
│   ├── graph.rs    # Dependency graph building
│   └── types.rs    # Build options, errors
├── context/        # Diff context expansion
│   ├── mod.rs      # Context module exports
│   ├── expander.rs # ContextExpander implementation
│   └── types.rs    # DiffChange, ContextDepth types
├── query.rs        # Call graph query API (find_symbol, get_callers, etc.)
├── storage.rs      # Bincode serialization to .infiniloom/
├── lazy.rs         # On-the-fly context generation (no index required)
├── convert.rs      # Type conversions between index and query types
└── patterns.rs     # Pre-compiled regex patterns
```

**Key Design**: Index is built once and cached (`.infiniloom/`), then used for fast diff context queries.

### tokenizer/ Module Structure

```
tokenizer/
├── mod.rs     # Public API, Tokenizer struct
├── core.rs    # Token counting implementation
├── counts.rs  # TokenCounts struct
└── models.rs  # TokenModel enum (27 models)
```

**Key Design**: Uses tiktoken-rs for exact OpenAI counts (o200k, cl100k encodings) and calibrated estimation for other models.

### chunking/ Module Structure

```
chunking/
├── mod.rs        # Public API, Chunker struct
├── types.rs      # Chunk, ChunkFile, ChunkStrategy
└── strategies.rs # Chunking strategy implementations
```

**Key Design**: Multiple chunking strategies (Fixed, File, Module, Symbol, Semantic, Dependency) allow breaking large repos into manageable pieces.

## Performance Patterns

### 1. Thread-Local Parsers

Tree-sitter parsers are not thread-safe, but we use parallel file processing. Solution:

```rust
thread_local! {
    static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

// In parallel iterator
files.par_iter().map(|f| {
    THREAD_PARSER.with(|p| p.borrow_mut().parse(&content, lang))
})
```

### 2. Lazy Initialization

Parsers for each language are only initialized when first needed:

```rust
pub struct Parser {
    parsers: HashMap<Language, TSParser>,  // Lazy-filled
    queries: HashMap<Language, Query>,
}
```

### 3. Pre-compiled Regex

Security patterns and import regexes are compiled once:

```rust
// patterns.rs
pub static PYTHON_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^import\s+(.+)$").expect("valid regex")
});
```

### 4. PageRank Early Convergence

PageRank computation exits early when convergence threshold is met:

```rust
const CONVERGENCE_THRESHOLD: f64 = 1e-6;
if diff < CONVERGENCE_THRESHOLD {
    break;  // Exit iteration early
}
```

## Error Handling Strategy

- **Test code**: `.unwrap()` is acceptable
- **Static regex**: `.expect()` on `Lazy<Regex>` (compile-time constants)
- **Production code**: Proper `Result<T, E>` with `thiserror` errors
- **Safe patterns**: `unwrap_or`, `unwrap_or_default`, `map().unwrap_or()`

## Configuration

Configuration is loaded via `figment` with priority:

1. CLI arguments (highest)
2. Environment variables (`INFINILOOM_*`)
3. Config file (`.infiniloom.yaml` or `.infiniloom.toml`)
4. Defaults (lowest)

See `src/config.rs` for all configuration options.

## Testing Strategy

| Test Type | Location | Count |
|-----------|----------|-------|
| Unit tests | `src/*/tests` modules | ~900+ |
| Property tests | `tests/property_tests.rs` | 34 |
| Integration tests | `tests/` | Various |
| Benchmarks | `benches/` | Performance comparisons |

Property tests cover:
- Tokenizer invariants (truncation, UTF-8, determinism)
- Security scanner (no panic on arbitrary input)
- Parser (valid line numbers, deterministic)

## Adding a New Language

1. Add variant to `Language` enum in `parser/language.rs`
2. Add extension mapping in `from_extension()`
3. Create tree-sitter query in `parser/queries.rs`
4. Add initializer in `parser/init.rs`
5. Add super-query builder in `parser/query_builder.rs`
6. Update `ensure_initialized()` in `parser/core.rs`

## Key Constants

Located in `src/constants.rs` with namespaced modules:

| Module | Constant | Value | Description |
|--------|----------|-------|-------------|
| `budget` | `DEFAULT_MAP_BUDGET` | 2000 | Default tokens for repo map |
| `budget` | `DEFAULT_CHUNK_SIZE` | 8000 | Default chunk size |
| `budget` | `DEFAULT_BUDGET` | 100000 | Default total token budget |
| `pagerank` | `DAMPING_FACTOR` | 0.85 | PageRank damping factor |
| `pagerank` | `MAX_ITERATIONS` | 100 | Max PageRank iterations |
| `pagerank` | `CONVERGENCE_THRESHOLD` | 1e-6 | Early exit threshold |
| `timeouts` | `GIT_OPERATION_SECS` | 30 | Git command timeout |
| `timeouts` | `REMOTE_CLONE_SECS` | 300 | Remote clone timeout |
| `files` | `MAX_FILE_SIZE_BYTES` | 10MB | Skip files larger than this |
| `files` | `BINARY_CHECK_BYTES` | 8192 | Bytes to check for binary |
| `repomap` | `TOKENS_PER_SYMBOL` | 25 | Estimated tokens per symbol |

## Further Reading

- [README.md](../README.md) - User documentation and CLI reference
- [CLAUDE.md](../CLAUDE.md) - Development guide for Claude Code
- [IMPLEMENTATION_STATUS.md](../docs/IMPLEMENTATION_STATUS.md) - Feature implementation status
