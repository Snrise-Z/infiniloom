//! BM25-friendly identifier extraction from code chunks
//!
//! This module extracts unique identifiers from source code using Tree-sitter AST
//! parsing, filters out language keywords and noise, splits camelCase/snake_case
//! names, and produces a space-separated string optimized for BM25 text indexing.

use std::collections::{BTreeSet, HashSet};

use crate::parser::Language;

use super::chunker::split_identifier;

/// Extract a space-separated string of unique identifiers from chunk content,
/// optimized for BM25/sparse text indexing.
///
/// Algorithm:
/// 1. Parse the content with Tree-sitter to collect AST identifier nodes
/// 2. Deduplicate raw identifiers
/// 3. Filter out language keywords, single-char names, and noise words
/// 4. Split camelCase/snake_case into sub-tokens
/// 5. Deduplicate again, sort alphabetically, join with spaces
///
/// Returns `None` if no identifiers are found after filtering.
pub fn extract_identifiers(content: &str, language: Option<Language>) -> Option<String> {
    let raw_identifiers = collect_ast_identifiers(content, language);

    if raw_identifiers.is_empty() {
        return None;
    }

    let keywords = language_keywords(language);
    let mut result_set: BTreeSet<String> = BTreeSet::new();

    for ident in &raw_identifiers {
        let lower = ident.to_lowercase();

        // Filter out keywords, single-char, and noise
        if !should_include(&lower, &keywords) {
            continue;
        }

        // Add the original identifier (lowercased)
        result_set.insert(lower.clone());

        // Split camelCase/snake_case and add parts
        let parts = split_identifier(ident);
        for part in &parts {
            let part_lower = part.to_lowercase();
            if should_include(&part_lower, &keywords) {
                result_set.insert(part_lower);
            }
        }
    }

    if result_set.is_empty() {
        None
    } else {
        // BTreeSet is already sorted
        let joined: String = result_set.into_iter().collect::<Vec<_>>().join(" ");
        Some(joined)
    }
}

/// Check if a lowercased identifier should be included in the output
fn should_include(lower: &str, keywords: &HashSet<&str>) -> bool {
    // Must be at least 2 characters
    if lower.len() < 2 {
        return false;
    }

    // Filter language keywords
    if keywords.contains(lower) {
        return false;
    }

    // Filter common noise identifiers
    if NOISE_IDENTIFIERS.contains(&lower) {
        return false;
    }

    // Filter pure numeric strings
    if lower.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    true
}

/// Collect identifiers from AST nodes using Tree-sitter.
/// Falls back to regex-like splitting if parsing fails.
fn collect_ast_identifiers(content: &str, language: Option<Language>) -> Vec<String> {
    if let Some(lang) = language {
        if lang.has_parser_support() {
            if let Some(identifiers) = try_ast_extraction(content, lang) {
                return identifiers;
            }
        }
    }

    // Fallback: split on non-alphanumeric boundaries
    fallback_extraction(content)
}

/// Try to extract identifiers using Tree-sitter AST parsing
fn try_ast_extraction(content: &str, lang: Language) -> Option<Vec<String>> {
    let ts_lang = lang.tree_sitter_language()?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).ok()?;
    let tree = parser.parse(content, None)?;

    let mut identifiers = Vec::new();
    collect_identifiers_iterative(tree.root_node(), content.as_bytes(), &mut identifiers);

    Some(identifiers)
}

/// Walk the AST and collect identifier node text.
fn collect_identifiers_iterative(
    root: tree_sitter::Node<'_>,
    source: &[u8],
    identifiers: &mut Vec<String>,
) {
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        let kind = node.kind();

        // Collect nodes that represent identifiers
        if is_identifier_node(kind) {
            if let Ok(text) = node.utf8_text(source) {
                if !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    identifiers.push(text.to_owned());
                }
            }
        }

        let child_count = node.child_count();
        for i in (0..child_count).rev() {
            if let Some(child) = node.child(i as u32) {
                stack.push(child);
            }
        }
    }
}

/// Check if a Tree-sitter node kind represents an identifier
fn is_identifier_node(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "type_identifier"
            | "field_identifier"
            | "property_identifier"
            | "shorthand_property_identifier"
            | "shorthand_property_identifier_pattern"
            | "attribute_item"
            | "name"
            | "simple_identifier"
            | "word"
    )
}

/// Fallback identifier extraction using simple tokenization
fn fallback_extraction(content: &str) -> Vec<String> {
    content
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty() && s.len() >= 2)
        .filter(|s| s.chars().any(|c| c.is_alphabetic()))
        .map(|s| s.to_owned())
        .collect()
}

/// Common noise identifiers that should always be filtered out
const NOISE_IDENTIFIERS: &[&str] = &[
    "self",
    "this",
    "super",
    "none",
    "null",
    "true",
    "false",
    "undefined",
    "nil",
    "ok",
    "err",
    "some",
];

/// Get the set of language keywords for filtering
#[allow(deprecated)]
fn language_keywords(language: Option<Language>) -> HashSet<&'static str> {
    match language {
        Some(Language::Rust) => rust_keywords(),
        Some(Language::Python) => python_keywords(),
        Some(Language::JavaScript) => js_ts_keywords(),
        Some(Language::TypeScript) => js_ts_keywords(),
        Some(Language::Go) => go_keywords(),
        Some(Language::Java) => java_keywords(),
        Some(Language::C) => c_keywords(),
        Some(Language::Cpp) => cpp_keywords(),
        Some(Language::CSharp) => csharp_keywords(),
        Some(Language::Ruby) => ruby_keywords(),
        Some(Language::Kotlin) => kotlin_keywords(),
        Some(Language::Swift) => swift_keywords(),
        Some(Language::Php) => php_keywords(),
        Some(Language::Scala) => scala_keywords(),
        _ => generic_keywords(),
    }
}

