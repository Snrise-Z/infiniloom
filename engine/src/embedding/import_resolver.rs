//! Import-aware call graph resolution for embedding chunks.
//!
//! This module builds per-file import scope maps and resolves raw call names
//! against imported symbols, enabling more accurate `called_by` relationships.
//! Supports Rust, TypeScript, and Python import patterns.
//!
//! # Design
//!
//! Instead of matching calls purely by unqualified name (which produces false
//! positives when multiple files define symbols with the same name), the resolver
//! uses each file's import statements to determine which definition a call refers to.
//!
//! The resolver produces two new fields on `ChunkContext`:
//! - `qualified_calls`: calls successfully resolved to a qualified name via imports
//! - `unresolved_calls`: calls that could not be matched to any import or local symbol

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::types::EmbedChunk;

/// Import resolver that builds per-file import maps from chunk metadata.
///
/// After construction from a set of chunks, it can resolve a raw call name
/// from a given file to a qualified name (module + symbol) or report it as
/// unresolved.
pub struct ImportResolver {
    /// Per-file import maps: file_path -> imported_name -> resolved target FQNs.
    file_imports: HashMap<String, HashMap<String, BTreeSet<String>>>,
    /// Per-file local symbols: file_path -> symbol_name -> resolved target FQNs.
    file_symbols: HashMap<String, HashMap<String, BTreeSet<String>>>,
}

impl ImportResolver {
    /// Build an `ImportResolver` from the generated chunks.
    ///
    /// For each chunk we:
    /// 1. Record concrete internal definitions by file and module path.
    /// 2. Parse import strings and resolve them to known internal FQNs.
    pub fn from_chunks(chunks: &[EmbedChunk]) -> Self {
        let mut file_imports: HashMap<String, HashMap<String, BTreeSet<String>>> = HashMap::new();
        let mut file_symbols: HashMap<String, HashMap<String, BTreeSet<String>>> = HashMap::new();
        let mut module_symbols: HashMap<(String, String), BTreeSet<String>> = HashMap::new();

        for chunk in chunks {
            if !is_resolvable_definition(chunk) {
                continue;
            }

            let file = &chunk.source.file;
            let symbol = &chunk.source.symbol;
            let target = chunk.source.fqn.as_deref().unwrap_or(symbol).to_owned();

            file_symbols
                .entry(file.clone())
                .or_default()
                .entry(symbol.clone())
                .or_default()
                .insert(target.clone());

            for module in module_variants_for_chunk(chunk) {
                module_symbols
                    .entry((module, symbol.clone()))
                    .or_default()
                    .insert(target.clone());
            }
        }

        for chunk in chunks {
            if chunk.kind == super::types::ChunkKind::Imports && chunk.part.is_some() {
                continue;
            }

            let file = &chunk.source.file;
            let caller_module = chunk.source.module_path.as_deref();

            for import_str in &chunk.context.imports {
                for (name, source) in parse_import(import_str) {
                    if let Some(target) =
                        resolve_import_target(file, caller_module, &source, &name, &module_symbols)
                    {
                        file_imports
                            .entry(file.clone())
                            .or_default()
                            .entry(name)
                            .or_default()
                            .insert(target);
                    }
                }
            }
        }

        Self { file_imports, file_symbols }
    }

    /// Resolve a call from a given file.
    ///
    /// Resolution order:
    /// 1. If `call_name` resolves unambiguously in the same file, return that FQN.
    /// 2. If `call_name` resolves unambiguously through imports, return that FQN.
    /// 3. Otherwise return `None` (unresolved)
    pub fn resolve_call(&self, file: &str, call_name: &str) -> Option<String> {
        if let Some(targets) = self
            .file_symbols
            .get(file)
            .and_then(|symbols| symbols.get(call_name))
        {
            if let Some(target) = unique_target(targets) {
                return Some(target);
            }
        }

        if let Some(targets) = self
            .file_imports
            .get(file)
            .and_then(|imports| imports.get(call_name))
        {
            if let Some(target) = unique_target(targets) {
                return Some(target);
            }
        }

        None
    }

