//! Lazy/on-the-fly diff context generation.
//!
//! This module provides diff context without requiring a pre-built index.
//! It parses only the changed files and their dependencies on-the-fly.

use super::context::{ContextDepth, ContextExpander, DiffChange, ExpandedContext};
use super::convert::convert_symbol_kind;
use super::patterns::{
    GO_IMPORT, JAVA_IMPORT, JS_IMPORT, JS_IMPORT_MULTILINE, JS_REQUIRE, PYTHON_FROM_IMPORT,
    PYTHON_IMPORT, RUST_USE,
};
use super::types::{
    DepGraph, FileEntry, FileId, Import, IndexSymbol, Language, Span, SymbolId, SymbolIndex,
    Visibility,
};
use crate::parser::{Language as ParserLanguage, Parser};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors from lazy indexing
#[derive(Error, Debug)]
pub enum LazyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),
}

static IDENT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("IDENT_RE: invalid regex pattern"));

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

/// Lazy context builder that generates context on-the-fly
pub struct LazyContextBuilder {
    repo_root: PathBuf,
    /// Maximum depth to follow imports
    max_import_depth: usize,
    /// Already processed files (to avoid cycles)
    processed_files: HashSet<String>,
}

impl LazyContextBuilder {
    /// Create a new lazy context builder
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
            max_import_depth: 3,
            processed_files: HashSet::new(),
        }
    }

    /// Set maximum import depth
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_import_depth = depth;
        self
    }

    /// Generate context for a diff without pre-built index
    pub fn generate_context(
        &mut self,
        changes: &[DiffChange],
        depth: ContextDepth,
        token_budget: u32,
    ) -> Result<ExpandedContext, LazyError> {
        // Build a minimal index just for the changed files and their dependencies
        let mut index = SymbolIndex::new();
        let mut graph = DepGraph::new();

        let changed_files: Vec<String> = changes
            .iter()
            .map(|c| Self::normalize_path(&c.file_path))
            .collect();

        // Process changed files first
        for change in changes {
            let normalized = Self::normalize_path(&change.file_path);
            self.process_file(&normalized, &mut index, &mut graph, 0)?;
        }

        // Follow imports to build the dependency graph
        // Use minimum of context depth and configured max_import_depth
        let context_depth = match depth {
            ContextDepth::L1 => 0,
            ContextDepth::L2 => 1,
            ContextDepth::L3 => 2,
        };
        let import_depth = context_depth.min(self.max_import_depth);

        if import_depth > 0 {
            let mut files_to_process: Vec<String> = changed_files.clone();
            let mut current_depth = 0;

            while current_depth < import_depth && !files_to_process.is_empty() {
                // Collect all imports first to avoid borrow conflict
                let mut imports_to_resolve: Vec<(String, String)> = Vec::new();
                for file_path in &files_to_process {
                    if let Some(file) = index.get_file(file_path) {
                        for import in &file.imports {
                            imports_to_resolve.push((file.path.clone(), import.source.clone()));
                        }
                    }
                }

                // Now resolve and process imports
                let mut next_files = Vec::new();
                for (importing_file, import_source) in &imports_to_resolve {
                    if let Some(resolved) = self.resolve_import(import_source, importing_file) {
                        if !self.processed_files.contains(&resolved) {
                            next_files.push(resolved.clone());
                            self.process_file(
                                &resolved,
                                &mut index,
                                &mut graph,
                                current_depth + 1,
                            )?;
                        }
                    }
                }

                files_to_process = next_files;
                current_depth += 1;
            }
        }

        // Rebuild lookups after all files are added
        index.rebuild_lookups();

        // Build edges for the graph
        self.build_import_edges(&index, &mut graph);
        self.add_symbol_reference_edges(&index, &mut graph);

        // Now use the regular context expander
        let expander = ContextExpander::new(&index, &graph);
        let context = expander.expand(changes, depth, token_budget);

        Ok(context)
    }

    /// Process a single file and add it to the index
    fn process_file(
        &mut self,
        relative_path: &str,
        index: &mut SymbolIndex,
        _graph: &mut DepGraph,
        _depth: usize,
    ) -> Result<(), LazyError> {
        let normalized_path = Self::normalize_path(relative_path);

        if self.processed_files.contains(&normalized_path) {
            return Ok(());
        }

        let file_path = self.repo_root.join(&normalized_path);
        if !file_path.exists() {
            // File might have been deleted, skip it
            return Ok(());
        }

        let content = match fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => return Ok(()), // Skip binary files
        };

        self.processed_files.insert(normalized_path.clone());

        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let language = Language::from_extension(ext);

        // Content hash (simple for now)
        let content_hash = blake3::hash(content.as_bytes());

        // Count lines
        let lines = content.lines().count() as u32;

        // Estimate tokens
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

        let imports = self.extract_imports(&content, language);

        // Track symbol range for this file
        let symbol_start = index.symbols.len() as u32;
        let file_id = index.files.len() as u32;

        if let Some(lang) = parser_lang {
            let mut parser = Parser::new();
            if let Ok(parsed_symbols) = parser.parse(&content, lang) {
                for sym in parsed_symbols {
                    let symbol_id = index.symbols.len() as u32;
                    index.symbols.push(IndexSymbol {
                        id: SymbolId::new(symbol_id),
                        name: sym.name,
                        kind: convert_symbol_kind(sym.kind),
                        file_id: FileId::new(file_id),
                        span: Span::new(sym.start_line, 0, sym.end_line, 0),
                        signature: sym.signature,
                        parent: None,
                        visibility: Visibility::Public,
                        docstring: sym.docstring,
                    });
                }
            }
        }

        index.files.push(FileEntry {
            id: FileId::new(file_id),
            path: normalized_path,
            language,
            content_hash: *content_hash.as_bytes(),
            symbols: symbol_start..index.symbols.len() as u32,
            imports,
            lines,
            tokens,
        });

        Ok(())
    }

    /// Extract imports from source code
    fn extract_imports(&self, content: &str, language: Language) -> Vec<Import> {
        let mut imports = Vec::new();

        if matches!(language, Language::JavaScript | Language::TypeScript) {
            use std::collections::HashSet;

            let mut seen_sources: HashSet<String> = HashSet::new();
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

        // Use pre-compiled regex patterns (no runtime compilation cost)
        let patterns: &[(&Regex, bool)] = match language {
            Language::Python => &[(&PYTHON_IMPORT, false), (&PYTHON_FROM_IMPORT, false)],
            Language::Rust => &[(&RUST_USE, false)],
            Language::Go => &[(&GO_IMPORT, true)],
            Language::Java => &[(&JAVA_IMPORT, false)],
            _ => &[],
        };

        for (line_num, line) in content.lines().enumerate() {
            for (re, check_external) in patterns {
                if let Some(captures) = re.captures(line) {
                    if let Some(source) = captures.get(1) {
                        let source_str = source.as_str().to_owned();
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

        imports
    }

    /// Resolve an import path to a file path
    fn resolve_import(&self, source: &str, importing_file: &str) -> Option<String> {
        if source.starts_with("./") || source.starts_with("../") {
            let base_dir = Path::new(importing_file)
                .parent()
                .unwrap_or_else(|| Path::new(""));
            let relative_source = source.strip_prefix("./").unwrap_or(source);
            let resolved = base_dir.join(relative_source);
            let resolved_str = resolved.to_string_lossy().replace('\\', "/");

            let relative_candidates = [
                resolved_str.clone(),
                format!("{}.rs", resolved_str),
                format!("{}.py", resolved_str),
                format!("{}.ts", resolved_str),
                format!("{}.js", resolved_str),
                format!("{}/mod.rs", resolved_str),
                format!("{}/__init__.py", resolved_str),
                format!("{}/index.ts", resolved_str),
                format!("{}/index.js", resolved_str),
            ];

            for candidate in relative_candidates {
                let normalized = Self::normalize_path(&candidate);
                let path = self.repo_root.join(&normalized);
                if path.exists() {
                    return Some(normalized);
                }
            }
        }

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
            let normalized = Self::normalize_path(&candidate);
            let path = self.repo_root.join(&normalized);
            if path.exists() {
                return Some(normalized);
            }
        }

        None
    }

    fn normalize_path(path: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        let normalized = path.replace('\\', "/");
        for part in normalized.split('/') {
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

    /// Build import edges after all files are processed
    fn build_import_edges(&self, index: &SymbolIndex, graph: &mut DepGraph) {
        for file in &index.files {
            for import in &file.imports {
                if let Some(resolved) = self.resolve_import(&import.source, &file.path) {
                    if let Some(&target_id) = index.file_by_path.get(&resolved) {
                        graph.add_file_import(file.id.as_u32(), target_id);
                    }
                }
            }
        }
    }

    fn add_symbol_reference_edges(&self, index: &SymbolIndex, graph: &mut DepGraph) {
        use std::collections::HashMap;

        let mut symbol_name_to_ids: HashMap<&str, Vec<u32>> = HashMap::new();
        for sym in &index.symbols {
            symbol_name_to_ids
                .entry(&sym.name)
                .or_default()
                .push(sym.id.as_u32());
        }

        let mut added: HashSet<(u32, u32)> = HashSet::new();

        for file in &index.files {
            let content = match fs::read_to_string(self.repo_root.join(&file.path)) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let imported_file_ids: HashSet<u32> =
                graph.get_imports(file.id.as_u32()).into_iter().collect();

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
                                candidate_ids.iter().find(|&&id| {
                                    imported_file_ids
                                        .contains(&index.symbols[id as usize].file_id.as_u32())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::ChangeType;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_lazy_context_simple() {
        let tmp = TempDir::new().unwrap();

        // Create test files
        fs::write(
            tmp.path().join("main.py"),
            r#"from utils import helper

def main():
    helper()
"#,
        )
        .unwrap();

        fs::write(
            tmp.path().join("utils.py"),
            r#"def helper():
    print("Hello")
"#,
        )
        .unwrap();

        let mut builder = LazyContextBuilder::new(tmp.path());

        let changes = vec![DiffChange {
            file_path: "main.py".to_owned(),
            old_path: None,
            line_ranges: vec![(1, 5)],
            change_type: ChangeType::Modified,
            diff_content: None,
        }];

        let context = builder
            .generate_context(&changes, ContextDepth::L2, 10000)
            .unwrap();

        // Should find main.py as changed
        assert_eq!(context.changed_files.len(), 1);
        assert_eq!(context.changed_files[0].path, "main.py");
    }

    #[test]
    fn test_lazy_context_relative_parent_import() {
        let tmp = TempDir::new().unwrap();
        let src_dir = tmp.path().join("src");
        let foo_dir = src_dir.join("foo");
        fs::create_dir_all(&foo_dir).unwrap();

        fs::write(
            src_dir.join("utils.js"),
            r#"export function helper() {
  return "ok";
}
"#,
        )
        .unwrap();

        fs::write(
            foo_dir.join("bar.js"),
            r#"import { helper } from "../utils";

export function run() {
  return helper();
}
"#,
        )
        .unwrap();

        let builder = LazyContextBuilder::new(tmp.path());
        let resolved = builder.resolve_import("../utils", "src/foo/bar.js");
        assert_eq!(resolved.as_deref(), Some("src/utils.js"));
    }
}
