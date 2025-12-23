//! Index builder - constructs the symbol index and dependency graph.
//!
//! This module handles parsing files, extracting symbols, resolving imports,
//! and building the dependency graph.

use super::convert::{convert_symbol_kind, convert_visibility};
use super::patterns::{
    GO_IMPORT, JAVA_IMPORT, JS_IMPORT, JS_IMPORT_MULTILINE, JS_REQUIRE, PYTHON_FROM_IMPORT,
    PYTHON_IMPORT, RUST_USE,
};
use super::types::{
    DepGraph, FileEntry, FileId, Import, IndexSymbol, Language, Span, SymbolId, SymbolIndex,
};
use crate::parser::{Language as ParserLanguage, Parser};
use crate::SymbolKind;
use ignore::WalkBuilder;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use regex::Regex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

// Thread-local parser storage to avoid re-initialization
thread_local! {
    static THREAD_PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// Errors that can occur during index building
#[derive(Error, Debug)]
pub enum BuildError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error in {file}: {message}")]
    Parse { file: String, message: String },

    #[error("Repository not found: {0}")]
    RepoNotFound(PathBuf),

    #[error("Git error: {0}")]
    Git(String),
}

static IDENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap());

static COMMON_KEYWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "if",
        "else",
        "for",
        "while",
        "return",
        "break",
        "continue",
        "match",
        "case",
        "switch",
        "default",
        "try",
        "catch",
        "throw",
        "finally",
        "yield",
        "await",
        "async",
        "new",
        "in",
        "of",
        "do",
        "fn",
        "function",
        "def",
        "class",
        "struct",
        "enum",
        "trait",
        "interface",
        "type",
        "impl",
        "let",
        "var",
        "const",
        "static",
        "public",
        "private",
        "protected",
        "internal",
        "use",
        "import",
        "from",
        "package",
        "module",
        "export",
        "super",
        "self",
        "this",
        "crate",
        "pub",
        "mod",
    ]
    .into_iter()
    .collect()
});

/// Options for index building
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Respect .gitignore files
    pub respect_gitignore: bool,
    /// Maximum file size to index (in bytes)
    pub max_file_size: u64,
    /// File extensions to include (empty = all supported)
    pub include_extensions: Vec<String>,
    /// Directories to exclude
    pub exclude_dirs: Vec<String>,
    /// Whether to compute PageRank
    pub compute_pagerank: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            max_file_size: 10 * 1024 * 1024, // 10 MB
            include_extensions: vec![],
            exclude_dirs: vec![
                "node_modules".into(),
                ".git".into(),
                "target".into(),
                "build".into(),
                "dist".into(),
                "__pycache__".into(),
                ".venv".into(),
                "venv".into(),
            ],
            compute_pagerank: true,
        }
    }
}

/// Index builder
pub struct IndexBuilder {
    /// Repository root path
    repo_root: PathBuf,
    /// Build options
    options: BuildOptions,
}