fn rust_keywords() -> HashSet<&'static str> {
    [
        "fn",
        "let",
        "mut",
        "const",
        "pub",
        "use",
        "mod",
        "struct",
        "enum",
        "impl",
        "trait",
        "where",
        "for",
        "while",
        "loop",
        "if",
        "else",
        "match",
        "return",
        "self",
        "super",
        "crate",
        "as",
        "in",
        "ref",
        "move",
        "async",
        "await",
        "unsafe",
        "dyn",
        "type",
        "static",
        "extern",
        "true",
        "false",
        "break",
        "continue",
        "pub",
        "priv",
        "macro",
        "macro_rules",
    ]
    .into_iter()
    .collect()
}

fn python_keywords() -> HashSet<&'static str> {
    [
        "def", "class", "import", "from", "return", "if", "elif", "else", "for", "while", "with",
        "as", "try", "except", "finally", "raise", "pass", "break", "continue", "and", "or", "not",
        "is", "in", "lambda", "yield", "global", "nonlocal", "assert", "true", "false", "none",
        "self", "cls", "del", "print", "async", "await",
    ]
    .into_iter()
    .collect()
}

fn js_ts_keywords() -> HashSet<&'static str> {
    [
        "function",
        "const",
        "let",
        "var",
        "class",
        "extends",
        "implements",
        "import",
        "export",
        "from",
        "return",
        "if",
        "else",
        "for",
        "while",
        "do",
        "switch",
        "case",
        "break",
        "continue",
        "new",
        "this",
        "super",
        "typeof",
        "instanceof",
        "void",
        "null",
        "undefined",
        "true",
        "false",
        "async",
        "await",
        "try",
        "catch",
        "throw",
        "finally",
        "yield",
        "of",
        "in",
        "default",
        "delete",
        "interface",
        "type",
        "enum",
        "abstract",
        "static",
        "readonly",
        "private",
        "protected",
        "public",
        "declare",
        "module",
        "namespace",
        "require",
    ]
    .into_iter()
    .collect()
}

fn go_keywords() -> HashSet<&'static str> {
    [
        "func",
        "var",
        "const",
        "type",
        "struct",
        "interface",
        "package",
        "import",
        "return",
        "if",
        "else",
        "for",
        "range",
        "switch",
        "case",
        "break",
        "continue",
        "go",
        "defer",
        "select",
        "chan",
        "map",
        "nil",
        "true",
        "false",
        "default",
        "fallthrough",
        "goto",
    ]
    .into_iter()
    .collect()
}

fn java_keywords() -> HashSet<&'static str> {
    [
        "public",
        "private",
        "protected",
        "static",
        "final",
        "abstract",
        "class",
        "interface",
        "extends",
        "implements",
        "import",
        "package",
        "return",
        "if",
        "else",
        "for",
        "while",
        "do",
        "switch",
        "case",
        "break",
        "continue",
        "new",
        "this",
        "super",
        "void",
        "null",
        "true",
        "false",
        "try",
        "catch",
        "throw",
        "throws",
        "finally",
        "synchronized",
        "native",
        "volatile",
        "transient",
        "instanceof",
        "enum",
        "default",
    ]
    .into_iter()
    .collect()
}

fn c_keywords() -> HashSet<&'static str> {
    [
        "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
        "enum", "extern", "float", "for", "goto", "if", "int", "long", "register", "return",
        "short", "signed", "sizeof", "static", "struct", "switch", "typedef", "union", "unsigned",
        "void", "volatile", "while", "inline", "restrict", "bool", "true", "false", "null",
        "include", "define", "ifdef", "ifndef", "endif", "pragma",
    ]
    .into_iter()
    .collect()
}

fn cpp_keywords() -> HashSet<&'static str> {
    let mut kw = c_keywords();
    kw.extend([
        "class",
        "namespace",
        "template",
        "virtual",
        "override",
        "final",
        "public",
        "private",
        "protected",
        "new",
        "delete",
        "this",
        "throw",
        "try",
        "catch",
        "using",
        "friend",
        "operator",
        "dynamic_cast",
        "static_cast",
        "const_cast",
        "reinterpret_cast",
        "typeid",
        "typename",
        "explicit",
        "mutable",
        "nullptr",
        "constexpr",
        "decltype",
        "noexcept",
        "auto",
        "concept",
        "requires",
        "co_await",
        "co_yield",
        "co_return",
    ]);
    kw
}

fn csharp_keywords() -> HashSet<&'static str> {
    [
        "abstract",
        "as",
        "base",
        "bool",
        "break",
        "byte",
        "case",
        "catch",
        "char",
        "checked",
        "class",
        "const",
        "continue",
        "decimal",
        "default",
        "delegate",
        "do",
        "double",
        "else",
        "enum",
        "event",
        "explicit",
        "extern",
        "false",
        "finally",
        "fixed",
        "float",
        "for",
        "foreach",
        "goto",
        "if",
        "implicit",
        "in",
        "int",
        "interface",
        "internal",
        "is",
        "lock",
        "long",
        "namespace",
        "new",
        "null",
        "object",
        "operator",
        "out",
        "override",
        "params",
        "private",
        "protected",
        "public",
        "readonly",
        "ref",
        "return",
        "sbyte",
        "sealed",
        "short",
        "sizeof",
        "stackalloc",
        "static",
        "string",
        "struct",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "typeof",
        "uint",
        "ulong",
        "unchecked",
        "unsafe",
        "ushort",
        "using",
        "virtual",
        "void",
        "volatile",
        "while",
        "async",
        "await",
        "var",
    ]
    .into_iter()
    .collect()
}

