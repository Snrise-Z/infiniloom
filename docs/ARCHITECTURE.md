# Infiniloom Architecture

**Last Updated**: 2026-03-15
**Version**: 0.7.0

This document provides a comprehensive overview of Infiniloom's architecture, design patterns, and data flow.

---

## Table of Contents

- [System Overview](#system-overview)
- [Module Architecture](#module-architecture)
- [Core Data Structures](#core-data-structures)
- [Processing Pipeline](#processing-pipeline)
- [Parser Subsystem](#parser-subsystem)
- [Symbol Ranking](#symbol-ranking)
- [Token Counting Strategy](#token-counting-strategy)
- [Output Formatting](#output-formatting)
- [Security Architecture](#security-architecture)
- [Performance Design](#performance-design)
- [Index and Call Graph](#index-and-call-graph)
- [Design Patterns](#design-patterns)
- [Extension Points](#extension-points)

---

## System Overview

Infiniloom transforms codebases into optimized context for Large Language Models. The architecture follows these principles:

**Core Principles**:
1. **Accuracy over speed** - Exact token counts via tiktoken where possible
2. **Parallel by default** - Thread-local parsers, Rayon parallel iterators
3. **Modular design** - Clear separation of concerns, trait-based abstractions
4. **Zero-cost abstractions** - Rust's ownership model enables safe parallelism without runtime overhead
5. **Progressive enhancement** - Fast path for basic operations, comprehensive path for full analysis

**System Architecture (High Level)**:
```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Infiniloom CLI                                 │
│  ┌─────────────────┐  ┌──────────────────┐  ┌────────────────────┐    │
│  │ Command Parser  │  │  Config Loader   │  │  Output Generator  │    │
│  │    (clap)       │  │ (YAML/TOML/JSON) │  │   (stdout/file)    │    │
│  └────────┬────────┘  └────────┬─────────┘  └─────────┬──────────┘    │
│           │                    │                       │                │
│           └────────────────────┴───────────────────────┘                │
└────────────────────────────────┬────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Infiniloom Engine                                 │
│                                                                          │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────┐  ┌──────────────┐  │
│  │  Scanner    │→ │   Parser     │→ │   Ranker   │→ │  Formatter   │  │
│  │  (files)    │  │  (symbols)   │  │ (PageRank) │  │  (XML/MD)    │  │
│  └──────┬──────┘  └──────┬───────┘  └─────┬──────┘  └──────┬───────┘  │
│         │                │                 │                │           │
│         ▼                ▼                 ▼                ▼           │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Core Data Structures                          │   │
│  │  Repository → RepoFile → Symbol → SymbolIndex → DepGraph        │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                   Cross-Cutting Concerns                         │   │
│  │  Security Scanner • Tokenizer • Budget Enforcer • Cache          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Module Architecture

Infiniloom is organized into a workspace with three main crates:

```
infiniloom/
├── engine/          # Core Rust library (infiniloom_engine)
├── cli/             # Command-line interface
└── bindings/        # Language bindings (Python, Node.js)
```

### Engine Module Structure

```
engine/src/
├── types.rs                   # Core data structures (Repository, RepoFile, Symbol)
├── newtypes.rs                # Type-safe ID wrappers (SymbolId, FileId, LineNumber)
├── constants.rs               # Shared constants and magic numbers
├── error.rs                   # Unified error types
│
├── parser/                    # AST-based symbol extraction
│   ├── core.rs                # Parser struct and methods
│   ├── language.rs            # Language enum and detection (23 detected, 21 with full AST support)
│   ├── extraction.rs          # Symbol extraction from AST
│   ├── queries.rs             # Tree-sitter query definitions
│   ├── query_builder.rs       # Dynamic query construction
│   └── thread_local.rs        # Thread-local parser optimization
│
├── tokenizer/                 # Multi-model token counting
│   ├── core.rs                # Tokenizer struct with tiktoken integration
│   ├── models.rs              # TokenizerModel enum (27 models)
│   └── counts.rs              # TokenCounts struct
│
├── repomap/                   # Symbol importance ranking
│   ├── mod.rs                 # RepoMapGenerator
│   └── graph.rs               # SymbolGraph with PageRank
│
├── output/                    # Format generators
│   ├── mod.rs                 # Formatter traits (Formatter, StreamingFormatter)
│   ├── escaping.rs            # Text escaping utilities (XML, YAML)
│   ├── xml.rs                 # Claude-optimized XML formatter
│   ├── markdown.rs            # GPT-optimized Markdown formatter
│   └── toon.rs                # Token-efficient TOON formatter
│
├── index/                     # Symbol index for fast diff context
│   ├── builder/               # Index building with parallel parsing
│   │   ├── core.rs            # IndexBuilder implementation
│   │   ├── graph.rs           # Dependency graph construction
│   │   └── types.rs           # Builder-specific types
│   ├── context/               # Diff context expansion
│   │   ├── expander.rs        # ContextExpander implementation
│   │   └── types.rs           # Context types (DiffChange, etc.)
│   ├── lazy.rs                # On-the-fly context generation
│   ├── storage.rs             # Bincode serialization
│   ├── types.rs               # Index types (SymbolIndex, DepGraph)
│   ├── query.rs               # Call graph query API
│   ├── convert.rs             # Type conversion utilities
│   └── patterns.rs            # Pre-compiled regex patterns
│
├── security.rs                # Secret detection and redaction
├── ranking.rs                 # File importance ranking
├── budget.rs                  # Token budget enforcement
├── semantic.rs                # Character-frequency compression (heuristic)
├── chunking/                  # Semantic code chunking
├── embedding/                 # Embedding chunks for vector DBs
│   ├── mod.rs                 # Module exports
│   ├── chunker.rs             # EmbedChunker with parallel processing
│   ├── types.rs               # EmbedChunk, EmbedSettings, ChunkKind
│   ├── manifest.rs            # Manifest for incremental updates
│   ├── streaming.rs           # Streaming JSONL output mode
│   ├── hasher.rs              # BLAKE3 content-addressable hashing
│   ├── normalizer.rs          # Cross-platform content normalization
│   ├── limits.rs              # Resource limits (DoS protection)
│   ├── progress.rs            # Progress reporting
│   ├── complexity.rs          # Cyclomatic complexity scoring
│   ├── identifiers.rs         # BM25-friendly identifier extraction
│   ├── type_extraction.rs     # Type signature extraction
│   └── error.rs               # Embedding-specific errors
├── filtering.rs               # File pattern matching with caching
├── content_processing.rs      # Base64 truncation utilities
├── content_transformation.rs  # Code compression functions
├── config.rs                  # Configuration loading (YAML/TOML/JSON)
├── git.rs                     # Git operations (log, status, diff)
├── remote.rs                  # Remote repository cloning
├── dependencies.rs            # Dependency graph resolution
├── mmap_scanner.rs            # Memory-mapped file scanning
└── incremental.rs             # File-level caching with change detection
```

### CLI Module Structure

```
cli/src/
├── main.rs                    # CLI entry point with clap argument parsing
├── config.rs                  # Configuration loading utilities
├── scanner.rs                 # Repository scanning with parallel processing
├── watch.rs                   # File watching for incremental updates
├── error.rs                   # CLI-specific error types
└── commands/                  # Command implementations
    ├── pack.rs                # Pack command (main output generation)
    ├── scan.rs                # Scan command (statistics)
    ├── map.rs                 # Map command (symbol ranking)
    ├── chunk.rs               # Chunk command (repo splitting)
    ├── diff.rs                # Diff command (context-aware diffs)
    ├── index.rs               # Index command (build symbol index)
    ├── impact.rs              # Impact command (change analysis)
    ├── embed.rs               # Embed command (vector DB chunks)
    ├── init.rs                # Init command (config creation)
    └── info.rs                # Info command (version/config display)
```

---

## Core Data Structures

### Repository Hierarchy

```
Repository
├── name: String
├── path: PathBuf
├── files: Vec<RepoFile>
└── metadata: RepoMetadata
    ├── total_files: usize
    ├── total_lines: usize
    ├── languages: HashMap<String, usize>
    └── total_tokens: TokenCounts

RepoFile
├── relative_path: String
├── absolute_path: PathBuf
├── size: u64
├── content: String
├── language: Option<String>
├── symbols: Vec<Symbol>
├── token_count: TokenCounts
└── importance: f64

Symbol
├── id: Option<SymbolId>           # Unique ID (set by IndexBuilder)
├── name: String
├── kind: SymbolKind
├── start_line: usize
├── end_line: usize
├── signature: Option<String>
├── visibility: Visibility
└── documentation: Option<String>
```

### Symbol Kinds

The `SymbolKind` enum distinguishes 11 types of symbols:

```rust
pub enum SymbolKind {
    Function,    // Standalone functions
    Class,       // Class definitions
    Method,      // Methods within classes/structs
    Struct,      // Struct definitions (Rust, C, etc.)
    Enum,        // Enum definitions
    Trait,       // Traits (Rust) or Interfaces
    Interface,   // Interfaces (TypeScript, Java, etc.)
    Constant,    // Named constants
    Variable,    // Module-level variables
    Import,      // Import statements
    Type,        // Type aliases/definitions
}
```

### Newtype Wrappers

Type-safe ID wrappers prevent confusion:

```rust
pub struct SymbolId(u32);        // Unique symbol identifier
pub struct FileId(u32);          // Unique file identifier
pub struct LineNumber(usize);    // 1-indexed line number (not 0-indexed!)
pub struct ByteOffset(usize);    // Byte offset in file
pub struct FileSize(u64);        // File size in bytes
pub struct TokenCount(u32);      // Token count
pub struct ImportanceScore(f64); // Importance score (0.0-1.0)
```

### Token Counts

`TokenCounts` stores token counts for multiple models grouped by encoding family:

```rust
pub struct TokenCounts {
    pub o200k: u32,      // OpenAI modern (GPT-5.x, GPT-4o, O1/O3/O4) - exact
    pub cl100k: u32,     // OpenAI legacy (GPT-4, GPT-3.5-turbo) - exact
    pub claude: u32,     // Anthropic Claude - estimation
    pub gemini: u32,     // Google Gemini - estimation
    pub llama: u32,      // Meta Llama - estimation
    pub mistral: u32,    // Mistral - estimation
    pub deepseek: u32,   // DeepSeek - estimation
    pub qwen: u32,       // Qwen - estimation
    pub cohere: u32,     // Cohere - estimation
    pub grok: u32,       // xAI Grok - estimation
}
```

**Design Rationale**: Store all token counts upfront to avoid recomputation. OpenAI models use exact tiktoken-based counts, while other models use calibrated estimation (~95% for prose, ~85% for code).

---

## Processing Pipeline

The core pipeline has 5 stages:

```
1. SCANNING         2. PARSING          3. RANKING          4. FORMATTING       5. OUTPUT
┌──────────┐       ┌──────────┐        ┌──────────┐        ┌──────────┐       ┌──────────┐
│ Walk     │       │ Extract  │        │ PageRank │        │ XML/MD   │       │ File/    │
│ Files    │   →   │ Symbols  │   →    │ Compute  │   →    │ Generate │   →   │ Stdout   │
│ (ignore) │       │ (AST)    │        │ Scores   │        │ Output   │       │ Stream   │
└──────────┘       └──────────┘        └──────────┘        └──────────┘       └──────────┘
     ↓                  ↓                    ↓                   ↓                  ↓
  Parallel         Thread-local          Graph              Streaming          Low-memory
 (Rayon iter)       parsers             algorithm            optional            buffering
```

### 1. Scanning Stage (`cli/scanner.rs`)

**Purpose**: Walk directory tree, detect languages, filter files.

**Key Components**:
- **`WalkBuilder`** (from `ignore` crate): Gitignore-respecting directory traversal
- **Pattern filtering**: Include/exclude patterns with glob support
- **Language detection**: File extension-based, 23 languages detected (21 with full AST support)
- **Binary detection**: First 8KB checked for binary content

**Parallel Strategy**:
```rust
file_infos
    .into_par_iter()           // Rayon parallel iterator
    .filter_map(|file_info| {
        // Each worker thread processes files independently
        let content = fs::read_to_string(&file_info.path).ok()?;
        let symbols = parse_with_thread_local(&content, lang);
        Some(RepoFile { content, symbols, ... })
    })
    .collect()
```

### 2. Parsing Stage (`engine/src/parser/`)

**Purpose**: Extract symbols from source code using Tree-sitter AST parsing.

**Key Components**:
- **Tree-sitter**: Fast, incremental parsing with error recovery
- **Query system**: S-expression queries for each language
- **Thread-local parsers**: One parser per thread to avoid mutex contention

**Thread-Local Pattern**:
```rust
thread_local! {
    static THREAD_PARSER: OnceCell<Parser> = const { OnceCell::new() };
}

pub fn parse_file_symbols(path: &Path, content: &str) -> Option<Vec<Symbol>> {
    let ext = path.extension()?.to_str()?;
    let lang = Language::from_extension(ext)?;

    THREAD_PARSER.with(|cell| {
        let parser = cell.get_or_init(|| Parser::new());  // Lazy init once per thread
        parser.parse(content, lang).ok()
    })
}
```

**Performance**: Thread-local parsers provide 2-3x speedup over shared mutex-protected parser on multi-core systems.

### 3. Ranking Stage (`engine/src/repomap/graph.rs`)

**Purpose**: Compute importance scores using PageRank algorithm.

**Algorithm**:
```
PageRank(symbol) = (1-d) + d * Σ(PageRank(caller) / out_degree(caller))
```

Where:
- `d = 0.85` (damping factor)
- Iterations: 20 (convergence threshold: 0.001)
- Graph: Directed edges from importers to imported symbols

**Implementation**:
```rust
pub fn compute_pagerank(&self) -> HashMap<String, f64> {
    let mut ranks: HashMap<String, f64> = self.nodes
        .iter()
        .map(|name| (name.clone(), 1.0))
        .collect();

    for _ in 0..20 {  // 20 iterations for convergence
        let mut new_ranks = HashMap::new();

        for node in &self.nodes {
            let mut rank = 1.0 - DAMPING_FACTOR;

            // Sum contributions from incoming edges
            for (from, to) in &self.edges {
                if to == node {
                    let from_rank = ranks.get(from).copied().unwrap_or(1.0);
                    let out_degree = self.out_degree(from);
                    rank += DAMPING_FACTOR * from_rank / out_degree as f64;
                }
            }

            new_ranks.insert(node.clone(), rank);
        }

        ranks = new_ranks;
    }

    ranks
}
```

### 4. Formatting Stage (`engine/src/output/`)

**Purpose**: Generate model-specific output formats.

**Formatter Trait**:
```rust
pub trait Formatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String;
    fn format_repo(&self, repo: &Repository) -> String;
    fn name(&self) -> &'static str;
}

pub trait StreamingFormatter {
    fn format_to_writer<W: Write>(
        &self,
        repo: &Repository,
        map: &RepoMap,
        writer: &mut W,
    ) -> io::Result<()>;
}
```

**Format Selection**:
- **XML**: Claude-optimized, CDATA sections for code
- **Markdown**: GPT-optimized, fenced code blocks with syntax highlighting
- **TOON**: Token-efficient (30-40% fewer tokens), custom format
- **YAML**: Compatible with Gemini and other models, query at end
- **JSON**: Machine-readable, fully structured
- **Plain**: Simple text, no formatting

### 5. Output Stage

**Purpose**: Write formatted output to destination.

**Strategies**:
- **In-memory**: Build entire string, write once (small repos)
- **Streaming**: Write incrementally via `BufWriter` (large repos)
- **Clipboard**: Copy to system clipboard if requested
- **File**: Write to specified output path
- **Stdout**: Default output destination

---

## Parser Subsystem

### Tree-sitter Integration

Tree-sitter provides fast, incremental, error-tolerant parsing.

**Query System**:
```rust
// Example query for Rust functions
const RUST_QUERY: &str = r#"
    (function_item
        name: (identifier) @name
        parameters: (parameters) @params
        return_type: (return_type)? @return
    ) @function

    (impl_item
        type: (type_identifier) @impl_type
        body: (declaration_list
            (function_item
                name: (identifier) @method_name
            ) @method
        )
    )
"#;
```

**Symbol Extraction Flow**:
```
Source Code
    ↓
Tree-sitter Parse → AST
    ↓
Query Match
    ↓
Extract: name, kind, start_line, end_line, signature, visibility
    ↓
Symbol struct
```

### Language Support Matrix

| Language   | Extension(s)       | Tree-sitter Parser | Symbol Extraction |
|------------|-------------------|-------------------|-------------------|
| Python     | .py               | ✅ Yes             | ✅ Full           |
| JavaScript | .js, .jsx         | ✅ Yes             | ✅ Full           |
| TypeScript | .ts, .tsx         | ✅ Yes             | ✅ Full           |
| Rust       | .rs               | ✅ Yes             | ✅ Full           |
| Go         | .go               | ✅ Yes             | ✅ Full           |
| Java       | .java             | ✅ Yes             | ✅ Full           |
| C          | .c, .h            | ✅ Yes             | ✅ Full           |
| C++        | .cpp, .hpp, .cc   | ✅ Yes             | ✅ Full           |
| C#         | .cs               | ✅ Yes             | ✅ Full           |
| Ruby       | .rb               | ✅ Yes             | ✅ Full           |
| PHP        | .php              | ✅ Yes             | ✅ Full           |
| Kotlin     | .kt               | ✅ Yes             | ✅ Full           |
| Swift      | .swift            | ✅ Yes             | ✅ Full           |
| Scala      | .scala            | ✅ Yes             | ✅ Full           |
| Bash       | .sh, .bash        | ✅ Yes             | ✅ Full           |
| Haskell    | .hs               | ✅ Yes             | ✅ Full           |
| Elixir     | .ex, .exs         | ✅ Yes             | ✅ Full           |
| Clojure    | .clj, .cljs       | ⚠️ Deprecated      | ⚠️ Limited        |
| OCaml      | .ml, .mli         | ✅ Yes             | ✅ Full           |
| Lua        | .lua              | ✅ Yes             | ✅ Full           |
| R          | .r, .R            | ✅ Yes             | ✅ Full           |
| HCL        | .tf, .hcl             | ✅ Yes             | ✅ Full           |
| Zig        | .zig                  | ✅ Yes             | ✅ Full           |
| Dart       | .dart                 | ✅ Yes             | ✅ Full           |
| F#         | .fs, .fsx         | ❌ Not yet         | ⚠️ Basic          |

**Note:** Clojure and F# are deprecated as of v0.7.0 (no compatible tree-sitter grammars). Files are still detected but receive text-only processing.

**Extension Strategy**: If no parser available, fall back to basic regex-based extraction for common patterns.

---

## Symbol Ranking

### PageRank Algorithm

Infiniloom uses PageRank (same algorithm as Google Search) to rank symbol importance.

**Graph Construction**:
1. **Nodes**: All symbols (functions, classes, etc.)
2. **Edges**: Directed edges from caller → callee (imports, function calls)

**Why PageRank?**
- **Centrality measure**: Identifies "hub" symbols that many others depend on
- **Transitive**: Symbols important to important symbols become important
- **Stable**: Converges to fixed point after ~20 iterations

**Damping Factor** (`d = 0.85`):
- 85% probability: Follow an edge to a dependency
- 15% probability: Random jump to any symbol
- Prevents infinite loops and ensures convergence

**Example**:
```
Symbol Graph:
    main() → parse_args(), run_server()
    run_server() → handle_request(), send_response()
    handle_request() → validate_input(), process_data()

PageRank Scores:
    handle_request(): 0.85  (called by server, calls validators)
    run_server(): 0.72      (called by main, calls handlers)
    main(): 0.65            (entry point, calls multiple)
    parse_args(): 0.42      (utility, called by main only)
```

### File Ranking

File importance combines multiple signals:

```rust
pub fn rank_files(repo: &mut Repository, symbol_ranks: &HashMap<String, f64>) {
    for file in &mut repo.files {
        let mut score = 0.0;

        // 1. Symbol importance (primary signal)
        for symbol in &file.symbols {
            score += symbol_ranks.get(&symbol.name).copied().unwrap_or(0.0);
        }

        // 2. File size penalty (prefer concise files)
        let size_penalty = 1.0 / (1.0 + file.content.len() as f64 / 10000.0);

        // 3. Language bonus (core languages ranked higher)
        let lang_bonus = match file.language.as_deref() {
            Some("rust") | Some("python") | Some("typescript") => 1.2,
            Some("javascript") | Some("go") | Some("java") => 1.1,
            _ => 1.0,
        };

        file.importance = score * size_penalty * lang_bonus;
    }
}
```

---

## Token Counting Strategy

### Multi-Model Support

Infiniloom counts tokens for 27 different LLM tokenizers grouped by encoding:

**Exact Counting (via tiktoken)**:
- **o200k_base** (OpenAI modern): GPT-5.2, GPT-5.1, GPT-5, O4-mini, O3, O1, GPT-4o, GPT-4o-mini
- **cl100k_base** (OpenAI legacy): GPT-4, GPT-3.5-turbo

**Calibrated Estimation** (~95% for prose, ~85% for code):
- Claude (Anthropic)
- Gemini (Google)
- Llama, CodeLlama (Meta)
- Mistral, DeepSeek, Qwen, Cohere, Grok

### Tiktoken Integration

```rust
use tiktoken_rs::{cl100k_base, o200k_base};

pub struct Tokenizer {
    o200k: OnceCell<CoreBPE>,   // Lazy-initialized
    cl100k: OnceCell<CoreBPE>,  // Lazy-initialized
}

impl Tokenizer {
    pub fn count(&self, text: &str, model: TokenizerModel) -> u32 {
        match model {
            // Exact via tiktoken
            TokenizerModel::Gpt4o | TokenizerModel::Gpt5 => {
                let bpe = self.o200k.get_or_init(|| o200k_base().unwrap());
                bpe.encode_with_special_tokens(text).len() as u32
            }

            // Exact via tiktoken (legacy)
            TokenizerModel::Gpt4 | TokenizerModel::Gpt35Turbo => {
                let bpe = self.cl100k.get_or_init(|| cl100k_base().unwrap());
                bpe.encode_with_special_tokens(text).len() as u32
            }

            // Calibrated estimation
            TokenizerModel::Claude => {
                // Approximate: characters / 3.8 (based on empirical testing)
                (text.len() as f64 / 3.8).ceil() as u32
            }

            _ => self.estimate_tokens(text, model),
        }
    }
}
```

**Why Estimation?**:
- Tiktoken only supports OpenAI tokenizers
- Other vendors don't provide public tokenizers
- Estimation is ~95% accurate for prose and ~85% for code based on benchmark testing

---

## Output Formatting

### Format Design Principles

Each format is optimized for its target LLM:

**XML (Claude)**:
```xml
<documents>
  <document index="1">
    <source>src/main.rs</source>
    <document_content><![CDATA[
      fn main() {
          println!("Hello, world!");
      }
    ]]></document_content>
  </document>
</documents>
```

**Design choices**:
- CDATA sections: Preserve code structure without escaping
- Numeric indices: Clear document ordering
- Flat structure: No deep nesting (Claude prefers shallow)

**Markdown (GPT)**:
````markdown
# Repository: my-project

## Files

### src/main.rs

```rust
fn main() {
    println!("Hello, world!");
}
```
````

**Design choices**:
- Fenced code blocks: Syntax highlighting signals
- Hierarchical headers: GPT-4 understands markdown structure well
- Language tags: Enables syntax-aware reasoning

**TOON (Token-efficient)**:
```
REPO:my-project
FILE:src/main.rs|rust|15
fn main() {
    println!("Hello, world!");
}
```

**Design choices**:
- Minimal delimiters: `|` instead of XML tags
- No whitespace padding: Every character counts
- Inline metadata: No separate header sections
- 30-40% fewer tokens than XML/Markdown

### Streaming Architecture

For large repositories (100MB+), streaming avoids memory exhaustion:

```rust
pub trait StreamingFormatter {
    fn format_to_writer<W: Write>(
        &self,
        repo: &Repository,
        map: &RepoMap,
        writer: &mut W,
    ) -> io::Result<()> {
        // Write header
        self.write_header(writer)?;

        // Stream files one at a time
        for file in &repo.files {
            self.write_file(file, writer)?;
        }

        // Write footer
        self.write_footer(writer)?;

        Ok(())
    }
}
```

**Benefits**:
- Constant memory usage (buffered I/O only)
- Can process 10GB+ repositories
- No intermediate String allocation

---

## Security Architecture

### Secret Detection

Infiniloom uses 17 pre-compiled regex patterns to detect secrets:

**Pattern Categories**:
1. **API Keys**: AWS, Stripe, SendGrid, MailChimp, Twilio, MailGun, PayPal
2. **Access Tokens**: Generic tokens, Bearer tokens, Basic auth
3. **Credentials**: Private keys, passwords in URLs, database connection strings
4. **Cloud**: GCP, Azure credentials

**Implementation**:
```rust
use once_cell::sync::Lazy;
use regex::Regex;

static AWS_KEY_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"AKIA[0-9A-Z]{16}").unwrap()
});

static BEARER_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"Bearer\s+[A-Za-z0-9\-._~+/]+=*").unwrap()
});

// ... 15 more patterns

pub fn scan_content(content: &str) -> Vec<SecretMatch> {
    let mut matches = Vec::new();

    if AWS_KEY_PATTERN.is_match(content) {
        matches.push(SecretMatch { pattern: "AWS_ACCESS_KEY", ... });
    }

    // ... check all patterns

    matches
}
```

**Performance**: `Lazy` ensures patterns compile once at first use, then reused across all scans.

### Redaction Strategy

When secrets are detected, multiple redaction strategies are available:

1. **Replace with [REDACTED]**: Simple, preserves line structure
2. **Replace with placeholder**: `<SECRET_TYPE_REDACTED>`
3. **Remove line**: Delete entire line containing secret
4. **Mask partial**: Show first/last 4 characters: `AKIA************WXYZ`

**Allowlist Support**:
```yaml
# .infiniloom.yaml
security:
  scan_secrets: true
  redact_secrets: true
  allowlist:
    - "EXAMPLE_KEY"
    - "test_token_"
```

---

## Performance Design

### Thread-Local Parsers

**Problem**: Tree-sitter parsers are not thread-safe. Mutex-protected shared parser creates contention bottleneck.

**Solution**: Thread-local parsers with lazy initialization.

```rust
thread_local! {
    static THREAD_PARSER: OnceCell<Parser> = const { OnceCell::new() };
}

pub fn parse_file_symbols(path: &Path, content: &str) -> Option<Vec<Symbol>> {
    THREAD_PARSER.with(|cell| {
        // Each thread has its own parser - no contention!
        let parser = cell.get_or_init(|| Parser::new());
        parser.parse(content, Language::from_path(path)?).ok()
    })
}
```

**Performance**:
- No mutex locks
- CPU cache locality (thread affinity)
- 2-3x faster than shared mutex parser on 8-core systems

### Parallel File Processing

Uses Rayon's parallel iterators for automatic work-stealing:

```rust
use rayon::prelude::*;

let files: Vec<RepoFile> = file_paths
    .into_par_iter()                    // Parallel iterator
    .filter_map(|path| {
        let content = fs::read_to_string(&path).ok()?;
        let symbols = parse_file_symbols(&path, &content)?;
        Some(RepoFile { path, content, symbols, ... })
    })
    .collect();
```

**Rayon Benefits**:
- Work-stealing scheduler: Idle threads steal tasks from busy threads
- Zero-cost abstraction: No runtime overhead vs manual threading
- Automatic scaling: Uses all available CPU cores

### Incremental Caching

File-level caching with change detection:

```rust
pub struct CachedFile {
    pub path: PathBuf,
    pub mtime: SystemTime,      // Fast check
    pub size: u64,              // Fast check
    pub content_hash: u64,      // Accurate check
    pub symbols: Vec<Symbol>,   // Cached result
    pub token_count: TokenCounts,
}

impl RepoCache {
    pub fn needs_rescan(&self, path: &Path) -> bool {
        let Some(cached) = self.files.get(path) else {
            return true;  // Not cached - need scan
        };

        let Ok(metadata) = fs::metadata(path) else {
            return true;  // Can't read - need scan
        };

        // Fast path: mtime/size comparison
        metadata.modified().ok() != Some(cached.mtime) || metadata.len() != cached.size
    }

    pub fn needs_rescan_with_content(&self, path: &Path, content: &[u8]) -> bool {
        let Some(cached) = self.files.get(path) else {
            return true;
        };

        // Accurate path: content hash comparison
        let content_hash = hash_content(content);
        content_hash != cached.content_hash
    }
}
```

**Two-Level Strategy**:
1. **Fast check**: `mtime` + `size` (no I/O)
2. **Accurate check**: `content_hash` (catches same-size changes)

---

## Index and Call Graph

### Symbol Index

The `SymbolIndex` provides fast symbol lookup and call graph querying:

**Structure**:
```rust
pub struct SymbolIndex {
    pub symbols: HashMap<SymbolId, Symbol>,      // All symbols
    pub by_name: HashMap<String, Vec<SymbolId>>, // Lookup by name
    pub by_file: HashMap<PathBuf, Vec<SymbolId>>, // Symbols per file
}

impl SymbolIndex {
    pub fn find_symbol(&self, name: &str) -> Vec<SymbolInfo> {
        self.by_name.get(name)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.symbols.get(id))
                    .map(|sym| SymbolInfo::from(sym))
                    .collect()
            })
            .unwrap_or_default()
    }
}
```

### Dependency Graph

The `DepGraph` stores call relationships:

**Structure**:
```rust
pub struct DepGraph {
    pub edges: HashSet<(SymbolId, SymbolId)>,  // (caller_id, callee_id)
}

impl DepGraph {
    pub fn get_callers(&self, symbol_id: SymbolId) -> HashSet<SymbolId> {
        self.edges.iter()
            .filter(|(_, callee)| *callee == symbol_id)
            .map(|(caller, _)| *caller)
            .collect()
    }

    pub fn get_callees(&self, symbol_id: SymbolId) -> HashSet<SymbolId> {
        self.edges.iter()
            .filter(|(caller, _)| *caller == symbol_id)
            .map(|(_, callee)| *callee)
            .collect()
    }
}
```

### Index Building

Parallel index construction:

```rust
pub struct IndexBuilder {
    index: SymbolIndex,
    graph: DepGraph,
    next_symbol_id: AtomicU32,
}

impl IndexBuilder {
    pub fn build(&mut self, repo_path: &Path) -> Result<()> {
        // 1. Scan all files (parallel)
        let files = self.scan_files_parallel(repo_path)?;

        // 2. Extract symbols (parallel, thread-local parsers)
        let all_symbols: Vec<_> = files.par_iter()
            .flat_map(|file| {
                parse_file_symbols(&file.path, &file.content)
                    .unwrap_or_default()
            })
            .collect();

        // 3. Assign IDs
        for symbol in all_symbols {
            let id = SymbolId(self.next_symbol_id.fetch_add(1, Ordering::Relaxed));
            self.index.symbols.insert(id, symbol);
        }

        // 4. Build dependency graph
        self.build_graph()?;

        Ok(())
    }
}
```

### Diff Context Expansion

Expands changed files to include dependencies:

```rust
pub struct ContextExpander {
    index: SymbolIndex,
    graph: DepGraph,
}

impl ContextExpander {
    pub fn expand_context(
        &self,
        changed_files: &[DiffChange],
        depth: u8,
    ) -> Vec<PathBuf> {
        let mut context_files = HashSet::new();

        // Level 1: Changed files themselves
        for change in changed_files {
            context_files.insert(change.file_path.clone());
        }

        if depth >= 2 {
            // Level 2: Direct dependencies
            for change in changed_files {
                let symbols = self.index.symbols_in_file(&change.file_path);
                for symbol in symbols {
                    let callers = self.graph.get_callers(symbol.id);
                    for caller_id in callers {
                        if let Some(caller) = self.index.symbols.get(&caller_id) {
                            context_files.insert(caller.file_path.clone());
                        }
                    }
                }
            }
        }

        if depth >= 3 {
            // Level 3: Transitive dependencies
            // ... BFS traversal
        }

        context_files.into_iter().collect()
    }
}
```

---

## Design Patterns

### 1. Builder Pattern

Used extensively for configuration:

```rust
let config = PackConfig::builder()
    .path(PathBuf::from("/repo"))
    .output(OutputOptions {
        format: Some(OutputFormat::Xml),
        model: Some(TokenModel::Claude),
        compression: Some(CompressionLevel::Balanced),
        max_tokens: 10000,
        ..Default::default()
    })
    .scan(ScanOptions {
        include_hidden: false,
        respect_gitignore: true,
        ..Default::default()
    })
    .build()?;
```

**Why**: Type-safe construction with compile-time validation of required fields.

### 2. Trait-Based Abstraction

Output formatters use traits for extensibility:

```rust
pub trait Formatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String;
    fn format_repo(&self, repo: &Repository) -> String;
    fn name(&self) -> &'static str;
}

pub struct XmlFormatter;
pub struct MarkdownFormatter;
pub struct ToonFormatter;

impl Formatter for XmlFormatter { /* ... */ }
impl Formatter for MarkdownFormatter { /* ... */ }
impl Formatter for ToonFormatter { /* ... */ }
```

### 3. Newtype Pattern

Type-safe wrappers prevent ID confusion:

```rust
pub struct SymbolId(u32);
pub struct FileId(u32);
pub struct LineNumber(usize);

// Compile error: can't pass SymbolId where FileId expected
fn process_file(id: FileId) { /* ... */ }
let symbol_id = SymbolId(42);
process_file(symbol_id);  // ❌ Type error!
```

### 4. Lazy Static Pattern

Compile regex once, reuse forever:

```rust
use once_cell::sync::Lazy;
use regex::Regex;

static PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[0-9]+").unwrap()
});

