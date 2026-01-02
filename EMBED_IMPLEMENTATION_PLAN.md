# Embedding Chunks Implementation Plan

## Overview

Add a new `infiniloom embed` command that generates deterministic, content-addressable code chunks optimized for embedding in vector databases. The system tracks changes between runs via a manifest, enabling efficient incremental updates.

## Goals

1. **Deterministic**: Same repo + same commit = identical output every time
2. **Content-addressable**: Same code anywhere = same chunk ID (enables deduplication)
3. **Code-aware**: Chunks respect AST boundaries (never split mid-function)
4. **Incremental**: Track added/modified/removed chunks between runs
5. **Cross-platform**: Identical output on Windows/Linux/macOS

## Non-Goals (v1)

- Semantic similarity hashing (same logic, different variable names)
- Cross-language deduplication
- Embedding generation (user brings their own embedder)
- Vector DB integration (output is JSONL, user handles storage)

---

## Architecture

### New Module Structure

```
engine/src/
├── embedding/
│   ├── mod.rs              # Public API exports
│   ├── chunker.rs          # Core chunking logic
│   ├── normalizer.rs       # Content normalization for hashing
│   ├── id_generator.rs     # BLAKE3-based chunk ID generation
│   ├── manifest.rs         # Manifest storage and diffing
│   ├── splitter.rs         # Large symbol splitting logic
│   └── types.rs            # Data structures
│
cli/src/commands/
├── embed.rs                # CLI command implementation
```

### Data Flow

```
Repository
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ 1. File Discovery (sorted lexicographically)            │
└─────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ 2. Parse with Tree-sitter (extract symbols per file)    │
│    - Sort symbols by (start_line, start_col, name)      │
└─────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ 3. Generate Chunks                                      │
│    - Symbol-based: one chunk per function/class/etc.    │
│    - Large symbols: split at AST block boundaries       │
│    - Small symbols: optionally merge                    │
└─────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ 4. Normalize & Hash                                     │
│    - Normalize: CRLF→LF, trim trailing whitespace       │
│    - Hash: BLAKE3(normalized_content) → chunk ID        │
└─────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ 5. Compare with Manifest (if exists)                    │
│    - Identify: added, modified, removed, unchanged      │
└─────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ 6. Output (JSONL/JSON) + Update Manifest                │
└─────────────────────────────────────────────────────────┘
```

---

## Data Structures

