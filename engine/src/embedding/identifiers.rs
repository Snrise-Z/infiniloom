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
    let mut cursor = tree.walk();
    collect_identifiers_recursive(&mut cursor, content.as_bytes(), &mut identifiers);

    Some(identifiers)
}

/// Recursively walk the AST and collect identifier node text
fn collect_identifiers_recursive(
    cursor: &mut tree_sitter::TreeCursor,
    source: &[u8],
    identifiers: &mut Vec<String>,
) {
    let node = cursor.node();
    let kind = node.kind();

    // Collect nodes that represent identifiers
    if is_identifier_node(kind) {
        if let Ok(text) = node.utf8_text(source) {
            if !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_') {
                identifiers.push(text.to_owned());
            }
        }
    }

    // Recurse into children
    if cursor.goto_first_child() {
        loop {
            collect_identifiers_recursive(cursor, source, identifiers);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
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
}
