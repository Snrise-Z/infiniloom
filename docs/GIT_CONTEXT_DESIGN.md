# Git Context Engine - Design Document

## Overview

Deep git integration for Infiniloom: given any diff, instantly get all relevant context (affected files, imports, call graph, tests) optimized for LLM consumption.

## Scope

- **Single repo** (monorepo support later)
- **All languages** (via tree-sitter)
- **Index in repo** (`.infiniloom/` directory)
- **Committed to git** (deterministic, reproducible)

---

## CLI Interface

```bash
# Index management
infiniloom index                    # Build or update index (incremental)
infiniloom index --force            # Force full rebuild
infiniloom index --status           # Show index stats

# Diff context (core feature)
infiniloom diff .                   # Unstaged changes context
infiniloom diff . --staged          # Staged changes context
infiniloom diff . HEAD~1            # Last commit context
infiniloom diff . HEAD~3..HEAD      # Range of commits
infiniloom diff . main..feature     # Branch comparison
infiniloom diff . abc1234           # Specific commit

# Options
infiniloom diff . --depth 1         # L1: containing functions only
infiniloom diff . --depth 2         # L2: + direct dependents (default)
infiniloom diff . --depth 3         # L3: + transitive dependents
infiniloom diff . --budget 50000    # Token budget limit
infiniloom diff . --format xml      # Output format (xml/json/markdown)
infiniloom diff . --output context.xml
infiniloom diff . --include-diff    # Include actual diff content (+/- lines)

# Impact analysis
infiniloom impact . src/auth.rs             # What depends on this file?
infiniloom impact . --symbol "authenticate" # What calls this?
```

Note: `diff` and `impact` take the repository path first; any commit/range or target comes after the path.

---

## Directory Structure

```
.infiniloom/
├── config.toml          # Index configuration
├── index.bin            # Main symbol index (mmap-able)
├── graph.bin            # Dependency graph (forward + reverse)
├── meta.json            # Index metadata (version, timestamp, commit)
└── .gitignore           # Ignore temporary files only
```

**Committed files:** `config.toml`, `index.bin`, `graph.bin`, `meta.json`
**Gitignored:** Temporary/lock files only

---

## Data Structures