### Core Types (`types.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single embedding chunk with stable, content-addressable ID
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedChunk {
    /// Content-addressable ID: BLAKE3 hash of normalized content
    /// Format: "ec_" + 16 hex chars (64 bits of hash)
    pub id: String,

    /// The actual code content (normalized)
    pub content: String,

    /// Token count for the target model
    pub tokens: u32,

    /// Symbol kind: function, method, class, struct, etc.
    pub kind: ChunkKind,

    /// Source location metadata (not part of ID)
    pub source: ChunkSource,

    /// For split chunks: part N of M
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<ChunkPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkSource {
    /// Relative file path
    pub file: String,
    /// Line range (1-indexed, inclusive)
    pub lines: (u32, u32),
    /// Symbol name (e.g., "validate_password" or "AuthService::login")
    pub symbol: String,
    /// Fully qualified name if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fqn: Option<String>,
    /// Programming language
    pub language: String,
    /// Parent symbol (for methods inside classes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Module,
    Constant,
    Variable,
    Imports,      // Import block for a file
    TopLevel,     // Top-level code outside symbols
    FunctionPart, // Part of a split large function
    ClassPart,    // Part of a split large class
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkPart {
    pub part: u32,
    pub of: u32,
    /// ID of the parent chunk (if this is a split)
    pub parent_id: String,
}

/// Settings that affect chunk generation
/// Changing these invalidates the manifest
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedSettings {
    /// Maximum tokens per chunk (0 = no limit)
    pub max_tokens: u32,
    /// Minimum tokens per chunk (smaller chunks merged)
    pub min_tokens: u32,
    /// Lines of context around symbols
    pub context_lines: u32,
    /// Include import statements as separate chunks
    pub include_imports: bool,
    /// Include top-level code outside symbols
    pub include_top_level: bool,
    /// Token counting model
    pub token_model: String,
    /// Version of chunking algorithm (for compatibility)
    pub algorithm_version: u32,
}

impl Default for EmbedSettings {
    fn default() -> Self {
        Self {
            max_tokens: 500,
            min_tokens: 50,
            context_lines: 2,
            include_imports: true,
            include_top_level: true,
            token_model: "claude".to_string(),
            algorithm_version: 1,
        }
    }
}
```

### Manifest Types (`manifest.rs`)

```rust
/// Manifest tracking all chunks for incremental updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedManifest {
    /// Manifest format version
    pub version: u32,
    /// Repository root path (absolute, for validation)
    pub repo_path: String,
    /// Git commit hash when manifest was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    /// Timestamp of last update (Unix seconds)
    pub updated_at: u64,
    /// Settings used to generate chunks
    pub settings: EmbedSettings,
    /// All chunks indexed by location key
    /// Key format: "file_path::symbol_name::kind"
    pub chunks: BTreeMap<String, ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Content-addressable chunk ID
    pub chunk_id: String,
    /// Content hash for verification
    pub content_hash: String,
    /// Token count
    pub tokens: u32,
    /// Line range
    pub lines: (u32, u32),
}

impl EmbedManifest {
    pub const CURRENT_VERSION: u32 = 1;

    /// Generate location key for a chunk
    pub fn location_key(file: &str, symbol: &str, kind: ChunkKind) -> String {
        format!("{}::{}::{:?}", file, symbol, kind)
    }
}

/// Result of diffing current state against manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedDiff {
    pub summary: DiffSummary,
    pub added: Vec<EmbedChunk>,
    pub modified: Vec<ModifiedChunk>,
    pub removed: Vec<RemovedChunk>,
    /// IDs of unchanged chunks (not full content)
    pub unchanged: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub total_chunks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedChunk {
    pub old_id: String,
    pub new_id: String,
    pub chunk: EmbedChunk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedChunk {
    pub id: String,
    pub source: ChunkSource,
}
```

---

## Core Algorithms

### Content Normalization (`normalizer.rs`)

```rust
/// Normalize content for deterministic, cross-platform hashing
///
/// Guarantees:
/// - CRLF and CR converted to LF
/// - Trailing whitespace removed from each line
/// - Leading/trailing blank lines removed
/// - Consistent output across platforms
pub fn normalize_for_hash(content: &str) -> String {
    let normalized = content
        .replace("\r\n", "\n")
        .replace("\r", "\n");

    let lines: Vec<&str> = normalized
        .lines()
        .map(|line| line.trim_end())
        .collect();

    // Remove leading blank lines
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
    // Remove trailing blank lines
    let end = lines.iter().rposition(|l| !l.is_empty()).map(|i| i + 1).unwrap_or(0);

    if start >= end {
        return String::new();
    }

    lines[start..end].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_platform() {
        let unix = "fn foo() {\n    bar();\n}";
        let windows = "fn foo() {\r\n    bar();\r\n}";
        let mac_old = "fn foo() {\r    bar();\r}";

        let n1 = normalize_for_hash(unix);
        let n2 = normalize_for_hash(windows);
        let n3 = normalize_for_hash(mac_old);

        assert_eq!(n1, n2);
        assert_eq!(n2, n3);
    }

    #[test]
    fn test_trailing_whitespace() {
        let with_trailing = "fn foo() {   \n    bar();   \n}   ";
        let clean = "fn foo() {\n    bar();\n}";

        assert_eq!(normalize_for_hash(with_trailing), normalize_for_hash(clean));
    }
}
```

### ID Generation (`id_generator.rs`)

```rust
use blake3::Hasher;

/// Generate a deterministic, content-addressable chunk ID
///
/// Format: "ec_" + 16 hex characters (64 bits of BLAKE3 hash)
///
/// Properties:
/// - Same content always produces same ID
/// - Different content (practically) never collides
/// - Prefix "ec_" identifies embedding chunks
pub fn generate_chunk_id(content: &str) -> String {
    let normalized = super::normalizer::normalize_for_hash(content);
    let hash = blake3::hash(normalized.as_bytes());
    format!("ec_{}", &hash.to_hex()[..16])
}

/// Generate full content hash for verification
pub fn generate_content_hash(content: &str) -> String {
    let normalized = super::normalizer::normalize_for_hash(content);
    blake3::hash(normalized.as_bytes()).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic() {
        let content = "fn foo() { bar(); }";
        let id1 = generate_chunk_id(content);
        let id2 = generate_chunk_id(content);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_format() {
        let id = generate_chunk_id("test");
        assert!(id.starts_with("ec_"));
        assert_eq!(id.len(), 3 + 16); // "ec_" + 16 hex chars
    }

    #[test]
    fn test_different_content() {
        let id1 = generate_chunk_id("fn foo() {}");
        let id2 = generate_chunk_id("fn bar() {}");
        assert_ne!(id1, id2);
    }
}
```

### Chunking Algorithm (`chunker.rs`)

```rust
use std::collections::BTreeMap;
use std::path::Path;
use rayon::prelude::*;

pub struct EmbedChunker {
    settings: EmbedSettings,
    tokenizer: Tokenizer,
    parser: Parser,
}

impl EmbedChunker {
    pub fn new(settings: EmbedSettings) -> Self {
        Self {
            tokenizer: Tokenizer::new(),
            parser: Parser::new(),
            settings,
        }
    }

    /// Generate all chunks for a repository
    ///
    /// Guarantees deterministic output:
    /// 1. Files processed in sorted order
    /// 2. Symbols sorted by position
    /// 3. Output sorted by (file, line, id)
    pub fn chunk_repository(&self, repo_path: &Path) -> Result<Vec<EmbedChunk>, Error> {
        // 1. Collect and sort files
        let mut files = self.collect_files(repo_path)?;
        files.sort();

        // 2. Process files in parallel
        let chunks: Vec<EmbedChunk> = files
            .par_iter()
            .flat_map(|file| self.chunk_file(file).unwrap_or_default())
            .collect();

        // 3. Sort for deterministic output
        let mut chunks = chunks;
        chunks.sort_by(|a, b| {
            a.source.file.cmp(&b.source.file)
                .then_with(|| a.source.lines.0.cmp(&b.source.lines.0))
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(chunks)
    }

    /// Chunk a single file
    fn chunk_file(&self, path: &Path) -> Result<Vec<EmbedChunk>, Error> {
        let content = std::fs::read_to_string(path)?;
        let relative_path = self.relative_path(path);
        let language = detect_language(path);

        // Extract and sort symbols
        let mut symbols = self.parser.extract_symbols(&content, &language)?;
        symbols.sort_by_key(|s| (s.start_line, s.start_col, s.name.clone()));

        let mut chunks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        // Track covered line ranges to find top-level code
        let mut covered_lines = vec![false; lines.len()];

        for symbol in &symbols {
            // Skip imports if configured
            if symbol.kind == SymbolKind::Import && !self.settings.include_imports {
                continue;
            }

            // Extract symbol content with context
            let (chunk_content, start, end) = self.extract_symbol_content(
                &lines,
                symbol,
                self.settings.context_lines
            );

            // Mark lines as covered
            for i in (start as usize).saturating_sub(1)..std::cmp::min(end as usize, lines.len()) {
                covered_lines[i] = true;
            }

            let tokens = self.tokenizer.count(&chunk_content, &self.settings.token_model);

            // Handle large symbols
            if self.settings.max_tokens > 0 && tokens > self.settings.max_tokens {
                let split_chunks = self.split_large_symbol(
                    &chunk_content,
                    symbol,
                    &relative_path,
                    &language,
                );
                chunks.extend(split_chunks);
            } else {
                let id = generate_chunk_id(&chunk_content);
                chunks.push(EmbedChunk {
                    id,
                    content: chunk_content,
                    tokens,
                    kind: symbol.kind.into(),
                    source: ChunkSource {
                        file: relative_path.clone(),
                        lines: (start, end),
                        symbol: symbol.name.clone(),
                        fqn: symbol.fqn.clone(),
                        language: language.clone(),
                        parent: symbol.parent.clone(),
                    },
                    part: None,
                });
            }
        }

        // Handle top-level code if configured
        if self.settings.include_top_level {
            let top_level_chunks = self.extract_top_level_chunks(
                &lines,
                &covered_lines,
                &relative_path,
                &language,
            );
            chunks.extend(top_level_chunks);
        }

        Ok(chunks)
    }

    /// Split a large symbol into multiple chunks at AST boundaries
    fn split_large_symbol(
        &self,
        content: &str,
        symbol: &Symbol,
        file: &str,
        language: &str,
    ) -> Vec<EmbedChunk> {
        // Parse the symbol content to find split points
        let split_points = self.find_split_points(content, language);

        let mut chunks = Vec::new();
        let mut current_start = 0;
        let mut part_num = 1;

        // Generate parent ID for reference
        let parent_id = generate_chunk_id(content);

        for split_point in split_points {
            let part_content = &content[current_start..split_point];
            let tokens = self.tokenizer.count(part_content, &self.settings.token_model);

            if tokens >= self.settings.min_tokens {
                let id = generate_chunk_id(part_content);
                chunks.push(EmbedChunk {
                    id,
                    content: part_content.to_string(),
                    tokens,
                    kind: ChunkKind::FunctionPart,
                    source: ChunkSource {
                        file: file.to_string(),
                        lines: (0, 0), // Computed later
                        symbol: format!("{}_part{}", symbol.name, part_num),
                        fqn: symbol.fqn.clone(),
                        language: language.to_string(),
                        parent: Some(symbol.name.clone()),
                    },
                    part: Some(ChunkPart {
                        part: part_num,
                        of: 0, // Updated after all parts generated
                        parent_id: parent_id.clone(),
                    }),
                });
                part_num += 1;
                current_start = split_point;
            }
        }

        // Handle remaining content
        if current_start < content.len() {
            let part_content = &content[current_start..];
            let id = generate_chunk_id(part_content);
            chunks.push(EmbedChunk {
                id,
                content: part_content.to_string(),
                tokens: self.tokenizer.count(part_content, &self.settings.token_model),
                kind: ChunkKind::FunctionPart,
                source: ChunkSource {
                    file: file.to_string(),
                    lines: (0, 0),
                    symbol: format!("{}_part{}", symbol.name, part_num),
                    fqn: symbol.fqn.clone(),
                    language: language.to_string(),
                    parent: Some(symbol.name.clone()),
                },
                part: Some(ChunkPart {
                    part: part_num,
                    of: 0,
                    parent_id: parent_id.clone(),
                }),
            });
        }

        // Update total part count
        let total_parts = chunks.len() as u32;
        for chunk in &mut chunks {
            if let Some(ref mut part) = chunk.part {
                part.of = total_parts;
            }
        }

        chunks
    }

    /// Find AST-aware split points for large content
    fn find_split_points(&self, content: &str, language: &str) -> Vec<usize> {
        // Use Tree-sitter to find block boundaries
        // Prefer splitting at:
        // 1. End of top-level statements within the symbol
        // 2. End of control flow blocks (if/else, match arms, loops)
        // 3. End of logical sections (blank line followed by comment)

        let mut points = Vec::new();
        let target_size = self.settings.max_tokens as usize;

        // Parse and find block nodes
        if let Ok(tree) = self.parser.parse_content(content, language) {
            self.collect_block_boundaries(&tree.root_node(), content, &mut points);
        }

        // If no good split points found, fall back to line boundaries
        if points.is_empty() {
            let lines: Vec<_> = content.match_indices('\n').map(|(i, _)| i + 1).collect();
            let tokens_per_char = self.tokenizer.count(content, &self.settings.token_model) as f32
                / content.len() as f32;
            let chars_per_chunk = (target_size as f32 / tokens_per_char) as usize;

            for line_end in lines {
                if line_end >= chars_per_chunk {
                    points.push(line_end);
                }
            }
        }

        points.sort();
        points.dedup();
        points
    }

    fn collect_block_boundaries(
        &self,
        node: &tree_sitter::Node,
        content: &str,
        points: &mut Vec<usize>
    ) {
        // Collect end positions of block-like nodes
        let block_kinds = [
            "block", "statement_block", "compound_statement",
            "if_statement", "match_arm", "case_clause",
            "for_statement", "while_statement", "loop_expression",
            "function_item", "impl_item", "method_definition",
        ];

        if block_kinds.iter().any(|k| node.kind() == *k) {
            points.push(node.end_byte());
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_block_boundaries(&child, content, points);
        }
    }
}
```

### Manifest Diffing (`manifest.rs`)

```rust
impl EmbedManifest {
    /// Load manifest from file
    pub fn load(path: &Path) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        let manifest: Self = serde_json::from_str(&content)?;

        if manifest.version != Self::CURRENT_VERSION {
            return Err(Error::VersionMismatch {
                expected: Self::CURRENT_VERSION,
                found: manifest.version,
            });
        }

        Ok(manifest)
    }

    /// Save manifest to file
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let content = serde_json::to_string_pretty(self)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, content)?;
        Ok(())
    }

    /// Compute diff between current chunks and manifest
    pub fn diff(&self, current_chunks: &[EmbedChunk]) -> EmbedDiff {
        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut unchanged = Vec::new();

        // Build map of current chunks by location key
        let current_map: BTreeMap<String, &EmbedChunk> = current_chunks
            .iter()
            .map(|c| (Self::location_key(&c.source.file, &c.source.symbol, c.kind), c))
            .collect();

        // Find modified and unchanged
        for (key, entry) in &self.chunks {
            if let Some(current) = current_map.get(key) {
                if current.id == entry.chunk_id {
                    unchanged.push(current.id.clone());
                } else {
                    modified.push(ModifiedChunk {
                        old_id: entry.chunk_id.clone(),
                        new_id: current.id.clone(),
                        chunk: (*current).clone(),
                    });
                }
            }
        }

        // Find added (in current but not in manifest)
        let manifest_keys: std::collections::HashSet<_> = self.chunks.keys().collect();
        for (key, chunk) in &current_map {
            if !manifest_keys.contains(key) {
                added.push((*chunk).clone());
            }
        }

        // Find removed (in manifest but not in current)
        let current_keys: std::collections::HashSet<_> = current_map.keys().collect();
        let removed: Vec<RemovedChunk> = self.chunks
            .iter()
            .filter(|(key, _)| !current_keys.contains(key))
            .map(|(_, entry)| RemovedChunk {
                id: entry.chunk_id.clone(),
                source: ChunkSource {
                    file: String::new(), // Parsed from key
                    lines: entry.lines,
                    symbol: String::new(),
                    fqn: None,
                    language: String::new(),
                    parent: None,
                },
            })
            .collect();

        let summary = DiffSummary {
            added: added.len(),
            modified: modified.len(),
            removed: removed.len(),
            unchanged: unchanged.len(),
            total_chunks: current_chunks.len(),
        };

        EmbedDiff { summary, added, modified, removed, unchanged }
    }

    /// Update manifest with current chunks
    pub fn update(&mut self, chunks: &[EmbedChunk]) {
        self.chunks.clear();

        for chunk in chunks {
            let key = Self::location_key(&chunk.source.file, &chunk.source.symbol, chunk.kind);
            self.chunks.insert(key, ManifestEntry {
                chunk_id: chunk.id.clone(),
                content_hash: generate_content_hash(&chunk.content),
                tokens: chunk.tokens,
                lines: chunk.source.lines,
            });
        }

        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}
```

---

## CLI Interface (`cli/src/commands/embed.rs`)

```rust
use clap::Args;

#[derive(Args, Debug)]
pub struct EmbedArgs {
    /// Repository path (default: current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output file (default: stdout)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output format
    #[arg(short, long, default_value = "jsonl")]
    pub format: OutputFormat,

    /// Show only changes since last run
    #[arg(long)]
    pub diff: bool,

    /// Maximum tokens per chunk (0 = no limit)
    #[arg(long, default_value = "500")]
    pub max_tokens: u32,

    /// Minimum tokens per chunk (smaller merged)
    #[arg(long, default_value = "50")]
    pub min_tokens: u32,

    /// Context lines around symbols
    #[arg(long, default_value = "2")]
    pub context: u32,

    /// Custom manifest path
    #[arg(long)]
    pub manifest: Option<PathBuf>,

    /// Force full rebuild (ignore manifest)
    #[arg(long)]
    pub full: bool,

    /// Show what would be output without writing
    #[arg(long)]
    pub dry_run: bool,

    /// Token counting model
    #[arg(short, long, default_value = "claude")]
    pub model: String,

    /// Include patterns
    #[arg(short = 'i', long)]
    pub include: Vec<String>,

    /// Exclude patterns
    #[arg(short = 'e', long)]
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Jsonl,  // One chunk per line (streaming)
    Json,   // Full JSON array
    Ndjson, // Alias for jsonl
}

pub fn execute(args: EmbedArgs) -> Result<()> {
    let settings = EmbedSettings {
        max_tokens: args.max_tokens,
        min_tokens: args.min_tokens,
        context_lines: args.context,
        include_imports: true,
        include_top_level: true,
        token_model: args.model.clone(),
        algorithm_version: 1,
    };

    let chunker = EmbedChunker::new(settings.clone());
    let chunks = chunker.chunk_repository(&args.path)?;

    let manifest_path = args.manifest.unwrap_or_else(|| {
        args.path.join(".infiniloom-embed.json")
    });

    if args.diff && !args.full && manifest_path.exists() {
        // Incremental mode: output diff
        let manifest = EmbedManifest::load(&manifest_path)?;

        // Validate settings match
        if manifest.settings != settings {
            eprintln!("Warning: Settings changed, performing full rebuild");
            output_full(&chunks, &args)?;
        } else {
            let diff = manifest.diff(&chunks);
            output_diff(&diff, &args)?;
        }
    } else {
        // Full mode: output all chunks
        output_full(&chunks, &args)?;
    }

    // Update manifest (unless dry-run)
    if !args.dry_run {
        let mut manifest = EmbedManifest {
            version: EmbedManifest::CURRENT_VERSION,
            repo_path: args.path.canonicalize()?.to_string_lossy().to_string(),
            commit_hash: get_git_commit(&args.path).ok(),
            updated_at: 0,
            settings,
            chunks: BTreeMap::new(),
        };
        manifest.update(&chunks);
        manifest.save(&manifest_path)?;
    }

    Ok(())
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    // Normalization tests
    #[test] fn test_normalize_crlf() { ... }
    #[test] fn test_normalize_trailing_whitespace() { ... }
    #[test] fn test_normalize_empty_lines() { ... }

    // ID generation tests
    #[test] fn test_id_deterministic() { ... }
    #[test] fn test_id_format() { ... }
    #[test] fn test_id_different_content() { ... }

    // Chunking tests
    #[test] fn test_chunk_simple_function() { ... }
    #[test] fn test_chunk_class_with_methods() { ... }
    #[test] fn test_chunk_large_function_split() { ... }
    #[test] fn test_chunk_respects_ast_boundaries() { ... }

    // Manifest tests
    #[test] fn test_manifest_save_load() { ... }
    #[test] fn test_manifest_diff_added() { ... }
    #[test] fn test_manifest_diff_modified() { ... }
    #[test] fn test_manifest_diff_removed() { ... }
}
```

### Determinism Tests

```rust
#[test]
fn test_full_determinism() {
    let repo = create_test_repo();

    let results: Vec<_> = (0..10)
        .map(|_| {
            let chunker = EmbedChunker::new(EmbedSettings::default());
            chunker.chunk_repository(&repo).unwrap()
        })
        .collect();

    // All runs identical
    for i in 1..10 {
        assert_eq!(results[0], results[i]);
    }

    // JSON serialization identical
    let json0 = serde_json::to_string(&results[0]).unwrap();
    let json1 = serde_json::to_string(&results[1]).unwrap();
    assert_eq!(json0, json1);
}

#[test]
fn test_cross_platform_determinism() {
    // Test that normalized content produces same hash
    let variants = [
        "fn foo() {\n    bar();\n}",
        "fn foo() {\r\n    bar();\r\n}",
        "fn foo() {   \n    bar();   \n}   ",
    ];

    let ids: Vec<_> = variants.iter().map(|c| generate_chunk_id(c)).collect();
    assert!(ids.windows(2).all(|w| w[0] == w[1]));
}
```

### Integration Tests

```rust
#[test]
fn test_incremental_workflow() {
    let repo = create_test_repo();
    let manifest_path = repo.join(".infiniloom-embed.json");

    // First run: full output
    let chunks1 = run_embed(&repo, &[]);
    assert!(!manifest_path.exists() || ...);

    // Second run with --diff: no changes
    let diff1 = run_embed(&repo, &["--diff"]);
    assert_eq!(diff1.added.len(), 0);
    assert_eq!(diff1.modified.len(), 0);

    // Modify a file
    modify_file(&repo, "src/auth.rs");

    // Third run: detect modification
    let diff2 = run_embed(&repo, &["--diff"]);
    assert_eq!(diff2.modified.len(), 1);
}
```

---

## Implementation Phases

### Phase 1: Core Infrastructure (Day 1-2)
- [ ] Create `engine/src/embedding/` module structure
- [ ] Implement `normalizer.rs` with tests
- [ ] Implement `id_generator.rs` with BLAKE3
- [ ] Implement `types.rs` with all data structures

### Phase 2: Chunking Logic (Day 2-3)
- [ ] Implement basic `chunker.rs` (single file)
- [ ] Add symbol extraction using existing Tree-sitter infrastructure
- [ ] Implement large symbol splitting
- [ ] Add deterministic sorting

### Phase 3: Manifest & Diffing (Day 3-4)
- [ ] Implement `manifest.rs` save/load
- [ ] Implement diff algorithm
- [ ] Add manifest validation (settings match)

### Phase 4: CLI Integration (Day 4)
- [ ] Add `embed.rs` command
- [ ] Wire up all options
- [ ] Add output formatting (JSONL, JSON)

### Phase 5: Testing & Polish (Day 5)
- [ ] Comprehensive unit tests
- [ ] Determinism tests
- [ ] Integration tests
- [ ] Documentation

---

## Open Questions

1. **How to handle moved/renamed files?**
   - Current: Appears as removed + added (same content ID though)
   - Alternative: Track by content ID only, report location changes

2. **How to handle merge of small symbols?**
   - Option A: Each symbol is always its own chunk (simpler)
   - Option B: Merge adjacent small symbols up to min_tokens (more complex)

3. **Should imports be file-level or symbol-level?**
   - Current: One imports chunk per file
   - Alternative: Include relevant imports with each symbol

4. **Version the chunk ID algorithm?**
   - If algorithm changes, all IDs change
   - Include version in ID prefix? `ecv1_xxxx`

5. **Git integration depth?**
   - Current: Just store commit hash in manifest
   - Alternative: Use git diff to pre-filter changed files (optimization)

---

## Dependencies

```toml
# New dependencies for engine/Cargo.toml
blake3 = "1.5"  # Already used elsewhere, just need to use in embedding

# No new dependencies needed - reuses existing:
# - tree-sitter (parsing)
# - serde/serde_json (serialization)
# - rayon (parallelism)
# - tiktoken-rs (token counting)
```

---

## Success Criteria

1. **Determinism**: 100 runs on same repo produce byte-identical output
2. **Performance**: Process 10k files in < 30 seconds
3. **Correctness**: All chunks are valid, complete code units
4. **Incremental**: Diff mode correctly identifies all changes
5. **Cross-platform**: Same output on Windows/Linux/macOS
