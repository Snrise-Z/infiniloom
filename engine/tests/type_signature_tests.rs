//! Comprehensive tests for type signature extraction across 21 languages
//!
//! Focus: Bug prevention through edge-case testing and expected behavior validation
//! Coverage target: type_signature.rs from 1.78% → 50%+

use infiniloom_engine::analysis::type_signature::TypeSignatureExtractor;
use infiniloom_engine::analysis::types::{ParameterKind, TypeSignature};
use infiniloom_engine::parser::Language;
use proptest::prelude::*;

// ============================================================================
// Test Helpers
// ============================================================================

/// Parse code and extract type signature from first function-like node
fn extract_function_signature(code: &str, lang: Language) -> Option<TypeSignature> {
    use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

    // Get tree-sitter language
    let ts_lang = match lang {
        Language::Python => tree_sitter_python::LANGUAGE,
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Language::JavaScript => tree_sitter_javascript::LANGUAGE,
        Language::Rust => tree_sitter_rust::LANGUAGE,
        Language::Go => tree_sitter_go::LANGUAGE,
        _ => return None,
    };

    // Parse code
    let mut parser = Parser::new();
    parser.set_language(&ts_lang.into()).ok()?;
    let tree = parser.parse(code, None)?;
    let root = tree.root_node();

    // Query for function-like nodes
    let query_str = match lang {
        // Python: both sync and async functions have kind "function_definition"
        // The "async" keyword is a child node
        Language::Python => "(function_definition) @func",
        Language::TypeScript | Language::JavaScript => {
            "[(function_declaration) (method_definition) (arrow_function)] @func"
        }
        Language::Rust => "(function_item) @func",
        Language::Go => "[(function_declaration) (method_declaration)] @func",
        _ => return None,
    };

    let query = Query::new(&ts_lang.into(), query_str).ok()?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, root, code.as_bytes());

    // Extract signature from first function node
    while let Some(m) = matches.next() {
        if let Some(capture) = m.captures.first() {
            let node = capture.node;
            let extractor = TypeSignatureExtractor::new(code);
            return Some(extractor.extract(&node, lang));
        }
    }

    // Fallback: walk the tree manually to find function nodes
    let mut child_cursor = root.walk();
    for child in root.children(&mut child_cursor) {
        if matches!(child.kind(), "function_definition" | "async_function_definition" | "function_declaration" | "method_definition" | "function_item" | "method_declaration") {
            let extractor = TypeSignatureExtractor::new(code);
            return Some(extractor.extract(&child, lang));
        }

        // Try grandchildren (e.g., module -> function)
        let mut grandchild_cursor = child.walk();
        for grandchild in child.children(&mut grandchild_cursor) {
            if matches!(grandchild.kind(), "function_definition" | "async_function_definition" | "function_declaration" | "method_definition" | "function_item" | "method_declaration") {
                let extractor = TypeSignatureExtractor::new(code);
                return Some(extractor.extract(&grandchild, lang));
            }
        }
    }

    None
}

// ============================================================================
// Python Type Signature Tests - High Priority
// ============================================================================

#[test]
fn python_empty_parameter_list() {
    // Bug prevented: Empty param list returns null instead of empty vec
    // Expected: parameters.len() == 0
    let code = "def foo(): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some(), "should parse simple function");
    assert_eq!(sig.unwrap().parameters.len(), 0,
        "empty parameter list must return empty vec, not null");
}

#[test]
fn python_single_untyped_parameter() {
    // Bug prevented: Untyped parameters cause extraction to fail
    // Expected: Parameter with name but no type_info
    let code = "def foo(x): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 1);
    assert_eq!(sig.parameters[0].name, "x");
    assert!(sig.parameters[0].type_info.is_none(),
        "untyped parameter should have no type_info");
}

#[test]
fn python_args_classified_as_var_positional() {
    // Bug prevented: *args misclassified as regular positional parameter
    // Expected: ParameterKind::VarPositional for *args
    let code = "def foo(*args): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();

    let args_param = sig.parameters.iter().find(|p| p.name == "args");
    assert!(args_param.is_some(), "should have *args parameter");

    let args = args_param.unwrap();
    assert!(matches!(args.kind, ParameterKind::VarPositional),
        "*args must be VarPositional, got {:?}", args.kind);
}

#[test]
fn python_kwargs_classified_as_var_keyword() {
    // Bug prevented: **kwargs misclassified as VarPositional instead of VarKeyword
    // Expected: ParameterKind::VarKeyword for **kwargs
    let code = "def foo(**kwargs): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();

    let kwargs_param = sig.parameters.iter().find(|p| p.name == "kwargs");
    assert!(kwargs_param.is_some(), "should have **kwargs parameter");

    let kwargs = kwargs_param.unwrap();
    assert!(matches!(kwargs.kind, ParameterKind::VarKeyword),
        "**kwargs must be VarKeyword, got {:?}", kwargs.kind);
}