fn ruby_keywords() -> HashSet<&'static str> {
    [
        "def",
        "class",
        "module",
        "if",
        "else",
        "elsif",
        "unless",
        "while",
        "until",
        "for",
        "do",
        "end",
        "return",
        "yield",
        "begin",
        "rescue",
        "ensure",
        "raise",
        "retry",
        "and",
        "or",
        "not",
        "in",
        "then",
        "when",
        "case",
        "break",
        "next",
        "redo",
        "self",
        "super",
        "true",
        "false",
        "nil",
        "require",
        "include",
        "extend",
        "attr",
        "attr_reader",
        "attr_writer",
        "attr_accessor",
        "puts",
        "print",
    ]
    .into_iter()
    .collect()
}

fn kotlin_keywords() -> HashSet<&'static str> {
    [
        "fun",
        "val",
        "var",
        "class",
        "object",
        "interface",
        "abstract",
        "override",
        "open",
        "sealed",
        "data",
        "enum",
        "companion",
        "import",
        "package",
        "return",
        "if",
        "else",
        "when",
        "for",
        "while",
        "do",
        "break",
        "continue",
        "this",
        "super",
        "null",
        "true",
        "false",
        "is",
        "as",
        "in",
        "try",
        "catch",
        "finally",
        "throw",
        "suspend",
        "lateinit",
        "by",
        "init",
        "constructor",
        "private",
        "protected",
        "public",
        "internal",
    ]
    .into_iter()
    .collect()
}

fn swift_keywords() -> HashSet<&'static str> {
    [
        "func",
        "var",
        "let",
        "class",
        "struct",
        "enum",
        "protocol",
        "extension",
        "import",
        "return",
        "if",
        "else",
        "guard",
        "switch",
        "case",
        "for",
        "while",
        "repeat",
        "break",
        "continue",
        "self",
        "super",
        "nil",
        "true",
        "false",
        "try",
        "catch",
        "throw",
        "throws",
        "rethrows",
        "defer",
        "in",
        "as",
        "is",
        "init",
        "deinit",
        "subscript",
        "static",
        "override",
        "final",
        "open",
        "public",
        "private",
        "fileprivate",
        "internal",
        "mutating",
        "nonmutating",
        "inout",
        "weak",
        "unowned",
        "lazy",
        "optional",
        "required",
        "convenience",
        "typealias",
        "associatedtype",
        "where",
        "async",
        "await",
    ]
    .into_iter()
    .collect()
}

fn php_keywords() -> HashSet<&'static str> {
    [
        "function",
        "class",
        "interface",
        "trait",
        "extends",
        "implements",
        "abstract",
        "final",
        "public",
        "private",
        "protected",
        "static",
        "const",
        "var",
        "new",
        "return",
        "if",
        "else",
        "elseif",
        "while",
        "for",
        "foreach",
        "do",
        "switch",
        "case",
        "break",
        "continue",
        "default",
        "try",
        "catch",
        "finally",
        "throw",
        "use",
        "namespace",
        "require",
        "include",
        "echo",
        "print",
        "true",
        "false",
        "null",
        "self",
        "parent",
        "this",
        "array",
        "list",
        "yield",
        "match",
        "enum",
        "readonly",
        "fn",
    ]
    .into_iter()
    .collect()
}

fn scala_keywords() -> HashSet<&'static str> {
    [
        "def",
        "val",
        "var",
        "class",
        "object",
        "trait",
        "extends",
        "with",
        "abstract",
        "sealed",
        "final",
        "override",
        "implicit",
        "lazy",
        "import",
        "package",
        "return",
        "if",
        "else",
        "match",
        "case",
        "for",
        "while",
        "do",
        "try",
        "catch",
        "finally",
        "throw",
        "new",
        "this",
        "super",
        "true",
        "false",
        "null",
        "type",
        "yield",
        "forSome",
        "private",
        "protected",
        "given",
        "using",
        "enum",
    ]
    .into_iter()
    .collect()
}

