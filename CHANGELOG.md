# Changelog

All notable changes to Infiniloom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Semantic Compression Module** (`engine/src/semantic.rs`)
  - New `SemanticCompressor` for reducing content while preserving meaning
  - Heuristic-based compression with configurable `budget_ratio` and `similarity_threshold`
  - Optional neural embeddings-based compression when `embeddings` feature is enabled
  - `CodeChunk` type for semantic content chunking
  - Cosine similarity for embedding comparison

- **Fuzz Testing** (`fuzz/`)
  - Added `cargo-fuzz` targets for parser and security scanner
  - Fuzz targets: `fuzz_parse_rust`, `fuzz_parse_python`, `fuzz_parse_javascript`, `fuzz_security_scan`

- **CI/CD Benchmark Regression Testing**
  - New `benchmarks` job in GitHub Actions workflow
  - Automated benchmark comparison against main branch
  - PR comments with benchmark results
  - Artifact storage for benchmark history

- **Language Bindings Updates**
  - Python: Added `semantic_compress()` function
  - Node.js: Added `semanticCompress()` function
  - Updated token count fields to new API (o200k, cl100k, etc.)

### Changed

- **TokenCounts API** - Restructured to group by encoding family:
  - `o200k`: OpenAI modern models (GPT-5.x, GPT-4o, O1/O3/O4)
  - `cl100k`: OpenAI legacy models (GPT-4, GPT-3.5-turbo)
  - Added fields: `mistral`, `deepseek`, `qwen`, `cohere`, `grok`
  - Removed redundant `gpt4`, `gpt4o` fields (use encoding-based fields)

- **RepoMapGenerator API** - Migrated to builder pattern:
  - Now uses `RepoMapGenerator::builder().token_budget(...).model(...).build()`
  - `new(budget)` remains as convenience constructor
  - Removed `with_max_symbols()` and `with_model()` chain methods

- **Static Regex Patterns** - Pre-compiled regex using `once_cell::sync::Lazy`:
  - Security scanner patterns compiled once at startup
  - Base64 truncation patterns optimized

### Fixed

- **Security Scanner Comment Handling** - Now skips comment lines entirely
  - Lines starting with `//`, `#`, `/*`, `*` are skipped to reduce false positives
  - Fixes tests `test_skip_double_slash_comment` and `test_skip_hash_comment`
  - Consistent behavior in both `scan()` and `redact_content()` methods

- **CLI Token Budget Default** - Changed default from 100000 to 0 (no limit)
  - `--max-tokens 0` now correctly means "no limit" as documented
  - Config files can still override the default with `token_budget` setting

- **Incremental Cache Hash-Based Invalidation** - Content hash now used for change detection
  - Added `needs_rescan_with_hash()` method to `RepoCache`
  - Added `needs_rescan_with_content()` method to `IncrementalScanner`
  - Catches file changes that don't modify mtime/size (edge cases)

- **Language Parser Tests** - Fixed tests for Swift, Kotlin, Haskell, Lua, R
  - Updated tree-sitter queries for new grammar versions
  - Fixed pattern matching for language-specific constructs

- **Semantic Module Visibility** - Module now always available (not feature-gated at module level)
  - Internal functionality remains feature-gated for `embeddings`
  - Tests now run regardless of feature flags

- **Watch Mode Token Recomputation** - Now recomputes per-file token counts after transformations
  - Previously only estimated tokens on final output string
  - Ensures `--token-tree` shows accurate counts after compression/redaction
  - Matches behavior of initial pack command

- **README Security Scanning Documentation** - Corrected flag name and default behavior
  - Changed `--redact-secrets` to correct `--security-check` flag
  - Clarified that security scanning is disabled by default (for speed)
  - Removed misleading "enabled by default" comment

- **README --include-diffs Documentation** - Clarified what the flag actually does
  - `pack --include-diffs`: Adds list of changed files with status (A/M/D), not diff content
  - `diff --include-diff`: Actually includes +/- diff content (different command)

- **Index Module Code Consolidation** - Removed duplicate code in builder.rs and lazy.rs
  - Created shared `convert.rs` module with `convert_symbol_kind()` and `convert_visibility()`
  - Both builder.rs and lazy.rs now import from the shared module
  - Reduces maintenance burden and ensures consistent behavior

- **Symbol-Level PageRank** - Index builder now computes PageRank for symbols
  - Previously only file-level PageRank was computed
  - Added `compute_symbol_pagerank()` function to `IndexBuilder`
  - Uses call graph and symbol references for importance ranking
  - Enables better prioritization in diff context generation

- **Debug Logging in Scanner** - Added `log::debug!` calls for troubleshooting
  - Logs file read failures with path and error details
  - Logs binary file skips and UTF-8 encoding issues
  - Helps diagnose scanning problems in large repositories

### Performance

- **Zero-Copy Optimizations**
  - Static regex compilation eliminates per-call overhead
  - Reduced string allocations in hot paths

### Security

- Security scanner patterns remain up-to-date with latest secret formats

## [0.1.0] - 2025-01-15

### Added

- Initial release of Infiniloom
- AST-based symbol extraction using Tree-sitter (21 languages)
- PageRank-based symbol importance ranking
- Model-specific output formats (XML, Markdown, JSON, YAML)
- Automatic secret detection and redaction
- Accurate token counting via tiktoken-rs for OpenAI models
- Python bindings via PyO3
- Node.js bindings via NAPI-RS
- CLI application with `pack`, `scan`, `map`, `diff`, `index` commands
- Git context index for fast diff analysis
- Configuration file support (YAML/TOML/JSON)

[Unreleased]: https://github.com/Topos-Labs/infiniloom/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Topos-Labs/infiniloom/releases/tag/v0.1.0