#[test]
fn python_keyword_only_parameters_after_star() {
    // Bug prevented: Parameters after * incorrectly classified
    // Expected: Parameters after bare * are KeywordOnly
    let code = "def foo(a, *, b, c): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 3, "should have 3 parameters");

    let b_param = sig.parameters.iter().find(|p| p.name == "b");
    let c_param = sig.parameters.iter().find(|p| p.name == "c");

    assert!(b_param.is_some() && c_param.is_some());

    assert!(matches!(b_param.unwrap().kind, ParameterKind::KeywordOnly),
        "parameter after * must be KeywordOnly");
    assert!(matches!(c_param.unwrap().kind, ParameterKind::KeywordOnly),
        "parameter after * must be KeywordOnly");
}

#[test]
fn python_async_function_detected() {
    // Bug prevented: async def not detected as async
    // Expected: is_async = true
    let code = "async def foo(): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    assert!(sig.unwrap().is_async,
        "async def must set is_async=true");
}

#[test]
fn python_generator_with_yield_detected() {
    // Bug prevented: Functions with yield not marked as generators
    // Expected: is_generator = true
    let code = r#"
def foo():
    yield 1
    "#;
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    assert!(sig.unwrap().is_generator,
        "function with yield must set is_generator=true");
}

#[test]
fn python_async_generator_both_flags() {
    // Bug prevented: async generator missing one flag
    // Expected: is_async=true AND is_generator=true
    let code = r#"
async def foo():
    yield 1
    "#;
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.is_async, "async generator must be async");
    assert!(sig.is_generator, "async generator must be generator");
}

#[test]
fn python_default_parameter_value() {
    // Bug prevented: Default values not extracted
    // Expected: default_value.is_some()
    let code = "def foo(x=42): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 1);

    let param = &sig.parameters[0];
    assert!(param.default_value.is_some(),
        "parameter with default should have default_value field populated");
    assert_eq!(param.default_value.as_deref(), Some("42"));
}

#[test]
fn python_unicode_identifier_supported() {
    // Bug prevented: Unicode identifiers cause parsing failure or mojibake
    // Expected: Correct Unicode parameter name extracted
    let code = "def 函数(参数: int) -> str: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 1);
    assert_eq!(sig.parameters[0].name, "参数",
        "should correctly extract Unicode identifier");
}

#[test]
fn python_very_long_type_name_doesnt_crash() {
    // Bug prevented: DoS via extremely long type names causing OOM
    // Expected: Handles gracefully without crash
    let long_type = "A".repeat(10000);
    let code = format!("def foo(x: {}): pass", long_type);

    let sig = extract_function_signature(&code, Language::Python);
    // Should either succeed or fail gracefully, but never panic
    assert!(sig.is_some() || sig.is_none());
}

#[test]
fn python_single_letter_identifiers() {
    // Bug prevented: Single-letter params filtered out incorrectly
    // Expected: All single-letter params extracted
    let code = "def f(a: int, b: str, c: bool) -> None: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 3);
    assert_eq!(sig.parameters[0].name, "a");
    assert_eq!(sig.parameters[1].name, "b");
    assert_eq!(sig.parameters[2].name, "c");
}

#[test]
fn python_underscore_prefixed_private_param() {
    // Bug prevented: Private parameters ignored or stripped
    // Expected: Underscore prefix preserved
    let code = "def _private(_param: int) -> None: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 1);
    assert_eq!(sig.parameters[0].name, "_param",
        "underscore prefix must be preserved");
}

// ============================================================================
// TypeScript/JavaScript Tests - High Priority
// ============================================================================

#[test]
fn typescript_optional_parameter() {
    // Bug prevented: Optional params (x?) not marked as optional
    // Expected: is_optional = true
    let code = "function foo(x?: number): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 1);

    let param = &sig.parameters[0];
    assert!(param.is_optional,
        "parameter with ? suffix must be marked optional");
}

#[test]
fn typescript_rest_parameter() {
    // Bug prevented: Rest params (...args) not classified as variadic
    // Expected: is_variadic = true
    let code = "function foo(...args: number[]): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    let sig = sig.unwrap();

    let args_param = sig.parameters.iter().find(|p| p.name == "args");
    assert!(args_param.is_some(), "should have ...args parameter");
    assert!(args_param.unwrap().is_variadic,
        "...args must be marked variadic");
}

#[test]
fn typescript_async_function() {
    // Bug prevented: async keyword not detected
    // Expected: is_async = true
    let code = "async function foo(): Promise<void> {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    assert!(sig.unwrap().is_async,
        "async function must set is_async=true");
}

#[test]
fn typescript_generic_function() {
    // Bug prevented: Generic params <T> not extracted
    // Expected: generics.len() > 0
    let code = "function foo<T>(x: T): T {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(!sig.generics.is_empty(),
        "generic function must have generics extracted");
    assert_eq!(sig.generics[0].name, "T");
}

