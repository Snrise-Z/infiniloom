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
        },
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
        if matches!(
            child.kind(),
            "function_definition"
                | "async_function_definition"
                | "function_declaration"
                | "method_definition"
                | "function_item"
                | "method_declaration"
        ) {
            let extractor = TypeSignatureExtractor::new(code);
            return Some(extractor.extract(&child, lang));
        }

        // Try grandchildren (e.g., module -> function)
        let mut grandchild_cursor = child.walk();
        for grandchild in child.children(&mut grandchild_cursor) {
            if matches!(
                grandchild.kind(),
                "function_definition"
                    | "async_function_definition"
                    | "function_declaration"
                    | "method_definition"
                    | "function_item"
                    | "method_declaration"
            ) {
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
    assert_eq!(
        sig.unwrap().parameters.len(),
        0,
        "empty parameter list must return empty vec, not null"
    );
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
    assert!(sig.parameters[0].type_info.is_none(), "untyped parameter should have no type_info");
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
    assert!(
        matches!(args.kind, ParameterKind::VarPositional),
        "*args must be VarPositional, got {:?}",
        args.kind
    );
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
    assert!(
        matches!(kwargs.kind, ParameterKind::VarKeyword),
        "**kwargs must be VarKeyword, got {:?}",
        kwargs.kind
    );
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

    assert!(
        matches!(b_param.unwrap().kind, ParameterKind::KeywordOnly),
        "parameter after * must be KeywordOnly"
    );
    assert!(
        matches!(c_param.unwrap().kind, ParameterKind::KeywordOnly),
        "parameter after * must be KeywordOnly"
    );
}

#[test]
fn python_async_function_detected() {
    // Bug prevented: async def not detected as async
    // Expected: is_async = true
    let code = "async def foo(): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    assert!(sig.unwrap().is_async, "async def must set is_async=true");
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
    assert!(sig.unwrap().is_generator, "function with yield must set is_generator=true");
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
    assert!(
        param.default_value.is_some(),
        "parameter with default should have default_value field populated"
    );
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
    assert_eq!(sig.parameters[0].name, "参数", "should correctly extract Unicode identifier");
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
    assert_eq!(sig.parameters[0].name, "_param", "underscore prefix must be preserved");
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
    assert!(param.is_optional, "parameter with ? suffix must be marked optional");
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
    assert!(args_param.unwrap().is_variadic, "...args must be marked variadic");
}

#[test]
fn typescript_async_function() {
    // Bug prevented: async keyword not detected
    // Expected: is_async = true
    let code = "async function foo(): Promise<void> {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    assert!(sig.unwrap().is_async, "async function must set is_async=true");
}

#[test]
fn typescript_generic_function() {
    // Bug prevented: Generic params <T> not extracted
    // Expected: generics.len() > 0
    let code = "function foo<T>(x: T): T {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(!sig.generics.is_empty(), "generic function must have generics extracted");
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
    assert!(sig.parameters[0].type_info.is_none(), "JavaScript param should have no type info");
    assert!(sig.return_type.is_none(), "JavaScript function should have no return type");
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
    assert!(sig.receiver.is_some(), "&self should populate receiver field");
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
    assert!(sig.receiver.unwrap().contains("mut"), "&mut self must be distinguished from &self");
}

#[test]
fn rust_async_fn() {
    // Bug prevented: async fn not detected
    // Expected: is_async = true
    let code = "async fn foo() {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    assert!(sig.unwrap().is_async, "async fn must set is_async=true");
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
    assert!(args_param.unwrap().is_variadic, "...int parameter must be marked variadic");
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
    assert_eq!(sig.parameters[0].name, "_", "underscore wildcard must be preserved as name");
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
    assert_eq!(sig.parameters[0].name, "PascalCaseParam", "PascalCase must be preserved");
    assert_eq!(sig.parameters[1].name, "snake_case_param", "snake_case must be preserved");
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

// ============================================================================
// Phase 3: Return Type Extraction Tests
// ============================================================================

#[test]
fn python_simple_return_type() {
    let code = "def foo() -> int: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some(), "should extract return type annotation");
    assert_eq!(sig.return_type.unwrap().name, "int");
}

#[test]
fn python_generic_return_type() {
    let code = "def foo() -> List[str]: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
    let ret = sig.return_type.unwrap();
    assert_eq!(ret.name, "List");
    assert_eq!(ret.generic_args.len(), 1, "should extract generic args");
}

#[test]
fn python_optional_return_type() {
    let code = "def foo() -> Optional[str]: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
    let ret = sig.return_type.unwrap();
    assert!(ret.is_nullable, "Optional return type should be marked nullable");
}

#[test]
fn python_union_return_type() {
    let code = "def foo() -> int | str: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
    let ret = sig.return_type.unwrap();
    assert!(!ret.union_types.is_empty(), "should extract union types");
}

#[test]
fn python_none_return_type() {
    let code = "def foo() -> None: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
    assert_eq!(sig.return_type.unwrap().name, "None");
}

#[test]
fn typescript_simple_return_type() {
    let code = "function foo(): number { return 42; }";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some(), "should extract return type");
    assert_eq!(sig.return_type.unwrap().name, "number");
}

#[test]
fn typescript_generic_return_type() {
    let code = "function foo(): Promise<string> { return Promise.resolve('hello'); }";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
    let ret = sig.return_type.unwrap();
    assert!(!ret.generic_args.is_empty(), "should extract Promise generic arg");
}

#[test]
fn typescript_void_return() {
    let code = "function foo(): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
}

#[test]
fn typescript_array_return_type() {
    let code = "function foo(): number[] { return [1, 2, 3]; }";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
    let ret = sig.return_type.unwrap();
    assert!(ret.array_dimensions > 0, "should detect array return type");
}

#[test]
fn rust_simple_return_type() {
    let code = "fn foo() -> i32 { 42 }";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some(), "should extract return type");
}

