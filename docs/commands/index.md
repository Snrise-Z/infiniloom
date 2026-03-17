# `infiniloom index` Command

## Overview

The `index` command builds or updates a persistent symbol index for fast diff-to-context operations. The index stores parsed symbols, dependency relationships, and file metadata, enabling sub-second context expansion for code changes.

## Synopsis

```bash
infiniloom index [PATH] [OPTIONS]
```

**Default PATH**: Current directory (`.`)

## Description

The `index` command:

1. **Scans Repository**: Walks all source files
2. **Parses Symbols**: Extracts functions, classes, types via Tree-sitter
3. **Builds Dependency Graph**: Analyzes imports and references
4. **Persists to Disk**: Saves index in `.infiniloom/` directory
5. **Enables Fast Queries**: Used by `diff` and `impact` commands

## Index Storage

```
.infiniloom/
├── index.bin       # Symbol index (bincode serialized)
├── graph.bin       # Dependency graph (bincode serialized)
├── meta.json       # Human-readable metadata
└── config.toml     # Index configuration
```

### meta.json Structure

```json
{
  "version": "1.0.0",
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T14:22:00Z",
  "file_count": 127,
  "symbol_count": 1432,
  "languages": ["rust", "python", "typescript"],
  "build_time_ms": 1234
}
```

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--force` | | Force full rebuild (ignore existing index) | `false` |
| `--incremental` | | Only re-index changed files (faster for large repos) | `false` |
| `--status` | | Show index status without rebuilding | `false` |
| `--verbose` | `-v` | Show detailed progress | `false` |
| `--watch` | | Watch for file changes and auto-rebuild | `false` |
| `--include <PATTERN>` | `-i` | Include only files matching glob pattern (repeatable) | all |
| `--exclude <PATTERN>` | `-e` | Exclude directories/patterns from indexing (comma-separated) | none |
| `--include-tests` | | Include test files in the index (excluded by default) | `false` |

## Index Contents

### SymbolIndex

```rust
struct SymbolIndex {
    // File path -> list of symbols
    files: HashMap<String, Vec<IndexSymbol>>,
    // Symbol name -> locations
    symbols: HashMap<String, Vec<SymbolLocation>>,
    // Quick lookup tables
    by_kind: HashMap<SymbolKind, Vec<String>>,
}

struct IndexSymbol {
    name: String,
    kind: IndexSymbolKind,  // Function, Class, Struct, etc.
    signature: Option<String>,
    visibility: Visibility,  // Public, Private, Protected
    span: Span,  // Start/end lines
    references: Vec<Reference>,
}
```

### DepGraph (Dependency Graph)

```rust
struct DepGraph {
    // File -> files it imports
    imports: HashMap<String, HashSet<String>>,
    // File -> files that import it
    importers: HashMap<String, HashSet<String>>,
    // Symbol -> symbols it calls
    calls: HashMap<String, HashSet<String>>,
    // Symbol -> symbols that call it
    callers: HashMap<String, HashSet<String>>,
}
```

## Technical Implementation

### Incremental Updates

The index supports incremental updates based on file modification times:

```rust
// Check if file needs re-indexing
fn needs_reindex(file: &Path, index: &SymbolIndex) -> bool {
    let current_mtime = file.metadata()?.modified()?;
    let indexed_mtime = index.get_mtime(file)?;
    current_mtime > indexed_mtime
}
```

### Parallel Parsing

Uses thread-local Tree-sitter parsers for lock-free parallel processing:

```rust
let results: Vec<_> = files
    .par_iter()
    .map(|file| {
        THREAD_PARSER.with(|parser| {
            parser.borrow_mut().parse(&content, language)
        })
    })
    .collect();
```

### Language Support

| Language | Import Pattern | Reference Detection |
|----------|---------------|---------------------|
| Rust | `use path::to::module` | Function calls, type references |
| Python | `import module`, `from module import *` | Function calls, class instantiation |
| JavaScript/TypeScript | `import`, `require()` | Function calls, property access |
| Go | `import "package"` | Function calls |
| Java | `import package.Class` | Method calls, class references |

## Examples

### Build Index

```bash
# Build index for current directory
infiniloom index

