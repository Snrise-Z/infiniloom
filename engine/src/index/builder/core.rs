//! Core index builder implementation.
//!
//! Contains the IndexBuilder struct and main build logic.

use super::graph::GraphBuilder;
use super::types::{BuildError, BuildOptions, ParsedFile, ParsedSymbol, THREAD_PARSER};
use crate::index::convert::{convert_symbol_kind, convert_visibility};
use crate::index::patterns::{
    GO_IMPORT, JAVA_IMPORT, JS_IMPORT, JS_IMPORT_MULTILINE, JS_REQUIRE, PYTHON_FROM_IMPORT,
    PYTHON_IMPORT, RUST_USE,
};
use crate::index::types::{
    DepGraph, FileEntry, FileId, Import, IndexSymbol, Language, Span, SymbolId, SymbolIndex,
};
use crate::parser::Language as ParserLanguage;
use ignore::WalkBuilder;
use rayon::prelude::*;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Index builder
pub struct IndexBuilder {
    /// Repository root path
    pub(super) repo_root: PathBuf,
    /// Build options
    pub(super) options: BuildOptions,
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
        tracing::info!("Found {} files to index", files.len());

        // Parse files in parallel
        let t1 = Instant::now();
        let parsed_files = self.parse_files_parallel(&files)?;
        let parse_time = t1.elapsed();
        tracing::info!("Parsed {} files", parsed_files.len());

        // Debug timing (when INFINILOOM_TIMING is set)
        let show_timing = std::env::var("INFINILOOM_TIMING").is_ok();
        if show_timing {
            tracing::info!("  [timing] collect: {:?}", collect_time);
            tracing::info!("  [timing] parse: {:?}", parse_time);
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
        let graph_builder = GraphBuilder::new(&self.repo_root);
        graph_builder.build_graph(&index, &file_path_to_id, &symbol_calls, &mut graph);
        let graph_time = t3.elapsed();

        // Compute PageRank if enabled
        let mut pagerank_time = std::time::Duration::ZERO;
        if self.options.compute_pagerank {
            let t4 = Instant::now();
            graph_builder.compute_pagerank(&index, &mut graph);
            pagerank_time = t4.elapsed();
        }

        if show_timing {
            tracing::info!("  [timing] lookups: {:?}", lookup_time);
            tracing::info!("  [timing] graph: {:?}", graph_time);
            tracing::info!("  [timing] pagerank: {:?}", pagerank_time);
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
                Err(e) => tracing::warn!("Failed to parse file: {}", e),
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

    /// Get current git commit hash
    pub(super) fn get_current_commit(&self) -> Option<String> {
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