    /// Resolve all calls for all chunks, populating `qualified_calls` and `unresolved_calls`.
    ///
    /// Also builds an improved reverse call map using qualified names for the
    /// `called_by` pass, reducing false-positive matches.
    pub fn resolve_all_calls(&self, chunks: &mut [EmbedChunk]) {
        for chunk in chunks.iter_mut() {
            let file = &chunk.source.file;
            let caller_fqn = chunk.source.fqn.as_deref();
            let mut qualified = BTreeSet::new();
            let mut unresolved = BTreeSet::new();

            for call_name in &chunk.context.calls {
                match self.resolve_call(file, call_name) {
                    Some(qname) => {
                        if caller_fqn != Some(qname.as_str()) {
                            qualified.insert(qname);
                        }
                    },
                    None => {
                        unresolved.insert(call_name.clone());
                    },
                }
            }

            chunk.context.qualified_calls = qualified.into_iter().collect();
            chunk.context.unresolved_calls = unresolved.into_iter().collect();
        }
    }

    /// Build a reverse call map using qualified names for more accurate `called_by`.
    ///
    /// Returns a map from qualified callee name -> set of caller identifiers (FQN or symbol name).
    /// This is used alongside the existing unqualified matching to improve accuracy.
    pub fn build_qualified_reverse_map(
        &self,
        chunks: &[EmbedChunk],
    ) -> BTreeMap<String, BTreeSet<String>> {
        let mut reverse: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for chunk in chunks {
            let caller_fqn = chunk
                .source
                .fqn
                .as_deref()
                .unwrap_or(&chunk.source.symbol)
                .to_owned();

            for qcall in &chunk.context.qualified_calls {
                if qcall == &caller_fqn {
                    continue;
                }
                reverse
                    .entry(qcall.clone())
                    .or_default()
                    .insert(caller_fqn.clone());
            }
        }

        reverse
    }
}

fn is_resolvable_definition(chunk: &EmbedChunk) -> bool {
    !chunk.source.symbol.is_empty()
        && chunk.source.symbol != "<top_level>"
        && !matches!(
            chunk.kind,
            super::types::ChunkKind::Imports | super::types::ChunkKind::TopLevel
        )
}

fn unique_target(targets: &BTreeSet<String>) -> Option<String> {
    if targets.len() == 1 {
        targets.iter().next().cloned()
    } else {
        None
    }
}

fn module_variants_for_chunk(chunk: &EmbedChunk) -> BTreeSet<String> {
    let mut variants = BTreeSet::new();

    if let Some(module_path) = chunk.source.module_path.as_deref() {
        add_module_variants(&mut variants, module_path);
    }

    let file_module = strip_known_extension(&chunk.source.file);
    add_module_variants(&mut variants, &file_module);

    if let Some(parent) = std::path::Path::new(&chunk.source.file).parent() {
        if let Some(parent) = parent.to_str() {
            if !parent.is_empty() {
                add_module_variants(&mut variants, parent);
            }
        }
    }

    variants
}

fn resolve_import_target(
    file: &str,
    caller_module: Option<&str>,
    source: &str,
    symbol: &str,
    module_symbols: &HashMap<(String, String), BTreeSet<String>>,
) -> Option<String> {
    let mut targets = BTreeSet::new();
    for module in import_module_variants(file, caller_module, source) {
        if let Some(found) = module_symbols.get(&(module, symbol.to_owned())) {
            targets.extend(found.iter().cloned());
        }
    }
    unique_target(&targets)
}

