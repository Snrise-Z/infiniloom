//! Cyclomatic complexity scoring for embedding chunks
//!
//! Computes cyclomatic complexity by counting branch-point AST node types
//! using Tree-sitter. The formula is: `complexity = 1 + count(branch_nodes)`.
//!
//! Supports Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#,
//! Ruby, Kotlin, Swift, PHP, and Dart. Returns `None` for unsupported languages.

use crate::parser::Language;

/// Compute the cyclomatic complexity score for a chunk of source code.
///
/// Returns `Some(score)` where score >= 1 for supported languages,
/// or `None` if the language is unsupported or parsing fails.
///
/// The score is `1 + count(branch_nodes)` where branch nodes are
/// language-specific AST nodes representing control flow decisions.
pub fn compute_complexity(content: &str, language: Language) -> Option<u32> {
    let ts_lang = language.tree_sitter_language()?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang).ok()?;

    let tree = parser.parse(content, None)?;
    let root = tree.root_node();

    let branch_types = branch_node_types(language)?;
    let logical_ops = logical_operator_types(language);

    let mut count = 0u32;
    count_branch_nodes(root, content.as_bytes(), &branch_types, &logical_ops, &mut count);

    Some(1 + count)
}

/// Recursively walk the AST and count branch-point nodes.
fn count_branch_nodes(
    node: tree_sitter::Node,
    source: &[u8],
    branch_types: &[&str],
    logical_ops: &LogicalOps,
    count: &mut u32,
) {
    let kind = node.kind();

    if branch_types.contains(&kind) {
        *count += 1;
    } else if is_logical_operator_node(node, source, kind, logical_ops) {
        *count += 1;
    }

    let child_count = node.child_count();
    for i in 0..child_count {
        if let Some(child) = node.child(i as u32) {
            count_branch_nodes(child, source, branch_types, logical_ops, count);
        }
    }
}

/// Language-specific configuration for logical operator detection.
struct LogicalOps {
    /// The AST node kind that may contain a logical operator (e.g., "binary_expression")
    binary_node_kind: &'static str,
    /// The operators that count as branch points
    operators: &'static [&'static str],
}

/// Check if a node represents a logical operator (&&, ||, `and`, `or`).
fn is_logical_operator_node(
    node: tree_sitter::Node,
    source: &[u8],
    kind: &str,
    ops: &LogicalOps,
) -> bool {
    if kind != ops.binary_node_kind {
        return false;
    }

    // Look for the operator child node
    let child_count = node.child_count();
    for i in 0..child_count {
        if let Some(child) = node.child(i as u32) {
            // Check if this child is an operator token
            let child_kind = child.kind();
            if ops.operators.contains(&child_kind) {
                return true;
            }
            // Check the text content of the operator node
            if let Ok(text) = child.utf8_text(source) {
                if ops.operators.contains(&text) {
                    return true;
                }
            }
        }
    }
    false
}

/// Return the list of branch-point AST node types for a given language.
///
/// These are control flow constructs that increase cyclomatic complexity.
/// Returns `None` for unsupported languages.
#[allow(deprecated)]
fn branch_node_types(language: Language) -> Option<Vec<&'static str>> {
    let types: Vec<&str> = match language {
        Language::Rust => vec![
            "if_expression",
            "else_clause",
            "match_arm",
            "for_expression",
            "while_expression",
            "loop_expression",
        ],
        Language::Python => vec![
            "if_statement",
            "elif_clause",
            "for_statement",
            "while_statement",
            "except_clause",
            "conditional_expression",
        ],
        Language::JavaScript | Language::TypeScript => vec![
            "if_statement",
            "else_clause",
            "switch_case",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "ternary_expression",
            "catch_clause",
        ],
        Language::Go => vec!["if_statement", "expression_case", "for_statement"],
        Language::Java => vec![
            "if_statement",
            "switch_block_statement_group",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
            "catch_clause",
            "ternary_expression",
        ],
        Language::C | Language::Cpp => vec![
            "if_statement",
            "else_clause",
            "case_statement",
            "for_statement",
            "while_statement",
            "do_statement",
            "conditional_expression",
        ],
        Language::CSharp => vec![
            "if_statement",
            "else_clause",
            "switch_section",
            "for_statement",
            "for_each_statement",
            "while_statement",
            "do_statement",
            "catch_clause",
            "conditional_expression",
        ],
        Language::Ruby => {
            vec!["if", "elsif", "unless", "while", "until", "for", "when", "rescue", "conditional"]
        },
        Language::Php => vec![
            "if_statement",
            "else_clause",
            "case_statement",
            "for_statement",
            "foreach_statement",
            "while_statement",
            "do_statement",
            "catch_clause",
        ],
        Language::Kotlin => vec![
            "if_expression",
            "when_entry",
            "for_statement",
            "while_statement",
            "do_while_statement",
            "catch_block",
        ],
        Language::Swift => vec![
            "if_statement",
            "guard_statement",
            "switch_case",
            "for_in_statement",
            "while_statement",
            "repeat_while_statement",
            "catch_clause",
        ],
        Language::Dart => vec![
            "if_statement",
            "else_clause",
            "switch_case",
            "for_statement",
            "while_statement",
            "do_statement",
            "catch_clause",
            "conditional_expression",
        ],
        _ => return None,
    };
    Some(types)
}

