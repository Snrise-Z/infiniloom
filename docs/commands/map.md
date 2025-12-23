# `infiniloom map` Command

## Overview

The `map` command generates a repository map - a condensed view of the most important symbols (functions, classes, types) in the codebase. It uses PageRank algorithm on the symbol dependency graph to identify key architectural components.

## Synopsis

```bash
infiniloom map [PATH] [OPTIONS]
```

**Default PATH**: Current directory (`.`)

## Description

The `map` command:

1. **Scans Repository**: Walks directory with gitignore respect
2. **Extracts Symbols**: Uses Tree-sitter for AST-based symbol extraction
3. **Builds Dependency Graph**: Analyzes imports/references between symbols
4. **Computes PageRank**: Ranks symbols by importance in the dependency graph
5. **Generates Map**: Outputs top symbols within token budget

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--budget <TOKENS>` | `-b` | Token budget for map output | `2000` |
| `--output <PATH>` | `-o` | Output file (default: stdout) | stdout |

## Output Format

```
Repository Map: infiniloom
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Files: 127 | Symbols: 1,432 | Budget: 2,000 tokens

Key Symbols (by importance):

engine/src/parser.rs
  ├─ Parser::parse (function) [importance: 0.892]
  ├─ Language::from_extension (function) [importance: 0.756]
  └─ Symbol (struct) [importance: 0.723]

engine/src/tokenizer.rs
  ├─ Tokenizer::count (function) [importance: 0.845]
  ├─ TokenModel (enum) [importance: 0.712]
  └─ Tokenizer::new (function) [importance: 0.698]

engine/src/types.rs
  ├─ Repository (struct) [importance: 0.801]
  ├─ RepoFile (struct) [importance: 0.778]
  └─ Symbol (struct) [importance: 0.654]

cli/src/main.rs
  ├─ cmd_pack (function) [importance: 0.634]
  └─ cmd_scan (function) [importance: 0.589]

[Budget used: 1,847/2,000 tokens]
```

## Technical Implementation

### PageRank Algorithm

The importance ranking uses a graph-based approach:

```rust
// Build symbol graph from imports and references
let graph = SymbolGraph::new();
for file in &repo.files {
    for symbol in &file.symbols {
        graph.add_symbol(&file.path, symbol);
    }
    for import in extract_imports(&file) {
        graph.add_edge(&import.from, &import.to);
    }
}

// Compute PageRank (damping=0.85, iterations=20)
let ranks = graph.compute_pagerank(0.85, 20);
```

### Damping Factor

The damping factor (0.85) represents the probability of following a link vs. jumping to a random node:
- Higher values (0.9+): Emphasizes highly-connected symbols
- Lower values (0.7-): More even distribution

### Symbol Extraction by Language

| Language | Extracted Symbols |
|----------|-------------------|
| Rust | `fn`, `struct`, `enum`, `trait`, `impl`, `mod`, `const`, `static`, `type` |
| Python | `def`, `class`, `async def` |
| JavaScript/TypeScript | `function`, `class`, `const`, `let`, `var`, arrow functions |
| Go | `func`, `type`, `struct`, `interface` |
| Java | `class`, `interface`, `enum`, methods |
| C/C++ | `function`, `struct`, `class`, `enum`, `typedef` |

### Budget Allocation

Symbols are added to the map in PageRank order until budget is exhausted:

```rust
let mut used_tokens = 0;
let mut selected = Vec::new();

for (symbol, rank) in ranks.iter().sorted_by_rank() {
    let symbol_tokens = estimate_symbol_tokens(symbol);
    if used_tokens + symbol_tokens <= budget {
        selected.push((symbol, rank));
        used_tokens += symbol_tokens;
    }
}
```

## Examples

### Basic Usage

```bash
# Generate map to stdout
infiniloom map

# Save to file
infiniloom map -o repo-map.txt
```

### Budget Control

```bash
# Minimal map (top symbols only)
infiniloom map --budget 500

# Detailed map
infiniloom map --budget 5000
```

### Integration with Pack

```bash
# Use map as pre-context for pack
infiniloom map --budget 3000 > map.txt
infiniloom pack --header-text "$(cat map.txt)" -o context.xml
```

## Best Practices for LLM Context

### Optimal Budget Selection

| Use Case | Recommended Budget | Rationale |
|----------|-------------------|-----------|
| Quick overview | 500-1000 | Core architecture only |
| Code review | 2000-3000 | Key symbols + relationships |
| Deep analysis | 5000-10000 | Comprehensive view |
| Full context | Match pack budget | Include in pack output |

### Using Maps Effectively

1. **Pre-prompt context**: Include map before asking about architecture
2. **Navigation aid**: Use map to identify files to include in pack
3. **Dependency analysis**: Understand which symbols are central

```bash
# Generate map, then pack relevant files
infiniloom map --budget 1000 > map.txt
# Review map, identify key files
infiniloom pack --include "src/core/*" -o focused-context.xml
```

## Performance Characteristics

### Current Implementation

- **Symbol extraction**: Tree-sitter parsing (parallel)
- **Graph construction**: O(n) where n = number of symbols
- **PageRank**: O(i * e) where i = iterations, e = edges

### Bottlenecks

1. **Tree-sitter initialization**: ~50ms per language
2. **Graph memory**: Large repos may have 10K+ symbols
3. **Single-threaded PageRank**: Could be parallelized

### Potential Improvements

1. **Cached symbol index**
   ```bash
   # Future: reuse index from pack --cache
   infiniloom map --use-index
   ```

2. **Filtered graph building**
   ```bash
   # Future: only analyze specified paths
   infiniloom map --scope src/core
   ```

3. **Alternative ranking algorithms**
   ```bash
   # Future: use different centrality measures
   infiniloom map --algorithm betweenness
   infiniloom map --algorithm eigenvector
   ```

4. **Incremental updates**
   ```bash
   # Future: update only changed files
   infiniloom map --incremental
   ```

5. **Language-specific importance weights**
   - Public APIs weighted higher than private
   - Test files weighted lower
   - Entry points (main, index) weighted higher

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Path not found or parsing error |

## Related Commands

- [`pack`](pack.md) - Generate full context with embedded map
- [`index`](index.md) - Build persistent symbol index
- [`impact`](impact.md) - Analyze symbol dependencies