# Build index for specific path
infiniloom index /path/to/repo

# Force full rebuild
infiniloom index --force

# Incremental update (only re-index changed files)
infiniloom index --incremental

# Index only specific files
infiniloom index -i "src/**" -e "vendor,generated"

# Include test files in the index
infiniloom index --include-tests
```

### Check Status

```bash
# View index status
infiniloom index --status
```

Output:
```
Index Status: infiniloom
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Path:        /Users/dev/infiniloom
  Version:     1.0.0
  Created:     2024-01-15 10:30:00
  Updated:     2024-01-15 14:22:00
  Files:       127
  Symbols:     1,432
  Languages:   rust, python, typescript
  Size:        2.4 MiB
  Build Time:  1.2s
```

### Verbose Build

```bash
# Show detailed progress
infiniloom index -v
```

Output:
```
Building index for infiniloom...
  Scanning files...          127 files found
  Parsing symbols...         1,432 symbols extracted
    rust:                    1,102 symbols
    python:                  234 symbols
    typescript:              96 symbols
  Building dependency graph...
    Imports:                 456 edges
    References:              2,341 edges
  Saving index...            2.4 MiB written
  Done in 1.2s
```

### Watch Mode

Continuously monitor for file changes and automatically rebuild the index:

```bash
# Start watch mode
infiniloom index --watch

# Watch with verbose output
infiniloom index --watch -v
```

Output:
```
👀 Watching for changes... (Ctrl+C to stop)
🔄 Change detected, rebuilding index...
✓ Index updated: 127 files, 1,432 symbols (1.2s)
👀 Watching for changes... (Ctrl+C to stop)
```

**Watch Mode Features:**
- Uses polling (500ms interval) for cross-platform compatibility
- Debounces rapid changes to avoid excessive rebuilds
- Graceful shutdown on Ctrl+C
- Status indicators: 👀 (watching), 🔄 (rebuilding), ✓ (success), ✗ (error)

## Best Practices for LLM Context

### When to Use Index

1. **Active development**: Build index once, use for fast diff context
2. **Code review**: Quick impact analysis of changes
3. **Large repositories**: Index makes queries O(1) instead of O(n)

### Index Maintenance

```bash
# Rebuild after major refactoring
infiniloom index --force

# Check status before diff operations
infiniloom index --status
```

### Integration with Workflow

```bash
# Git pre-commit hook: update index
#!/bin/bash
infiniloom index

# CI pipeline: build index for artifact
infiniloom index
tar -czf index.tar.gz .infiniloom/
```

## Performance Characteristics

### Build Time

| Repository Size | Files | Build Time |
|-----------------|-------|------------|
| Small | <100 | <1s |
| Medium | 100-1000 | 1-5s |
| Large | 1000-10000 | 5-30s |
| Huge | 10000+ | 30s-2min |

### Query Performance

| Operation | Without Index | With Index |
|-----------|--------------|------------|
| Find symbol | O(n) file scan | O(1) lookup |
| Get callers | O(n) full parse | O(1) lookup |
| Expand diff | O(n) file scan | O(k) where k = changed files |

## Potential Improvements

### 1. Partial Index Rebuild

```bash
# Future: only rebuild specific paths
infiniloom index --path src/core
```

### 2. Index Compression

```bash
# Future: compress index for storage
infiniloom index --compress
# Could reduce index size by 60-80%
```

### 3. Distributed Index

```bash
# Future: share index across team
infiniloom index --server
infiniloom index --remote https://team-index.example.com
```

### 4. Cross-Language Reference Detection

Current limitation: References only tracked within same language. Future improvement could track:
- Python calling Rust via PyO3
- TypeScript calling WebAssembly
- JNI calls between Java and C++

### 5. Semantic Versioning of Index

```bash
# Future: version index format
infiniloom index --migrate  # Migrate from older format
```

### 6. Index Validation

```bash
# Future: verify index integrity
infiniloom index --verify
infiniloom index --repair
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Path not found or build error |

## Related Commands

- [`diff`](diff.md) - Use index for fast diff context
- [`impact`](impact.md) - Analyze symbol impact using index
- [`map`](map.md) - Generate symbol map