impl IndexBuilder {
    /// Create a new index builder
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        Self { repo_root: repo_root.as_ref().to_path_buf(), options: BuildOptions::default() }
    }

    /// Set build options
    pub fn with_options(mut self, options: BuildOptions) -> Self {
        self.options = options;
        self
    }

    /// Build the symbol index and dependency graph.
    ///
    /// This parses all source files in the repository, extracts symbols,
    /// resolves imports, and computes PageRank scores.
    ///
    /// # Returns
    ///
    /// A tuple of (SymbolIndex, DepGraph) that can be used for fast
    /// diff context generation.
    #[must_use = "index should be used for context queries or saved to disk"]
    pub fn build(&self) -> Result<(SymbolIndex, DepGraph), BuildError> {
        use std::time::Instant;

        if !self.repo_root.exists() {
            return Err(BuildError::RepoNotFound(self.repo_root.clone()));
        }

        let repo_name = self
            .repo_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_owned();

        // Collect files to index
        let t0 = Instant::now();
        let files = self.collect_files()?;
        let collect_time = t0.elapsed();
        log::info!("Found {} files to index", files.len());

        // Parse files in parallel
        let t1 = Instant::now();
        let parsed_files = self.parse_files_parallel(&files)?;
        let parse_time = t1.elapsed();
        log::info!("Parsed {} files", parsed_files.len());

        // Debug timing (when INFINILOOM_TIMING is set)
        let show_timing = std::env::var("INFINILOOM_TIMING").is_ok();
        if show_timing {
            log::info!("  [timing] collect: {:?}", collect_time);
            log::info!("  [timing] parse: {:?}", parse_time);
        }

        // Build the index
        let mut index = SymbolIndex::new();
        index.repo_name = repo_name;
        index.created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Try to get current git commit
        index.commit_hash = self.get_current_commit();

        // Assign IDs and build index
        let mut symbol_id_counter = 0u32;
        let mut file_path_to_id: HashMap<String, u32> = HashMap::new();
        let mut symbol_calls: Vec<(u32, Vec<String>)> = Vec::new();
        let mut symbol_parents: Vec<(u32, String)> = Vec::new();

        for (file_id, parsed) in parsed_files.into_iter().enumerate() {
            let file_id = file_id as u32;
            file_path_to_id.insert(parsed.path.clone(), file_id);

            let symbol_start = symbol_id_counter;

            // Convert parsed symbols to index symbols
            for sym in parsed.symbols {
                index.symbols.push(IndexSymbol {
                    id: SymbolId::new(symbol_id_counter),
                    name: sym.name.clone(),
                    kind: convert_symbol_kind(sym.kind),
                    file_id: FileId::new(file_id),
                    span: Span::new(sym.start_line, 0, sym.end_line, 0),
                    signature: sym.signature,
                    parent: None, // Will be resolved after all symbols are indexed
                    visibility: convert_visibility(sym.visibility),
                    docstring: sym.docstring,
                });
                // Store calls for later graph building (symbol_id -> call names)
                if !sym.calls.is_empty() {
                    symbol_calls.push((symbol_id_counter, sym.calls));
                }
                // Store parent name for later resolution
                if let Some(parent_name) = sym.parent {
                    symbol_parents.push((symbol_id_counter, parent_name));
                }
                symbol_id_counter += 1;
            }

            index.files.push(FileEntry {
                id: FileId::new(file_id),
                path: parsed.path,
                language: parsed.language,
                content_hash: parsed.content_hash,
                symbols: symbol_start..symbol_id_counter,
                imports: parsed.imports,
                lines: parsed.lines,
                tokens: parsed.tokens,
            });
        }

        // Build lookup tables
        let t2 = Instant::now();
        index.rebuild_lookups();
        let lookup_time = t2.elapsed();

        // Resolve parent symbols
        for (symbol_id, parent_name) in &symbol_parents {
            // Find the parent symbol by name (in the same file)
            let symbol = &index.symbols[*symbol_id as usize];
            let file_id = symbol.file_id;
            if let Some(parent_sym) = index
                .symbols
                .iter()
                .find(|s| s.file_id == file_id && s.name == *parent_name && s.kind.is_scope())
            {
                index.symbols[*symbol_id as usize].parent = Some(parent_sym.id);
            }
        }

        // Build dependency graph
        let t3 = Instant::now();
        let mut graph = DepGraph::new();
        self.build_graph(&index, &file_path_to_id, &symbol_calls, &mut graph);
        let graph_time = t3.elapsed();

        // Compute PageRank if enabled
        let mut pagerank_time = std::time::Duration::ZERO;
        if self.options.compute_pagerank {
            let t4 = Instant::now();
            self.compute_pagerank(&index, &mut graph);
            pagerank_time = t4.elapsed();
        }

        if show_timing {
            log::info!("  [timing] lookups: {:?}", lookup_time);
            log::info!("  [timing] graph: {:?}", graph_time);
            log::info!("  [timing] pagerank: {:?}", pagerank_time);
        }

        Ok((index, graph))
    }

    /// Collect files to index using gitignore-aware walking
    fn collect_files(&self) -> Result<Vec<PathBuf>, BuildError> {
        let mut files = Vec::new();
        // Clone exclude_dirs so the closure owns it (needs 'static lifetime for WalkBuilder)
        let exclude_dirs = self.options.exclude_dirs.clone();

        // Use ignore crate for gitignore-aware file walking
        let walker = WalkBuilder::new(&self.repo_root)
            .hidden(false) // Don't skip hidden files by default (we filter below)
            .git_ignore(self.options.respect_gitignore)
            .git_global(self.options.respect_gitignore)
            .git_exclude(self.options.respect_gitignore)
            .filter_entry(move |entry| {
                let path = entry.path();
                // Always skip .git directory
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == ".git" {
                        return false;
                    }
                    // Skip excluded directories
                    if path.is_dir() && exclude_dirs.iter().any(|dir| dir == name) {
                        return false;
                    }
                    // Skip hidden directories (but not hidden files)
                    if path.is_dir() && name.starts_with('.') {
                        return false;
                    }
                }
                true
            })
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if path.is_file() && self.should_index_file(path) {
                files.push(path.to_path_buf());
            }
        }

        Ok(files)
    }

    fn should_index_file(&self, path: &Path) -> bool {
        // Check file size
        if let Ok(metadata) = fs::metadata(path) {
            if metadata.len() > self.options.max_file_size {
                return false;
            }
        }

        // Check extension
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = Language::from_extension(ext);

        if lang == Language::Unknown {
            return false;
        }

        // Check include filter
        if !self.options.include_extensions.is_empty()
            && !self
                .options
                .include_extensions
                .iter()
                .any(|entry| entry == ext)
        {
            return false;
        }

        true
    }

    /// Parse files in parallel
    fn parse_files_parallel(&self, files: &[PathBuf]) -> Result<Vec<ParsedFile>, BuildError> {
        let results: Vec<Result<ParsedFile, BuildError>> =
            files.par_iter().map(|path| self.parse_file(path)).collect();

        // Collect results, logging errors
        let mut parsed = Vec::with_capacity(results.len());
        for result in results {
            match result {
                Ok(f) => parsed.push(f),
                Err(e) => log::warn!("Failed to parse file: {}", e),
            }
        }

        Ok(parsed)
    }

    /// Parse a single file
    fn parse_file(&self, path: &Path) -> Result<ParsedFile, BuildError> {
        let content = fs::read_to_string(path)?;
        let relative_path = path
            .strip_prefix(&self.repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let language = Language::from_extension(ext);

        // Compute content hash
        let content_hash = blake3::hash(content.as_bytes());

        // Count lines
        let lines = content.lines().count() as u32;

        // Estimate tokens (simple approximation)
        let tokens = (content.len() / 4) as u32;

        // Parse symbols using tree-sitter - all 21 languages supported
        let parser_lang = match language {
            Language::Rust => Some(ParserLanguage::Rust),
            Language::Python => Some(ParserLanguage::Python),
            Language::JavaScript => Some(ParserLanguage::JavaScript),
            Language::TypeScript => Some(ParserLanguage::TypeScript),
            Language::Go => Some(ParserLanguage::Go),
            Language::Java => Some(ParserLanguage::Java),
            Language::C => Some(ParserLanguage::C),
            Language::Cpp => Some(ParserLanguage::Cpp),
            Language::CSharp => Some(ParserLanguage::CSharp),
            Language::Ruby => Some(ParserLanguage::Ruby),
            Language::Bash => Some(ParserLanguage::Bash),
            Language::Php => Some(ParserLanguage::Php),
            Language::Kotlin => Some(ParserLanguage::Kotlin),
            Language::Swift => Some(ParserLanguage::Swift),
            Language::Scala => Some(ParserLanguage::Scala),
            Language::Haskell => Some(ParserLanguage::Haskell),
            Language::Elixir => Some(ParserLanguage::Elixir),
            Language::Clojure => Some(ParserLanguage::Clojure),
            Language::OCaml => Some(ParserLanguage::OCaml),
            Language::Lua => Some(ParserLanguage::Lua),
            Language::R => Some(ParserLanguage::R),
            Language::Unknown => None,
        };

        let mut symbols = Vec::new();
        let imports = self.extract_imports(&content, language);

        if let Some(lang) = parser_lang {
            // Use thread-local parser to avoid re-initialization overhead
            THREAD_PARSER.with(|parser_cell| {
                let mut parser = parser_cell.borrow_mut();
                if let Ok(parsed_symbols) = parser.parse(&content, lang) {
                    for sym in parsed_symbols {
                        symbols.push(ParsedSymbol {
                            name: sym.name,
                            kind: sym.kind,
                            start_line: sym.start_line,
                            end_line: sym.end_line,
                            signature: sym.signature,
                            docstring: sym.docstring,
                            parent: sym.parent,
                            visibility: sym.visibility,
                            calls: sym.calls,
                        });
                    }
                }
            });
        }

        Ok(ParsedFile {
            path: relative_path,
            language,
            content_hash: *content_hash.as_bytes(),
            lines,
            tokens,
            symbols,
            imports,
        })
    }

    /// Extract import statements from source code using pre-compiled regexes
    fn extract_imports(&self, content: &str, language: Language) -> Vec<Import> {
        let mut imports = Vec::new();

        if matches!(language, Language::JavaScript | Language::TypeScript) {
            use std::collections::HashSet;

            let mut seen_sources: HashSet<String> = HashSet::new();

            // Line-based imports first (fast path)
            let patterns: &[(&Regex, bool)] = &[(&JS_IMPORT, true), (&JS_REQUIRE, true)];
            for (line_num, line) in content.lines().enumerate() {
                for (re, check_external) in patterns {
                    if let Some(captures) = re.captures(line) {
                        if let Some(source) = captures.get(1) {
                            let source_str = source.as_str().to_owned();
                            if !seen_sources.insert(source_str.clone()) {
                                continue;
                            }
                            let is_external = if *check_external {
                                !source_str.starts_with('.')
                                    && !source_str.starts_with('/')
                                    && !source_str.starts_with("src/")
                            } else {
                                false
                            };
                            imports.push(Import {
                                source: source_str,
                                resolved_file: None,
                                symbols: vec![],
                                span: Span::new(line_num as u32 + 1, 0, line_num as u32 + 1, 0),
                                is_external,
                            });
                        }
                    }
                }
            }

            // Multi-line imports (e.g., import { a, b } from 'x';)
            for caps in JS_IMPORT_MULTILINE.captures_iter(content) {
                if let Some(source) = caps.get(1) {
                    let source_str = source.as_str().to_owned();
                    if !seen_sources.insert(source_str.clone()) {
                        continue;
                    }
                    let line_num = content[..source.start()].matches('\n').count() as u32 + 1;
                    let is_external = !source_str.starts_with('.')
                        && !source_str.starts_with('/')
                        && !source_str.starts_with("src/");
                    imports.push(Import {
                        source: source_str,
                        resolved_file: None,
                        symbols: vec![],
                        span: Span::new(line_num, 0, line_num, 0),
                        is_external,
                    });
                }
            }

            return imports;
        }

        // Get pre-compiled regexes for this language (from shared patterns module)
        let patterns: &[(&Regex, bool)] = match language {
            Language::Python => &[(&PYTHON_IMPORT, false), (&PYTHON_FROM_IMPORT, false)],
            Language::Rust => &[(&RUST_USE, false)],
            Language::Go => &[(&GO_IMPORT, true)],
            Language::Java => &[(&JAVA_IMPORT, false)],
            _ => return imports, // Early return for unsupported languages
        };

        for (line_num, line) in content.lines().enumerate() {
            for (re, check_external) in patterns {
                if let Some(captures) = re.captures(line) {
                    if let Some(source) = captures.get(1) {
                        let source_str = source.as_str().to_owned();
                        let is_external = if *check_external {
                            // Check if it looks like an external package
                            !source_str.starts_with('.')
                                && !source_str.starts_with('/')
                                && !source_str.starts_with("src/")
                        } else {
                            false
                        };

                        imports.push(Import {
                            source: source_str,
                            resolved_file: None,
                            symbols: vec![],
                            span: Span::new(line_num as u32 + 1, 0, line_num as u32 + 1, 0),
                            is_external,
                        });
                    }
                }
            }
        }

        imports
    }

    /// Build dependency graph from index
    fn build_graph(
        &self,
        index: &SymbolIndex,
        file_path_to_id: &HashMap<String, u32>,
        symbol_calls: &[(u32, Vec<String>)],
        graph: &mut DepGraph,
    ) {
        let mut file_imports_by_file: HashMap<u32, Vec<u32>> = HashMap::new();

        // Resolve file imports and build edges
        for file in &index.files {
            for import in &file.imports {
                // Try to resolve the import to a file in the repository
                if let Some(resolved_id) =
                    self.resolve_import(&import.source, &file.path, file_path_to_id)
                {
                    graph.add_file_import(file.id.as_u32(), resolved_id);
                    file_imports_by_file
                        .entry(file.id.as_u32())
                        .or_default()
                        .push(resolved_id);
                }
            }
        }

        // Build symbol name to ID lookup for call resolution
        let mut symbol_name_to_ids: HashMap<&str, Vec<u32>> = HashMap::new();
        for sym in &index.symbols {
            symbol_name_to_ids
                .entry(&sym.name)
                .or_default()
                .push(sym.id.as_u32());
        }

        // Resolve function calls to symbol IDs and add call edges
        for (caller_id, call_names) in symbol_calls {
            let caller = &index.symbols[*caller_id as usize];
            let caller_file_id = caller.file_id;
            let imported_file_ids = file_imports_by_file
                .get(&caller_file_id.as_u32())
                .map(|ids| ids.iter().copied().collect::<HashSet<u32>>());

            for call_name in call_names {
                if let Some(callee_ids) = symbol_name_to_ids.get(call_name.as_str()) {
                    // Prefer symbols in the same file, then any match
                    let callee_id = callee_ids
                        .iter()
                        .find(|&&id| index.symbols[id as usize].file_id == caller_file_id)
                        .or_else(|| {
                            imported_file_ids.as_ref().and_then(|imports| {
                                callee_ids.iter().find(|&&id| {
                                    imports.contains(&index.symbols[id as usize].file_id.as_u32())
                                })
                            })
                        })
                        .or_else(|| callee_ids.first())
                        .copied();

                    if let Some(callee_id) = callee_id {
                        // Don't add self-calls
                        if callee_id != *caller_id {
                            graph.add_call(*caller_id, callee_id);
                        }
                    }
                }
            }
        }

        self.add_symbol_reference_edges(index, &file_imports_by_file, &symbol_name_to_ids, graph);
    }

    fn add_symbol_reference_edges(
        &self,
        index: &SymbolIndex,
        file_imports_by_file: &HashMap<u32, Vec<u32>>,
        symbol_name_to_ids: &HashMap<&str, Vec<u32>>,
        graph: &mut DepGraph,
    ) {
        let mut added: HashSet<(u32, u32)> = HashSet::new();

        for file in &index.files {
            let content = match fs::read_to_string(self.repo_root.join(&file.path)) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let imported_file_ids = file_imports_by_file
                .get(&file.id.as_u32())
                .map(|ids| ids.iter().copied().collect::<HashSet<u32>>());

            for (line_idx, line) in content.lines().enumerate() {
                if self.should_skip_reference_line(line, file.language) {
                    continue;
                }
                let line_no = line_idx as u32 + 1;
                let referencer = match index.find_symbol_at_line(file.id, line_no) {
                    Some(symbol) => symbol,
                    None => continue,
                };

                for mat in IDENT_RE.find_iter(line) {
                    let name = mat.as_str();
                    if name.len() <= 1 || self.is_reference_keyword(name) {
                        continue;
                    }
                    if referencer.span.start_line == line_no && referencer.name == name {
                        continue;
                    }

                    let target_id = symbol_name_to_ids.get(name).and_then(|candidate_ids| {
                        candidate_ids
                            .iter()
                            .find(|&&id| index.symbols[id as usize].file_id == file.id)
                            .or_else(|| {
                                imported_file_ids.as_ref().and_then(|imports| {
                                    candidate_ids.iter().find(|&&id| {
                                        imports
                                            .contains(&index.symbols[id as usize].file_id.as_u32())
                                    })
                                })
                            })
                            .or_else(|| candidate_ids.first())
                            .copied()
                    });

                    if let Some(target_id) = target_id {
                        if target_id != referencer.id.as_u32()
                            && added.insert((referencer.id.as_u32(), target_id))
                        {
                            graph.add_symbol_ref(referencer.id.as_u32(), target_id);
                        }
                    }
                }
            }
        }
    }

    fn should_skip_reference_line(&self, line: &str, language: Language) -> bool {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            return true;
        }

        let comment_prefixes: &[&str] = match language {
            Language::Python | Language::R => &["#"],
            Language::Bash => &["#"],
            Language::Ruby => &["#"],
            Language::Lua => &["--"],
            Language::JavaScript
            | Language::TypeScript
            | Language::C
            | Language::Cpp
            | Language::CSharp
            | Language::Go
            | Language::Java
            | Language::Php
            | Language::Kotlin
            | Language::Swift
            | Language::Scala => &["//"],
            _ => &["//"],
        };

        if comment_prefixes.iter().any(|p| trimmed.starts_with(p)) {
            return true;
        }

        let import_prefixes: &[&str] = match language {
            Language::Python => &["import ", "from "],
            Language::Rust => &["use "],
            Language::Go => &["import "],
            Language::Java => &["import "],
            Language::JavaScript | Language::TypeScript => &["import ", "export ", "require("],
            _ => &[],
        };

        import_prefixes.iter().any(|p| trimmed.starts_with(p))
    }

    fn is_reference_keyword(&self, name: &str) -> bool {
        COMMON_KEYWORDS.contains(name)
    }

    /// Resolve an import path to a file ID
    ///
    /// Handles both absolute and relative imports:
    /// - `./utils` resolves relative to the importing file's directory
    /// - `../shared` resolves to parent directory
    /// - `module` resolves using various strategies (src/, extensions, etc.)
    fn resolve_import(
        &self,
        source: &str,
        importing_file: &str,
        file_path_to_id: &HashMap<String, u32>,
    ) -> Option<u32> {
        // Handle relative imports (./foo, ../bar)
        if source.starts_with("./") || source.starts_with("../") {
            let import_dir = Path::new(importing_file).parent().unwrap_or(Path::new(""));

            // Strip leading ./ for resolution
            let relative_source = source.strip_prefix("./").unwrap_or(source);

            // Resolve the relative path
            let resolved = import_dir.join(relative_source);
            let resolved_str = resolved.to_string_lossy();
            let resolved_str = resolved_str.as_ref();

            // Try different extensions for the relative path
            let relative_candidates = [
                resolved_str.to_owned(),
                format!("{}.ts", resolved_str),
                format!("{}.js", resolved_str),
                format!("{}.tsx", resolved_str),
                format!("{}.jsx", resolved_str),
                format!("{}/index.ts", resolved_str),
                format!("{}/index.js", resolved_str),
                format!("{}.py", resolved_str),
                format!("{}/__init__.py", resolved_str),
            ];

            for candidate in relative_candidates {
                // Normalize path (remove ../ segments)
                let normalized = self.normalize_path(&candidate);
                if let Some(&id) = file_path_to_id.get(&normalized) {
                    return Some(id);
                }
            }
        }

        // Try absolute resolution strategies
        let candidates = [
            source.to_owned(),
            format!("{}.rs", source.replace("::", "/")),
            format!("{}/mod.rs", source.replace("::", "/")),
            format!("{}.py", source.replace(".", "/")),
            format!("{}/__init__.py", source.replace(".", "/")),
            format!("{}.ts", source),
            format!("{}.js", source),
            format!("{}/index.ts", source),
            format!("{}/index.js", source),
            format!("src/{}.rs", source.replace("::", "/")),
            format!("src/{}.py", source.replace(".", "/")),
            format!("src/{}.ts", source),
            format!("src/{}.js", source),
        ];

        for candidate in candidates {
            if let Some(&id) = file_path_to_id.get(&candidate) {
                return Some(id);
            }
        }

        None
    }

    /// Normalize a path by resolving . and .. segments
    fn normalize_path(&self, path: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split('/') {
            match part {
                "" | "." => continue,
                ".." => {
                    parts.pop();
                },
                _ => parts.push(part),
            }
        }
        parts.join("/")
    }

    /// Compute PageRank for files and symbols
    fn compute_pagerank(&self, index: &SymbolIndex, graph: &mut DepGraph) {
        // Compute file-level PageRank
        self.compute_file_pagerank(index, graph);

        // Compute symbol-level PageRank
        self.compute_symbol_pagerank(index, graph);
    }

    /// Compute PageRank for files based on import graph
    fn compute_file_pagerank(&self, index: &SymbolIndex, graph: &mut DepGraph) {
        let n = index.files.len();
        if n == 0 {
            return;
        }

        let damping = 0.85f32;
        let iterations = 20;
        let initial_rank = 1.0 / n as f32;

        let mut ranks: Vec<f32> = vec![initial_rank; n];
        let mut new_ranks: Vec<f32> = vec![0.0; n];

        // Build adjacency for PageRank
        let mut outgoing: Vec<Vec<u32>> = vec![vec![]; n];
        for &(from, to) in &graph.file_imports {
            if (from as usize) < n && (to as usize) < n {
                outgoing[from as usize].push(to);
            }
        }

        for _ in 0..iterations {
            // Reset new ranks
            for r in &mut new_ranks {
                *r = (1.0 - damping) / n as f32;
            }

            // Distribute rank
            for (i, neighbors) in outgoing.iter().enumerate() {
                if !neighbors.is_empty() {
                    let contribution = damping * ranks[i] / neighbors.len() as f32;
                    for &j in neighbors {
                        new_ranks[j as usize] += contribution;
                    }
                } else {
                    // Dangling node: distribute to all
                    let contribution = damping * ranks[i] / n as f32;
                    for r in &mut new_ranks {
                        *r += contribution;
                    }
                }
            }

            std::mem::swap(&mut ranks, &mut new_ranks);
        }

        graph.file_pagerank = ranks;
    }

    /// Compute PageRank for symbols based on call graph
    fn compute_symbol_pagerank(&self, index: &SymbolIndex, graph: &mut DepGraph) {
        let n = index.symbols.len();
        if n == 0 {
            graph.symbol_pagerank = Vec::new();
            return;
        }

        let damping = 0.85f32;
        let iterations = 20;
        let initial_rank = 1.0 / n as f32;

        let mut ranks: Vec<f32> = vec![initial_rank; n];
        let mut new_ranks: Vec<f32> = vec![0.0; n];

        // Build adjacency for symbol PageRank using call graph
        // A symbol's importance is determined by how many other symbols call it
        let mut outgoing: Vec<Vec<u32>> = vec![vec![]; n];
        for &(caller, callee) in &graph.calls {
            if (caller as usize) < n && (callee as usize) < n {
                outgoing[caller as usize].push(callee);
            }
        }

        // Also consider symbol references
        for &(from, to) in &graph.symbol_refs {
            if (from as usize) < n && (to as usize) < n {
                // Avoid duplicate edges
                if !outgoing[from as usize].contains(&to) {
                    outgoing[from as usize].push(to);
                }
            }
        }

        for _ in 0..iterations {
            // Reset new ranks
            for r in &mut new_ranks {
                *r = (1.0 - damping) / n as f32;
            }

            // Distribute rank
            for (i, neighbors) in outgoing.iter().enumerate() {
                if !neighbors.is_empty() {
                    let contribution = damping * ranks[i] / neighbors.len() as f32;
                    for &j in neighbors {
                        new_ranks[j as usize] += contribution;
                    }
                } else {
                    // Dangling node: distribute to all (but with smaller contribution)
                    let contribution = damping * ranks[i] / n as f32;
                    for r in &mut new_ranks {
                        *r += contribution;
                    }
                }
            }

            std::mem::swap(&mut ranks, &mut new_ranks);
        }

        graph.symbol_pagerank = ranks;
    }

    /// Get current git commit hash
    fn get_current_commit(&self) -> Option<String> {
        let git_head = self.repo_root.join(".git/HEAD");
        if let Ok(content) = fs::read_to_string(&git_head) {
            if content.starts_with("ref: ") {
                // It's a reference to a branch
                let ref_path = content.trim_start_matches("ref: ").trim();
                let ref_file = self.repo_root.join(".git").join(ref_path);
                if let Ok(hash) = fs::read_to_string(&ref_file) {
                    return Some(hash.trim().to_owned());
                }
            } else {
                // It's a direct commit hash
                return Some(content.trim().to_owned());
            }
        }
        None
    }
}