fn import_module_variants(
    file: &str,
    caller_module: Option<&str>,
    source: &str,
) -> BTreeSet<String> {
    let mut variants = BTreeSet::new();
    let source = source.trim().trim_matches('\'').trim_matches('"');

    add_module_variants(&mut variants, source);

    if source.starts_with('.') {
        add_python_relative_module_variants(&mut variants, caller_module, source);
        add_js_relative_module_variants(&mut variants, file, source);
    }

    if let Some(rest) = source
        .strip_prefix("crate::")
        .or_else(|| source.strip_prefix("self::"))
    {
        add_module_variants(&mut variants, rest);
    }

    if let Some(rest) = source.strip_prefix("super::") {
        if let Some(caller_module) = caller_module {
            let parent = drop_last_segment(caller_module, "::");
            if parent.is_empty() {
                add_module_variants(&mut variants, rest);
            } else {
                add_module_variants(&mut variants, &format!("{parent}::{rest}"));
            }
        }
    }

    variants
}

fn add_python_relative_module_variants(
    variants: &mut BTreeSet<String>,
    caller_module: Option<&str>,
    source: &str,
) {
    let Some(caller_module) = caller_module else {
        return;
    };
    let dot_count = source.chars().take_while(|ch| *ch == '.').count();
    if dot_count == 0 {
        return;
    }
    let suffix = source[dot_count..].replace('/', ".");
    let mut base = caller_module.to_owned();
    for _ in 0..dot_count {
        base = drop_last_segment(&base, ".");
    }
    let resolved = if suffix.is_empty() {
        base
    } else if base.is_empty() {
        suffix
    } else {
        format!("{base}.{suffix}")
    };
    add_module_variants(variants, &resolved);
}

fn add_js_relative_module_variants(variants: &mut BTreeSet<String>, file: &str, source: &str) {
    let dir = std::path::Path::new(file)
        .parent()
        .and_then(|path| path.to_str())
        .unwrap_or("");
    let joined = if dir.is_empty() {
        source.to_owned()
    } else {
        format!("{dir}/{source}")
    };
    add_module_variants(variants, &normalize_slash_path(&joined));
}

fn add_module_variants(variants: &mut BTreeSet<String>, module: &str) {
    let cleaned = clean_module_name(module);
    if cleaned.is_empty() {
        return;
    }

    let mut candidates = BTreeSet::new();
    candidates.insert(cleaned.clone());
    candidates.insert(cleaned.replace('\\', "/"));
    candidates.insert(cleaned.replace('/', "."));
    candidates.insert(cleaned.replace('/', "::"));
    candidates.insert(cleaned.replace('.', "::"));
    candidates.insert(cleaned.replace("::", "."));
    candidates.insert(cleaned.replace("::", "/"));

    for candidate in candidates.clone() {
        for prefix in ["src/", "lib/", "src.", "lib.", "src::", "lib::"] {
            if let Some(stripped) = candidate.strip_prefix(prefix) {
                candidates.insert(stripped.to_owned());
            }
        }
    }

    variants.extend(
        candidates
            .into_iter()
            .filter(|candidate| !candidate.is_empty()),
    );
}

fn clean_module_name(module: &str) -> String {
    let mut cleaned = module
        .trim()
        .trim_end_matches(';')
        .trim_matches('\'')
        .trim_matches('"')
        .replace('\\', "/");
    while let Some(rest) = cleaned.strip_prefix("./") {
        cleaned = rest.to_owned();
    }
    strip_known_extension(&cleaned)
}

fn strip_known_extension(path: &str) -> String {
    let mut path = path.to_owned();
    for suffix in [
        "/__init__.py",
        "/index.ts",
        "/index.tsx",
        "/index.js",
        "/index.jsx",
        ".py",
        ".rs",
        ".ts",
        ".tsx",
        ".js",
        ".jsx",
        ".go",
        ".java",
    ] {
        if let Some(stripped) = path.strip_suffix(suffix) {
            path = stripped.to_owned();
            break;
        }
    }
    path
}