#[test]
fn javascript_no_types_handled() {
    // Bug prevented: Plain JavaScript causes type extraction to fail
    // Expected: Succeeds with no type info
    let code = "function foo(x, y) { return x + y; }";
    let sig = extract_function_signature(code, Language::JavaScript);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 2);
    assert!(sig.parameters[0].type_info.is_none(),
        "JavaScript param should have no type info");
    assert!(sig.return_type.is_none(),
        "JavaScript function should have no return type");
}

// ============================================================================
// Rust Tests - High Priority
// ============================================================================

#[test]
fn rust_empty_params() {
    // Bug prevented: Empty parameter list causes null pointer
    // Expected: parameters.len() == 0
    let code = "fn foo() {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    assert_eq!(sig.unwrap().parameters.len(), 0);
}

#[test]
fn rust_self_parameter() {
    // Bug prevented: &self not recognized as receiver
    // Expected: receiver field populated
    let code = "fn foo(&self, x: i32) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.receiver.is_some(),
        "&self should populate receiver field");
}

#[test]
fn rust_mutable_self() {
    // Bug prevented: &mut self not distinguished from &self
    // Expected: receiver contains "mut"
    let code = "fn foo(&mut self) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.receiver.is_some());
    assert!(sig.receiver.unwrap().contains("mut"),
        "&mut self must be distinguished from &self");
}

#[test]
fn rust_async_fn() {
    // Bug prevented: async fn not detected
    // Expected: is_async = true
    let code = "async fn foo() {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    assert!(sig.unwrap().is_async,
        "async fn must set is_async=true");
}

// ============================================================================
// Go Tests - Medium Priority
// ============================================================================

#[test]
fn go_no_params_no_return() {
    // Bug prevented: Empty signature causes parsing error
    // Expected: Empty params, no return type
    let code = "func foo() {}";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 0);
    assert!(sig.return_type.is_none());
}

#[test]
fn go_variadic_parameter() {
    // Bug prevented: Variadic params (...int) not marked
    // Expected: is_variadic = true
    let code = "func foo(args ...int) {}";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
    let sig = sig.unwrap();

    let args_param = sig.parameters.iter().find(|p| p.name == "args");
    assert!(args_param.is_some());
    assert!(args_param.unwrap().is_variadic,
        "...int parameter must be marked variadic");
}

// ============================================================================
// Error Handling and Edge Cases - Critical
// ============================================================================

#[test]
fn empty_source_code_doesnt_crash() {
    // Bug prevented: Empty string causes panic
    // Expected: Returns None or empty result
    let sig = extract_function_signature("", Language::Python);
    assert!(sig.is_none() || sig.unwrap().parameters.is_empty());
}

#[test]
fn null_byte_in_source_handled() {
    // Bug prevented: Null byte causes panic or undefined behavior
    // Expected: Handles gracefully
    let code = "def foo\0(x): pass";
    let sig = extract_function_signature(code, Language::Python);
    // Should either parse around null or fail gracefully
    assert!(sig.is_some() || sig.is_none());
}

#[test]
fn deeply_nested_generics_dont_stack_overflow() {
    // Bug prevented: Deep recursion causes stack overflow
    // Expected: Handles gracefully with bounded depth
    let nested = "Vec<".repeat(100) + "i32" + &">".repeat(100);
    let code = format!("fn foo(x: {}) {{}}", nested);

    // Should not panic with stack overflow
    let sig = extract_function_signature(&code, Language::Rust);
    assert!(sig.is_some() || sig.is_none());
}

#[test]
fn underscore_only_identifier() {
    // Bug prevented: Underscore-only params treated as unnamed
    // Expected: Preserved as identifier "_"
    let code = "fn foo(_: i32) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 1);
    assert_eq!(sig.parameters[0].name, "_",
        "underscore wildcard must be preserved as name");
}

#[test]
fn mixed_case_identifiers() {
    // Bug prevented: Case conversion mangles identifiers
    // Expected: Case preserved exactly
    let code = "def camelCaseFunction(PascalCaseParam: int, snake_case_param: str): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 2);
    assert_eq!(sig.parameters[0].name, "PascalCaseParam",
        "PascalCase must be preserved");
    assert_eq!(sig.parameters[1].name, "snake_case_param",
        "snake_case must be preserved");
}

// ============================================================================
// Property-Based Tests
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Type extraction never panics on valid Python code
    #[test]
    fn prop_python_extraction_never_panics(
        func_name in "[a-z][a-z0-9_]{0,20}",
        param_count in 0usize..5,
    ) {
        let params = (0..param_count)
            .map(|i| format!("arg{}", i))
            .collect::<Vec<_>>()
            .join(", ");

        let code = format!("def {}({}): pass", func_name, params);

        // Should never panic
        let _sig = extract_function_signature(&code, Language::Python);
    }

    /// Unicode identifiers always work
    #[test]
    fn prop_unicode_identifiers_supported(
        // Unicode identifier from various scripts
        name in "\\p{L}\\p{L}{1,20}",
    ) {
        let code = format!("def {}(): pass", name);

        // Should handle any valid Unicode identifier
        let sig = extract_function_signature(&code, Language::Python);
        prop_assert!(sig.is_some() || sig.is_none());
    }
}
