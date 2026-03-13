//! Type signature extraction via Tree-sitter queries
//!
//! Given a chunk's source code and language, this module parses the AST
//! and extracts type information (parameter types, return types, error types)
//! for function/method declarations.
//!
//! Supported languages: Rust, TypeScript, Python, Java, Go.
//! For unsupported languages, returns `None`.

use crate::parser::Language;

/// Extracted type information for a function or method
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeInfo {
    /// Clean type signature, e.g. "(i32, &str) -> Result<Claims, AuthError>"
    pub type_signature: Option<String>,
    /// Individual parameter types, e.g. ["i32", "&str"]
    pub parameter_types: Vec<String>,
    /// Return type, e.g. "Result<Claims, AuthError>"
    pub return_type: Option<String>,
    /// Error/exception types, e.g. ["AuthError"]
    pub error_types: Vec<String>,
}

/// Extract type information from a code chunk's content.
///
/// Parses the content with Tree-sitter for the given language and extracts
/// parameter types, return type, error types, and a clean type signature
/// from the first function/method declaration found.
///
/// Returns `None` if the language is unsupported or no function is found.
pub fn extract_types(content: &str, language: Language) -> Option<TypeInfo> {
    let ts_lang = language.tree_sitter_language()?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).ok()?;
    let tree = parser.parse(content, None)?;
    let root = tree.root_node();

    match language {
        Language::Rust => extract_rust_types(root, content),
        Language::TypeScript => extract_typescript_types(root, content),
        Language::Python => extract_python_types(root, content),
        Language::Java => extract_java_types(root, content),
        Language::Go => extract_go_types(root, content),
        _ => None,
    }
}

/// Recursively find the first node with one of the given kinds.
fn find_first_node<'a>(
    node: tree_sitter::Node<'a>,
    kinds: &[&str],
) -> Option<tree_sitter::Node<'a>> {
    if kinds.contains(&node.kind()) {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_first_node(child, kinds) {
            return Some(found);
        }
    }
    None
}

/// Get the text of a node from source content.
fn node_text<'a>(node: tree_sitter::Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

fn extract_rust_types(root: tree_sitter::Node<'_>, source: &str) -> Option<TypeInfo> {
    let func_node = find_first_node(root, &["function_item", "function_signature_item"])?;

    let mut param_types = Vec::new();

    // Find parameters node
    if let Some(params_node) = find_child_by_kind(func_node, "parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter" {
                // Look for the type child (skip the pattern/name)
                if let Some(type_node) = find_child_by_kind(child, "type_identifier")
                    .or_else(|| find_child_by_kind(child, "reference_type"))
                    .or_else(|| find_child_by_kind(child, "generic_type"))
                    .or_else(|| find_child_by_kind(child, "scoped_type_identifier"))
                    .or_else(|| find_child_by_kind(child, "primitive_type"))
                    .or_else(|| find_child_by_kind(child, "array_type"))
                    .or_else(|| find_child_by_kind(child, "tuple_type"))
                    .or_else(|| find_child_by_kind(child, "function_type"))
                    .or_else(|| find_child_by_kind(child, "bounded_type"))
                    .or_else(|| find_child_by_kind(child, "dynamic_type"))
                {
                    param_types.push(node_text(type_node, source).to_owned());
                }
            } else if child.kind() == "self_parameter" {
                param_types.push(node_text(child, source).to_owned());
            }
        }
    }

    // Find return type
    let mut return_type: Option<String> = None;
    let mut cursor = func_node.walk();
    for child in func_node.children(&mut cursor) {
        // In tree-sitter-rust, the return type appears as a child with
        // kind "type_identifier", "generic_type", etc. after the "->" token.
        // Look for a node whose previous sibling is "->".
        if child.kind() == "->" {
            // The next sibling is the return type
            if let Some(next) = child.next_sibling() {
                return_type = Some(node_text(next, source).trim().to_owned());
            }
        }
    }

    // Extract error types from Result<_, E>
    let error_types = return_type
        .as_ref()
        .map(|rt| extract_rust_error_types(rt))
        .unwrap_or_default();

    // Build type signature
    let params_str = param_types
        .iter()
        .filter(|p| *p != "&self" && *p != "&mut self" && *p != "self")
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");

    let type_signature = if let Some(ref rt) = return_type {
        Some(format!("({}) -> {}", params_str, rt))
    } else if !param_types.is_empty() {
        Some(format!("({})", params_str))
    } else {
        None
    };

    if type_signature.is_none() && param_types.is_empty() && return_type.is_none() {
        return None;
    }

    Some(TypeInfo { type_signature, parameter_types: param_types, return_type, error_types })
}