// Used in tight loop - no recompilation overhead
for line in file.lines() {
    if PATTERN.is_match(line) { /* ... */ }
}
```

### 5. OnceCell Pattern

Thread-local lazy initialization:

```rust
use std::cell::OnceCell;

thread_local! {
    static PARSER: OnceCell<Parser> = const { OnceCell::new() };
}

PARSER.with(|cell| {
    let parser = cell.get_or_init(|| Parser::new());  // Init once per thread
    parser.parse(content, lang)
})
```

---

## Extension Points

### Adding a New Language

1. **Add to Language enum** (`engine/src/parser/language.rs`):
```rust
pub enum Language {
    // ... existing
    NewLang,
}
```

2. **Add extension mapping**:
```rust
impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            // ... existing
            "newlang" => Some(Language::NewLang),
            _ => None,
        }
    }
}
```

3. **Add Tree-sitter query** (`engine/src/parser/queries.rs`):
```rust
const NEWLANG_QUERY: &str = r#"
    (function_declaration
        name: (identifier) @name
    ) @function
"#;
```

4. **Update supported languages list** in README and docs.

### Adding a New Output Format

1. **Create formatter module** (`engine/src/output/newformat.rs`):
```rust
pub struct NewFormatter;

impl Formatter for NewFormatter {
    fn format(&self, repo: &Repository, map: &RepoMap) -> String {
        // ... generate output
    }