/// Intermediate parsed file structure
struct ParsedFile {
    path: String,
    language: Language,
    content_hash: [u8; 32],
    lines: u32,
    tokens: u32,
    symbols: Vec<ParsedSymbol>,
    imports: Vec<Import>,
}

/// Intermediate parsed symbol
struct ParsedSymbol {
    name: String,
    kind: SymbolKind,
    start_line: u32,
    end_line: u32,
    signature: Option<String>,
    docstring: Option<String>,
    parent: Option<String>,
    visibility: crate::types::Visibility,
    calls: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_build_simple_index() {
        let tmp = TempDir::new().unwrap();

        // Create test files directly in tmp root (simpler test)
        fs::write(
            tmp.path().join("main.rs"),
            r#"fn main() {
    println!("Hello, world!");
    helper();
}

fn helper() {
    // Do something
}
"#,
        )
        .unwrap();

        fs::write(
            tmp.path().join("lib.rs"),
            r#"pub mod utils;

pub fn public_fn() {}
"#,
        )
        .unwrap();

        // Build index
        let builder = IndexBuilder::new(tmp.path());
        let (index, graph) = builder.build().unwrap();

        // Verify index found the files
        assert_eq!(
            index.files.len(),
            2,
            "Expected 2 files, found {:?}",
            index.files.iter().map(|f| &f.path).collect::<Vec<_>>()
        );