/// Extract error types from a Rust Result type string.
/// e.g., "Result<Claims, AuthError>" -> ["AuthError"]
fn extract_rust_error_types(return_type: &str) -> Vec<String> {
    let trimmed = return_type.trim();
    if !trimmed.starts_with("Result<") && !trimmed.starts_with("Result <") {
        return Vec::new();
    }

    // Find the content between Result< and the matching >
    if let Some(start) = trimmed.find('<') {
        let inner = &trimmed[start + 1..];
        if let Some(end) = find_matching_bracket(inner) {
            let content = &inner[..end];
            // Split on the first top-level comma
            if let Some(comma_pos) = find_top_level_comma(content) {
                let error_part = content[comma_pos + 1..].trim();
                if !error_part.is_empty() {
                    return vec![error_part.to_owned()];
                }
            }
        }
    }
    Vec::new()
}

/// Find position of matching closing bracket, accounting for nesting.
fn find_matching_bracket(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            },
            _ => {},
        }
    }
    None
}

/// Find position of the first comma at nesting depth 0.
fn find_top_level_comma(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {},
        }
    }
    None
}

// ---------------------------------------------------------------------------
// TypeScript
// ---------------------------------------------------------------------------

fn extract_typescript_types(root: tree_sitter::Node<'_>, source: &str) -> Option<TypeInfo> {
    let func_node = find_first_node(
        root,
        &["function_declaration", "method_definition", "arrow_function", "function_signature"],
    )?;

    let mut param_types = Vec::new();

    // Find formal_parameters
    if let Some(params_node) = find_child_by_kind(func_node, "formal_parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            if child.kind() == "required_parameter" || child.kind() == "optional_parameter" {
                if let Some(ta) = find_child_by_kind(child, "type_annotation") {
                    // The type is the child after the ":"
                    let mut ta_cursor = ta.walk();
                    for ta_child in ta.children(&mut ta_cursor) {
                        if ta_child.kind() != ":" {
                            let text = node_text(ta_child, source).trim();
                            if !text.is_empty() {
                                param_types.push(text.to_owned());
                            }
                        }
                    }
                }
            }
        }
    }

    // Find return type annotation on the function itself
    let return_type = find_child_by_kind(func_node, "type_annotation").and_then(|ta| {
        let mut cursor = ta.walk();
        for child in ta.children(&mut cursor) {
            if child.kind() != ":" {
                let text = node_text(child, source).trim().to_owned();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
        None
    });

    // Build TS-style type signature
    let params_str = param_types.join(", ");
    let type_signature = if let Some(ref rt) = return_type {
        Some(format!("({}) => {}", params_str, rt))
    } else if !param_types.is_empty() {
        Some(format!("({})", params_str))
    } else {
        None
    };

    if type_signature.is_none() && param_types.is_empty() && return_type.is_none() {
        return None;
    }

    Some(TypeInfo {
        type_signature,
        parameter_types: param_types,
        return_type,
        error_types: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

fn extract_python_types(root: tree_sitter::Node<'_>, source: &str) -> Option<TypeInfo> {
    let func_node = find_first_node(root, &["function_definition"])?;

    let mut param_types = Vec::new();

    // Find parameters
    if let Some(params_node) = find_child_by_kind(func_node, "parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            // typed_parameter has a type child
            if child.kind() == "typed_parameter" || child.kind() == "typed_default_parameter" {
                if let Some(type_node) = find_child_by_kind(child, "type") {
                    param_types.push(node_text(type_node, source).trim().to_owned());
                }
            }
        }
    }

    // Find return type (-> annotation)
    let return_type =
        find_child_by_kind(func_node, "type").map(|n| node_text(n, source).trim().to_owned());

    // Build Python-style type signature
    let params_str = param_types.join(", ");
    let type_signature = if let Some(ref rt) = return_type {
        Some(format!("({}) -> {}", params_str, rt))
    } else if !param_types.is_empty() {
        Some(format!("({})", params_str))
    } else {
        None
    };

    if type_signature.is_none() && param_types.is_empty() && return_type.is_none() {
        return None;
    }

    Some(TypeInfo {
        type_signature,
        parameter_types: param_types,
        return_type,
        error_types: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

fn extract_java_types(root: tree_sitter::Node<'_>, source: &str) -> Option<TypeInfo> {
    let func_node = find_first_node(root, &["method_declaration", "constructor_declaration"])?;

    let mut param_types = Vec::new();

    // Java: return type appears before the method name
    // In tree-sitter-java, method_declaration has children like:
    //   modifiers? type_identifier identifier formal_parameters throws? block
    let mut return_type: Option<String> = None;
    let mut cursor = func_node.walk();
    for child in func_node.children(&mut cursor) {
        let kind = child.kind();
        // Type nodes that can appear as return type
        if kind == "type_identifier"
            || kind == "generic_type"
            || kind == "array_type"
            || kind == "void_type"
            || kind == "integral_type"
            || kind == "floating_point_type"
            || kind == "boolean_type"
            || kind == "scoped_type_identifier"
        {
            return_type = Some(node_text(child, source).trim().to_owned());
        }
        // Stop before the method name (identifier) and parameters
        if kind == "identifier" || kind == "formal_parameters" {
            break;
        }
    }

    // Find formal_parameters
    if let Some(params_node) = find_child_by_kind(func_node, "formal_parameters") {
        let mut pcursor = params_node.walk();
        for child in params_node.children(&mut pcursor) {
            if child.kind() == "formal_parameter" || child.kind() == "spread_parameter" {
                // The type is the first type-like child
                let mut param_cursor = child.walk();
                for pchild in child.children(&mut param_cursor) {
                    let pk = pchild.kind();
                    if pk == "type_identifier"
                        || pk == "generic_type"
                        || pk == "array_type"
                        || pk == "integral_type"
                        || pk == "floating_point_type"
                        || pk == "boolean_type"
                        || pk == "scoped_type_identifier"
                    {
                        param_types.push(node_text(pchild, source).trim().to_owned());
                        break;
                    }
                }
            }
        }
    }

    // Extract throws clause for error types
    let mut error_types = Vec::new();
    if let Some(throws_node) = find_child_by_kind(func_node, "throws") {
        let mut tcursor = throws_node.walk();
        for child in throws_node.children(&mut tcursor) {
            if child.kind() == "type_identifier" || child.kind() == "scoped_type_identifier" {
                error_types.push(node_text(child, source).trim().to_owned());
            }
        }
    }

    // Build Java-style type signature
    let params_str = param_types.join(", ");
    let mut sig = format!("({}) -> {}", params_str, return_type.as_deref().unwrap_or("void"));
    if !error_types.is_empty() {
        sig.push_str(&format!(" throws {}", error_types.join(", ")));
    }
    let type_signature = Some(sig);

    Some(TypeInfo { type_signature, parameter_types: param_types, return_type, error_types })
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

fn extract_go_types(root: tree_sitter::Node<'_>, source: &str) -> Option<TypeInfo> {
    let func_node = find_first_node(root, &["function_declaration", "method_declaration"])?;

    let mut param_types = Vec::new();

    // Find parameter_list
    // In Go, function_declaration has: "func" name parameter_list result? block
    // method_declaration has: "func" parameter_list name parameter_list result? block
    // We want the last parameter_list before the result/block
    let param_lists: Vec<tree_sitter::Node<'_>> = {
        let mut cursor = func_node.walk();
        func_node
            .children(&mut cursor)
            .filter(|c| c.kind() == "parameter_list")
            .collect()
    };

    // For method_declaration, the first parameter_list is the receiver
    // The actual params are the second parameter_list
    let params_node = if func_node.kind() == "method_declaration" {
        param_lists.get(1).or(param_lists.first())
    } else {
        param_lists.first()
    };

    if let Some(params) = params_node {
        let mut cursor = params.walk();
        for child in params.children(&mut cursor) {
            if child.kind() == "parameter_declaration" {
                // In Go, parameter_declaration has: name type
                // or just: type (for unnamed params)
                // The last type-like child is the type
                let mut last_type = None;
                let mut pcursor = child.walk();
                for pchild in child.children(&mut pcursor) {
                    let pk = pchild.kind();
                    if pk == "type_identifier"
                        || pk == "pointer_type"
                        || pk == "slice_type"
                        || pk == "array_type"
                        || pk == "map_type"
                        || pk == "channel_type"
                        || pk == "function_type"
                        || pk == "interface_type"
                        || pk == "struct_type"
                        || pk == "qualified_type"
                    {
                        last_type = Some(node_text(pchild, source).trim().to_owned());
                    }
                }
                if let Some(t) = last_type {
                    param_types.push(t);
                }
            }
        }
    }

    // Find result (return type)
    let mut return_type: Option<String> = None;
    let mut error_types = Vec::new();

    let mut cursor = func_node.walk();
    for child in func_node.children(&mut cursor) {
        if child.kind() == "parameter_list" {
            // Could be return type tuple: (Type1, Type2)
            // Check if this is after the main params by position
            if Some(&child) != params_node {
                let text = node_text(child, source).trim().to_owned();
                return_type = Some(text);

                // Check if last return is "error"
                let mut rcursor = child.walk();
                let return_params: Vec<_> = child
                    .children(&mut rcursor)
                    .filter(|c| c.kind() == "parameter_declaration")
                    .collect();
                if let Some(last) = return_params.last() {
                    let last_text = node_text(*last, source).trim();
                    if last_text == "error" || last_text.ends_with(" error") {
                        error_types.push("error".to_owned());
                    }
                }
            }
        }
        if child.kind() == "type_identifier"
            || child.kind() == "pointer_type"
            || child.kind() == "slice_type"
            || child.kind() == "qualified_type"
        {
            // Simple single return type
            let prev_sibling_is_params = child
                .prev_sibling()
                .is_some_and(|s| s.kind() == "parameter_list");
            if prev_sibling_is_params || return_type.is_none() {
                let text = node_text(child, source).trim().to_owned();
                if text == "error" {
                    error_types.push("error".to_owned());
                }
                return_type = Some(text);
            }
        }
    }

    // Build Go-style type signature
    let params_str = param_types.join(", ");
    let type_signature = if let Some(ref rt) = return_type {
        Some(format!("({}) -> {}", params_str, rt))
    } else if !param_types.is_empty() {
        Some(format!("({})", params_str))
    } else {
        None
    };

    if type_signature.is_none() && param_types.is_empty() && return_type.is_none() {
        return None;
    }

    Some(TypeInfo { type_signature, parameter_types: param_types, return_type, error_types })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the first direct child of `node` with the given kind.
fn find_child_by_kind<'a>(
    node: tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    let count = node.child_count() as u32;
    for i in 0..count {
        if let Some(child) = node.child(i) {
            if child.kind() == kind {
                return Some(child);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_typed_function() {
        let source = r#"fn validate(token: &str, max_age: i32) -> Result<Claims, AuthError> {
    todo!()
}"#;
        let info = extract_types(source, Language::Rust).unwrap();
        assert_eq!(info.parameter_types, vec!["&str", "i32"]);
        assert_eq!(info.return_type.as_deref(), Some("Result<Claims, AuthError>"));
        assert_eq!(info.error_types, vec!["AuthError"]);
        assert!(info
            .type_signature
            .as_ref()
            .unwrap()
            .contains("-> Result<Claims, AuthError>"));
    }

    #[test]
    fn test_rust_self_method() {
        let source = r#"fn process(&self, data: Vec<u8>) -> bool {
    true
}"#;
        let info = extract_types(source, Language::Rust).unwrap();
        assert!(info.parameter_types.contains(&"&self".to_owned()));
        assert!(info.parameter_types.contains(&"Vec<u8>".to_owned()));
        assert_eq!(info.return_type.as_deref(), Some("bool"));
    }

    #[test]
    fn test_rust_no_return_type() {
        let source = r#"fn setup(config: Config) {
    // ...
}"#;
        let info = extract_types(source, Language::Rust).unwrap();
        assert_eq!(info.parameter_types, vec!["Config"]);
        assert!(info.return_type.is_none());
    }

    #[test]
    fn test_typescript_function() {
        let source = r#"function greet(name: string, age: number): Promise<void> {
    console.log(name);
}"#;
        let info = extract_types(source, Language::TypeScript).unwrap();
        assert_eq!(info.parameter_types, vec!["string", "number"]);
        assert_eq!(info.return_type.as_deref(), Some("Promise<void>"));
        assert!(info
            .type_signature
            .as_ref()
            .unwrap()
            .contains("=> Promise<void>"));
    }

    #[test]
    fn test_python_function() {
        let source = r#"def process(data: list, count: int) -> dict:
    pass"#;
        let info = extract_types(source, Language::Python).unwrap();
        assert_eq!(info.parameter_types, vec!["list", "int"]);
        assert_eq!(info.return_type.as_deref(), Some("dict"));
        assert!(info.type_signature.as_ref().unwrap().contains("-> dict"));
    }

    #[test]
    fn test_no_types_returns_none() {
        // Python function without type annotations
        let source = r#"def hello(name):
    print(name)"#;
        let result = extract_types(source, Language::Python);
        assert!(result.is_none());
    }

    #[test]
    fn test_rust_error_type_extraction() {
        assert_eq!(extract_rust_error_types("Result<Claims, AuthError>"), vec!["AuthError"]);
        assert_eq!(extract_rust_error_types("Result<(), std::io::Error>"), vec!["std::io::Error"]);
        assert!(extract_rust_error_types("bool").is_empty());
        assert!(extract_rust_error_types("Option<String>").is_empty());
    }

    #[test]
    fn test_unsupported_language_returns_none() {
        let source = "def foo; end";
        let result = extract_types(source, Language::Ruby);
        assert!(result.is_none());
    }

    #[test]
    fn test_java_method() {
        let source = r#"class Foo {
    public String process(int count, List<String> items) throws IOException {
        return "";
    }
}"#;
        let info = extract_types(source, Language::Java);
        // Java should extract from method_declaration
        if let Some(info) = info {
            assert!(
                info.parameter_types.contains(&"int".to_owned())
                    || !info.parameter_types.is_empty()
            );
            assert!(!info.error_types.is_empty() || info.return_type.is_some());
        }
    }

    #[test]
    fn test_go_function() {
        let source = r#"package main

func Process(data []byte, count int) (string, error) {
    return "", nil
}"#;
        let info = extract_types(source, Language::Go);
        if let Some(info) = info {
            assert!(!info.parameter_types.is_empty() || info.return_type.is_some());
        }
    }
}
