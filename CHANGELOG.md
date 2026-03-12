# Changelog

All notable changes to Infiniloom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **GPT Formatter Token Model** - The GPT output formatter (`OutputFormatter::gpt()`) now uses `Gpt4o` (o200k_base) for token counting instead of `Claude` estimation. This produces exact token counts but may report different numbers than previous versions. (PR #75)

### Fixed

- **Markdown Parser Formatting** - Fix formatting in markdown parser (PR #53)
- **Critical Bug Fixes** - Address critical bugs, security issues and correctness problems across engine (PR #53)
  - Constant-time comparison for embedding manifest checksums
  - BLAKE3 hashing for cache change detection (replacing SipHash)
  - Atomic file writes for cache and index storage (write-to-temp then rename)
  - ZIP bomb protection in DOCX parser (bounded reads)
  - HTML parser iteration limits to prevent DoS
  - Improved PII detection (SSN area code validation, Luhn-validated credit cards)
  - CSV delimiter detection priority fix
  - Distillation filler-phrase iteration caps
- **Post-v0.6.3 Review Fixes** - Address correctness, security and robustness issues (PRs #51, #52)
  - bincode 2.0 safe deserialization with 1GB size limits
  - Version-checked cache/index/manifest loading with graceful fallback
  - Adjacency map rebuilding after index deserialization

## [0.6.3] - 2026-02-15

### Added

- **Document Ingestion Module** - New `ingest` command for converting documents to LLM-optimized formats
  - Parsers for Markdown, HTML, CSV, DOCX, and XLSX (feature-gated)
  - Distillation pipeline with content compression levels (minimal, balanced, aggressive)
  - PII detection and redaction (SSN, credit cards, emails, phone numbers, IP addresses)
  - Document chunking for multi-turn conversations
  - Output formats: XML, Markdown, JSON
  - Feature flags: `document` (default), `document-xlsx` (optional, requires `calamine`)

- **HCL/Terraform Language Support** - Full Tree-sitter AST parsing for `.tf` and `.hcl` files
  - Resource, data source, variable, output, module, provider, and locals extraction
  - Language #23 in enum, #21 with full parser support

### Changed

- **Tree-sitter 0.25 → 0.26** - Major parser framework upgrade
  - All 21 grammar crates updated to tree-sitter 0.26 compatible versions
  - Clojure parser removed (tree-sitter-clojure incompatible with 0.26; graceful degradation)
  - FSharp remains without parser support

- **bincode 1.3 → 2.0** - Serialization framework upgrade
  - New wire format (incompatible with v0.6.2 cached files)
  - Existing index/cache/manifest files are automatically rebuilt on version mismatch
  - Safe deserialization with 1GB size limits via `bincode_safe` module

- **PyO3 0.20 → 0.28** - Python binding framework upgrade
- **thiserror 1.0 → 2.0** - Error derive macro upgrade
- **tiktoken-rs 0.5 → 0.9** - OpenAI tokenizer library upgrade
- **notify 6.1 → 8.2** - File watcher library upgrade
- **criterion 0.5 → 0.8** - Benchmark framework upgrade
- **@napi-rs/cli 2.x → 3.5** - Node.js binding toolchain upgrade

### Breaking Changes

- **Cache/Index File Format** - bincode 2.0 uses a different wire format. All cached files (`.infiniloom/`) will be automatically rebuilt on first use. Run `infiniloom index --force` if needed.
- **Clojure Parser** - Clojure files are still detected but no longer receive AST-based symbol extraction. Files are still included in output with content but without parsed symbols.

## [0.6.2] - 2026-01-20

### Added

- **Circular Dependency Detection** - `find_circular_dependencies()` detects import cycles in the dependency graph
- **Exported Symbols Query** - `get_exported_symbols()` returns all public/exported symbols per file
- **Comprehensive Type Safety** - Added validation across all Python and Node.js bindings

## [0.6.0] - 2026-01-10

### Added

- **`embed` Command** - Generate content-addressable chunks for vector databases and RAG systems
  - AST-aware chunking preserves semantic boundaries (functions, classes, methods)
  - BLAKE3 content-addressable IDs for cross-repo deduplication
  - Incremental updates via manifest-based diffing
  - Auto-generated semantic tags (async, security, database, http, etc.)
  - Call graph context (`calls`, `called_by` fields)
  - Hierarchical chunks with parent-child relationships
  - JSONL and JSON output formats
  - Resource limits for DoS protection
  - Cross-platform deterministic output

## [0.6.1] - 2026-01-04

### Added

- **Analysis Module** - Comprehensive code analysis capabilities (`engine/src/analysis/`)
  - **Type Signature Extraction** - Extract complete type signatures for functions, methods, and classes across 21 languages
  - **Type Hierarchy Navigation** - Navigate class inheritance and interface implementations
    - `get_type_hierarchy()` - Get complete hierarchy with ancestors and descendants
    - `get_type_ancestors()` - Get all parent classes/interfaces
    - `get_type_descendants()` - Get all child classes
    - `get_implementors()` - Get all classes implementing an interface/trait
  - **Documentation Extraction** - Parse JSDoc, Python docstrings, Rust doc comments into structured format
    - `extract_documentation()` - Returns summary, params, returns, throws, examples, deprecation info
  - **Complexity Metrics** - Calculate code complexity metrics
    - `calculate_complexity()` - Cyclomatic, cognitive, Halstead metrics, maintainability index
    - `check_complexity()` - Validate against thresholds, return issues
  - **Dead Code Detection** - Find unused code across codebase
    - `detect_dead_code()` - Returns unused exports, unreachable code, unused imports/variables
  - **Breaking Change Detection** - Detect API breaking changes between versions
    - `detect_breaking_changes()` - Compare two git refs, report signature changes, removals

- **Python Bindings** - All 9 analysis functions exposed with full type stubs
  - Added `extract_documentation()`, `detect_dead_code()`, `detect_breaking_changes()`
  - Added `get_type_hierarchy()`, `get_type_ancestors()`, `get_type_descendants()`, `get_implementors()`
  - Added `calculate_complexity()`, `check_complexity()`
  - Complete TypedDict definitions in `infiniloom.pyi` for IDE support

- **Node.js Bindings** - All analysis functions exposed with TypeScript definitions
  - Added `extractDocumentation()`, `detectDeadCode()`, `detectBreakingChanges()`
  - Added `getTypeHierarchy()`, `getTypeAncestors()`, `getTypeDescendants()`, `getImplementors()`
  - Added `calculateComplexity()`, `checkComplexity()`

### Fixed

- **OCaml Tree-sitter Query** - Fixed "Impossible pattern" error in module_binding query
  - Changed `name: (module_name) @name` to `(module_name) @name` (OCaml grammar doesn't have `name` field)
  - Both `ocaml_query()` and `ocaml_super_query()` now work correctly

- **Documentation Regex** - Fixed lookahead regex not supported by Rust regex crate
  - Changed JSDoc example pattern to avoid `(?=...)` lookahead syntax

- **Dead Code Test** - Fixed test using wrong language for entry point detection
  - Changed from Rust (where all public items are entry points) to C for accurate testing

### Changed

- **Test Coverage** - All 998 engine tests passing (was 997)
- **Bindings Validation** - Both Python and Node.js bindings compile and pass all tests

## [0.5.4] - 2025-12-29

### Fixed

- **CRITICAL: UTF-8 Boundary Crash in Parser** - Fixed panic on large repos with multi-byte UTF-8 characters
  - Fixed `byte index is not a char boundary` panic in parser/extraction.rs (lines 212, 271)
  - Bug affected Rust and C# docstring extraction when special UTF-8 characters (©, °, —, ├) appeared before functions
  - Crash occurred on: Linux kernel, Kubernetes, VSCode, Rust compiler, and any repo with special UTF-8 chars
  - Fix: Use `floor_char_boundary()` to find safe UTF-8 boundary before string slicing
  - **Node.js v0.5.3 users**: Update to v0.5.4 immediately - v0.5.3 crashes on most large repos
  - All 790 engine tests pass with fix
  - Root cause: `source_code[..start_byte]` sliced at byte index from tree-sitter without UTF-8 boundary check

### Impact

- **Before Fix (v0.5.3)**: Node.js bindings crashed on 4/6 large repos tested (67% failure rate)
- **After Fix (v0.5.4)**: All repos process successfully
- CLI and Python bindings were not affected (different code path)

## [0.5.3] - 2025-12-29

### Fixed

- **Release Workflow** - Fixed PyPI version conflict
  - PyPI doesn't allow re-uploading same version number
  - Bumped to v0.5.3 to enable successful PyPI publish
  - No code changes from v0.5.2

### Changed

- **Version**: All packages bumped from 0.5.2 → 0.5.3 for PyPI compatibility
  - CLI (infiniloom): 0.5.2 → 0.5.3
  - Engine (infiniloom-engine): 0.5.2 → 0.5.3
  - Common bindings (infiniloom-bindings-common): 0.5.2 → 0.5.3
  - Node.js bindings (infiniloom-node): 0.5.2 → 0.5.3
  - Python bindings (infiniloom): 0.5.2 → 0.5.3
  - npm CLI package (infiniloom): 0.5.0 → 0.5.3

## [0.5.2] - 2025-12-29

### Fixed

- **Node.js Bindings - Missing `Infiniloom.securityScan()` Method**
  - Added missing `securityScan()` method to `Infiniloom` class
  - Method scans pre-loaded repository files for secrets and sensitive information
  - Returns array of `SecurityFinding` objects with file, line, severity, kind, and pattern
  - Fixes 2 failing tests: "Infiniloom class exposes stats, map, pack, and security scan" and "Infiniloom.securityScan returns structured findings"

- **Node.js Bindings - Missing Empty Path Validation**
  - Added path validation in `Infiniloom` constructor to reject empty strings
  - Throws `InvalidArg` error with message "Path cannot be empty"
  - Fixes 1 failing test: "Infiniloom handles empty path in constructor"
  - Ensures consistent validation across all path inputs

- **Documentation Examples (Doctests)**
  - Fixed 14 failing doctest examples in engine code
  - Updated `quick_estimate()` example to include required `TokenModel` parameter
  - Fixed `CallGraphStats` field names: `total_nodes` → `total_symbols`, `total_edges` → `total_calls`
  - Fixed `get_call_graph_filtered()` calls to use `Option` parameters
  - Marked examples requiring test setup or file I/O as `no_run`
  - All doctests now pass

### Changed

- **Version Alignment**: All packages now at v0.5.2
  - CLI (infiniloom): 0.5.0 → 0.5.2
  - Engine (infiniloom-engine): 0.5.0 → 0.5.2
  - Common bindings (infiniloom-bindings-common): 0.5.0 → 0.5.2
  - Node.js bindings (infiniloom-node): already 0.5.2
  - Python bindings (infiniloom): already 0.5.2
- All 212 Node.js tests passing (was 209/212 in v0.5.1)
- All documentation examples (doctests) now pass

## [0.5.1] - 2025-12-29

### Fixed

- **Node.js Bindings - Missing Exports Restored** - Critical regression fix from v0.5.0
  - Restored 6 async wrapper functions that were missing: `packAsync`, `scanAsync`, `buildIndexAsync`, `chunkAsync`, `analyzeImpactAsync`, `getDiffContextAsync`
  - Restored `Infiniloom` class with `new()`, `getStats()`, `generateMap()`, and `pack()` methods
  - Restored `semanticCompress()` function for heuristic compression
  - All 8 exports now available in Node.js bindings

- **Property Tests - Empty Pattern Edge Case** - Fixed filtering tests failing on empty string patterns
  - Added empty string guards in `matches_exclude_pattern()` and `matches_include_pattern()`
  - Fixed `prop_exclude_empty_pattern_no_match` test (empty pattern should not match anything)
  - Fixed `prop_include_empty_pattern_no_match` test (empty pattern should not match anything)
  - Prevents `"".starts_with("")` edge case where empty strings match everything

### Added

- **Comprehensive Test Coverage for Async Functions** - 18 new tests for Node.js bindings
  - `packAsync`: 3 tests (JSON output, XML format, error handling)
  - `scanAsync`: 3 tests (claude model, gpt4o model, error handling)
  - `buildIndexAsync`: 3 tests (create index, force rebuild, error handling)
  - `chunkAsync`: 3 tests (file strategy, module strategy, validation)
  - `analyzeImpactAsync`: 3 tests (single file, multiple files, error handling)
  - `getDiffContextAsync`: 3 tests (commit range, different ranges, error handling)
  - All tests passing with 0 failures

### Changed

- **Python Bindings** - Verified all exports present and functional
  - `Infiniloom` class already implemented
  - `semantic_compress()` function already implemented
  - No changes needed, compiles successfully

## [0.4.9] - 2025-12-26

### Added

- **`version()` Function** - Returns the package version string for programmatic access
  - Node.js: `const { version } = require('infiniloom-node'); version()` returns "0.4.9"
  - Python: `from infiniloom import version; version()` returns "0.4.9"

### Fixed

- **Null/Undefined Input Handling (Node.js)** - All major functions now throw clean errors instead of crashing
  - `pack()`, `scan()`, `scanWithOptions()`, `buildIndex()` - validate path parameter
  - `findSymbol()`, `getCallers()`, `getCallees()`, `getReferences()` - validate path and symbol name
  - `getCallGraph()`, `getSymbolSource()`, `scanSecurity()` - validate path parameter
  - `semanticCompress()` - validates text parameter (null/undefined/empty)
  - `GitRepo` constructor - validates path parameter
  - Error messages are user-friendly: "Path cannot be null or undefined", "Symbol name cannot be empty"

- **semanticCompress Options API** - Now accepts options object instead of positional parameters
  - Old API: `semanticCompress(text, 0.7, 0.5)` (still works for backward compatibility)
  - New API: `semanticCompress(text, { budgetRatio: 0.5, similarityThreshold: 0.7 })`
  - Additional options: `minChunkSize`, `maxChunkSize`

- **Infiniloom.generateMap Options API** - Now accepts options object
  - Old API: `loom.generateMap(2000, 50)` (still works)
  - New API: `loom.generateMap({ budget: 2000, maxSymbols: 50 })`

- **getSymbolSource Return Type** - Now returns structured `SymbolSourceResult` object
  - Old: Returned plain string with source code
  - New: Returns `{ source, path, startLine, endLine, name, kind }`
  - Provides complete context about the symbol location and type

### Changed

- **Test Suite Expansion** - Added ~100 new tests for bug fixes
  - Tests for `version()` function
  - Tests for null/undefined handling across all functions
  - Tests for new options object APIs
  - Tests for `SymbolSourceResult` return type
  - All 161 tests passing

## [0.4.8] - 2025-12-25

### Performance

- **Major Performance Optimization** - Fixed critical bottleneck in diff context APIs
  - `getDiffContext()` / `get_diff_context()` now 20-27x faster (from ~2s to 75-103ms for 51 files)
  - Changed from O(N) git subprocess calls to O(1) - single git call for all hunks
  - Added `reconstruct_diff_from_hunks()` helper to reconstruct diff content in memory
  - Applied optimization to both Node.js and Python bindings

- **getChangedSymbolsFiltered Optimization** - Same O(1) git call pattern
  - Previously made separate `diff_hunks()` call per file
  - Now fetches all hunks once and groups by file in memory

### Added

- **Performance Test Suite** - 24 comprehensive benchmarks for Node.js bindings
  - Tests all core functions, index queries, git/diff operations
  - Scalability tests verify linear performance with commit range
  - Located at `bindings/node/test/performance.test.js`

### Fixed

- **Stack Overflow on Large Repos** - Critical fix for repositories with 75K+ files
  - Changed recursive visitor pattern to non-recursive iteration in file traversal
  - Prevents stack overflow on deeply nested or extremely large repositories

- **Stack Overflow on Non-Git Directories** - Critical fix for standalone directory scanning
  - Same non-recursive visitor pattern applied to non-git directory processing
  - `scan()` and `pack()` now handle large non-git directories safely

- **tiktoken Panic Recovery** - Critical fix for certain file contents causing crashes
  - Added `catch_unwind` wrapper around tiktoken BPE encoding calls
  - Falls back to estimation-based counting if tiktoken panics on unusual byte sequences
  - Affects GPT-4o (o200k_base) and GPT-4 (cl100k_base) token counting

- **Token Budget Validation** - Medium fix for negative budget values
  - `pack()` now validates `tokenBudget` parameter and rejects negative values
  - Added `validate_token_budget()` helper function
  - Throws clear error: "Token budget cannot be negative"

- **countTokens Null/Undefined Handling** - Low priority fix for JavaScript edge cases
  - `countTokens(null, model)` and `countTokens(undefined, model)` now return 0
  - Previously crashed with TypeError

- **semanticCompress budget_ratio Effectiveness** - Low priority fix for compression parameters
  - `budget_ratio` now affects content as small as 10 characters (previously required 100+)
  - Fixed threshold in `compress_heuristic()` to enable truncation for small content
  - Added truncation marker showing percentage and character counts

## [0.4.7] - 2025-12-25

### Changed

- **Bindings API Updates** (Python & Node.js)
  - Add `model`, `exclude`, `include` parameters to `pack()` and `scan()`
  - Fix Python `GitRepo.last_built` timestamp format (ISO 8601 strings)
  - Update TypeScript definitions and tests

### Fixed

- **Clippy Warnings** - Fix `derive_ord_xor_partial_ord` in engine
- **Scan Command** - Read file contents for proper line/language detection

### Documentation

- Add missing CLI options to diff, impact, chunk, map command docs
- Add `focused` and `semantic` compression levels to all docs
- Fix chunking strategies in large-repos guide (module, not directory)
- Add filter options (-i, -e) documentation throughout

## [0.4.6] - 2025-12-25

### Fixed

- **Unicode Boundary Panic** - Fix UTF-8 panic in `scan_security()` Python binding
  - Fixed `is_binary_content()` in `bindings/common/scanner.rs`
  - Multi-byte Unicode characters spanning 8192 byte boundary caused panic
  - Now uses `floor_char_boundary()` for safe string slicing

### Added

- Tests for Unicode boundary handling (Chinese, Japanese, Korean, Emoji, Arabic)

## [0.4.5] - 2025-12-25

### Added

- **New APIs** (Node.js & Python bindings)
  - `getChangedSymbolsFiltered()` - Filter changed symbols by kind with changeType field
  - `getTransitiveCallers()` - BFS traversal to find all direct/indirect callers with call paths
  - `getCallSitesWithContext()` - Get call sites with surrounding code context

### Fixed

- Fix duplicate call sites in call graph queries
- Fix contextSymbols being empty when hunk parsing fails
- Fix relatedTests not finding test files by import graph
- Fix contextSnippets population in getDiffContext
- Fix testFiles detection in analyzeImpact

### Added

- 26 Rust engine tests for v0.4.5 features
- 14 Node.js integration tests for new APIs

## [0.4.4] - 2025-12-25

### Added

- **New APIs** (High Priority Features)
  - `getCallSites()` - Find where a symbol is called with line numbers
  - `getChangedSymbols()` - Get symbols modified in a diff
  - `getSymbolsInFile()` - List all symbols in a file
  - `getSymbolSource()` - Get symbol source code
  - `getTestsForFile()` - Find related test files
  - All new APIs include async variants (*Async suffix)

### Fixed

- contextSymbols now populated in getDiffContext (fallback for empty line_ranges)
- relatedTests now finds test files via import graph + naming conventions
- contextSnippets now generated from file content around changed lines
- testFiles in analyzeImpact uses same fix
- getCallSites returns actual call site line numbers (scans caller body)

### Added

- 34 new tests covering all bug fixes and features

## [0.4.3] - 2025-01-24

### Added

- **Documentation Overhaul** - Comprehensive restructure of project documentation
  - New `docs/README.md` - Documentation index and hub
  - New `docs/CONFIGURATION.md` - Comprehensive configuration reference
  - New `docs/getting-started/installation.md` - All installation methods
  - New `docs/getting-started/quick-start.md` - 5-minute tutorial
  - New `docs/guides/llm-optimization.md` - Model-specific tips
  - New `docs/guides/large-repos.md` - Scaling strategies
  - New `docs/guides/ci-integration.md` - CI/CD workflows

### Changed

- **README.md** - Reduced from 703 to 276 lines with better structure
  - Added CI, coverage, npm, PyPI, MSRV badges
  - Added collapsible sections for detailed tables
  - Added real performance benchmarks
  - Cleaner structure with quick navigation

### Fixed

- **CI Formatting** - Run `cargo fmt` on test files that were failing CI
- **Version Bump** - Fixed v0.4.2 PyPI version conflict

## [0.4.2] - 2025-01-24

### Fixed

- **UTF-8 Boundary Bugs** - Fixed panics when handling multi-byte Unicode characters
  - Fixed `security.rs:redact()` panic when secrets contain multi-byte chars
  - Fixed `semantic.rs:compress_repetitive()` panic at pattern boundaries
  - Both fixes use character-based operations instead of byte slicing

### Added

- **112 New Tests** for Unicode safety across all string operations
  - `unicode_boundary_tests.rs` - Parser UTF-8 safety (28 tests)
  - `query_deduplication_tests.rs` - Symbol lookup deduplication (24 tests)
  - `context_expansion_tests.rs` - ContextExpander functionality (23 tests)
  - `string_slicing_boundary_tests.rs` - BudgetEnforcer/Tokenizer safety (37 tests)
  - Coverage for Chinese, Japanese, Korean, Arabic, Hebrew, Cyrillic, Thai, Tamil, Khmer, Myanmar, Gujarati, emoji, and combining characters

## [0.4.1] - 2025-01-24

### Fixed

- **CI Test Failures** - Fixed multiple CI issues
  - Fixed clippy `derive_ord_xor_partial_ord` error in `ImportanceScore`
  - Fixed Python `scan()` to read file contents for proper line counts
  - Fixed Node.js test regex to match "Unknown model" error message
  - Applied `cargo fmt` formatting fixes

## [0.4.0] - 2025-01-24

### Changed

- **Major Architecture Refactoring** - Improved code organization and maintainability
  - Modularized parser into `parser/` directory (21 languages)
  - Modularized tokenizer into `tokenizer/` directory
  - Modularized index into `builder/` and `context/` subdirectories
  - Added shared bindings code in `bindings/common/`
  - Refactored CLI commands into `cli/src/commands/` directory

### Added

- **Type Safety Improvements**
  - New `newtypes.rs` for type-safe wrappers (`SymbolId`, `FileId`, `LineNumber`)
  - New `constants.rs` for centralized configuration values

- **Architecture Documentation**
  - New `engine/ARCHITECTURE.md` with module dependency graph
  - Moved `TEST_SPECIFICATION.md` to `docs/`
  - Updated README.md with correct project structure

### Removed

- Removed `benchmarks/competitive/` (not production-ready)
- Removed `tests/e2e/` (outdated test scripts)
- Removed `docs/IMPROVEMENTS_TRACKER.md` (completed)

## [0.3.4] - 2025-12-23

### Added

- **Call Graph Query API** - Query caller/callee relationships and navigate codebases programmatically
  - `find_symbol()` - Find symbols by name in the index
  - `get_callers()` - Get all functions/methods that call a target symbol
  - `get_callees()` - Get all functions/methods that a target symbol calls
  - `get_references()` - Get all references to a symbol (calls, imports, inheritance)
  - `get_call_graph()` - Get complete call graph with nodes, edges, and statistics
  - New `engine/src/index/query.rs` module with high-level query functions
  - Full Python bindings with sync and async versions
  - Full Node.js bindings with sync and async versions
  - TypeScript type definitions for all call graph types

## [0.3.3] - 2025-12-23

### Added

- **Python Async API** - Non-blocking async operations via `_async.py` module
  - All main functions available as async versions
  - Uses thread pool executor for non-blocking I/O

- **Expanded Git Bindings**
  - Python: `file_at_ref()`, `parse_diff_hunks()` added to `GitRepo`
  - Python: Export `build_index`, `index_status`, `chunk`, `analyze_impact`, `get_diff_context`
  - Node.js: `changed_only`, `base_sha`, `head_sha`, `staged_only` pack options
  - Node.js: `include_related`, `related_depth` for context expansion

### Fixed

- **PyPI Wheel Builds** - All platforms now have working wheels
  - macOS x64 wheel now built (was missing target parameter)
  - Linux ARM64 wheel re-enabled with QEMU + manylinux_2_28
  - npm CLI wrapper script permissions fixed (644 → 755)

## [0.3.2] - 2025-12-23

### Fixed

- **XML Output Validation** - Fixed invalid XML output in `<how_to_use>` section
  - Escaped angle brackets in tip text (`<overview>` → `&lt;overview&gt;`)
  - XML output now passes `xmllint` validation

- **NPM Installation 404 Error** - Fixed binary download URLs in `install.js`
  - Updated artifact names to match actual GitHub release assets
  - Changed from Rust target triples (e.g., `infiniloom-aarch64-apple-darwin.tar.gz`) to simplified names (e.g., `infiniloom-darwin-arm64.tar.gz`)

- **Secret Detection Patterns** - Expanded coverage for additional secret types
  - Added `postgresql://` support to connection string pattern (was only `postgres://`)
  - Added GitHub OAuth, user-to-server, server-to-server, and refresh tokens (`gho_`, `ghu_`, `ghs_`, `ghr_`)
  - Added OpenAI API key pattern (`sk-...`)
  - Added Anthropic API key pattern (`sk-ant-...`)
  - Added MariaDB, CockroachDB, and MSSQL to connection string patterns

### Changed

- **Homebrew Formula Update** - Release workflow now automatically updates the homebrew-infiniloom tap with correct SHA-256 hashes for v0.3.2

## [0.3.1] - 2025-12-23

### Added

- **Node.js Async API** - All Node.js binding functions now support async/await
  - `pack()`, `scan()`, `countTokens()`, etc. return Promises
  - Enables non-blocking operation in Node.js applications

- **Index, Chunk, Impact, Diff Context APIs** - Full API parity for Python and Node.js bindings
  - `buildIndex()` / `build_index()` - Build symbol index for fast diff context
  - `indexStatus()` / `index_status()` - Get index status information
  - `chunk()` - Split repository into manageable chunks
  - `analyzeImpact()` / `analyze_impact()` - Analyze change impact
  - `getDiffContext()` / `get_diff_context()` - Get context-aware diffs

### Fixed

- **Node.js semanticCompress** - Fixed function binding (was undefined)
- **Node.js mapBudget** - Fixed map_budget parameter handling in Infiniloom class

## [0.3.0] - 2025-01-23

### Added

- **npm CLI Package** for global installation via `npm install -g infiniloom`
  - Downloads platform-specific binary from GitHub releases during postinstall
  - Supports darwin-x64, darwin-arm64, linux-x64, linux-arm64, win32-x64, win32-arm64
  - Falls back to cargo/Homebrew installation instructions if download fails

- **Complete Python Binding Documentation**
  - Added `GitRepo` class documentation with all 15 methods
  - Added `is_git_repo()` function documentation
  - Fixed `scan_security()` return type documentation (kind/pattern fields)

- **Complete Node.js Binding Documentation**
  - Added `GitRepo` class documentation with all 15 methods
  - Added `isGitRepo()` function documentation
  - Added `scanSecurity()` function documentation
  - Added TypeScript interfaces: `SecurityFinding`, `GitCommit`, `GitFileStatus`, etc.

### Changed

- **Documentation URLs** - All documentation links now point to https://toposlabs.ai/infiniloom/
  - Updated cli/src/main.rs info command output
  - Updated Python bindings pyproject.toml and README.md
  - Updated Node.js bindings package.json and README.md
  - Updated all doc/*.md files

### Fixed

- **CLI Clippy Warnings** - Fixed all clippy warnings in cli/src/main.rs
  - Changed redundant `match` to `unwrap_or()`
  - Changed `&PathBuf` parameters to `&Path`
  - Changed `.chars().rev().next()` to `.chars().next_back()`

## [0.2.0] - 2025-01-20

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

[Unreleased]: https://github.com/Topos-Labs/infiniloom/compare/v0.6.3...HEAD
[0.6.3]: https://github.com/Topos-Labs/infiniloom/compare/v0.6.2...v0.6.3
[0.6.2]: https://github.com/Topos-Labs/infiniloom/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/Topos-Labs/infiniloom/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/Topos-Labs/infiniloom/compare/v0.5.4...v0.6.0
[0.5.4]: https://github.com/Topos-Labs/infiniloom/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/Topos-Labs/infiniloom/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/Topos-Labs/infiniloom/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.9...v0.5.1
[0.4.9]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.8...v0.4.9
[0.4.8]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.7...v0.4.8
[0.4.7]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.6...v0.4.7
[0.4.6]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.5...v0.4.6
[0.4.5]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.4...v0.4.5
[0.4.4]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/Topos-Labs/infiniloom/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/Topos-Labs/infiniloom/compare/v0.3.4...v0.4.0
[0.3.4]: https://github.com/Topos-Labs/infiniloom/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/Topos-Labs/infiniloom/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/Topos-Labs/infiniloom/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Topos-Labs/infiniloom/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/Topos-Labs/infiniloom/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Topos-Labs/infiniloom/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Topos-Labs/infiniloom/releases/tag/v0.1.0