### Symbol

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct Symbol {
    pub id: u32,
    pub name: String,
    pub kind: SymbolKind,
    pub file_id: u32,
    pub span: Span,
    pub signature: Option<String>,  // For functions: full signature
    pub parent: Option<u32>,        // Containing class/module
    pub visibility: Visibility,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    Constant,
    Variable,
    Module,
    Import,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Span {
    pub start_line: u32,
    pub start_col: u16,
    pub end_line: u32,
    pub end_col: u16,
}
```

### File Entry

```rust
#[derive(Serialize, Deserialize)]
pub struct FileEntry {
    pub id: u32,
    pub path: String,
    pub language: Language,
    pub content_hash: [u8; 32],     // BLAKE3 hash for change detection
    pub symbols: Range<u32>,        // Index range into symbols vec
    pub imports: Vec<Import>,
    pub lines: u32,
    pub tokens: u32,                // Pre-computed token count
}

#[derive(Serialize, Deserialize)]
pub struct Import {
    pub source: String,             // "src/utils" or "lodash"
    pub resolved_file: Option<u32>, // Resolved to FileId if internal
    pub symbols: Vec<String>,       // Imported symbol names
    pub span: Span,
}
```

### Symbol Index

```rust
#[derive(Serialize, Deserialize)]
pub struct SymbolIndex {
    pub version: u32,
    pub files: Vec<FileEntry>,
    pub symbols: Vec<Symbol>,

    // Lookup tables (built on load, not serialized)
    #[serde(skip)]
    pub file_by_path: HashMap<String, u32>,
    #[serde(skip)]
    pub symbols_by_name: HashMap<String, Vec<u32>>,
}
```

### Dependency Graph

```rust
#[derive(Serialize, Deserialize)]
pub struct DepGraph {
    // Forward edges: X depends on Y
    pub file_imports: Vec<(u32, u32)>,      // (file_id, imported_file_id)
    pub symbol_refs: Vec<(u32, u32)>,       // (symbol_id, referenced_symbol_id)

    // Reverse edges: Y is depended on by X (crucial for impact analysis)
    pub file_imported_by: Vec<(u32, u32)>,  // (file_id, importing_file_id)
    pub symbol_ref_by: Vec<(u32, u32)>,     // (symbol_id, referencing_symbol_id)

    // Call graph (function calls)
    pub calls: Vec<(u32, u32)>,             // (caller_symbol, callee_symbol)
    pub called_by: Vec<(u32, u32)>,         // (callee_symbol, caller_symbol)

    // Pre-computed metrics
    pub pagerank: Vec<f32>,                 // Importance score per file
}
```

### Diff Change

```rust
/// Represents a change in a git diff
#[derive(Debug, Clone)]
pub struct DiffChange {
    /// File path (for renames, this is the NEW path)
    pub file_path: String,
    /// Old file path (only set for renames)
    pub old_path: Option<String>,
    /// Changed line ranges (start, end)
    pub line_ranges: Vec<(u32, u32)>,
    /// Type of change
    pub change_type: ChangeType,
    /// Raw diff content (the actual +/- lines), if requested
    pub diff_content: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}
```

### Reference

```rust
#[derive(Serialize, Deserialize)]
pub struct Reference {
    pub symbol_id: u32,
    pub file_id: u32,
    pub span: Span,
    pub kind: RefKind,
}

#[derive(Serialize, Deserialize)]
pub enum RefKind {
    Call,           // Function call
    Read,           // Variable read
    Write,          // Variable write
    Import,         // Import statement
    TypeRef,        // Type annotation
    Inheritance,    // Class extends/implements
}
```

---

## Index Building Algorithm

```
1. SCAN FILES
   - Walk repository respecting .gitignore
   - Compute content hash for each file
   - Detect language from extension/content

2. PARSE SYMBOLS (parallel, per-file)
   - Use tree-sitter to parse AST
   - Extract function/class/method definitions
   - Extract import statements
   - Extract symbol references (calls, usages)

3. RESOLVE IMPORTS
   - Map import paths to file IDs where possible
   - Handle relative imports, aliases, re-exports
   - Mark external dependencies (npm, crates, etc.)

4. BUILD DEPENDENCY GRAPH
   - Create forward edges (X imports Y)
   - Create reverse edges (Y imported by X)
   - Create call graph edges

5. COMPUTE METRICS
   - Run PageRank on file dependency graph
   - Store importance scores

6. SERIALIZE
   - Write index.bin (bincode format)
   - Write graph.bin (bincode format)
   - Write meta.json (human-readable metadata)
```

### Incremental Update

```
1. DETECT CHANGES
   - Compare file hashes with stored hashes
   - Identify: added, modified, deleted files

2. PARTIAL REBUILD
   - Re-parse only changed files
   - Update symbol entries
   - Rebuild affected edges

3. PROPAGATE CHANGES
   - Mark files that import changed files as "edges dirty"
   - Rebuild edge list for dirty files
   - Recompute PageRank (fast converge from previous values)
```

---

## Diff Context Algorithm

```
INPUT: Git diff (unified diff format or structured)
OUTPUT: Relevant context for LLM

1. PARSE DIFF
   - Extract changed files (including both old and new paths for renames)
   - For each file: extract changed line ranges

2. MAP TO SYMBOLS
   For each changed line range:
   - Find containing symbol (function/class)
   - If line is in import section, mark as import change
   - Collect: added_symbols, modified_symbols, deleted_symbols

3. EXPAND CONTEXT (depth-based)

   L1 (depth=1): Containing context
   - Full definition of containing function/class
   - Docstrings and signatures

   L2 (depth=2): Direct dependents
   - Files that import the changed file
   - Symbols that call/reference changed symbols
   - Include relevant sections only (not full files)

   L3 (depth=3): Transitive dependents
   - Files that import L2 files
   - Symbols that reference L2 symbols
   - Apply relevance cutoff

4. FIND RELATED TESTS
   - Match test files by naming convention (test_X, X_test, X.spec)
   - Check for explicit test imports of changed symbols
   - Include relevant test functions

5. RANK AND SELECT
   - Score each context piece:
     - PageRank of file (importance)
     - Distance from change (closer = higher)
     - Reference count (more refs = higher)
   - Sort by score
   - Select top items within token budget

6. FORMAT OUTPUT
   - Structure for target LLM (XML for Claude, MD for GPT)
   - Include navigation hints
   - Add summary of changes and impact
```

---

## Output Format

### XML (Claude-optimized)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<diff_context
    repository="infiniloom"
    base="main"
    head="feature/auth"
    generated="2024-01-15T10:30:00Z"
    tokens="12450">

  <summary>
    <description>Modified authentication to support OAuth2 providers</description>
    <stats files_changed="3" symbols_modified="5" dependents="8"/>
    <impact level="high" score="8.2">
      Breaking change: authenticate() signature modified
    </impact>
  </summary>

  <changes>
    <file path="src/auth/service.rs" language="rust" importance="0.92">
      <change type="modified">
        <symbol name="authenticate" kind="function">
          <before start_line="45" end_line="52"><![CDATA[
pub fn authenticate(email: &str, password: &str) -> Result<Token> {
    let user = self.find_user(email)?;
    self.verify_password(password, &user.hash)?;
    Ok(self.create_token(&user))
}
          ]]></before>
          <after start_line="45" end_line="58"><![CDATA[
pub fn authenticate(credentials: AuthCredentials) -> Result<Token> {
    match credentials {
        AuthCredentials::Password { email, password } => {
            let user = self.find_user(&email)?;
            self.verify_password(&password, &user.hash)?;
            Ok(self.create_token(&user))
        }
        AuthCredentials::OAuth { provider, code } => {
            self.oauth_authenticate(provider, code)
        }
    }
}
          ]]></after>
        </symbol>
      </change>

      <change type="added">
        <symbol name="AuthCredentials" kind="enum">
          <content start_line="12" end_line="20"><![CDATA[
pub enum AuthCredentials {
    Password { email: String, password: String },
    OAuth { provider: OAuthProvider, code: String },
}
          ]]></content>
        </symbol>
      </change>
    </file>
  </changes>

  <dependents>
    <file path="src/handlers/login.rs" relevance="0.95" reason="calls authenticate">
      <context start_line="30" end_line="45"><![CDATA[
pub async fn login_handler(req: LoginRequest) -> Response {
    // This needs to be updated for new signature
    let token = auth_service.authenticate(&req.email, &req.password)?;
    Ok(Response::json(token))
}
      ]]></context>
      <action_needed>Update to use AuthCredentials::Password</action_needed>
    </file>

    <file path="src/handlers/oauth.rs" relevance="0.88" reason="calls authenticate">
      <context start_line="25" end_line="35"><![CDATA[
pub async fn oauth_callback(req: OAuthCallback) -> Response {
    let token = auth_service.authenticate(
        AuthCredentials::OAuth {
            provider: req.provider,
            code: req.code
        }
    )?;
    Ok(Response::json(token))
}
      ]]></context>
    </file>
  </dependents>

  <call_graph>
    <chain>
      login_handler → authenticate → verify_password → argon2_verify
    </chain>
    <chain>
      oauth_callback → authenticate → oauth_authenticate → fetch_oauth_user
    </chain>
  </call_graph>

  <tests>
    <test path="tests/auth_test.rs" covers="authenticate" status="needs_update">
      <function name="test_password_auth" line="15"/>
      <function name="test_invalid_password" line="28"/>
    </test>
    <test path="tests/oauth_test.rs" covers="oauth_authenticate" status="new">
      <function name="test_github_oauth" line="10"/>
    </test>
  </tests>

  <file_tree changed="true">
    src/
    ├── auth/
    │   ├── service.rs    [MODIFIED] ← OAuth support added
    │   ├── oauth.rs      [NEW]
    │   └── types.rs      [MODIFIED] ← AuthCredentials enum
    └── handlers/
        ├── login.rs      [AFFECTED] ← Needs update
        └── oauth.rs      [NEW]
  </file_tree>

</diff_context>
```

---

## File Structure (Engine)

```
engine/src/
├── git/                        # Git integration module
│   ├── mod.rs
│   ├── diff.rs                 # Diff parsing
│   ├── repo.rs                 # Repository operations
│   └── history.rs              # Commit history
│
├── index/                      # Index management
│   ├── mod.rs
│   ├── builder.rs              # Index building
│   ├── incremental.rs          # Incremental updates
│   ├── storage.rs              # Serialization/deserialization
│   └── types.rs                # Data structures
│
├── graph/                      # Dependency graph
│   ├── mod.rs
│   ├── builder.rs              # Graph construction
│   ├── query.rs                # Graph queries (dependents, etc.)
│   └── pagerank.rs             # Importance ranking
│
├── context/                    # Diff context generation
│   ├── mod.rs
│   ├── expander.rs             # Context expansion
│   ├── ranker.rs               # Relevance ranking
│   └── selector.rs             # Budget-aware selection
│
└── lib.rs                      # Public API
```

---

## CLI Commands (New)

```
cli/src/
├── commands/
│   ├── mod.rs
│   ├── pack.rs                 # Existing
│   ├── scan.rs                 # Existing
│   ├── map.rs                  # Existing
│   ├── index.rs                # NEW: infiniloom index
│   ├── diff.rs                 # NEW: infiniloom diff
│   └── impact.rs               # NEW: infiniloom impact
```

---

## Performance Targets

| Operation | Target | Strategy |
|-----------|--------|----------|
| Full index (10K files) | <5s | Parallel parsing with Rayon |
| Incremental update | <500ms | Only re-parse changed files |
| Index load | <50ms | Memory-mapped binary format |
| Diff context query | <100ms | Pre-computed reverse edges |
| Watch mode latency | <300ms | Debounced incremental |

---

## Implementation Phases (Completed)

### Phase 1: Index Infrastructure (Week 1)
- [x] Define data structures in `engine/src/index/types.rs`
- [x] Implement serialization with bincode
- [x] Index builder (parallel, tree-sitter based)
- [x] CLI: `infiniloom index` command

### Phase 2: Symbol Extraction (Week 2)
- [x] Extend tree-sitter parsing for symbol extraction
- [x] Import statement parsing (multi-language)
- [x] Symbol reference extraction
- [x] Test with Rust, Python, TypeScript, Go

### Phase 3: Dependency Graph (Week 3)
- [x] Build forward dependency edges
- [x] Build reverse dependency edges
- [x] Import resolution (relative paths, aliases)
- [x] PageRank computation

### Phase 4: Git Integration (Week 4)
- [x] Git integration via system `git` CLI (std::process::Command)
- [x] Diff parsing (staged, commits, ranges)
- [x] Map diff hunks to symbols
- [x] CLI: `infiniloom diff` command

### Phase 5: Context Expansion (Week 5)
- [x] L1: Containing function context
- [x] L2: Direct dependents
- [x] L3: Transitive dependents
- [x] Test file detection

### Phase 6: Output & Polish (Week 6)
- [x] XML output format
- [x] Markdown output format
- [x] Token budget selection
- [x] Impact summary generation

---

## Open Questions

1. **Cross-language imports**: How to handle JS importing from TS, Python importing C extensions?
   Current behavior: extension-based resolution across JS/TS/JSX/TSX/Rust/Python; C-extension imports are unresolved.
2. **Dynamic imports**: `import()` in JS, `__import__` in Python - track or ignore?
   Current behavior: `import ... from`/`require()` are parsed; `import()` and `__import__` are not resolved to internal files.
3. **Macro expansion**: Rust macros generate code - analyze expanded or source?
   Current behavior: macros are captured as invocations; no expansion beyond source text.
4. **Index size**: For very large repos, should we compress or split the index?
   Current behavior: bincode files stored under `.infiniloom/` with no compression or sharding.
5. **Merge conflicts**: Should `infiniloom diff` help with merge conflict resolution?
   Current behavior: relies on `git diff`; no merge-conflict specific handling.

---

## Future Extensions (Phase 2 - intentionally not implemented unless noted)

- **Watch mode (index updates)**: Planned. `infiniloom pack --watch` is implemented for output regeneration.
- **IDE integration**: LSP-based context provider
- **GitHub integration**: Auto-comment PR context
- **Semantic search**: "Find similar code to this change"
- **AI suggestions**: "This change might break X, consider updating Y"
