//! Symbol extraction utilities for parsing
//!
//! This module contains standalone functions for extracting metadata from AST nodes:
//! - Signatures
//! - Docstrings
//! - Visibility modifiers
//! - Function calls
//! - Inheritance relationships

use super::language::Language;
use crate::types::{SymbolKind, Visibility};
use std::collections::HashSet;
use tree_sitter::Node;

/// Find a safe character boundary at or before the given byte index.
/// This prevents panics when slicing strings with multi-byte UTF-8 characters.
fn safe_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    // Walk backwards to find a valid char boundary
    while index > 0 && !s.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Extract function/method signature
pub fn extract_signature(node: Node<'_>, source_code: &str, language: Language) -> Option<String> {
    let sig_node = match language {
        Language::Python => {
            if node.kind() == "function_definition" {
                let start = node.start_byte();
                let mut end = start;
                for byte in &source_code.as_bytes()[start..] {
                    end += 1;
                    if *byte == b':' || *byte == b'\n' {
                        break;
                    }
                }
                return Some(source_code[start..end].trim().to_owned().replace('\n', " "));
            }
            None
        },
        Language::JavaScript | Language::TypeScript => {
            if node.kind().contains("function") || node.kind().contains("method") {
                let start = node.start_byte();
                let mut end = start;
                let mut brace_count = 0;
                for byte in &source_code.as_bytes()[start..] {
                    if *byte == b'{' {
                        brace_count += 1;
                        if brace_count == 1 {
                            break;
                        }
                    }
                    end += 1;
                }
                return Some(source_code[start..end].trim().to_owned().replace('\n', " "));
            }
            None
        },
        Language::Rust => {
            if node.kind() == "function_item" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        let start = node.start_byte();
                        let end = child.start_byte();
                        return Some(source_code[start..end].trim().to_owned().replace('\n', " "));
                    }
                }
            }
            None
        },
        Language::Go => {
            if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        let start = node.start_byte();
                        let end = child.start_byte();
                        return Some(source_code[start..end].trim().to_owned().replace('\n', " "));
                    }
                }
            }
            None
        },
        Language::Java => {
            if node.kind() == "method_declaration" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "block" {
                        let start = node.start_byte();
                        let end = child.start_byte();
                        return Some(source_code[start..end].trim().to_owned().replace('\n', " "));
                    }
                }
            }
            None
        },
        Language::C
        | Language::Cpp
        | Language::CSharp
        | Language::Php
        | Language::Kotlin
        | Language::Swift
        | Language::Scala => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "block"
                    || child.kind() == "compound_statement"
                    || child.kind() == "function_body"
                {
                    let start = node.start_byte();
                    let end = child.start_byte();
                    return Some(source_code[start..end].trim().to_owned().replace('\n', " "));
                }
            }
            None
        },
        Language::Ruby | Language::Lua => {
            let start = node.start_byte();
            let mut end = start;
            for byte in &source_code.as_bytes()[start..] {
                end += 1;
                if *byte == b'\n' {
                    break;
                }
            }
            Some(source_code[start..end].trim().to_owned())
        },
        Language::Bash => {
            let start = node.start_byte();
            let mut end = start;
            for byte in &source_code.as_bytes()[start..] {
                if *byte == b'{' {
                    break;
                }
                end += 1;
            }
            Some(source_code[start..end].trim().to_owned())
        },
        Language::Haskell
        | Language::OCaml
        | Language::FSharp
        | Language::Elixir
        | Language::Clojure
        | Language::R => {
            let start = node.start_byte();
            let mut end = start;
            for byte in &source_code.as_bytes()[start..] {
                end += 1;
                if *byte == b'\n' || *byte == b'=' {
                    break;
                }
            }
            Some(source_code[start..end].trim().to_owned())
        },
    };

    sig_node.or_else(|| {
        let start = node.start_byte();
        let end = std::cmp::min(start + 200, source_code.len());
        // Ensure we slice at valid UTF-8 character boundaries
        let safe_start = safe_char_boundary(source_code, start);
        let safe_end = safe_char_boundary(source_code, end);
        if safe_start >= safe_end {
            return None;
        }
        let text = &source_code[safe_start..safe_end];
        text.lines().next().map(|s| s.trim().to_owned())
    })
}