    fn format_repo(&self, repo: &Repository) -> String {
        // ... format without map
    }

    fn name(&self) -> &'static str {
        "newformat"
    }
}
```

2. **Add to OutputFormat enum**:
```rust
pub enum OutputFormat {
    // ... existing
    NewFormat,
}
```

3. **Update formatter factory**:
```rust
pub fn by_format(format: OutputFormat) -> Box<dyn Formatter> {
    match format {
        // ... existing
        OutputFormat::NewFormat => Box::new(NewFormatter),
    }
}
```

### Adding a New Tokenizer Model

1. **Add to TokenizerModel enum** (`engine/src/tokenizer/models.rs`):
```rust
pub enum TokenizerModel {
    // ... existing
    NewModel,
}
```

2. **Add counting logic** (`engine/src/tokenizer/core.rs`):
```rust
impl Tokenizer {
    pub fn count(&self, text: &str, model: TokenizerModel) -> u32 {
        match model {
            // ... existing
            TokenizerModel::NewModel => {
                // Estimation formula based on testing
                (text.len() as f64 / 3.5).ceil() as u32
            }
        }
    }
}
```

3. **Update TokenCounts struct** if needed for new encoding family.

### Adding a New CLI Command

1. **Add command enum variant** (`cli/src/main.rs`):
```rust
enum Commands {
    // ... existing
    NewCmd {
        #[arg(short, long)]
        option: Option<String>,
    },
}
```

2. **Create command module** (`cli/src/commands/newcmd.rs`):
```rust
pub fn cmd_newcmd(option: Option<String>) -> Result<()> {
    // ... implementation
    Ok(())
}
```

3. **Add match arm** in `main()`:
```rust
Commands::NewCmd { option } => commands::cmd_newcmd(option),
```

---

## Conclusion

Infiniloom's architecture prioritizes:
- **Correctness**: Type-safe, validated data structures
- **Performance**: Parallel processing, lazy initialization, efficient algorithms
- **Extensibility**: Trait-based abstractions, modular design
- **Usability**: Simple API, sensible defaults, comprehensive documentation

For questions or contributions, see:
- [CONTRIBUTING.md](../CONTRIBUTING.md)
- [GitHub Issues](https://github.com/Topos-Labs/infiniloom/issues)
- [GitHub Discussions](https://github.com/Topos-Labs/infiniloom/discussions)