        // Verify symbols were extracted
        assert!(
            index.symbols.len() >= 3,
            "Expected at least 3 symbols, got {}",
            index.symbols.len()
        );

        // Verify lookups work
        assert!(index.get_file("main.rs").is_some(), "main.rs not found in index");
        assert!(index.get_file("lib.rs").is_some(), "lib.rs not found in index");

        // Verify PageRank was computed
        assert_eq!(graph.file_pagerank.len(), 2);
    }

    #[test]
    fn test_symbol_reference_edges() {
        let tmp = TempDir::new().unwrap();

        fs::write(
            tmp.path().join("lib.rs"),
            r#"pub struct Foo;
"#,
        )
        .unwrap();

        fs::write(
            tmp.path().join("main.rs"),
            r#"mod lib;

fn main() {
    let _value: Foo;
}
"#,
        )
        .unwrap();

        let builder = IndexBuilder::new(tmp.path());
        let (index, graph) = builder.build().unwrap();

        let foo = index.find_symbols("Foo");
        let main = index.find_symbols("main");
        assert!(!foo.is_empty(), "Expected Foo symbol");
        assert!(!main.is_empty(), "Expected main symbol");

        let foo_id = foo[0].id.as_u32();
        let main_id = main[0].id.as_u32();

        let referencers = graph.get_referencers(foo_id);
        assert!(referencers.contains(&main_id), "Expected main to reference Foo");
    }

    #[test]
    fn test_language_detection() {
        // Original languages
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("py"), Language::Python);
        assert_eq!(Language::from_extension("ts"), Language::TypeScript);
        assert_eq!(Language::from_extension("tsx"), Language::TypeScript);
        assert_eq!(Language::from_extension("go"), Language::Go);
        assert_eq!(Language::from_extension("java"), Language::Java);
        assert_eq!(Language::from_extension("js"), Language::JavaScript);
        assert_eq!(Language::from_extension("c"), Language::C);
        assert_eq!(Language::from_extension("cpp"), Language::Cpp);
        assert_eq!(Language::from_extension("cs"), Language::CSharp);
        assert_eq!(Language::from_extension("rb"), Language::Ruby);
        assert_eq!(Language::from_extension("sh"), Language::Bash);
        // New languages
        assert_eq!(Language::from_extension("php"), Language::Php);
        assert_eq!(Language::from_extension("kt"), Language::Kotlin);
        assert_eq!(Language::from_extension("swift"), Language::Swift);
        assert_eq!(Language::from_extension("scala"), Language::Scala);
        assert_eq!(Language::from_extension("hs"), Language::Haskell);
        assert_eq!(Language::from_extension("ex"), Language::Elixir);
        assert_eq!(Language::from_extension("clj"), Language::Clojure);
        assert_eq!(Language::from_extension("ml"), Language::OCaml);
        assert_eq!(Language::from_extension("lua"), Language::Lua);
        assert_eq!(Language::from_extension("r"), Language::R);
    }
}