/// Extract docstring/documentation comment
pub fn extract_docstring(node: Node<'_>, source_code: &str, language: Language) -> Option<String> {
    match language {
        Language::Python => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "block" {
                    for stmt in child.children(&mut child.walk()) {
                        if stmt.kind() == "expression_statement" {
                            for expr in stmt.children(&mut stmt.walk()) {
                                if expr.kind() == "string" {
                                    if let Ok(text) = expr.utf8_text(source_code.as_bytes()) {
                                        return Some(
                                            text.trim_matches(|c| c == '"' || c == '\'')
                                                .trim()
                                                .to_owned(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None
        },
        Language::JavaScript | Language::TypeScript => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        if text.starts_with("/**") {
                            return Some(clean_jsdoc(text));
                        }
                    }
                }
            }
            None
        },
        Language::Rust => {
            let start_byte = node.start_byte();
            // SAFETY: Use floor_char_boundary to avoid panics on multi-byte UTF-8 characters
            let safe_boundary = source_code.floor_char_boundary(start_byte);
            let lines_before: Vec<_> = source_code[..safe_boundary]
                .lines()
                .rev()
                .take_while(|line| line.trim().starts_with("///") || line.trim().is_empty())
                .collect();

            if !lines_before.is_empty() {
                let doc: Vec<String> = lines_before
                    .into_iter()
                    .rev()
                    .filter_map(|line| {
                        let trimmed = line.trim();
                        trimmed.strip_prefix("///").map(|s| s.trim().to_owned())
                    })
                    .collect();

                if !doc.is_empty() {
                    return Some(doc.join(" "));
                }
            }
            None
        },
        Language::Go => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        return Some(text.trim_start_matches("//").trim().to_owned());
                    }
                }
            }
            None
        },
        Language::Java => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "block_comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        if text.starts_with("/**") {
                            return Some(clean_javadoc(text));
                        }
                    }
                }
            }
            None
        },
        Language::C | Language::Cpp => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        if text.starts_with("/**") || text.starts_with("/*") {
                            return Some(clean_jsdoc(text));
                        }
                        return Some(text.trim_start_matches("//").trim().to_owned());
                    }
                }
            }
            None
        },
        Language::CSharp => {
            let start_byte = node.start_byte();
            // SAFETY: Use floor_char_boundary to avoid panics on multi-byte UTF-8 characters
            let safe_boundary = source_code.floor_char_boundary(start_byte);
            let lines_before: Vec<_> = source_code[..safe_boundary]
                .lines()
                .rev()
                .take_while(|line| line.trim().starts_with("///") || line.trim().is_empty())
                .collect();

            if !lines_before.is_empty() {
                let doc: Vec<String> = lines_before
                    .into_iter()
                    .rev()
                    .filter_map(|line| {
                        let trimmed = line.trim();
                        trimmed.strip_prefix("///").map(|s| s.trim().to_owned())
                    })
                    .collect();

                if !doc.is_empty() {
                    return Some(doc.join(" "));
                }
            }
            None
        },
        Language::Ruby => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        return Some(text.trim_start_matches('#').trim().to_owned());
                    }
                }
            }
            None
        },
        Language::Php | Language::Kotlin | Language::Swift | Language::Scala => {
            if let Some(prev_sibling) = node.prev_sibling() {
                let kind = prev_sibling.kind();
                if kind == "comment" || kind == "multiline_comment" || kind == "block_comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        if text.starts_with("/**") {
                            return Some(clean_jsdoc(text));
                        }
                    }
                }
            }
            None
        },
        Language::Bash => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        return Some(text.trim_start_matches('#').trim().to_owned());
                    }
                }
            }
            None
        },
        Language::Haskell => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        let cleaned = text
                            .trim_start_matches("{-")
                            .trim_end_matches("-}")
                            .trim_start_matches("--")
                            .trim();
                        return Some(cleaned.to_owned());
                    }
                }
            }
            None
        },
        Language::Elixir => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        return Some(text.trim_start_matches('#').trim().to_owned());
                    }
                }
            }
            None
        },
        Language::Clojure => None,
        Language::OCaml | Language::FSharp => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        let cleaned = text
                            .trim_start_matches("(**")
                            .trim_start_matches("(*")
                            .trim_end_matches("*)")
                            .trim();
                        return Some(cleaned.to_owned());
                    }
                }
            }
            None
        },
        Language::Lua => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        let cleaned = text
                            .trim_start_matches("--[[")
                            .trim_end_matches("]]")
                            .trim_start_matches("--")
                            .trim();
                        return Some(cleaned.to_owned());
                    }
                }
            }
            None
        },
        Language::R => {
            if let Some(prev_sibling) = node.prev_sibling() {
                if prev_sibling.kind() == "comment" {
                    if let Ok(text) = prev_sibling.utf8_text(source_code.as_bytes()) {
                        return Some(text.trim_start_matches('#').trim().to_owned());
                    }
                }
            }
            None
        },
    }
}

/// Extract parent class/struct name for methods
pub fn extract_parent(node: Node<'_>, source_code: &str) -> Option<String> {
    let mut current = node.parent()?;

    while let Some(parent) = current.parent() {
        if ["class_definition", "class_declaration", "struct_item", "impl_item"]
            .contains(&parent.kind())
        {
            for child in parent.children(&mut parent.walk()) {
                if child.kind() == "identifier" || child.kind() == "type_identifier" {
                    if let Ok(name) = child.utf8_text(source_code.as_bytes()) {
                        return Some(name.to_owned());
                    }
                }
            }
        }
        current = parent;
    }

    None
}

/// Extract visibility modifier from a node
pub fn extract_visibility(node: Node<'_>, source_code: &str, language: Language) -> Visibility {
    match language {
        Language::Python => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_code.as_bytes()) {
                    if name.starts_with("__") && !name.ends_with("__") {
                        return Visibility::Private;
                    } else if name.starts_with('_') {
                        return Visibility::Protected;
                    }
                }
            }
            Visibility::Public
        },
        Language::Rust => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "visibility_modifier" {
                    if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                        if text.contains("pub(crate)") || text.contains("pub(super)") {
                            return Visibility::Internal;
                        } else if text.starts_with("pub") {
                            return Visibility::Public;
                        }
                    }
                }
            }
            Visibility::Private
        },
        Language::JavaScript | Language::TypeScript => {
            for child in node.children(&mut node.walk()) {
                let kind = child.kind();
                if kind == "private" || kind == "accessibility_modifier" {
                    if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                        return match text {
                            "private" => Visibility::Private,
                            "protected" => Visibility::Protected,
                            _ => Visibility::Public,
                        };
                    }
                }
            }
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_code.as_bytes()) {
                    if name.starts_with('#') {
                        return Visibility::Private;
                    }
                }
            }
            Visibility::Public
        },
        Language::Go => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_code.as_bytes()) {
                    if let Some(first_char) = name.chars().next() {
                        if first_char.is_lowercase() {
                            return Visibility::Private;
                        }
                    }
                }
            }
            Visibility::Public
        },
        Language::Java => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "modifiers" {
                    if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                        if text.contains("private") {
                            return Visibility::Private;
                        } else if text.contains("protected") {
                            return Visibility::Protected;
                        } else if text.contains("public") {
                            return Visibility::Public;
                        }
                    }
                }
            }
            Visibility::Internal
        },
        Language::C | Language::Cpp => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "storage_class_specifier" {
                    if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                        if text == "static" {
                            return Visibility::Private;
                        }
                    }
                }
            }
            Visibility::Public
        },
        Language::CSharp | Language::Kotlin | Language::Swift | Language::Scala => {
            for child in node.children(&mut node.walk()) {
                let kind = child.kind();
                if kind == "modifier" || kind == "modifiers" || kind == "visibility_modifier" {
                    if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                        if text.contains("private") {
                            return Visibility::Private;
                        } else if text.contains("protected") {
                            return Visibility::Protected;
                        } else if text.contains("internal") {
                            return Visibility::Internal;
                        } else if text.contains("public") {
                            return Visibility::Public;
                        }
                    }
                }
            }
            Visibility::Internal
        },
        Language::Ruby => {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source_code.as_bytes()) {
                    if name.starts_with('_') {
                        return Visibility::Private;
                    }
                }
            }
            Visibility::Public
        },
        Language::Php => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "visibility_modifier" {
                    if let Ok(text) = child.utf8_text(source_code.as_bytes()) {
                        return match text {
                            "private" => Visibility::Private,
                            "protected" => Visibility::Protected,
                            "public" => Visibility::Public,
                            _ => Visibility::Public,
                        };
                    }
                }
            }
            Visibility::Public
        },
        Language::Bash => Visibility::Public,
        Language::Haskell
        | Language::Elixir
        | Language::Clojure
        | Language::OCaml
        | Language::FSharp
        | Language::Lua
        | Language::R => Visibility::Public,
    }
}

/// Extract function calls from a function/method body
pub fn extract_calls(node: Node<'_>, source_code: &str, language: Language) -> Vec<String> {
    let mut calls = HashSet::new();

    let body_node = find_body_node(node, language);
    if let Some(body) = body_node {
        collect_calls_recursive(body, source_code, language, &mut calls);
    }

    if calls.is_empty() {
        collect_calls_recursive(node, source_code, language, &mut calls);
    }

    calls.into_iter().collect()
}

/// Find the body node of a function/method
pub fn find_body_node(node: Node<'_>, language: Language) -> Option<Node<'_>> {
    match language {
        Language::Python => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "block" {
                    return Some(child);
                }
            }
        },
        Language::Rust => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "block" {
                    return Some(child);
                }
            }
        },
        Language::JavaScript | Language::TypeScript => {
            for child in node.children(&mut node.walk()) {
                let kind = child.kind();
                if kind == "statement_block" {
                    return Some(child);
                }
                if kind == "arrow_function" {
                    if let Some(body) = find_body_node(child, language) {
                        return Some(body);
                    }
                    return Some(child);
                }
            }
            if node.kind() == "arrow_function" {
                for child in node.children(&mut node.walk()) {
                    let kind = child.kind();
                    if kind != "formal_parameters"
                        && kind != "identifier"
                        && kind != "=>"
                        && kind != "("
                        && kind != ")"
                        && kind != ","
                    {
                        return Some(child);
                    }
                }
                return Some(node);
            }
        },
        Language::Go => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "block" {
                    return Some(child);
                }
            }
        },
        Language::Java => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "block" {
                    return Some(child);
                }
            }
        },
        Language::C | Language::Cpp => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "compound_statement" {
                    return Some(child);
                }
            }
        },
        Language::CSharp | Language::Php | Language::Kotlin | Language::Swift | Language::Scala => {
            for child in node.children(&mut node.walk()) {
                let kind = child.kind();
                if kind == "block" || kind == "compound_statement" || kind == "function_body" {
                    return Some(child);
                }
            }
        },
        Language::Ruby => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "body_statement" || child.kind() == "do_block" {
                    return Some(child);
                }
            }
        },
        Language::Bash => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "compound_statement" {
                    return Some(child);
                }
            }
        },
        Language::Haskell
        | Language::Elixir
        | Language::Clojure
        | Language::OCaml
        | Language::FSharp
        | Language::R => {
            return Some(node);
        },
        Language::Lua => {
            for child in node.children(&mut node.walk()) {
                if child.kind() == "block" {
                    return Some(child);
                }
            }
        },
    }
    None
}

/// Recursively collect function calls from a node
pub fn collect_calls_recursive(
    node: Node<'_>,
    source_code: &str,
    language: Language,
    calls: &mut HashSet<String>,
) {
    let kind = node.kind();

    let call_name = match language {
        Language::Python => {
            if kind == "call" {
                node.child_by_field_name("function").and_then(|f| {
                    if f.kind() == "identifier" {
                        f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                    } else if f.kind() == "attribute" {
                        f.child_by_field_name("attribute")
                            .and_then(|a| a.utf8_text(source_code.as_bytes()).ok())
                            .map(String::from)
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        },
        Language::Rust => {
            if kind == "call_expression" {
                node.child_by_field_name("function").and_then(|f| {
                    if f.kind() == "identifier" {
                        f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                    } else if f.kind() == "field_expression" {
                        f.child_by_field_name("field")
                            .and_then(|a| a.utf8_text(source_code.as_bytes()).ok())
                            .map(String::from)
                    } else if f.kind() == "scoped_identifier" {
                        f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                    } else {
                        None
                    }
                })
            } else if kind == "macro_invocation" {
                node.child_by_field_name("macro")
                    .and_then(|m| m.utf8_text(source_code.as_bytes()).ok())
                    .map(|s| format!("{}!", s))
            } else {
                None
            }
        },
        Language::JavaScript | Language::TypeScript => {
            if kind == "call_expression" {
                node.child_by_field_name("function").and_then(|f| {
                    if f.kind() == "identifier" {
                        f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                    } else if f.kind() == "member_expression" {
                        f.child_by_field_name("property")
                            .and_then(|p| p.utf8_text(source_code.as_bytes()).ok())
                            .map(String::from)
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        },
        Language::Go => {
            if kind == "call_expression" {
                node.child_by_field_name("function").and_then(|f| {
                    if f.kind() == "identifier" {
                        f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                    } else if f.kind() == "selector_expression" {
                        f.child_by_field_name("field")
                            .and_then(|a| a.utf8_text(source_code.as_bytes()).ok())
                            .map(String::from)
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        },
        Language::Java => {
            if kind == "method_invocation" {
                node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                    .map(String::from)
            } else {
                None
            }
        },
        Language::C | Language::Cpp => {
            if kind == "call_expression" {
                node.child_by_field_name("function").and_then(|f| {
                    if f.kind() == "identifier" {
                        f.utf8_text(source_code.as_bytes()).ok().map(String::from)
                    } else if f.kind() == "field_expression" {
                        f.child_by_field_name("field")
                            .and_then(|a| a.utf8_text(source_code.as_bytes()).ok())
                            .map(String::from)
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        },
        Language::CSharp | Language::Php | Language::Kotlin | Language::Swift | Language::Scala => {
            if kind == "invocation_expression" || kind == "call_expression" {
                node.children(&mut node.walk())
                    .find(|child| child.kind() == "identifier" || child.kind() == "simple_name")
                    .and_then(|child| child.utf8_text(source_code.as_bytes()).ok())
                    .map(|s| s.to_owned())
            } else {
                None
            }
        },
        Language::Ruby => {
            if kind == "call" || kind == "method_call" {
                node.child_by_field_name("method")
                    .and_then(|m| m.utf8_text(source_code.as_bytes()).ok())
                    .map(String::from)
            } else {
                None
            }
        },
        Language::Bash => {
            if kind == "command" {
                node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source_code.as_bytes()).ok())
                    .map(String::from)
            } else {
                None
            }
        },
        Language::Haskell
        | Language::Elixir
        | Language::Clojure
        | Language::OCaml
        | Language::FSharp
        | Language::Lua
        | Language::R => {
            if kind == "function_call" || kind == "call" || kind == "application" {
                node.children(&mut node.walk())
                    .find(|child| child.kind() == "identifier" || child.kind() == "variable")
                    .and_then(|child| child.utf8_text(source_code.as_bytes()).ok())
                    .map(|s| s.to_owned())
            } else {
                None
            }
        },
    };

    if let Some(name) = call_name {
        if !is_builtin(&name, language) {
            calls.insert(name);
        }
    }

    for child in node.children(&mut node.walk()) {
        collect_calls_recursive(child, source_code, language, calls);
    }
}

/// Check if a function name is a common built-in
pub fn is_builtin(name: &str, language: Language) -> bool {
    match language {
        Language::Python => {
            matches!(
                name,
                "print"
                    | "len"
                    | "range"
                    | "str"
                    | "int"
                    | "float"
                    | "list"
                    | "dict"
                    | "set"
                    | "tuple"
                    | "bool"
                    | "type"
                    | "isinstance"
                    | "hasattr"
                    | "getattr"
                    | "setattr"
                    | "super"
                    | "iter"
                    | "next"
                    | "open"
                    | "input"
                    | "format"
                    | "enumerate"
                    | "zip"
                    | "map"
                    | "filter"
                    | "sorted"
                    | "reversed"
                    | "sum"
                    | "min"
                    | "max"
                    | "abs"
                    | "round"
                    | "ord"
                    | "chr"
                    | "hex"
                    | "bin"
                    | "oct"
            )
        },
        Language::JavaScript | Language::TypeScript => {
            matches!(
                name,
                "console"
                    | "log"
                    | "error"
                    | "warn"
                    | "parseInt"
                    | "parseFloat"
                    | "setTimeout"
                    | "setInterval"
                    | "clearTimeout"
                    | "clearInterval"
                    | "JSON"
                    | "stringify"
                    | "parse"
                    | "toString"
                    | "valueOf"
                    | "push"
                    | "pop"
                    | "shift"
                    | "unshift"
                    | "slice"
                    | "splice"
                    | "map"
                    | "filter"
                    | "reduce"
                    | "forEach"
                    | "find"
                    | "findIndex"
                    | "includes"
                    | "indexOf"
                    | "join"
                    | "split"
                    | "replace"
            )
        },
        Language::Rust => {
            matches!(
                name,
                "println!"
                    | "print!"
                    | "eprintln!"
                    | "eprint!"
                    | "format!"
                    | "vec!"
                    | "panic!"
                    | "assert!"
                    | "assert_eq!"
                    | "assert_ne!"
                    | "debug!"
                    | "info!"
                    | "warn!"
                    | "error!"
                    | "trace!"
                    | "unwrap"
                    | "expect"
                    | "ok"
                    | "err"
                    | "some"
                    | "none"
                    | "clone"
                    | "to_string"
                    | "into"
                    | "from"
                    | "default"
                    | "iter"
                    | "into_iter"
                    | "collect"
                    | "map"
                    | "filter"
            )
        },
        Language::Go => {
            matches!(
                name,
                "fmt"
                    | "Println"
                    | "Printf"
                    | "Sprintf"
                    | "Errorf"
                    | "make"
                    | "new"
                    | "len"
                    | "cap"
                    | "append"
                    | "copy"
                    | "delete"
                    | "close"
                    | "panic"
                    | "recover"
                    | "print"
            )
        },
        Language::Java => {
            matches!(
                name,
                "println"
                    | "print"
                    | "printf"
                    | "toString"
                    | "equals"
                    | "hashCode"
                    | "getClass"
                    | "clone"
                    | "notify"
                    | "wait"
                    | "get"
                    | "set"
                    | "add"
                    | "remove"
                    | "size"
                    | "isEmpty"
                    | "contains"
                    | "iterator"
                    | "valueOf"
                    | "parseInt"
            )
        },
        Language::C | Language::Cpp => {
            matches!(
                name,
                "printf"
                    | "scanf"
                    | "malloc"
                    | "free"
                    | "memcpy"
                    | "memset"
                    | "strlen"
                    | "strcpy"
                    | "strcmp"
                    | "strcat"
                    | "sizeof"
                    | "cout"
                    | "cin"
                    | "endl"
                    | "cerr"
                    | "clog"
            )
        },
        Language::CSharp => {
            matches!(
                name,
                "WriteLine"
                    | "Write"
                    | "ReadLine"
                    | "ToString"
                    | "Equals"
                    | "GetHashCode"
                    | "GetType"
                    | "Add"
                    | "Remove"
                    | "Contains"
                    | "Count"
                    | "Clear"
                    | "ToList"
                    | "ToArray"
            )
        },
        Language::Ruby => {
            matches!(
                name,
                "puts"
                    | "print"
                    | "p"
                    | "gets"
                    | "each"
                    | "map"
                    | "select"
                    | "reject"
                    | "reduce"
                    | "inject"
                    | "find"
                    | "any?"
                    | "all?"
                    | "include?"
                    | "empty?"
                    | "nil?"
                    | "length"
                    | "size"
            )
        },
        Language::Php => {
            matches!(
                name,
                "echo"
                    | "print"
                    | "var_dump"
                    | "print_r"
                    | "isset"
                    | "empty"
                    | "array"
                    | "count"
                    | "strlen"
                    | "strpos"
                    | "substr"
                    | "explode"
                    | "implode"
                    | "json_encode"
                    | "json_decode"
            )
        },
        Language::Kotlin => {
            matches!(
                name,
                "println"
                    | "print"
                    | "readLine"
                    | "toString"
                    | "equals"
                    | "hashCode"
                    | "map"
                    | "filter"
                    | "forEach"
                    | "let"
                    | "also"
                    | "apply"
                    | "run"
                    | "with"
                    | "listOf"
                    | "mapOf"
                    | "setOf"
            )
        },
        Language::Swift => {
            matches!(
                name,
                "print"
                    | "debugPrint"
                    | "dump"
                    | "map"
                    | "filter"
                    | "reduce"
                    | "forEach"
                    | "contains"
                    | "count"
                    | "isEmpty"
                    | "append"
            )
        },
        Language::Scala => {
            matches!(
                name,
                "println"
                    | "print"
                    | "map"
                    | "filter"
                    | "flatMap"
                    | "foreach"
                    | "reduce"
                    | "fold"
                    | "foldLeft"
                    | "foldRight"
                    | "collect"
            )
        },
        Language::Bash
        | Language::Haskell
        | Language::Elixir
        | Language::Clojure
        | Language::OCaml
        | Language::FSharp
        | Language::Lua
        | Language::R => false,
    }
}

/// Clean JSDoc comment
pub fn clean_jsdoc(text: &str) -> String {
    text.lines()
        .map(|line| {
            line.trim()
                .trim_start_matches("/**")
                .trim_start_matches("/*")
                .trim_start_matches('*')
                .trim_end_matches("*/")
                .trim()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Clean JavaDoc comment
pub fn clean_javadoc(text: &str) -> String {
    clean_jsdoc(text)
}

/// Extract class inheritance (extends) and interface implementations (implements)
pub fn extract_inheritance(
    node: Node<'_>,
    source_code: &str,
    language: Language,
) -> (Option<String>, Vec<String>) {
    let mut extends = None;
    let mut implements = Vec::new();

    match language {
        Language::Python => {
            // Python: class Foo(Bar, Baz): - all are considered base classes
            if node.kind() == "class_definition" {
                if let Some(args) = node.child_by_field_name("superclasses") {
                    for child in args.children(&mut args.walk()) {
                        if child.kind() == "identifier" || child.kind() == "attribute" {
                            if let Ok(name) = child.utf8_text(source_code.as_bytes()) {
                                if extends.is_none() {
                                    extends = Some(name.to_owned());
                                } else {
                                    implements.push(name.to_owned());
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::JavaScript | Language::TypeScript => {
            // JS/TS: class Foo extends Bar implements Baz
            if node.kind() == "class_declaration" || node.kind() == "class" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "class_heritage" {
                        for heritage in child.children(&mut child.walk()) {
                            if heritage.kind() == "extends_clause" {
                                for type_node in heritage.children(&mut heritage.walk()) {
                                    if type_node.kind() == "identifier"
                                        || type_node.kind() == "type_identifier"
                                    {
                                        if let Ok(name) =
                                            type_node.utf8_text(source_code.as_bytes())
                                        {
                                            extends = Some(name.to_owned());
                                        }
                                    }
                                }
                            } else if heritage.kind() == "implements_clause" {
                                for type_node in heritage.children(&mut heritage.walk()) {
                                    if type_node.kind() == "identifier"
                                        || type_node.kind() == "type_identifier"
                                    {
                                        if let Ok(name) =
                                            type_node.utf8_text(source_code.as_bytes())
                                        {
                                            implements.push(name.to_owned());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::Rust => {
            // Rust doesn't have class inheritance, but has trait implementations
            // impl Trait for Struct
            if node.kind() == "impl_item" {
                let mut has_for = false;
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "for" {
                        has_for = true;
                    }
                    if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                        if let Ok(name) = child.utf8_text(source_code.as_bytes()) {
                            if has_for {
                                // This is the struct being implemented
                            } else {
                                // This is the trait being implemented
                                implements.push(name.to_owned());
                            }
                        }
                    }
                }
            }
        },
        Language::Go => {
            // Go uses embedding for "inheritance"
            if node.kind() == "type_declaration" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "type_spec" {
                        for spec_child in child.children(&mut child.walk()) {
                            if spec_child.kind() == "struct_type" {
                                for field in spec_child.children(&mut spec_child.walk()) {
                                    if field.kind() == "field_declaration" {
                                        // Embedded field (no name, just type)
                                        let has_name = field.child_by_field_name("name").is_some();
                                        if !has_name {
                                            if let Some(type_node) =
                                                field.child_by_field_name("type")
                                            {
                                                if let Ok(name) =
                                                    type_node.utf8_text(source_code.as_bytes())
                                                {
                                                    implements.push(name.to_owned());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::Java => {
            // Java: class Foo extends Bar implements Baz, Qux
            if node.kind() == "class_declaration" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "superclass" {
                        for type_node in child.children(&mut child.walk()) {
                            if type_node.kind() == "type_identifier" {
                                if let Ok(name) = type_node.utf8_text(source_code.as_bytes()) {
                                    extends = Some(name.to_owned());
                                }
                            }
                        }
                    } else if child.kind() == "super_interfaces" {
                        for type_list in child.children(&mut child.walk()) {
                            if type_list.kind() == "type_list" {
                                for type_node in type_list.children(&mut type_list.walk()) {
                                    if type_node.kind() == "type_identifier" {
                                        if let Ok(name) =
                                            type_node.utf8_text(source_code.as_bytes())
                                        {
                                            implements.push(name.to_owned());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::C | Language::Cpp => {
            // C++: class Foo : public Bar, public Baz
            if node.kind() == "class_specifier" || node.kind() == "struct_specifier" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "base_class_clause" {
                        for base in child.children(&mut child.walk()) {
                            if base.kind() == "type_identifier" {
                                if let Ok(name) = base.utf8_text(source_code.as_bytes()) {
                                    if extends.is_none() {
                                        extends = Some(name.to_owned());
                                    } else {
                                        implements.push(name.to_owned());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::CSharp => {
            // C#: class Foo : Bar, IBaz
            if node.kind() == "class_declaration" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "base_list" {
                        for base in child.children(&mut child.walk()) {
                            if base.kind() == "identifier" || base.kind() == "generic_name" {
                                if let Ok(name) = base.utf8_text(source_code.as_bytes()) {
                                    if name.starts_with('I') && name.len() > 1 {
                                        // Convention: interfaces start with I
                                        implements.push(name.to_owned());
                                    } else if extends.is_none() {
                                        extends = Some(name.to_owned());
                                    } else {
                                        implements.push(name.to_owned());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::Ruby => {
            // Ruby: class Foo < Bar; include Baz
            if node.kind() == "class" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "superclass" {
                        for type_node in child.children(&mut child.walk()) {
                            if type_node.kind() == "constant" {
                                if let Ok(name) = type_node.utf8_text(source_code.as_bytes()) {
                                    extends = Some(name.to_owned());
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::Php => {
            // PHP: class Foo extends Bar implements Baz
            if node.kind() == "class_declaration" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "base_clause" {
                        for type_node in child.children(&mut child.walk()) {
                            if type_node.kind() == "name" {
                                if let Ok(name) = type_node.utf8_text(source_code.as_bytes()) {
                                    extends = Some(name.to_owned());
                                }
                            }
                        }
                    } else if child.kind() == "class_interface_clause" {
                        for type_node in child.children(&mut child.walk()) {
                            if type_node.kind() == "name" {
                                if let Ok(name) = type_node.utf8_text(source_code.as_bytes()) {
                                    implements.push(name.to_owned());
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::Kotlin => {
            // Kotlin: class Foo : Bar(), Baz
            if node.kind() == "class_declaration" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "delegation_specifiers" {
                        for spec in child.children(&mut child.walk()) {
                            if spec.kind() == "delegation_specifier" {
                                for type_node in spec.children(&mut spec.walk()) {
                                    if type_node.kind() == "user_type" {
                                        if let Ok(name) =
                                            type_node.utf8_text(source_code.as_bytes())
                                        {
                                            if extends.is_none() {
                                                extends = Some(name.to_owned());
                                            } else {
                                                implements.push(name.to_owned());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::Swift => {
            // Swift: class Foo: Bar, Protocol
            if node.kind() == "class_declaration" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "type_inheritance_clause" {
                        for type_node in child.children(&mut child.walk()) {
                            if type_node.kind() == "type_identifier" {
                                if let Ok(name) = type_node.utf8_text(source_code.as_bytes()) {
                                    if extends.is_none() {
                                        extends = Some(name.to_owned());
                                    } else {
                                        implements.push(name.to_owned());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::Scala => {
            // Scala: class Foo extends Bar with Baz
            if node.kind() == "class_definition" {
                for child in node.children(&mut node.walk()) {
                    if child.kind() == "extends_clause" {
                        for type_node in child.children(&mut child.walk()) {
                            if type_node.kind() == "type_identifier" {
                                if let Ok(name) = type_node.utf8_text(source_code.as_bytes()) {
                                    if extends.is_none() {
                                        extends = Some(name.to_owned());
                                    } else {
                                        implements.push(name.to_owned());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Language::Bash
        | Language::Haskell
        | Language::Elixir
        | Language::Clojure
        | Language::OCaml
        | Language::FSharp
        | Language::Lua
        | Language::R => {},
    }

    (extends, implements)
}

/// Map capture name to SymbolKind
pub fn map_symbol_kind(capture_name: &str) -> SymbolKind {
    match capture_name {
        "function" => SymbolKind::Function,
        "class" => SymbolKind::Class,
        "method" => SymbolKind::Method,
        "struct" => SymbolKind::Struct,
        "enum" => SymbolKind::Enum,
        "interface" => SymbolKind::Interface,
        "trait" => SymbolKind::Trait,
        _ => SymbolKind::Function,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // safe_char_boundary tests
    // ==========================================================================

    #[test]
    fn test_safe_char_boundary_ascii() {
        let s = "hello world";
        assert_eq!(safe_char_boundary(s, 0), 0);
        assert_eq!(safe_char_boundary(s, 5), 5);
        assert_eq!(safe_char_boundary(s, 11), 11);
    }

    #[test]
    fn test_safe_char_boundary_beyond_length() {
        let s = "hello";
        assert_eq!(safe_char_boundary(s, 100), 5);
        assert_eq!(safe_char_boundary(s, 5), 5);
    }

    #[test]
    fn test_safe_char_boundary_empty_string() {
        let s = "";
        assert_eq!(safe_char_boundary(s, 0), 0);
        assert_eq!(safe_char_boundary(s, 10), 0);
    }

    #[test]
    fn test_safe_char_boundary_multibyte_utf8() {
        // Chinese character "中" is 3 bytes: E4 B8 AD
        let s = "中文";
        // Index 0 is valid (start of first char)
        assert_eq!(safe_char_boundary(s, 0), 0);
        // Index 1 is in the middle of "中", should back up to 0
        assert_eq!(safe_char_boundary(s, 1), 0);
        // Index 2 is also in the middle
        assert_eq!(safe_char_boundary(s, 2), 0);
        // Index 3 is the start of "文"
        assert_eq!(safe_char_boundary(s, 3), 3);
        // Index 4 is in the middle of "文"
        assert_eq!(safe_char_boundary(s, 4), 3);
    }

    #[test]
    fn test_safe_char_boundary_emoji() {
        // "👋" emoji is 4 bytes
        let s = "Hello 👋 World";
        // The emoji starts at byte 6
        assert_eq!(safe_char_boundary(s, 6), 6);
        // Middle of emoji should back up
        assert_eq!(safe_char_boundary(s, 7), 6);
        assert_eq!(safe_char_boundary(s, 8), 6);
        assert_eq!(safe_char_boundary(s, 9), 6);
        // After emoji (byte 10)
        assert_eq!(safe_char_boundary(s, 10), 10);
    }

    #[test]
    fn test_safe_char_boundary_mixed_content() {
        // Mix of ASCII and multi-byte
        let s = "aбв"; // 'a' is 1 byte, 'б' and 'в' are 2 bytes each
        assert_eq!(safe_char_boundary(s, 0), 0);
        assert_eq!(safe_char_boundary(s, 1), 1); // Start of 'б'
        assert_eq!(safe_char_boundary(s, 2), 1); // Middle of 'б', back to 1
        assert_eq!(safe_char_boundary(s, 3), 3); // Start of 'в'
        assert_eq!(safe_char_boundary(s, 4), 3); // Middle of 'в'
        assert_eq!(safe_char_boundary(s, 5), 5); // End
    }

    // ==========================================================================
    // clean_jsdoc tests
    // ==========================================================================

    #[test]
    fn test_clean_jsdoc_simple() {
        let input = "/** This is a simple doc */";
        assert_eq!(clean_jsdoc(input), "This is a simple doc");
    }

    #[test]
    fn test_clean_jsdoc_multiline() {
        let input = "/**\n * Line 1\n * Line 2\n */";
        let result = clean_jsdoc(input);
        // Trailing slash is kept when on its own line
        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 2"));
    }

    #[test]
    fn test_clean_jsdoc_with_asterisks() {
        let input = "/**\n * First line\n * Second line\n * Third line\n */";
        let result = clean_jsdoc(input);
        assert!(result.contains("First line"));
        assert!(result.contains("Second line"));
        assert!(result.contains("Third line"));
    }

    #[test]
    fn test_clean_jsdoc_empty() {
        let input = "/** */";
        assert_eq!(clean_jsdoc(input), "");
    }

    #[test]
    fn test_clean_jsdoc_c_style_comment() {
        let input = "/* Regular C comment */";
        assert_eq!(clean_jsdoc(input), "Regular C comment");
    }

    #[test]
    fn test_clean_jsdoc_with_tags() {
        let input = "/**\n * Description\n * @param x The x value\n * @returns Result\n */";
        let result = clean_jsdoc(input);
        assert!(result.contains("Description"));
        assert!(result.contains("@param x"));
        assert!(result.contains("@returns"));
    }

    #[test]
    fn test_clean_jsdoc_whitespace_handling() {
        let input = "/**   \n   *    Lots of spaces    \n   */";
        assert!(clean_jsdoc(input).contains("Lots of spaces"));
    }

    // ==========================================================================
    // clean_javadoc tests
    // ==========================================================================

    #[test]
    fn test_clean_javadoc_simple() {
        let input = "/** JavaDoc comment */";
        assert_eq!(clean_javadoc(input), "JavaDoc comment");
    }

    #[test]
    fn test_clean_javadoc_multiline() {
        let input = "/**\n * Method description.\n * @param name The name\n */";
        let result = clean_javadoc(input);
        assert!(result.contains("Method description"));
        assert!(result.contains("@param name"));
    }

    // ==========================================================================
    // map_symbol_kind tests
    // ==========================================================================

    #[test]
    fn test_map_symbol_kind_function() {
        assert_eq!(map_symbol_kind("function"), SymbolKind::Function);
    }

    #[test]
    fn test_map_symbol_kind_class() {
        assert_eq!(map_symbol_kind("class"), SymbolKind::Class);
    }

    #[test]
    fn test_map_symbol_kind_method() {
        assert_eq!(map_symbol_kind("method"), SymbolKind::Method);
    }

    #[test]
    fn test_map_symbol_kind_struct() {
        assert_eq!(map_symbol_kind("struct"), SymbolKind::Struct);
    }

    #[test]
    fn test_map_symbol_kind_enum() {
        assert_eq!(map_symbol_kind("enum"), SymbolKind::Enum);
    }

    #[test]
    fn test_map_symbol_kind_interface() {
        assert_eq!(map_symbol_kind("interface"), SymbolKind::Interface);
    }

    #[test]
    fn test_map_symbol_kind_trait() {
        assert_eq!(map_symbol_kind("trait"), SymbolKind::Trait);
    }

    #[test]
    fn test_map_symbol_kind_unknown() {
        // Unknown capture names default to Function
        assert_eq!(map_symbol_kind("unknown"), SymbolKind::Function);
        assert_eq!(map_symbol_kind(""), SymbolKind::Function);
        assert_eq!(map_symbol_kind("random"), SymbolKind::Function);
    }

    // ==========================================================================
    // is_builtin tests - Python
    // ==========================================================================

    #[test]
    fn test_is_builtin_python_print() {
        assert!(is_builtin("print", Language::Python));
        assert!(is_builtin("len", Language::Python));
        assert!(is_builtin("range", Language::Python));
        assert!(is_builtin("str", Language::Python));
        assert!(is_builtin("int", Language::Python));
        assert!(is_builtin("float", Language::Python));
        assert!(is_builtin("list", Language::Python));
        assert!(is_builtin("dict", Language::Python));
        assert!(is_builtin("set", Language::Python));
        assert!(is_builtin("tuple", Language::Python));
    }

    #[test]
    fn test_is_builtin_python_type_funcs() {
        assert!(is_builtin("bool", Language::Python));
        assert!(is_builtin("type", Language::Python));
        assert!(is_builtin("isinstance", Language::Python));
        assert!(is_builtin("hasattr", Language::Python));
        assert!(is_builtin("getattr", Language::Python));
        assert!(is_builtin("setattr", Language::Python));
        assert!(is_builtin("super", Language::Python));
    }

    #[test]
    fn test_is_builtin_python_itertools() {
        assert!(is_builtin("iter", Language::Python));
        assert!(is_builtin("next", Language::Python));
        assert!(is_builtin("enumerate", Language::Python));
        assert!(is_builtin("zip", Language::Python));
        assert!(is_builtin("map", Language::Python));
        assert!(is_builtin("filter", Language::Python));
        assert!(is_builtin("sorted", Language::Python));
        assert!(is_builtin("reversed", Language::Python));
    }

    #[test]
    fn test_is_builtin_python_math() {
        assert!(is_builtin("sum", Language::Python));
        assert!(is_builtin("min", Language::Python));
        assert!(is_builtin("max", Language::Python));
        assert!(is_builtin("abs", Language::Python));
        assert!(is_builtin("round", Language::Python));
    }

    #[test]
    fn test_is_builtin_python_not_builtin() {
        assert!(!is_builtin("my_function", Language::Python));
        assert!(!is_builtin("custom_print", Language::Python));
        assert!(!is_builtin("calculate", Language::Python));
    }

    // ==========================================================================
    // is_builtin tests - JavaScript/TypeScript
    // ==========================================================================

    #[test]
    fn test_is_builtin_js_console() {
        assert!(is_builtin("console", Language::JavaScript));
        assert!(is_builtin("log", Language::JavaScript));
        assert!(is_builtin("error", Language::JavaScript));
        assert!(is_builtin("warn", Language::JavaScript));
    }

    #[test]
    fn test_is_builtin_js_parsing() {
        assert!(is_builtin("parseInt", Language::JavaScript));
        assert!(is_builtin("parseFloat", Language::JavaScript));
        assert!(is_builtin("JSON", Language::JavaScript));
        assert!(is_builtin("stringify", Language::JavaScript));
        assert!(is_builtin("parse", Language::JavaScript));
    }

    #[test]
    fn test_is_builtin_js_timers() {
        assert!(is_builtin("setTimeout", Language::JavaScript));
        assert!(is_builtin("setInterval", Language::JavaScript));
        assert!(is_builtin("clearTimeout", Language::JavaScript));
        assert!(is_builtin("clearInterval", Language::JavaScript));
    }

    #[test]
    fn test_is_builtin_js_array_methods() {
        assert!(is_builtin("push", Language::JavaScript));
        assert!(is_builtin("pop", Language::JavaScript));
        assert!(is_builtin("shift", Language::JavaScript));
        assert!(is_builtin("unshift", Language::JavaScript));
        assert!(is_builtin("slice", Language::JavaScript));
        assert!(is_builtin("splice", Language::JavaScript));
        assert!(is_builtin("map", Language::JavaScript));
        assert!(is_builtin("filter", Language::JavaScript));
        assert!(is_builtin("reduce", Language::JavaScript));
        assert!(is_builtin("forEach", Language::JavaScript));
    }

    #[test]
    fn test_is_builtin_ts_same_as_js() {
        assert!(is_builtin("console", Language::TypeScript));
        assert!(is_builtin("map", Language::TypeScript));
        assert!(is_builtin("filter", Language::TypeScript));
    }

    #[test]
    fn test_is_builtin_js_not_builtin() {
        assert!(!is_builtin("myFunction", Language::JavaScript));
        assert!(!is_builtin("customLog", Language::JavaScript));
    }

    // ==========================================================================
    // is_builtin tests - Rust
    // ==========================================================================

    #[test]
    fn test_is_builtin_rust_macros() {
        assert!(is_builtin("println!", Language::Rust));
        assert!(is_builtin("print!", Language::Rust));
        assert!(is_builtin("eprintln!", Language::Rust));
        assert!(is_builtin("eprint!", Language::Rust));
        assert!(is_builtin("format!", Language::Rust));
        assert!(is_builtin("vec!", Language::Rust));
        assert!(is_builtin("panic!", Language::Rust));
        assert!(is_builtin("assert!", Language::Rust));
        assert!(is_builtin("assert_eq!", Language::Rust));
        assert!(is_builtin("assert_ne!", Language::Rust));
    }

    #[test]
    fn test_is_builtin_rust_logging() {
        assert!(is_builtin("debug!", Language::Rust));
        assert!(is_builtin("info!", Language::Rust));
        assert!(is_builtin("warn!", Language::Rust));
        assert!(is_builtin("error!", Language::Rust));
        assert!(is_builtin("trace!", Language::Rust));
    }

    #[test]
    fn test_is_builtin_rust_common_methods() {
        assert!(is_builtin("unwrap", Language::Rust));
        assert!(is_builtin("expect", Language::Rust));
        assert!(is_builtin("ok", Language::Rust));
        assert!(is_builtin("err", Language::Rust));
        assert!(is_builtin("some", Language::Rust));
        assert!(is_builtin("none", Language::Rust));
        assert!(is_builtin("clone", Language::Rust));
        assert!(is_builtin("to_string", Language::Rust));
        assert!(is_builtin("into", Language::Rust));
        assert!(is_builtin("from", Language::Rust));
        assert!(is_builtin("default", Language::Rust));
    }

    #[test]
    fn test_is_builtin_rust_iterators() {
        assert!(is_builtin("iter", Language::Rust));
        assert!(is_builtin("into_iter", Language::Rust));
        assert!(is_builtin("collect", Language::Rust));
        assert!(is_builtin("map", Language::Rust));
        assert!(is_builtin("filter", Language::Rust));
    }

    #[test]
    fn test_is_builtin_rust_not_builtin() {
        assert!(!is_builtin("my_function", Language::Rust));
        assert!(!is_builtin("process_data", Language::Rust));
    }

    // ==========================================================================
    // is_builtin tests - Go
    // ==========================================================================

    #[test]
    fn test_is_builtin_go_fmt() {
        assert!(is_builtin("fmt", Language::Go));
        assert!(is_builtin("Println", Language::Go));
        assert!(is_builtin("Printf", Language::Go));
        assert!(is_builtin("Sprintf", Language::Go));
        assert!(is_builtin("Errorf", Language::Go));
    }

    #[test]
    fn test_is_builtin_go_memory() {
        assert!(is_builtin("make", Language::Go));
        assert!(is_builtin("new", Language::Go));
        assert!(is_builtin("len", Language::Go));
        assert!(is_builtin("cap", Language::Go));
        assert!(is_builtin("append", Language::Go));
        assert!(is_builtin("copy", Language::Go));
        assert!(is_builtin("delete", Language::Go));
    }

    #[test]
    fn test_is_builtin_go_control() {
        assert!(is_builtin("close", Language::Go));
        assert!(is_builtin("panic", Language::Go));
        assert!(is_builtin("recover", Language::Go));
        assert!(is_builtin("print", Language::Go));
    }

    #[test]
    fn test_is_builtin_go_not_builtin() {
        assert!(!is_builtin("ProcessData", Language::Go));
        assert!(!is_builtin("handleRequest", Language::Go));
    }

    // ==========================================================================
    // is_builtin tests - Java
    // ==========================================================================

    #[test]
    fn test_is_builtin_java_io() {
        assert!(is_builtin("println", Language::Java));
        assert!(is_builtin("print", Language::Java));
        assert!(is_builtin("printf", Language::Java));
    }

    #[test]
    fn test_is_builtin_java_object() {
        assert!(is_builtin("toString", Language::Java));
        assert!(is_builtin("equals", Language::Java));
        assert!(is_builtin("hashCode", Language::Java));
        assert!(is_builtin("getClass", Language::Java));
        assert!(is_builtin("clone", Language::Java));
        assert!(is_builtin("notify", Language::Java));
        assert!(is_builtin("wait", Language::Java));
    }

    #[test]
    fn test_is_builtin_java_collections() {
        assert!(is_builtin("get", Language::Java));
        assert!(is_builtin("set", Language::Java));
        assert!(is_builtin("add", Language::Java));
        assert!(is_builtin("remove", Language::Java));
        assert!(is_builtin("size", Language::Java));
        assert!(is_builtin("isEmpty", Language::Java));
        assert!(is_builtin("contains", Language::Java));
        assert!(is_builtin("iterator", Language::Java));
    }

    #[test]
    fn test_is_builtin_java_not_builtin() {
        assert!(!is_builtin("processData", Language::Java));
        assert!(!is_builtin("calculateTotal", Language::Java));
    }

    // ==========================================================================
    // is_builtin tests - C/C++
    // ==========================================================================

    #[test]
    fn test_is_builtin_c_io() {
        assert!(is_builtin("printf", Language::C));
        assert!(is_builtin("scanf", Language::C));
    }

    #[test]
    fn test_is_builtin_c_memory() {
        assert!(is_builtin("malloc", Language::C));
        assert!(is_builtin("free", Language::C));
        assert!(is_builtin("memcpy", Language::C));
        assert!(is_builtin("memset", Language::C));
    }

    #[test]
    fn test_is_builtin_c_string() {
        assert!(is_builtin("strlen", Language::C));
        assert!(is_builtin("strcpy", Language::C));
        assert!(is_builtin("strcmp", Language::C));
        assert!(is_builtin("strcat", Language::C));
    }

    #[test]
    fn test_is_builtin_cpp_streams() {
        assert!(is_builtin("cout", Language::Cpp));
        assert!(is_builtin("cin", Language::Cpp));
        assert!(is_builtin("endl", Language::Cpp));
        assert!(is_builtin("cerr", Language::Cpp));
        assert!(is_builtin("clog", Language::Cpp));
    }

    #[test]
    fn test_is_builtin_c_not_builtin() {
        assert!(!is_builtin("process_data", Language::C));
        assert!(!is_builtin("custom_malloc", Language::C));
    }

    // ==========================================================================
    // is_builtin tests - C#
    // ==========================================================================

    #[test]
    fn test_is_builtin_csharp_console() {
        assert!(is_builtin("WriteLine", Language::CSharp));
        assert!(is_builtin("Write", Language::CSharp));
        assert!(is_builtin("ReadLine", Language::CSharp));
    }

    #[test]
    fn test_is_builtin_csharp_object() {
        assert!(is_builtin("ToString", Language::CSharp));
        assert!(is_builtin("Equals", Language::CSharp));
        assert!(is_builtin("GetHashCode", Language::CSharp));
        assert!(is_builtin("GetType", Language::CSharp));
    }

    #[test]
    fn test_is_builtin_csharp_collections() {
        assert!(is_builtin("Add", Language::CSharp));
        assert!(is_builtin("Remove", Language::CSharp));
        assert!(is_builtin("Contains", Language::CSharp));
        assert!(is_builtin("Count", Language::CSharp));
        assert!(is_builtin("Clear", Language::CSharp));
        assert!(is_builtin("ToList", Language::CSharp));
        assert!(is_builtin("ToArray", Language::CSharp));
    }

    // ==========================================================================
    // is_builtin tests - Ruby
    // ==========================================================================

    #[test]
    fn test_is_builtin_ruby_io() {
        assert!(is_builtin("puts", Language::Ruby));
        assert!(is_builtin("print", Language::Ruby));
        assert!(is_builtin("p", Language::Ruby));
        assert!(is_builtin("gets", Language::Ruby));
    }

    #[test]
    fn test_is_builtin_ruby_enumerable() {
        assert!(is_builtin("each", Language::Ruby));
        assert!(is_builtin("map", Language::Ruby));
        assert!(is_builtin("select", Language::Ruby));
        assert!(is_builtin("reject", Language::Ruby));
        assert!(is_builtin("reduce", Language::Ruby));
        assert!(is_builtin("inject", Language::Ruby));
        assert!(is_builtin("find", Language::Ruby));
    }

    #[test]
    fn test_is_builtin_ruby_predicates() {
        assert!(is_builtin("any?", Language::Ruby));
        assert!(is_builtin("all?", Language::Ruby));
        assert!(is_builtin("include?", Language::Ruby));
        assert!(is_builtin("empty?", Language::Ruby));
        assert!(is_builtin("nil?", Language::Ruby));
    }

    // ==========================================================================
    // is_builtin tests - PHP
    // ==========================================================================

    #[test]
    fn test_is_builtin_php_io() {
        assert!(is_builtin("echo", Language::Php));
        assert!(is_builtin("print", Language::Php));
        assert!(is_builtin("var_dump", Language::Php));
        assert!(is_builtin("print_r", Language::Php));
    }

    #[test]
    fn test_is_builtin_php_checks() {
        assert!(is_builtin("isset", Language::Php));
        assert!(is_builtin("empty", Language::Php));
    }

    #[test]
    fn test_is_builtin_php_array_string() {
        assert!(is_builtin("array", Language::Php));
        assert!(is_builtin("count", Language::Php));
        assert!(is_builtin("strlen", Language::Php));
        assert!(is_builtin("strpos", Language::Php));
        assert!(is_builtin("substr", Language::Php));
        assert!(is_builtin("explode", Language::Php));
        assert!(is_builtin("implode", Language::Php));
        assert!(is_builtin("json_encode", Language::Php));
        assert!(is_builtin("json_decode", Language::Php));
    }

    // ==========================================================================
    // is_builtin tests - Kotlin
    // ==========================================================================

    #[test]
    fn test_is_builtin_kotlin_io() {
        assert!(is_builtin("println", Language::Kotlin));
        assert!(is_builtin("print", Language::Kotlin));
        assert!(is_builtin("readLine", Language::Kotlin));
    }

    #[test]
    fn test_is_builtin_kotlin_scope() {
        assert!(is_builtin("let", Language::Kotlin));
        assert!(is_builtin("also", Language::Kotlin));
        assert!(is_builtin("apply", Language::Kotlin));
        assert!(is_builtin("run", Language::Kotlin));
        assert!(is_builtin("with", Language::Kotlin));
    }

    #[test]
    fn test_is_builtin_kotlin_collections() {
        assert!(is_builtin("listOf", Language::Kotlin));
        assert!(is_builtin("mapOf", Language::Kotlin));
        assert!(is_builtin("setOf", Language::Kotlin));
        assert!(is_builtin("map", Language::Kotlin));
        assert!(is_builtin("filter", Language::Kotlin));
        assert!(is_builtin("forEach", Language::Kotlin));
    }

    // ==========================================================================
    // is_builtin tests - Swift
    // ==========================================================================

    #[test]
    fn test_is_builtin_swift_io() {
        assert!(is_builtin("print", Language::Swift));
        assert!(is_builtin("debugPrint", Language::Swift));
        assert!(is_builtin("dump", Language::Swift));
    }

    #[test]
    fn test_is_builtin_swift_functional() {
        assert!(is_builtin("map", Language::Swift));
        assert!(is_builtin("filter", Language::Swift));
        assert!(is_builtin("reduce", Language::Swift));
        assert!(is_builtin("forEach", Language::Swift));
    }

    #[test]
    fn test_is_builtin_swift_collection() {
        assert!(is_builtin("contains", Language::Swift));
        assert!(is_builtin("count", Language::Swift));
        assert!(is_builtin("isEmpty", Language::Swift));
        assert!(is_builtin("append", Language::Swift));
    }

    // ==========================================================================
    // is_builtin tests - Scala
    // ==========================================================================

    #[test]
    fn test_is_builtin_scala_io() {
        assert!(is_builtin("println", Language::Scala));
        assert!(is_builtin("print", Language::Scala));
    }

    #[test]
    fn test_is_builtin_scala_functional() {
        assert!(is_builtin("map", Language::Scala));
        assert!(is_builtin("filter", Language::Scala));
        assert!(is_builtin("flatMap", Language::Scala));
        assert!(is_builtin("foreach", Language::Scala));
        assert!(is_builtin("reduce", Language::Scala));
        assert!(is_builtin("fold", Language::Scala));
        assert!(is_builtin("foldLeft", Language::Scala));
        assert!(is_builtin("foldRight", Language::Scala));
        assert!(is_builtin("collect", Language::Scala));
    }

    // ==========================================================================
    // is_builtin tests - Languages with no builtins
    // ==========================================================================

    #[test]
    fn test_is_builtin_bash_always_false() {
        assert!(!is_builtin("ls", Language::Bash));
        assert!(!is_builtin("echo", Language::Bash));
        assert!(!is_builtin("grep", Language::Bash));
    }

    #[test]
    fn test_is_builtin_haskell_always_false() {
        assert!(!is_builtin("putStrLn", Language::Haskell));
        assert!(!is_builtin("map", Language::Haskell));
    }

    #[test]
    fn test_is_builtin_elixir_always_false() {
        assert!(!is_builtin("IO.puts", Language::Elixir));
        assert!(!is_builtin("Enum.map", Language::Elixir));
    }

    #[test]
    fn test_is_builtin_clojure_always_false() {
        assert!(!is_builtin("println", Language::Clojure));
        assert!(!is_builtin("map", Language::Clojure));
    }

    #[test]
    fn test_is_builtin_ocaml_always_false() {
        assert!(!is_builtin("print_endline", Language::OCaml));
        assert!(!is_builtin("List.map", Language::OCaml));
    }

    #[test]
    fn test_is_builtin_fsharp_always_false() {
        assert!(!is_builtin("printfn", Language::FSharp));
        assert!(!is_builtin("List.map", Language::FSharp));
    }

    #[test]
    fn test_is_builtin_lua_always_false() {
        assert!(!is_builtin("print", Language::Lua));
        assert!(!is_builtin("pairs", Language::Lua));
    }

    #[test]
    fn test_is_builtin_r_always_false() {
        assert!(!is_builtin("print", Language::R));
        assert!(!is_builtin("cat", Language::R));
    }

    // ==========================================================================
    // Integration tests using tree-sitter parsing
    // ==========================================================================

    // Helper to parse code and get the first node of a specific kind
    fn parse_and_find_node(
        code: &str,
        language: Language,
        node_kind: &str,
    ) -> Option<(tree_sitter::Tree, usize)> {
        let mut parser = tree_sitter::Parser::new();

        let ts_language = match language {
            Language::Python => tree_sitter_python::LANGUAGE,
            Language::Rust => tree_sitter_rust::LANGUAGE,
            Language::JavaScript => tree_sitter_javascript::LANGUAGE,
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            Language::Go => tree_sitter_go::LANGUAGE,
            Language::Java => tree_sitter_java::LANGUAGE,
            _ => return None,
        };

        parser
            .set_language(&ts_language.into())
            .expect("Error loading grammar");

        let tree = parser.parse(code, None)?;
        let root = tree.root_node();

        fn find_node_recursive(node: tree_sitter::Node<'_>, kind: &str) -> Option<usize> {
            if node.kind() == kind {
                return Some(node.id());
            }
            for child in node.children(&mut node.walk()) {
                if let Some(id) = find_node_recursive(child, kind) {
                    return Some(id);
                }
            }
            None
        }

        find_node_recursive(root, node_kind).map(|_| (tree, 0))
    }

    // Helper to find node by kind in tree
    fn find_node_in_tree<'a>(
        node: tree_sitter::Node<'a>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == kind {
            return Some(node);
        }
        for child in node.children(&mut node.walk()) {
            if let Some(found) = find_node_in_tree(child, kind) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn test_extract_signature_python() {
        // Note: Python signature extraction stops at first ':' or '\n'
        // So type annotations in parameters are cut off at the first ':'
        let code = "def hello(name):\n    return f'Hello {name}'";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_definition").unwrap();

        let sig = extract_signature(func_node, code, Language::Python);
        assert!(sig.is_some());
        let sig = sig.unwrap();
        assert!(sig.contains("def hello"));
        assert!(sig.contains("name"));
    }

    #[test]
    fn test_extract_signature_rust() {
        let code = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_item").unwrap();

        let sig = extract_signature(func_node, code, Language::Rust);
        assert!(sig.is_some());
        let sig = sig.unwrap();
        assert!(sig.contains("fn add"));
        assert!(sig.contains("i32"));
    }

    #[test]
    fn test_extract_signature_javascript() {
        let code = "function greet(name) { return 'Hello ' + name; }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_declaration").unwrap();

        let sig = extract_signature(func_node, code, Language::JavaScript);
        assert!(sig.is_some());
        let sig = sig.unwrap();
        assert!(sig.contains("function greet"));
        assert!(sig.contains("name"));
    }

    #[test]
    fn test_extract_visibility_python_public() {
        let code = "def public_func():\n    pass";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_definition").unwrap();

        let vis = extract_visibility(func_node, code, Language::Python);
        assert_eq!(vis, Visibility::Public);
    }

    #[test]
    fn test_extract_visibility_python_private() {
        let code = "def __private_func():\n    pass";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_definition").unwrap();

        let vis = extract_visibility(func_node, code, Language::Python);
        assert_eq!(vis, Visibility::Private);
    }

    #[test]
    fn test_extract_visibility_python_protected() {
        let code = "def _protected_func():\n    pass";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_definition").unwrap();

        let vis = extract_visibility(func_node, code, Language::Python);
        assert_eq!(vis, Visibility::Protected);
    }

    #[test]
    fn test_extract_visibility_python_dunder() {
        // Note: Current implementation treats dunder methods as public because
        // the check for `starts_with("__") && !ends_with("__")` excludes them from Private,
        // and `starts_with('_')` is checked in an else-if, not reached for true dunders
        let code = "def __init__(self):\n    pass";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_definition").unwrap();

        let vis = extract_visibility(func_node, code, Language::Python);
        // __init__ starts with _ so hits the else-if branch, returning Protected
        // This is the actual behavior - dunder methods are treated as Protected
        assert_eq!(vis, Visibility::Protected);
    }

    #[test]
    fn test_extract_visibility_rust_pub() {
        let code = "pub fn public_func() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_item").unwrap();

        let vis = extract_visibility(func_node, code, Language::Rust);
        assert_eq!(vis, Visibility::Public);
    }

    #[test]
    fn test_extract_visibility_rust_private() {
        let code = "fn private_func() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_item").unwrap();

        let vis = extract_visibility(func_node, code, Language::Rust);
        assert_eq!(vis, Visibility::Private);
    }

    #[test]
    fn test_extract_visibility_rust_pub_crate() {
        let code = "pub(crate) fn crate_func() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_item").unwrap();

        let vis = extract_visibility(func_node, code, Language::Rust);
        assert_eq!(vis, Visibility::Internal);
    }

    #[test]
    fn test_extract_visibility_go_exported() {
        let code = "func Exported() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_declaration").unwrap();

        let vis = extract_visibility(func_node, code, Language::Go);
        assert_eq!(vis, Visibility::Public);
    }

    #[test]
    fn test_extract_visibility_go_unexported() {
        let code = "func unexported() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_declaration").unwrap();

        let vis = extract_visibility(func_node, code, Language::Go);
        assert_eq!(vis, Visibility::Private);
    }

    #[test]
    fn test_extract_visibility_bash_always_public() {
        let code = "my_func() { echo hello; }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_bash::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_definition").unwrap();

        let vis = extract_visibility(func_node, code, Language::Bash);
        assert_eq!(vis, Visibility::Public);
    }

    #[test]
    fn test_find_body_node_python() {
        let code = "def foo():\n    x = 1\n    return x";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_definition").unwrap();

        let body = find_body_node(func_node, Language::Python);
        assert!(body.is_some());
        assert_eq!(body.unwrap().kind(), "block");
    }

    #[test]
    fn test_find_body_node_rust() {
        let code = "fn foo() { let x = 1; x }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_item").unwrap();

        let body = find_body_node(func_node, Language::Rust);
        assert!(body.is_some());
        assert_eq!(body.unwrap().kind(), "block");
    }

    #[test]
    fn test_find_body_node_javascript() {
        let code = "function foo() { return 1; }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_declaration").unwrap();

        let body = find_body_node(func_node, Language::JavaScript);
        assert!(body.is_some());
        assert_eq!(body.unwrap().kind(), "statement_block");
    }

    #[test]
    fn test_extract_calls_python() {
        let code = "def foo():\n    bar()\n    custom_func(1, 2)";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_definition").unwrap();

        let calls = extract_calls(func_node, code, Language::Python);
        assert!(calls.contains(&"bar".to_string()));
        assert!(calls.contains(&"custom_func".to_string()));
    }

    #[test]
    fn test_extract_calls_python_filters_builtins() {
        let code = "def foo():\n    print('hello')\n    len([1,2,3])";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_definition").unwrap();

        let calls = extract_calls(func_node, code, Language::Python);
        // Built-ins should be filtered out
        assert!(!calls.contains(&"print".to_string()));
        assert!(!calls.contains(&"len".to_string()));
    }

    #[test]
    fn test_extract_calls_rust() {
        let code = "fn foo() { bar(); baz(1); }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_item").unwrap();

        let calls = extract_calls(func_node, code, Language::Rust);
        assert!(calls.contains(&"bar".to_string()));
        assert!(calls.contains(&"baz".to_string()));
    }

    #[test]
    fn test_extract_docstring_rust() {
        let code = "/// This is a doc comment\nfn foo() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_item").unwrap();

        let docstring = extract_docstring(func_node, code, Language::Rust);
        assert!(docstring.is_some());
        assert!(docstring.unwrap().contains("This is a doc comment"));
    }

    #[test]
    fn test_extract_docstring_rust_multiline() {
        let code = "/// Line 1\n/// Line 2\nfn foo() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(code, None).unwrap();
        let func_node = find_node_in_tree(tree.root_node(), "function_item").unwrap();

        let docstring = extract_docstring(func_node, code, Language::Rust);
        assert!(docstring.is_some());
        let doc = docstring.unwrap();
        assert!(doc.contains("Line 1"));
        assert!(doc.contains("Line 2"));
    }
}