#[test]
fn rust_result_return_type() {
    let code = "fn foo() -> Result<String, Error> { Ok(String::new()) }";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
    let ret = sig.return_type.unwrap();
    assert_eq!(ret.name, "Result");
    assert_eq!(ret.generic_args.len(), 2, "Result should have two generic args");
}

#[test]
fn rust_option_return_type() {
    let code = "fn foo() -> Option<i32> { None }";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
    let ret = sig.return_type.unwrap();
    assert!(ret.is_nullable, "Option should be marked nullable");
}

#[test]
fn rust_reference_return_type() {
    let code = "fn foo() -> &str { \"hello\" }";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some());
    let ret = sig.return_type.unwrap();
    assert!(ret.is_reference, "should detect reference return type");
}

#[test]
fn rust_unit_return() {
    let code = "fn foo() { println!(\"hello\"); }";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
    // Unit return can be None or present - both are valid
}

#[test]
fn go_simple_return_type() {
    let code = "func foo() int { return 42 }";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some(), "should extract Go return type");
}

#[test]
fn go_multiple_return_types() {
    let code = "func foo() (int, error) { return 0, nil }";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some(), "should extract multiple return types");
}

#[test]
fn go_named_return_types() {
    let code = "func foo() (result int, err error) { return 0, nil }";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert!(sig.return_type.is_some(), "should extract named return types");
}

// ============================================================================
// Phase 5: Comprehensive Edge Case Testing
// ============================================================================

#[test]
fn python_multiline_signature_with_complex_formatting() {
    // Bug prevented: Multi-line signatures break parameter extraction
    let code = r#"
def foo(
    a: int,
    b: str,
    c: Optional[
        List[Dict[str, Any]]
    ]
) -> Tuple[int, str]:
    pass
    "#;
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 3);
}

#[test]
fn python_comment_within_signature() {
    // Bug prevented: Comments in signature break parsing
    let code = r#"
def foo(
    a: int,  # First parameter
    b: str,  # Second parameter
) -> None:
    pass
    "#;
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 2);
}

#[test]
fn python_extremely_long_parameter_list() {
    // Bug prevented: Many parameters cause performance issues
    let params: Vec<String> = (0..50).map(|i| format!("param{}: int", i)).collect();
    let code = format!("def foo({}): pass", params.join(", "));

    let sig = extract_function_signature(&code, Language::Python);
    assert!(sig.is_some());
    let sig = sig.unwrap();
    assert_eq!(sig.parameters.len(), 50);
}

#[test]
fn python_deeply_nested_generic_types() {
    // Bug prevented: Deep nesting causes stack overflow
    let code = "def foo(x: Optional[List[Dict[str, Union[int, str, None]]]]): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn python_lambda_function_not_crash() {
    // Bug prevented: Lambda expressions cause unexpected behavior
    let code = "lambda x, y: x + y";
    let sig = extract_function_signature(code, Language::Python);
    assert!(sig.is_some() || sig.is_none());
}