fn generic_keywords() -> HashSet<&'static str> {
    [
        "fn",
        "function",
        "def",
        "fun",
        "func",
        "let",
        "var",
        "val",
        "const",
        "mut",
        "pub",
        "public",
        "private",
        "protected",
        "static",
        "class",
        "struct",
        "enum",
        "trait",
        "interface",
        "impl",
        "use",
        "import",
        "from",
        "export",
        "module",
        "package",
        "return",
        "if",
        "else",
        "elif",
        "elseif",
        "for",
        "while",
        "do",
        "loop",
        "match",
        "switch",
        "case",
        "break",
        "continue",
        "try",
        "catch",
        "except",
        "finally",
        "throw",
        "raise",
        "new",
        "this",
        "self",
        "super",
        "true",
        "false",
        "null",
        "nil",
        "none",
        "void",
        "async",
        "await",
        "yield",
        "in",
        "of",
        "as",
        "is",
        "not",
        "and",
        "or",
        "type",
        "where",
        "with",
        "default",
    ]
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_function_identifiers() {
        let code = r#"fn calculate_total(items: Vec<Item>, tax_rate: f64) -> f64 {
    let subtotal = items.iter().map(|item| item.price).sum::<f64>();
    let tax = subtotal * tax_rate;
    subtotal + tax
}"#;
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();

        // Should contain meaningful identifiers
        assert!(ids.contains("calculate"), "Should contain 'calculate' from split");
        assert!(ids.contains("total"), "Should contain 'total' from split");
        assert!(ids.contains("items"), "Should contain 'items'");
        assert!(ids.contains("subtotal"), "Should contain 'subtotal'");
        assert!(ids.contains("price"), "Should contain 'price'");
        assert!(ids.contains("tax"), "Should contain 'tax'");

        // Should NOT contain keywords
        let words: Vec<&str> = ids.split_whitespace().collect();
        assert!(!words.contains(&"fn"), "Should not contain keyword 'fn'");
        assert!(!words.contains(&"let"), "Should not contain keyword 'let'");
    }

    #[test]
    fn test_deep_rust_ast_identifier_walks_iteratively() {
        let depth = 1200;
        let mut code = String::from("fn deep() {\n");
        for _ in 0..depth {
            code.push_str("{\n");
        }
        code.push_str("let deeply_nested_identifier = 1;\n");
        for _ in 0..depth {
            code.push_str("}\n");
        }
        code.push_str("}\n");

        let result = extract_identifiers(&code, Some(Language::Rust)).unwrap();
        assert!(result.contains("deeply_nested_identifier"));
        assert!(result.contains("nested"));
        assert!(result.contains("identifier"));
    }

    #[test]
    fn test_camel_case_splitting() {
        let code = "fn test() { let x = getUserProfile(); }";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();

        assert!(ids.contains("getuserprofile"), "Should contain original lowercased");
        assert!(ids.contains("get"), "Should contain 'get' from split");
        assert!(ids.contains("user"), "Should contain 'user' from split");
        assert!(ids.contains("profile"), "Should contain 'profile' from split");
    }

    #[test]
    fn test_snake_case_splitting() {
        let code = "fn test() { let val = get_user_profile(); }";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();

        assert!(ids.contains("get"), "Should contain 'get' from snake_case split");
        assert!(ids.contains("user"), "Should contain 'user' from snake_case split");
        assert!(ids.contains("profile"), "Should contain 'profile' from snake_case split");
    }

    #[test]
    fn test_http_client_splitting() {
        let code = "fn test() { let client = HTTPClient::new(); }";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();

        assert!(ids.contains("http"), "Should contain 'http' from acronym split");
        assert!(ids.contains("client"), "Should contain 'client' from acronym split");
    }

    #[test]
    fn test_keyword_filtering() {
        let code = "fn main() { let x = if true { 1 } else { 2 }; }";
        let result = extract_identifiers(code, Some(Language::Rust));
        // "main" should be present, keywords should not
        if let Some(ids) = &result {
            assert!(ids.contains("main"), "Should contain 'main'");
            // Keywords should not appear as standalone words
            let words: Vec<&str> = ids.split_whitespace().collect();
            assert!(!words.contains(&"fn"), "Should not contain keyword 'fn'");
            assert!(!words.contains(&"let"), "Should not contain keyword 'let'");
            assert!(!words.contains(&"if"), "Should not contain keyword 'if'");
            assert!(!words.contains(&"else"), "Should not contain keyword 'else'");
            assert!(!words.contains(&"true"), "Should not contain keyword 'true'");
        }
    }

    #[test]
    fn test_single_char_filtering() {
        let code = "fn test() { let x = 1; let y = 2; let ab = x + y; }";
        let result = extract_identifiers(code, Some(Language::Rust));
        if let Some(ids) = &result {
            let words: Vec<&str> = ids.split_whitespace().collect();
            assert!(!words.contains(&"x"), "Should not contain single-char 'x'");
            assert!(!words.contains(&"y"), "Should not contain single-char 'y'");
            assert!(words.contains(&"ab"), "Should contain two-char 'ab'");
        }
    }

    #[test]
    fn test_deduplication() {
        let code = r#"fn test() {
    let user = get_user();
    let user_name = user.name;
    let user_email = user.email;
}"#;
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();
        let words: Vec<&str> = ids.split_whitespace().collect();

        // Check no duplicates
        let unique: BTreeSet<&str> = words.iter().copied().collect();
        assert_eq!(words.len(), unique.len(), "Should have no duplicate identifiers");
    }

    #[test]
    fn test_deterministic_sorted_output() {
        let code = "fn test() { let zebra = 1; let alpha = 2; let middle = 3; }";
        let result1 = extract_identifiers(code, Some(Language::Rust));
        let result2 = extract_identifiers(code, Some(Language::Rust));

        assert_eq!(result1, result2, "Should be deterministic");

        if let Some(ids) = &result1 {
            let words: Vec<&str> = ids.split_whitespace().collect();
            let mut sorted = words.clone();
            sorted.sort();
            assert_eq!(words, sorted, "Output should be alphabetically sorted");
        }
    }

    #[test]
    fn test_python_identifiers() {
        let code = r#"def process_data(input_data):
    result = transform(input_data)
    return result
"#;
        let result = extract_identifiers(code, Some(Language::Python));
        assert!(result.is_some());
        let ids = result.unwrap();

        assert!(ids.contains("process"), "Should contain 'process' from split");
        assert!(ids.contains("data"), "Should contain 'data' from split");
        assert!(ids.contains("transform"), "Should contain 'transform'");
        assert!(ids.contains("result"), "Should contain 'result'");

        let words: Vec<&str> = ids.split_whitespace().collect();
        assert!(!words.contains(&"def"), "Should not contain keyword 'def'");
        assert!(!words.contains(&"return"), "Should not contain keyword 'return'");
    }

    #[test]
    fn test_noise_identifiers_filtered() {
        let code = "fn test() { let val = self.data; let x = None; }";
        let result = extract_identifiers(code, Some(Language::Rust));
        if let Some(ids) = &result {
            let words: Vec<&str> = ids.split_whitespace().collect();
            assert!(!words.contains(&"self"), "Should not contain noise 'self'");
            assert!(!words.contains(&"none"), "Should not contain noise 'none'");
        }
    }

    #[test]
    fn test_empty_content() {
        let result = extract_identifiers("", Some(Language::Rust));
        assert!(result.is_none(), "Empty content should return None");
    }

    #[test]
    fn test_no_language_fallback() {
        let code = "function getData() { return fetchUserProfile(); }";
        let result = extract_identifiers(code, None);
        assert!(result.is_some(), "Should work without language via fallback");
        let ids = result.unwrap();
        assert!(ids.contains("getdata") || ids.contains("get"), "Should extract identifiers");
    }

    #[test]
    fn test_go_identifiers() {
        let code = r#"func handleRequest(w http.ResponseWriter, r *http.Request) {
    userID := r.URL.Query().Get("id")
    user := findUserByID(userID)
}"#;
        let result = extract_identifiers(code, Some(Language::Go));
        assert!(result.is_some());
        let ids = result.unwrap();

        let words: Vec<&str> = ids.split_whitespace().collect();
        assert!(!words.contains(&"func"), "Should not contain Go keyword 'func'");
    }

    // ===== should_include tests =====

    #[test]
    fn test_should_include_rejects_single_char() {
        let kw = HashSet::new();
        assert!(!should_include("a", &kw));
        assert!(!should_include("x", &kw));
        assert!(!should_include("_", &kw));
    }

    #[test]
    fn test_should_include_accepts_two_char() {
        let kw = HashSet::new();
        assert!(should_include("ab", &kw));
        assert!(should_include("id", &kw));
    }

    #[test]
    fn test_should_include_rejects_keywords() {
        let mut kw = HashSet::new();
        kw.insert("fn");
        kw.insert("let");
        assert!(!should_include("fn", &kw));
        assert!(!should_include("let", &kw));
        assert!(should_include("foo", &kw));
    }

    #[test]
    fn test_should_include_rejects_noise() {
        let kw = HashSet::new();
        for noise in NOISE_IDENTIFIERS {
            assert!(!should_include(noise, &kw), "Noise identifier '{}' should be excluded", noise);
        }
    }

    #[test]
    fn test_should_include_rejects_pure_numeric() {
        let kw = HashSet::new();
        assert!(!should_include("42", &kw));
        assert!(!should_include("123456", &kw));
        assert!(!should_include("00", &kw));
    }

    #[test]
    fn test_should_include_accepts_alphanumeric_mix() {
        let kw = HashSet::new();
        assert!(should_include("x2", &kw));
        assert!(should_include("2fast", &kw));
        assert!(should_include("item1", &kw));
    }

    // ===== is_identifier_node tests =====

    #[test]
    fn test_is_identifier_node_known_kinds() {
        assert!(is_identifier_node("identifier"));
        assert!(is_identifier_node("type_identifier"));
        assert!(is_identifier_node("field_identifier"));
        assert!(is_identifier_node("property_identifier"));
        assert!(is_identifier_node("shorthand_property_identifier"));
        assert!(is_identifier_node("shorthand_property_identifier_pattern"));
        assert!(is_identifier_node("attribute_item"));
        assert!(is_identifier_node("name"));
        assert!(is_identifier_node("simple_identifier"));
        assert!(is_identifier_node("word"));
    }

    #[test]
    fn test_is_identifier_node_rejects_non_identifier_kinds() {
        assert!(!is_identifier_node("string_literal"));
        assert!(!is_identifier_node("integer_literal"));
        assert!(!is_identifier_node("comment"));
        assert!(!is_identifier_node("binary_expression"));
        assert!(!is_identifier_node("function_declaration"));
        assert!(!is_identifier_node(""));
    }

    // ===== fallback_extraction tests =====

    #[test]
    fn test_fallback_extraction_basic() {
        // Fallback splits on chars that are NOT alphanumeric and NOT underscore.
        // So "get_user_name" stays as one token (underscores are kept).
        let result = fallback_extraction("get_user_name = 42");
        assert!(
            result.contains(&"get_user_name".to_string()),
            "Underscored identifier should be kept intact: {:?}",
            result
        );
        // Test splitting on spaces/operators
        let result2 = fallback_extraction("hello world");
        assert!(result2.contains(&"hello".to_string()));
        assert!(result2.contains(&"world".to_string()));
    }

    #[test]
    fn test_fallback_extraction_filters_single_char() {
        let result = fallback_extraction("a + b = c");
        assert!(result.is_empty(), "Single-char tokens should be filtered");
    }

    #[test]
    fn test_fallback_extraction_filters_pure_numbers() {
        let result = fallback_extraction("42 + 100 = 142");
        assert!(result.is_empty(), "Pure numeric tokens should be filtered (no alphabetic chars)");
    }

    #[test]
    fn test_fallback_extraction_empty_input() {
        let result = fallback_extraction("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_fallback_extraction_only_symbols() {
        let result = fallback_extraction("!@#$%^&*(){}[]");
        assert!(result.is_empty());
    }

    #[test]
    fn test_fallback_extraction_preserves_underscored_identifiers() {
        let result = fallback_extraction("__init__ some_func");
        // "__init__" splits on non-alnum boundaries but underscores are kept
        // The split is on chars that are NOT alphanumeric and NOT underscore
        assert!(result.contains(&"__init__".to_string()));
        assert!(result.contains(&"some_func".to_string()));
    }

    // ===== extract_identifiers edge cases =====

    #[test]
    fn test_extract_identifiers_empty_string() {
        assert_eq!(extract_identifiers("", Some(Language::Rust)), None);
        assert_eq!(extract_identifiers("", None), None);
    }

    #[test]
    fn test_extract_identifiers_only_keywords() {
        // Content that only contains keywords and single-char identifiers
        let code = "fn f() { if x { } else { } }";
        let result = extract_identifiers(code, Some(Language::Rust));
        // "f" and "x" are single-char, all others are keywords
        // Result may be None or only contain very limited identifiers
        if let Some(ids) = result {
            let words: Vec<&str> = ids.split_whitespace().collect();
            assert!(!words.contains(&"fn"), "Should not contain keyword 'fn'");
            assert!(!words.contains(&"if"), "Should not contain keyword 'if'");
            assert!(!words.contains(&"else"), "Should not contain keyword 'else'");
        }
    }

    #[test]
    fn test_extract_identifiers_screaming_snake_case() {
        let code = "fn test() { let val = MAX_RETRY_COUNT; }";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();

        assert!(ids.contains("max"), "Should split SCREAMING_SNAKE into 'max'");
        assert!(ids.contains("retry"), "Should split SCREAMING_SNAKE into 'retry'");
        assert!(ids.contains("count"), "Should split SCREAMING_SNAKE into 'count'");
    }

    #[test]
    fn test_extract_identifiers_pascal_case() {
        let code = "fn test() { let val = UserProfile::new(); }";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();

        assert!(ids.contains("userprofile"), "Should contain lowercased original");
        assert!(ids.contains("user"), "Should split PascalCase into 'user'");
        assert!(ids.contains("profile"), "Should split PascalCase into 'profile'");
    }

    #[test]
    fn test_extract_identifiers_mixed_case_with_numbers() {
        let code = "fn test() { let val = getItem2Name(); }";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();

        // Should contain some split parts
        assert!(ids.contains("get"), "Should split 'get' from camelCase");
    }

    #[test]
    fn test_extract_identifiers_leading_underscores() {
        let code = "fn test() { let _private_var = 1; let __dunder = 2; }";
        let result = extract_identifiers(code, Some(Language::Rust));
        // Leading underscores should not prevent extraction
        // The identifier characters include underscores and alphanums
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_identifiers_very_long_identifier() {
        let long_name = "a".repeat(200);
        let code = format!("fn test() {{ let {} = 1; }}", long_name);
        let result = extract_identifiers(&code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();
        assert!(ids.contains(&long_name), "Should handle very long identifiers");
    }

    #[test]
    fn test_extract_identifiers_only_comments() {
        let code = "// this is a comment\n/* block comment */";
        let result = extract_identifiers(code, Some(Language::Rust));
        // Comments should not produce identifier nodes in the AST
        // Result may be None since no actual code identifiers exist
        // (The fallback might pick up words, but AST parsing should filter them)
        if let Some(ids) = result {
            // If anything is found, it should still be valid identifiers
            for word in ids.split_whitespace() {
                assert!(word.len() >= 2, "All words should be at least 2 chars");
            }
        }
    }

    #[test]
    fn test_extract_identifiers_binary_like_data() {
        // Content with non-UTF8-friendly but still valid UTF-8 chars
        let code = "\x00\x01\x02\x03";
        let result = extract_identifiers(code, Some(Language::Rust));
        // Should handle gracefully, likely returning None
        // Binary data won't have valid identifiers
        if let Some(ids) = result {
            assert!(!ids.is_empty());
        }
    }

    #[test]
    fn test_extract_identifiers_whitespace_only() {
        let code = "   \n\t\n   ";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_none(), "Whitespace-only should return None");
    }

    #[test]
    fn test_extract_identifiers_numbers_only_code() {
        // Code with only numeric literals
        let code = "123 + 456 * 789";
        let result = extract_identifiers(code, None);
        assert!(result.is_none(), "Numeric-only content should return None");
    }

    #[test]
    fn test_extract_identifiers_no_language_uses_fallback() {
        let code = "calculateTotalPrice get_user_name MAX_VALUE";
        let result = extract_identifiers(code, None);
        assert!(result.is_some());
        let ids = result.unwrap();

        // Fallback splits on non-alphanumeric boundaries (underscore kept)
        // "calculateTotalPrice" kept as-is, then split_identifier splits it
        assert!(
            ids.contains("calculate") || ids.contains("calculatetotalprice"),
            "Should extract from fallback"
        );
    }

    #[test]
    fn test_extract_identifiers_javascript() {
        let code = r#"function fetchUserData(userId) {
    const response = await fetch(apiUrl);
    return response.json();
}"#;
        let result = extract_identifiers(code, Some(Language::JavaScript));
        assert!(result.is_some());
        let ids = result.unwrap();

        let words: Vec<&str> = ids.split_whitespace().collect();
        assert!(!words.contains(&"function"), "JS keyword 'function' should be filtered");
        assert!(!words.contains(&"const"), "JS keyword 'const' should be filtered");
        assert!(!words.contains(&"return"), "JS keyword 'return' should be filtered");
        assert!(!words.contains(&"await"), "JS keyword 'await' should be filtered");
        assert!(ids.contains("fetch"), "Should contain 'fetch'");
    }

    #[test]
    fn test_extract_identifiers_typescript_filters() {
        let code = "const val: string = getUserProfile();";
        let result = extract_identifiers(code, Some(Language::TypeScript));
        assert!(result.is_some());
        let ids = result.unwrap();

        let words: Vec<&str> = ids.split_whitespace().collect();
        // TypeScript uses js_ts_keywords
        assert!(!words.contains(&"const"), "TS keyword 'const' should be filtered");
    }

    // ===== language_keywords coverage =====

    #[test]
    fn test_language_keywords_java() {
        let kw = language_keywords(Some(Language::Java));
        assert!(kw.contains("class"));
        assert!(kw.contains("public"));
        assert!(kw.contains("synchronized"));
        assert!(!kw.contains("fn")); // Rust keyword, not Java
    }

    #[test]
    fn test_language_keywords_c() {
        let kw = language_keywords(Some(Language::C));
        assert!(kw.contains("int"));
        assert!(kw.contains("struct"));
        assert!(kw.contains("typedef"));
    }

    #[test]
    fn test_language_keywords_cpp_extends_c() {
        let kw = language_keywords(Some(Language::Cpp));
        // C++ should have C keywords plus its own
        assert!(kw.contains("int")); // from C
        assert!(kw.contains("class")); // C++ specific
        assert!(kw.contains("template")); // C++ specific
        assert!(kw.contains("nullptr")); // C++ specific
    }

    #[test]
    fn test_language_keywords_csharp() {
        let kw = language_keywords(Some(Language::CSharp));
        assert!(kw.contains("namespace"));
        assert!(kw.contains("sealed"));
        assert!(kw.contains("delegate"));
    }

    #[test]
    fn test_language_keywords_ruby() {
        let kw = language_keywords(Some(Language::Ruby));
        assert!(kw.contains("def"));
        assert!(kw.contains("end"));
        assert!(kw.contains("attr_accessor"));
    }

    #[test]
    fn test_language_keywords_kotlin() {
        let kw = language_keywords(Some(Language::Kotlin));
        assert!(kw.contains("fun"));
        assert!(kw.contains("suspend"));
        assert!(kw.contains("lateinit"));
    }

    #[test]
    fn test_language_keywords_swift() {
        let kw = language_keywords(Some(Language::Swift));
        assert!(kw.contains("func"));
        assert!(kw.contains("guard"));
        assert!(kw.contains("fileprivate"));
    }

    #[test]
    fn test_language_keywords_php() {
        let kw = language_keywords(Some(Language::Php));
        assert!(kw.contains("echo"));
        assert!(kw.contains("trait"));
        assert!(kw.contains("readonly"));
    }

    #[test]
    fn test_language_keywords_scala() {
        let kw = language_keywords(Some(Language::Scala));
        assert!(kw.contains("object"));
        assert!(kw.contains("implicit"));
        assert!(kw.contains("given"));
    }

    #[test]
    fn test_language_keywords_generic_for_unknown() {
        // Languages without specific keyword list should use generic
        let kw = language_keywords(None);
        assert!(kw.contains("function"));
        assert!(kw.contains("class"));
        assert!(kw.contains("return"));
    }

    // ===== collect_ast_identifiers tests =====

    #[test]
    fn test_collect_ast_identifiers_with_parser_support() {
        let code = "fn hello() { let world = 42; }";
        let result = collect_ast_identifiers(code, Some(Language::Rust));
        // Should use AST extraction since Rust has parser support
        assert!(result.iter().any(|s| s == "hello"));
        assert!(result.iter().any(|s| s == "world"));
    }

    #[test]
    fn test_collect_ast_identifiers_none_language_uses_fallback() {
        let code = "hello world foo_bar";
        let result = collect_ast_identifiers(code, None);
        // Should use fallback extraction
        assert!(!result.is_empty());
        assert!(result.iter().any(|s| s == "hello"));
        assert!(result.iter().any(|s| s == "world"));
        assert!(result.iter().any(|s| s == "foo_bar"));
    }

    // ===== Deduplication and sorting =====

    #[test]
    fn test_extract_identifiers_result_is_sorted() {
        let code = "fn test() { let zebra = 1; let apple = 2; let mango = 3; }";
        let result = extract_identifiers(code, Some(Language::Rust)).unwrap();
        let words: Vec<&str> = result.split_whitespace().collect();
        let mut sorted = words.clone();
        sorted.sort();
        assert_eq!(words, sorted, "Output must be alphabetically sorted");
    }

    #[test]
    fn test_extract_identifiers_no_duplicates_from_split() {
        // "user" appears both as a standalone identifier and as a split part of "user_name"
        let code = "fn test() { let user = get_user(); let user_name = user.name; }";
        let result = extract_identifiers(code, Some(Language::Rust)).unwrap();
        let words: Vec<&str> = result.split_whitespace().collect();
        let unique: BTreeSet<&str> = words.iter().copied().collect();
        assert_eq!(words.len(), unique.len(), "No duplicates allowed");
    }

    // ===== Noise identifier edge cases =====

    #[test]
    fn test_all_noise_identifiers_filtered_via_extract() {
        // Build code that uses all noise identifiers as variable names
        let code = r#"fn test() {
    let self_val = self.data;
    let this_val = this.data;
    let n = None;
    let nu = null;
    let t = true;
    let f = false;
    let u = undefined;
    let ni = nil;
    let o = ok;
    let e = err;
    let s = some;
}"#;
        let result = extract_identifiers(code, Some(Language::Rust));
        if let Some(ids) = result {
            let words: Vec<&str> = ids.split_whitespace().collect();
            // The noise words themselves should not appear as standalone words
            // (but compound words like "self_val" might produce "self" as a split part,
            // which would then be filtered by should_include)
            for noise in NOISE_IDENTIFIERS {
                if noise.len() >= 2 {
                    // "ok" has len 2, should still be filtered as noise
                    assert!(
                        !words.contains(noise),
                        "Noise identifier '{}' should be filtered",
                        noise
                    );
                }
            }
        }
    }

    // ===== Multi-language AST extraction =====

    #[test]
    fn test_python_keyword_filtering() {
        let code = r#"
class MyProcessor:
    def process_items(self, input_list):
        for item in input_list:
            if item.is_valid:
                yield item
"#;
        let result = extract_identifiers(code, Some(Language::Python));
        assert!(result.is_some());
        let ids = result.unwrap();
        let words: Vec<&str> = ids.split_whitespace().collect();

        assert!(!words.contains(&"def"), "Python 'def' should be filtered");
        assert!(!words.contains(&"class"), "Python 'class' should be filtered");
        assert!(!words.contains(&"for"), "Python 'for' should be filtered");
        assert!(!words.contains(&"yield"), "Python 'yield' should be filtered");
        assert!(ids.contains("process"), "Should contain split part 'process'");
        assert!(ids.contains("items"), "Should contain split part 'items'");
    }

    #[test]
    fn test_go_keyword_filtering_comprehensive() {
        let code = r#"
func processData(input []byte) ([]byte, error) {
    result := transform(input)
    return result, nil
}
"#;
        let result = extract_identifiers(code, Some(Language::Go));
        assert!(result.is_some());
        let ids = result.unwrap();
        let words: Vec<&str> = ids.split_whitespace().collect();

        assert!(!words.contains(&"func"), "Go 'func' should be filtered");
        assert!(!words.contains(&"return"), "Go 'return' should be filtered");
        assert!(
            ids.contains("processdata") || ids.contains("process"),
            "Should contain identifier from processData"
        );
    }

    // ===== Acronym handling in camelCase =====

    #[test]
    fn test_xml_http_request_splitting() {
        let code = "fn test() { let req = XMLHttpRequest::new(); }";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();

        // split_identifier should handle "XMLHttpRequest" -> ["XML", "Http", "Request"]
        assert!(ids.contains("xml"), "Should contain 'xml' from acronym split");
        assert!(ids.contains("http"), "Should contain 'http' from acronym split");
        assert!(ids.contains("request"), "Should contain 'request' from acronym split");
    }

    #[test]
    fn test_all_caps_identifier() {
        let code = "fn test() { let val = DATABASE; }";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();
        assert!(ids.contains("database"), "Should contain lowercased all-caps identifier");
    }

    // ===== Edge cases for content that produces empty results =====

    #[test]
    fn test_extract_identifiers_special_chars_only() {
        let code = "!@#$%^&*()+={}[]|\\:;'\"<>,.?/~`";
        let result = extract_identifiers(code, None);
        assert!(result.is_none(), "Special characters only should return None");
    }

    #[test]
    fn test_extract_identifiers_unicode_identifiers() {
        // Unicode characters that are not alphanumeric+underscore should be filtered
        // by the AST identifier filter
        let code = "fn test() { let cafe = 1; }";
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();
        assert!(ids.contains("cafe"), "Should handle ASCII identifiers fine");
    }

    #[test]
    fn test_extract_identifiers_syntax_error_code() {
        // Invalid Rust syntax - parser should still extract some identifiers
        // or fallback should handle it
        let code = "fn {{ broken syntax let foo = bar; }}}}";
        let result = extract_identifiers(code, Some(Language::Rust));
        // Even with syntax errors, tree-sitter can often extract partial results
        // The key thing is it should not panic
        if let Some(ids) = result {
            // If identifiers were found, verify basic properties
            for word in ids.split_whitespace() {
                assert!(word.len() >= 2);
            }
        }
    }

    #[test]
    fn test_extract_identifiers_multiline_complex() {
        let code = r#"
fn calculate_shipping_cost(
    order_total: f64,
    shipping_method: ShippingMethod,
    destination_country: Country,
) -> Result<f64, ShippingError> {
    let base_rate = get_base_rate(shipping_method);
    let country_multiplier = get_country_multiplier(destination_country);
    let total_cost = base_rate * country_multiplier;
    Ok(total_cost)
}
"#;
        let result = extract_identifiers(code, Some(Language::Rust));
        assert!(result.is_some());
        let ids = result.unwrap();

        // Verify split parts from snake_case
        assert!(ids.contains("calculate"), "Should split 'calculate'");
        assert!(ids.contains("shipping"), "Should split 'shipping'");
        assert!(ids.contains("cost"), "Should split 'cost'");
        assert!(ids.contains("order"), "Should split 'order'");
        assert!(ids.contains("destination"), "Should split 'destination'");
        assert!(ids.contains("country"), "Should split 'country'");
        assert!(ids.contains("base"), "Should split 'base'");
        assert!(ids.contains("rate"), "Should split 'rate'");
        assert!(ids.contains("multiplier"), "Should split 'multiplier'");

        // Verify compound forms are present too
        assert!(
            ids.contains("calculate_shipping_cost")
                || ids.contains("base_rate")
                || ids.contains("total_cost"),
            "Should contain some compound identifiers"
        );
    }
}