fn normalize_slash_path(path: &str) -> String {
    let mut parts = Vec::new();
    let normalized = path.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {},
            ".." => {
                parts.pop();
            },
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

fn drop_last_segment(value: &str, separator: &str) -> String {
    value
        .rsplit_once(separator)
        .map(|(head, _)| head.to_owned())
        .unwrap_or_default()
}

/// Parse a single import string into (imported_name, source_module) pairs.
///
/// Supports three patterns:
/// - **Rust**: `use crate::auth::jwt::verify_token` -> `("verify_token", "crate::auth::jwt")`
/// - **TypeScript**: `import { verify } from './auth/jwt'` -> `("verify", "./auth/jwt")`
/// - **Python**: `from auth.jwt import verify` -> `("verify", "auth.jwt")`
///
/// Also handles multi-import forms:
/// - Rust: `use crate::auth::{Token, verify}` -> two entries
/// - Python: `from auth import verify, Token` -> two entries
/// - TypeScript: `import { verify, Token } from './auth'` -> two entries
fn parse_import(import_str: &str) -> Vec<(String, String)> {
    let trimmed = import_str.trim();

    // Try Rust pattern: `use path::to::module::Symbol` or `use path::{A, B}`
    if let Some(result) = parse_rust_import(trimmed) {
        return result;
    }

    // Try TypeScript/JavaScript pattern: `import { X, Y } from 'module'`
    if let Some(result) = parse_typescript_import(trimmed) {
        return result;
    }

    // Try Python pattern: `from module import X, Y`
    if let Some(result) = parse_python_import(trimmed) {
        return result;
    }

    Vec::new()
}

/// Parse a Rust `use` statement.
///
/// Handles:
/// - `use crate::auth::jwt::verify_token;`
/// - `use crate::auth::{Token, verify};`
/// - `use std::collections::HashMap;`
/// - `use super::types::EmbedChunk;`
fn parse_rust_import(s: &str) -> Option<Vec<(String, String)>> {
    let s = s.strip_prefix("use ")?.trim_end_matches(';').trim();

    // Check for brace group: `path::{A, B}`
    if let Some(brace_start) = s.find("::{") {
        let module_path = &s[..brace_start];
        let brace_content = s.get(brace_start + 3..)?.strip_suffix('}')?.trim();

        let results: Vec<(String, String)> = brace_content
            .split(',')
            .filter_map(|item| {
                let name = item.trim();
                if name.is_empty() {
                    return None;
                }
                // Handle `Name as Alias`
                let imported_name = if let Some(alias_pos) = name.find(" as ") {
                    name[alias_pos + 4..].trim()
                } else {
                    name
                };
                Some((imported_name.to_owned(), module_path.to_owned()))
            })
            .collect();

        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    } else {
        // Simple use: `use path::to::Symbol`
        // or `use path::to::Symbol as Alias`
        let (path, alias) = if let Some(as_pos) = s.find(" as ") {
            (&s[..as_pos], Some(s[as_pos + 4..].trim()))
        } else {
            (s, None)
        };

        if let Some(last_sep) = path.rfind("::") {
            let module = &path[..last_sep];
            let symbol = alias.unwrap_or(&path[last_sep + 2..]);
            Some(vec![(symbol.to_owned(), module.to_owned())])
        } else {
            // Top-level import like `use serde;`
            let symbol = alias.unwrap_or(path);
            Some(vec![(symbol.to_owned(), String::new())])
        }
    }
}

/// Parse a TypeScript/JavaScript import statement.
///
/// Handles:
/// - `import { verify, Token } from './auth/jwt'`
/// - `import { verify as check } from './auth'`
/// - `import verify from './auth/jwt'` (default import)
fn parse_typescript_import(s: &str) -> Option<Vec<(String, String)>> {
    let s = s.strip_prefix("import ")?;

    // Extract the `from 'module'` or `from "module"` part
    let from_pos = s.rfind(" from ")?;
    let names_part = s[..from_pos].trim();
    let module_part = s[from_pos + 6..].trim();

    // Strip quotes from module path
    let module = module_part
        .trim_matches('\'')
        .trim_matches('"')
        .trim_end_matches(';');

    // Check for named imports: { A, B }
    if let Some(brace_content) = names_part
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
    {
        let results: Vec<(String, String)> = brace_content
            .split(',')
            .filter_map(|item| {
                let item = item.trim();
                if item.is_empty() {
                    return None;
                }
                // Handle `name as alias`
                let imported_name = if let Some(as_pos) = item.find(" as ") {
                    item[as_pos + 4..].trim()
                } else {
                    item
                };
                Some((imported_name.to_owned(), module.to_owned()))
            })
            .collect();

        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    } else {
        // Default import: `import verify from './auth'`
        let name = names_part.trim();
        if name.is_empty() {
            None
        } else {
            Some(vec![(name.to_owned(), module.to_owned())])
        }
    }
}

/// Parse a Python import statement.
///
/// Handles:
/// - `from auth.jwt import verify`
/// - `from auth.jwt import verify, Token`
/// - `from auth.jwt import verify as check`
/// - `import auth.jwt` (maps `jwt` -> `auth`)
fn parse_python_import(s: &str) -> Option<Vec<(String, String)>> {
    // `from module import names`
    if let Some(rest) = s.strip_prefix("from ") {
        let import_pos = rest.find(" import ")?;
        let module = rest[..import_pos].trim();
        let names_part = rest[import_pos + 8..].trim();

        let results: Vec<(String, String)> = names_part
            .split(',')
            .filter_map(|item| {
                let item = item.trim();
                if item.is_empty() {
                    return None;
                }
                let imported_name = if let Some(as_pos) = item.find(" as ") {
                    item[as_pos + 4..].trim()
                } else {
                    item
                };
                Some((imported_name.to_owned(), module.to_owned()))
            })
            .collect();

        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    }
    // `import module.submodule`
    else if let Some(rest) = s.strip_prefix("import ") {
        let module_path = rest.trim().trim_end_matches(';');
        // For `import auth.jwt`, map `jwt` -> `auth`
        if let Some(last_dot) = module_path.rfind('.') {
            let parent = &module_path[..last_dot];
            let name = &module_path[last_dot + 1..];
            Some(vec![(name.to_owned(), parent.to_owned())])
        } else {
            // Top-level import like `import os`
            Some(vec![(module_path.to_owned(), String::new())])
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Rust import parsing ===

    #[test]
    fn test_parse_rust_simple_import() {
        let result = parse_import("use crate::auth::jwt::verify_token;");
        assert_eq!(result, vec![("verify_token".to_owned(), "crate::auth::jwt".to_owned())]);
    }

    #[test]
    fn test_parse_rust_brace_import() {
        let result = parse_import("use crate::auth::{Token, verify};");
        assert_eq!(result.len(), 2);
        assert!(result.contains(&("Token".to_owned(), "crate::auth".to_owned())));
        assert!(result.contains(&("verify".to_owned(), "crate::auth".to_owned())));
    }

    #[test]
    fn test_parse_rust_alias_import() {
        let result = parse_import("use std::collections::HashMap as Map;");
        assert_eq!(result, vec![("Map".to_owned(), "std::collections".to_owned())]);
    }

    #[test]
    fn test_parse_rust_super_import() {
        let result = parse_import("use super::types::EmbedChunk;");
        assert_eq!(result, vec![("EmbedChunk".to_owned(), "super::types".to_owned())]);
    }

    // === TypeScript import parsing ===

    #[test]
    fn test_parse_typescript_named_import() {
        let result = parse_import("import { verify } from './auth/jwt'");
        assert_eq!(result, vec![("verify".to_owned(), "./auth/jwt".to_owned())]);
    }

    #[test]
    fn test_parse_typescript_multi_import() {
        let result = parse_import("import { verify, Token } from './auth'");
        assert_eq!(result.len(), 2);
        assert!(result.contains(&("verify".to_owned(), "./auth".to_owned())));
        assert!(result.contains(&("Token".to_owned(), "./auth".to_owned())));
    }

    #[test]
    fn test_parse_typescript_alias_import() {
        let result = parse_import("import { verify as check } from './auth'");
        assert_eq!(result, vec![("check".to_owned(), "./auth".to_owned())]);
    }

    #[test]
    fn test_parse_typescript_default_import() {
        let result = parse_import("import Router from 'express'");
        assert_eq!(result, vec![("Router".to_owned(), "express".to_owned())]);
    }

    #[test]
    fn test_parse_typescript_double_quotes() {
        let result = parse_import("import { verify } from \"./auth/jwt\"");
        assert_eq!(result, vec![("verify".to_owned(), "./auth/jwt".to_owned())]);
    }

    // === Python import parsing ===

    #[test]
    fn test_parse_python_from_import() {
        let result = parse_import("from auth.jwt import verify");
        assert_eq!(result, vec![("verify".to_owned(), "auth.jwt".to_owned())]);
    }

    #[test]
    fn test_parse_python_multi_import() {
        let result = parse_import("from auth.jwt import verify, Token");
        assert_eq!(result.len(), 2);
        assert!(result.contains(&("verify".to_owned(), "auth.jwt".to_owned())));
        assert!(result.contains(&("Token".to_owned(), "auth.jwt".to_owned())));
    }

    #[test]
    fn test_parse_python_alias_import() {
        let result = parse_import("from auth.jwt import verify as check");
        assert_eq!(result, vec![("check".to_owned(), "auth.jwt".to_owned())]);
    }

    #[test]
    fn test_parse_python_plain_import() {
        let result = parse_import("import os.path");
        assert_eq!(result, vec![("path".to_owned(), "os".to_owned())]);
    }

    #[test]
    fn test_parse_python_toplevel_import() {
        let result = parse_import("import os");
        assert_eq!(result, vec![("os".to_owned(), String::new())]);
    }

    // === Same-file resolution ===

    #[test]
    fn test_resolve_same_file() {
        let chunks = vec![make_chunk("src/lib.rs", "foo", &[], &[])];
        let resolver = ImportResolver::from_chunks(&chunks);
        let resolved = resolver.resolve_call("src/lib.rs", "foo");
        assert_eq!(resolved, Some("src::lib::foo".to_owned()));
    }

    #[test]
    fn test_resolve_via_import() {
        let chunks = vec![
            make_chunk("src/main.rs", "main", &["use crate::auth::verify;"], &["verify"]),
            make_chunk("src/auth.rs", "verify", &[], &[]),
        ];
        let resolver = ImportResolver::from_chunks(&chunks);
        let resolved = resolver.resolve_call("src/main.rs", "verify");
        assert_eq!(resolved, Some("src::auth::verify".to_owned()));
    }

    #[test]
    fn test_resolve_unresolved() {
        let chunks = vec![make_chunk("src/main.rs", "main", &[], &["unknown_fn"])];
        let resolver = ImportResolver::from_chunks(&chunks);
        let resolved = resolver.resolve_call("src/main.rs", "unknown_fn");
        assert_eq!(resolved, None);
    }

    #[test]
    fn test_resolve_all_calls() {
        let mut chunks = vec![
            make_chunk(
                "src/main.rs",
                "main",
                &["use crate::auth::verify;"],
                &["verify", "unknown"],
            ),
            make_chunk("src/auth.rs", "verify", &[], &[]),
        ];

        let resolver = ImportResolver::from_chunks(&chunks);
        resolver.resolve_all_calls(&mut chunks);

        assert_eq!(chunks[0].context.qualified_calls, vec!["src::auth::verify".to_owned()]);
        assert_eq!(chunks[0].context.unresolved_calls, vec!["unknown".to_owned()]);
    }

    #[test]
    fn test_resolve_all_calls_skips_same_fqn_self_call() {
        let mut chunks = vec![make_chunk("src/service.py", "Service", &[], &["Service"])];

        let resolver = ImportResolver::from_chunks(&chunks);
        resolver.resolve_all_calls(&mut chunks);

        assert!(
            chunks[0].context.qualified_calls.is_empty(),
            "a chunk should not resolve calls to its own FQN: {:?}",
            chunks[0].context.qualified_calls
        );
    }

    #[test]
    fn test_resolve_import_does_not_fallback_to_same_name() {
        let chunks = vec![
            make_chunk("pkg/main.py", "caller", &["from pkg.a import target"], &["target"]),
            make_chunk("pkg/a.py", "target", &[], &[]),
            make_chunk("pkg/b.py", "target", &[], &[]),
        ];
        let resolver = ImportResolver::from_chunks(&chunks);
        let resolved = resolver.resolve_call("pkg/main.py", "target");

        assert_eq!(resolved, Some("pkg::a::target".to_owned()));
    }

    #[test]
    fn test_ambiguous_unqualified_call_is_unresolved() {
        let chunks = vec![
            make_chunk("pkg/main.py", "caller", &[], &["target"]),
            make_chunk("pkg/a.py", "target", &[], &[]),
            make_chunk("pkg/b.py", "target", &[], &[]),
        ];
        let resolver = ImportResolver::from_chunks(&chunks);
        let resolved = resolver.resolve_call("pkg/main.py", "target");

        assert_eq!(resolved, None);
    }

    #[test]
    fn test_split_import_fragments_do_not_build_import_scope() {
        use super::super::types::ChunkPart;

        let mut caller =
            make_chunk("pkg/main.py", "caller", &["from pkg.a import target"], &["target"]);
        caller.kind = super::super::types::ChunkKind::Imports;
        caller.part = Some(ChunkPart {
            part: 1,
            of: 2,
            parent_id: caller.id.clone(),
            parent_signature: String::new(),
            overlap_lines: 0,
        });

        let chunks = vec![caller, make_chunk("pkg/a.py", "target", &[], &[])];
        let resolver = ImportResolver::from_chunks(&chunks);
        let resolved = resolver.resolve_call("pkg/main.py", "target");

        assert_eq!(resolved, None);
    }

    #[test]
    fn test_unrecognized_import_format() {
        let result = parse_import("require('some-module')");
        assert!(result.is_empty());
    }

    /// Helper to create a minimal test chunk
    fn make_chunk(file: &str, symbol: &str, imports: &[&str], calls: &[&str]) -> EmbedChunk {
        use super::super::types::{
            ChunkContext, ChunkKind, ChunkSource, RepoIdentifier, Visibility,
        };

        let fqn = symbol_fqn(file, symbol);
        let module_path = module_path(file);

        EmbedChunk {
            id: format!("ec_{}", symbol),
            full_hash: String::new(),
            content: String::new(),
            tokens: 0,
            kind: ChunkKind::Function,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: file.to_owned(),
                lines: (1, 10),
                symbol: symbol.to_owned(),
                fqn: Some(fqn),
                language: "Rust".to_owned(),
                parent: None,
                visibility: Visibility::Public,
                is_test: false,
                module_path: Some(module_path),
                parent_chunk_id: None,
            },
            context: ChunkContext {
                imports: imports.iter().map(|s| s.to_string()).collect(),
                calls: calls.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
            children_ids: Vec::new(),
            dedup_alias_chunk_ids: Vec::new(),
            repr: "code".to_string(),
            code_chunk_id: None,
            part: None,
        }
    }

    fn symbol_fqn(file: &str, symbol: &str) -> String {
        format!("{}::{}", strip_known_extension(file).replace(['/', '.'], "::"), symbol)
    }

    fn module_path(file: &str) -> String {
        strip_known_extension(file).replace('/', ".")
    }
}
