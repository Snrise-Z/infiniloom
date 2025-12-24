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
        let text = &source_code[start..end];
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
            let lines_before: Vec<_> = source_code[..start_byte]
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
            let lines_before: Vec<_> = source_code[..start_byte]
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
        Language::CSharp
        | Language::Php
        | Language::Kotlin
        | Language::Swift
        | Language::Scala => {
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
        Language::CSharp
        | Language::Php
        | Language::Kotlin
        | Language::Swift
        | Language::Scala => {
            if kind == "invocation_expression" || kind == "call_expression" {
                node.children(&mut node.walk())
                    .find(|child| {
                        child.kind() == "identifier" || child.kind() == "simple_name"
                    })
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
                    .find(|child| {
                        child.kind() == "identifier" || child.kind() == "variable"
                    })
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
                                        if let Ok(name) = type_node.utf8_text(source_code.as_bytes())
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
                                        if let Ok(name) = type_node.utf8_text(source_code.as_bytes())
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
                                            if let Some(type_node) = field.child_by_field_name("type")
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
                                        if let Ok(name) = type_node.utf8_text(source_code.as_bytes())
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
                                        if let Ok(name) = type_node.utf8_text(source_code.as_bytes())
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