/// Return logical operator detection configuration for a given language.
fn logical_operator_types(language: Language) -> LogicalOps {
    match language {
        Language::Python => {
            LogicalOps { binary_node_kind: "boolean_operator", operators: &["and", "or"] }
        },
        Language::Ruby => {
            LogicalOps { binary_node_kind: "binary", operators: &["&&", "||", "and", "or"] }
        },
        _ => LogicalOps { binary_node_kind: "binary_expression", operators: &["&&", "||"] },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_function_rust() {
        let code = r#"
fn add(a: i32, b: i32) -> i32 {
    let result = a + b;
    result
}
"#;
        let score = compute_complexity(code, Language::Rust).unwrap();
        assert_eq!(score, 1, "Linear function should have complexity 1");
    }

    #[test]
    fn test_single_if_rust() {
        let code = r#"
fn check(x: i32) -> bool {
    if x > 0 {
        return true;
    }
    false
}
"#;
        let score = compute_complexity(code, Language::Rust).unwrap();
        assert_eq!(score, 2, "Single if should have complexity 2");
    }

    #[test]
    fn test_if_else_rust() {
        let code = r#"
fn check(x: i32) -> &str {
    if x > 0 {
        "positive"
    } else {
        "non-positive"
    }
}
"#;
        let score = compute_complexity(code, Language::Rust).unwrap();
        // if_expression + else_clause = 2 branch points, so complexity = 3
        assert_eq!(score, 3, "if/else should have complexity 3");
    }

    #[test]
    fn test_nested_control_flow_rust() {
        let code = r#"
fn complex(items: &[i32]) -> i32 {
    let mut sum = 0;
    for item in items {
        if *item > 0 {
            sum += item;
        } else {
            if *item < -10 {
                continue;
            }
        }
    }
    sum
}
"#;
        let score = compute_complexity(code, Language::Rust).unwrap();
        // for_expression + if_expression + else_clause + if_expression = 4
        // complexity = 5
        assert_eq!(score, 5, "Nested control flow should have complexity 5");
    }

    #[test]
    fn test_logical_operators_rust() {
        let code = r#"
fn check(a: bool, b: bool, c: bool) -> bool {
    if a && b || c {
        true
    } else {
        false
    }
}
"#;
        let score = compute_complexity(code, Language::Rust).unwrap();
        // if_expression + else_clause + && + || = 4 branch points
        assert_eq!(score, 5, "Logical operators should add to complexity");
    }

    #[test]
    fn test_match_rust() {
        let code = r#"
fn classify(x: i32) -> &str {
    match x {
        0 => "zero",
        1..=10 => "small",
        _ => "large",
    }
}
"#;
        let score = compute_complexity(code, Language::Rust).unwrap();
        // 3 match_arms = 3 branch points, complexity = 4
        assert_eq!(score, 4, "Match with 3 arms should have complexity 4");
    }

    #[test]
    fn test_linear_function_python() {
        let code = r#"
def add(a, b):
    result = a + b
    return result
"#;
        let score = compute_complexity(code, Language::Python).unwrap();
        assert_eq!(score, 1, "Linear Python function should have complexity 1");
    }

    #[test]
    fn test_if_elif_python() {
        let code = r#"
def classify(x):
    if x > 0:
        return "positive"
    elif x == 0:
        return "zero"
    else:
        return "negative"
"#;
        let score = compute_complexity(code, Language::Python).unwrap();
        // if_statement + elif_clause = 2 branch points
        assert_eq!(score, 3, "if/elif/else should have complexity 3");
    }

    #[test]
    fn test_linear_function_javascript() {
        let code = r#"
function add(a, b) {
    const result = a + b;
    return result;
}
"#;
        let score = compute_complexity(code, Language::JavaScript).unwrap();
        assert_eq!(score, 1, "Linear JS function should have complexity 1");
    }

    #[test]
    fn test_if_else_javascript() {
        let code = r#"
function check(x) {
    if (x > 0) {
        return true;
    } else {
        return false;
    }
}
"#;
        let score = compute_complexity(code, Language::JavaScript).unwrap();
        // if_statement + else_clause = 2
        assert_eq!(score, 3, "if/else JS should have complexity 3");
    }

    #[test]
    fn test_unsupported_language_returns_none() {
        let code = "some code here";
        // Haskell is not in our branch_node_types list
        let score = compute_complexity(code, Language::Haskell);
        assert!(score.is_none(), "Unsupported language should return None");
    }

    #[test]
    fn test_go_if_for() {
        let code = r#"
func process(items []int) int {
    sum := 0
    for _, item := range items {
        if item > 0 {
            sum += item
        }
    }
    return sum
}
"#;
        let score = compute_complexity(code, Language::Go).unwrap();
        // for_statement + if_statement = 2
        assert_eq!(score, 3, "Go for+if should have complexity 3");
    }

    #[test]
    fn test_empty_content() {
        let score = compute_complexity("", Language::Rust).unwrap();
        assert_eq!(score, 1, "Empty content should have complexity 1");
    }
}