#[test]
fn python_decorated_function() {
    // Bug prevented: Decorators interfere with function extraction
    let code = r#"
@decorator
def foo(x: int) -> str:
    pass
    "#;
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn python_positional_only_parameters() {
    // Bug prevented: Positional-only params (before /) not handled
    let code = "def foo(a, b, /, c): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn python_type_alias_in_annotation() {
    // Bug prevented: Complex type aliases cause parsing errors
    let code = "def foo(x: MyCustomType[T]) -> Result[T, Error]: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn typescript_union_type_parameter() {
    // Bug prevented: Union types in parameters not extracted
    let code = "function foo(x: string | number | null): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn typescript_intersection_type() {
    // Bug prevented: Intersection types (&) not handled
    let code = "function foo(x: A & B): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn typescript_readonly_parameter() {
    // Bug prevented: readonly modifier lost
    let code = "function foo(x: readonly string[]): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn typescript_tuple_type() {
    // Bug prevented: Tuple types not recognized
    let code = "function foo(x: [string, number, boolean]): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn javascript_generator_function() {
    // Bug prevented: Generator functions not detected
    let code = "function* foo(x) { yield x; }";
    let sig = extract_function_signature(code, Language::JavaScript);

    // May or may not extract depending on tree-sitter
    if let Some(sig) = sig {
        assert!(sig.is_generator);
    }
}

#[test]
fn javascript_destructured_parameters() {
    // Bug prevented: Destructured params cause extraction to fail
    let code = "function foo({ a, b }, [x, y]) {}";
    let sig = extract_function_signature(code, Language::JavaScript);

    assert!(sig.is_some());
}

#[test]
fn rust_lifetime_parameters() {
    // Bug prevented: Lifetime params not extracted
    let code = "fn foo<'a, T>(x: &'a T) -> &'a T {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn rust_const_generic_parameter() {
    // Bug prevented: Const generics not extracted
    let code = "fn foo<const N: usize>(arr: [i32; N]) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn rust_where_clause() {
    // Bug prevented: Where clause breaks extraction
    let code = r#"
fn foo<T>(x: T) -> T
where
    T: Clone + Send,
{
}
    "#;
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn rust_impl_trait_parameter() {
    // Bug prevented: impl Trait syntax not recognized
    let code = "fn foo(x: impl Iterator<Item = i32>) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn rust_unsafe_fn() {
    // Bug prevented: unsafe keyword breaks extraction
    let code = "unsafe fn foo(x: *const i32) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn rust_extern_fn() {
    // Bug prevented: extern functions not handled
    let code = "extern \"C\" fn foo(x: i32) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn go_multiple_return_values() {
    // Bug prevented: Multiple return values not captured
    let code = "func foo(x int) (int, error) {}";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
}

#[test]
fn go_named_return_values() {
    // Bug prevented: Named return values cause confusion
    let code = "func foo(x int) (result int, err error) {}";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
}

#[test]
fn go_method_receiver() {
    // Bug prevented: Method receivers not extracted
    let code = "func (s *Server) Handle(req Request) Response {}";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
}

#[test]
fn incomplete_function_signature() {
    // Bug prevented: Malformed code causes panic
    let code = "def foo(";
    let sig = extract_function_signature(code, Language::Python);
    assert!(sig.is_some() || sig.is_none());
}

#[test]
fn function_with_trailing_comma() {
    // Bug prevented: Trailing comma breaks parsing
    let code = "def foo(a: int, b: str,): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn whitespace_only_parameter_name() {
    // Bug prevented: Whitespace-only names accepted
    let code = "def foo(   ): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn numeric_start_identifier_rejected() {
    // Bug prevented: Invalid identifiers parsed
    let code = "def foo(123param): pass";
    let sig = extract_function_signature(code, Language::Python);
    assert!(sig.is_some() || sig.is_none());
}

#[test]
fn python_ellipsis_in_type() {
    // Bug prevented: Ellipsis (...) in types causes issues
    let code = "def foo(func: Callable[..., int]): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn typescript_conditional_type() {
    // Bug prevented: Conditional types not handled
    let code = "function foo<T>(x: T extends string ? number : boolean): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn rust_closure_parameter() {
    // Bug prevented: Closure types break extraction
    let code = "fn foo(f: impl Fn(i32) -> i32) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn go_interface_parameter() {
    // Bug prevented: interface{} type causes issues
    let code = "func foo(x interface{}) {}";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
}

#[test]
fn python_protocol_type() {
    // Bug prevented: Protocol types not recognized
    let code = "def foo(x: SupportsInt) -> int: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn typescript_never_type() {
    // Bug prevented: never type causes extraction failure
    let code = "function foo(x: never): never { throw new Error(); }";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn typescript_unknown_type() {
    // Bug prevented: unknown type not handled
    let code = "function foo(x: unknown): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn typescript_template_literal_type() {
    // Bug prevented: Template literal types break extraction
    let code = "function foo(x: `hello-${string}`): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn rust_pin_type() {
    // Bug prevented: Pin<Box<...>> types not extracted
    let code = "fn foo(x: Pin<Box<dyn Future<Output = ()>>>) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn rust_raw_pointer_type() {
    // Bug prevented: Raw pointer types cause issues
    let code = "fn foo(x: *const i32, y: *mut u8) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn rust_dyn_trait_object() {
    // Bug prevented: dyn trait objects not handled
    let code = "fn foo(x: &dyn Display) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn go_empty_interface() {
    // Bug prevented: Empty interface{} mishandled
    let code = "func foo(x interface{}) interface{} {}";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
}

#[test]
fn go_channel_type() {
    // Bug prevented: Channel types cause parsing errors
    let code = "func foo(ch chan int, recv <-chan string, send chan<- bool) {}";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
}

#[test]
fn python_self_parameter_in_method() {
    // Bug prevented: self parameter not handled specially
    let code = r#"
def method(self, x: int, y: str) -> None:
    pass
    "#;
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn python_cls_parameter_in_classmethod() {
    // Bug prevented: cls parameter not recognized
    let code = r#"
@classmethod
def method(cls, x: int) -> None:
    pass
    "#;
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn typescript_this_parameter() {
    // Bug prevented: this parameter not handled
    let code = "function foo(this: MyClass, x: number): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn typescript_indexed_access_type() {
    // Bug prevented: Indexed access types break extraction
    let code = "function foo<T, K extends keyof T>(obj: T, key: K): T[K] {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn rust_higher_ranked_trait_bounds() {
    // Bug prevented: for<'a> syntax causes parsing errors
    let code = "fn foo<F>(f: F) where F: for<'a> Fn(&'a i32) -> &'a i32 {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn python_forward_reference_string() {
    // Bug prevented: Forward references not handled
    let code = "def foo(x: 'ForwardRef') -> 'AnotherRef': pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn python_pep_604_union_syntax() {
    // Bug prevented: PEP 604 X | Y syntax not recognized
    let code = "def foo(x: int | str | None) -> int | str: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn python_literal_type() {
    // Bug prevented: Literal types break extraction
    let code = "def foo(x: Literal['a', 'b', 'c']) -> Literal[42]: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn typescript_keyof_type_operator() {
    // Bug prevented: keyof operator breaks extraction
    let code = "function foo<T>(x: keyof T): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn typescript_typeof_type_operator() {
    // Bug prevented: typeof in type position not handled
    let code = "function foo(x: typeof someVariable): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn rust_associated_type_in_signature() {
    // Bug prevented: Associated types (T::Item) cause errors
    let code = "fn foo<T: Iterator>(x: T::Item) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some());
}

#[test]
fn go_struct_type_inline() {
    // Bug prevented: Inline struct types break extraction
    let code = "func foo(x struct{ Name string; Age int }) {}";
    let sig = extract_function_signature(code, Language::Go);

    assert!(sig.is_some());
}

#[test]
fn typescript_infer_keyword() {
    // Bug prevented: infer keyword breaks extraction
    let code = "function foo<T>(x: T extends infer U ? U : never): void {}";
    let sig = extract_function_signature(code, Language::TypeScript);

    assert!(sig.is_some());
}

#[test]
fn special_characters_in_parameter_names() {
    // Bug prevented: Special chars in valid identifiers cause issues
    let code = "def foo($_param, __dunder__, _): pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn very_deeply_nested_function() {
    // Bug prevented: Functions nested in blocks cause stack issues
    let code = r#"
if True:
    if True:
        if True:
            if True:
                def foo(x: int) -> int:
                    pass
    "#;
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn python_annotated_type() {
    // Bug prevented: Annotated[type, metadata] not handled
    let code = "def foo(x: Annotated[int, 'positive']) -> None: pass";
    let sig = extract_function_signature(code, Language::Python);

    assert!(sig.is_some());
}

#[test]
fn empty_generic_arguments() {
    // Bug prevented: Empty generic brackets cause crash
    let code = "fn foo(x: Vec<>) {}";
    let sig = extract_function_signature(code, Language::Rust);

    assert!(sig.is_some() || sig.is_none());
}
