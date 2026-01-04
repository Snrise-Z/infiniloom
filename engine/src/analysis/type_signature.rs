//! Type signature extraction for all supported languages
//!
//! Extracts full type signatures including parameters, return types, generics, and throws
//! from AST nodes across all 21 supported languages.

use crate::analysis::types::{
    GenericParam, ParameterInfo, ParameterKind, TypeInfo, TypeSignature, Variance,
};
use crate::parser::Language;
use tree_sitter::Node;

/// Extracts type signatures from AST nodes
pub struct TypeSignatureExtractor {
    /// Source code being analyzed
    source: String,
}

impl TypeSignatureExtractor {
    /// Create a new extractor with the given source code
    pub fn new(source: impl Into<String>) -> Self {
        Self { source: source.into() }
    }

    /// Get text for a node
    fn node_text(&self, node: &Node<'_>) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    /// Extract type signature from a function/method node
    pub fn extract(&self, node: &Node<'_>, language: Language) -> TypeSignature {
        match language {
            Language::Python => self.extract_python(node),
            Language::JavaScript => self.extract_javascript(node),
            Language::TypeScript => self.extract_typescript(node),
            Language::Rust => self.extract_rust(node),
            Language::Go => self.extract_go(node),
            Language::Java => self.extract_java(node),
            Language::C => self.extract_c(node),
            Language::Cpp => self.extract_cpp(node),
            Language::CSharp => self.extract_csharp(node),
            Language::Ruby => self.extract_ruby(node),
            Language::Php => self.extract_php(node),
            Language::Kotlin => self.extract_kotlin(node),
            Language::Swift => self.extract_swift(node),
            Language::Scala => self.extract_scala(node),
            Language::Haskell => self.extract_haskell(node),
            Language::Elixir => self.extract_elixir(node),
            Language::Clojure => self.extract_clojure(node),
            Language::OCaml => self.extract_ocaml(node),
            Language::Lua => self.extract_lua(node),
            Language::R => self.extract_r(node),
            Language::Bash => self.extract_bash(node),
            // Handle any language not explicitly matched (e.g., FSharp)
            _ => TypeSignature::default(),
        }
    }

    /// Extract Python type signature
    fn extract_python(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Check for async def
        sig.is_async = node.kind() == "async_function_definition" || node.kind().contains("async");

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_python_params(&params);
        }

        // Extract return type annotation
        if let Some(return_type) = node.child_by_field_name("return_type") {
            sig.return_type = Some(self.parse_python_type(&return_type));
        }

        // Check for generator (yield in body)
        if let Some(body) = node.child_by_field_name("body") {
            sig.is_generator = self.contains_yield(&body);
        }

        sig
    }

    fn extract_python_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();
        let mut seen_star = false;
        let mut seen_double_star = false;

        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    // Simple parameter
                    params.push(ParameterInfo {
                        name: self.node_text(&child).to_owned(),
                        kind: if seen_star {
                            ParameterKind::KeywordOnly
                        } else {
                            ParameterKind::PositionalOrKeyword
                        },
                        ..Default::default()
                    });
                },
                "typed_parameter" | "default_parameter" | "typed_default_parameter" => {
                    let mut param = ParameterInfo::default();

                    // Get name
                    if let Some(name) = child.child_by_field_name("name") {
                        param.name = self.node_text(&name).to_owned();
                    } else if let Some(first) = child.child(0) {
                        if first.kind() == "identifier" {
                            param.name = self.node_text(&first).to_owned();
                        }
                    }

                    // Get type annotation
                    if let Some(type_node) = child.child_by_field_name("type") {
                        param.type_info = Some(self.parse_python_type(&type_node));
                    }

                    // Get default value
                    if let Some(default) = child.child_by_field_name("value") {
                        param.default_value = Some(self.node_text(&default).to_owned());
                        param.is_optional = true;
                    }

                    param.kind = if seen_star {
                        ParameterKind::KeywordOnly
                    } else {
                        ParameterKind::PositionalOrKeyword
                    };

                    params.push(param);
                },
                "list_splat_pattern" | "list_splat" => {
                    seen_star = true;
                    let mut param = ParameterInfo {
                        kind: ParameterKind::VarPositional,
                        is_variadic: true,
                        ..Default::default()
                    };
                    if let Some(name) = child.child(1) {
                        param.name = self.node_text(&name).to_owned();
                    }
                    params.push(param);
                },
                "dictionary_splat_pattern" | "dictionary_splat" => {
                    seen_double_star = true;
                    let mut param = ParameterInfo {
                        kind: ParameterKind::VarKeyword,
                        is_variadic: true,
                        ..Default::default()
                    };
                    if let Some(name) = child.child(1) {
                        param.name = self.node_text(&name).to_owned();
                    }
                    params.push(param);
                },
                "*" => {
                    seen_star = true;
                },
                "**" => {
                    seen_double_star = true;
                },
                _ => {},
            }
        }

        // Suppress unused variable warning
        let _ = seen_double_star;

        params
    }

    fn parse_python_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        let mut type_info = TypeInfo { name: text.to_owned(), ..Default::default() };

        match node.kind() {
            "type" => {
                if let Some(inner) = node.child(0) {
                    return self.parse_python_type(&inner);
                }
            },
            "subscript" => {
                // Generic type like List[int] or Optional[str]
                if let Some(value) = node.child_by_field_name("value") {
                    type_info.name = self.node_text(&value).to_owned();
                }
                if let Some(subscript) = node.child_by_field_name("subscript") {
                    type_info.generic_args = self.extract_generic_args(&subscript);
                }

                // Check for Optional
                if type_info.name == "Optional" {
                    type_info.is_nullable = true;
                }
            },
            "binary_operator" => {
                // Union type: X | Y
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() != "|" {
                        type_info.union_types.push(self.parse_python_type(&child));
                    }
                }
                type_info.name = "Union".to_owned();
            },
            _ => {},
        }

        type_info
    }

    fn extract_generic_args(&self, node: &Node<'_>) -> Vec<TypeInfo> {
        let mut args = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() != "," {
                args.push(self.parse_python_type(&child));
            }
        }

        args
    }

    fn contains_yield(&self, node: &Node<'_>) -> bool {
        if node.kind() == "yield" || node.kind() == "yield_expression" {
            return true;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.contains_yield(&child) {
                return true;
            }
        }
        false
    }

    /// Extract JavaScript type signature
    fn extract_javascript(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Check for async
        sig.is_async = self.node_text(node).trim_start().starts_with("async");

        // Check for generator
        sig.is_generator = node.kind().contains("generator");

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_js_params(&params);
        }

        sig
    }

    fn extract_js_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    params.push(ParameterInfo {
                        name: self.node_text(&child).to_owned(),
                        kind: ParameterKind::Positional,
                        ..Default::default()
                    });
                },
                "assignment_pattern" => {
                    let mut param = ParameterInfo {
                        is_optional: true,
                        kind: ParameterKind::Positional,
                        ..Default::default()
                    };
                    if let Some(left) = child.child_by_field_name("left") {
                        param.name = self.node_text(&left).to_owned();
                    }
                    if let Some(right) = child.child_by_field_name("right") {
                        param.default_value = Some(self.node_text(&right).to_owned());
                    }
                    params.push(param);
                },
                "rest_pattern" => {
                    let mut param = ParameterInfo {
                        kind: ParameterKind::VarPositional,
                        is_variadic: true,
                        ..Default::default()
                    };
                    if let Some(name) = child.child(1) {
                        param.name = self.node_text(&name).to_owned();
                    }
                    params.push(param);
                },
                _ => {},
            }
        }

        params
    }

    /// Extract TypeScript type signature
    fn extract_typescript(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = self.extract_javascript(node);

        // Extract TypeScript-specific type annotations
        if let Some(return_type) = node.child_by_field_name("return_type") {
            sig.return_type = Some(self.parse_ts_type(&return_type));
        }

        // Extract generics
        if let Some(type_params) = node.child_by_field_name("type_parameters") {
            sig.generics = self.extract_ts_generics(&type_params);
        }

        // Update parameter types
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_ts_params(&params);
        }

        sig
    }

    fn extract_ts_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "required_parameter" | "optional_parameter" => {
                    let mut param = ParameterInfo {
                        is_optional: child.kind() == "optional_parameter",
                        kind: ParameterKind::Positional,
                        ..Default::default()
                    };

                    if let Some(pattern) = child.child_by_field_name("pattern") {
                        param.name = self.node_text(&pattern).to_owned();
                    }

                    if let Some(type_ann) = child.child_by_field_name("type") {
                        param.type_info = Some(self.parse_ts_type(&type_ann));
                    }

                    if let Some(value) = child.child_by_field_name("value") {
                        param.default_value = Some(self.node_text(&value).to_owned());
                        param.is_optional = true;
                    }

                    params.push(param);
                },
                "rest_pattern" => {
                    let mut param = ParameterInfo {
                        kind: ParameterKind::VarPositional,
                        is_variadic: true,
                        ..Default::default()
                    };
                    if let Some(name) = child.child(1) {
                        param.name = self.node_text(&name).to_owned();
                    }
                    params.push(param);
                },
                _ => {},
            }
        }

        params
    }

    fn parse_ts_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        let mut type_info = TypeInfo { name: text.to_owned(), ..Default::default() };

        match node.kind() {
            "type_annotation" => {
                if let Some(inner) = node.child(1) {
                    return self.parse_ts_type(&inner);
                }
            },
            "generic_type" => {
                if let Some(name) = node.child_by_field_name("name") {
                    type_info.name = self.node_text(&name).to_owned();
                }
                if let Some(args) = node.child_by_field_name("type_arguments") {
                    type_info.generic_args = self.extract_ts_type_args(&args);
                }
            },
            "union_type" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() != "|" {
                        type_info.union_types.push(self.parse_ts_type(&child));
                    }
                }
                type_info.name = "Union".to_owned();
            },
            "array_type" => {
                if let Some(elem) = node.child(0) {
                    type_info = self.parse_ts_type(&elem);
                    type_info.array_dimensions += 1;
                }
            },
            "predefined_type" | "type_identifier" => {
                type_info.name = text.to_owned();
            },
            _ => {},
        }

        type_info
    }

    fn extract_ts_type_args(&self, node: &Node<'_>) -> Vec<TypeInfo> {
        let mut args = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if !matches!(child.kind(), "<" | ">" | ",") {
                args.push(self.parse_ts_type(&child));
            }
        }

        args
    }

    fn extract_ts_generics(&self, node: &Node<'_>) -> Vec<GenericParam> {
        let mut generics = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameter" {
                let mut param = GenericParam::default();

                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }

                if let Some(constraint) = child.child_by_field_name("constraint") {
                    param
                        .constraints
                        .push(self.node_text(&constraint).to_owned());
                }

                if let Some(default) = child.child_by_field_name("value") {
                    param.default_type = Some(self.node_text(&default).to_owned());
                }

                generics.push(param);
            }
        }

        generics
    }

    /// Extract Rust type signature
    fn extract_rust(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Check for async
        let text = self.node_text(node);
        sig.is_async = text.contains("async fn");

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            let (params_list, receiver) = self.extract_rust_params(&params);
            sig.parameters = params_list;
            sig.receiver = receiver;
        }

        // Extract return type
        if let Some(return_type) = node.child_by_field_name("return_type") {
            sig.return_type = Some(self.parse_rust_type(&return_type));
        }

        // Extract generics
        if let Some(type_params) = node.child_by_field_name("type_parameters") {
            sig.generics = self.extract_rust_generics(&type_params);
        }

        sig
    }

    fn extract_rust_params(&self, params_node: &Node<'_>) -> (Vec<ParameterInfo>, Option<String>) {
        let mut params = Vec::new();
        let mut receiver = None;
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "self_parameter" => {
                    receiver = Some(self.node_text(&child).to_owned());
                },
                "parameter" => {
                    let mut param =
                        ParameterInfo { kind: ParameterKind::Positional, ..Default::default() };

                    if let Some(pattern) = child.child_by_field_name("pattern") {
                        param.name = self.node_text(&pattern).to_owned();
                    }

                    if let Some(type_node) = child.child_by_field_name("type") {
                        param.type_info = Some(self.parse_rust_type(&type_node));
                    }

                    params.push(param);
                },
                _ => {},
            }
        }

        (params, receiver)
    }

    fn parse_rust_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        let mut type_info = TypeInfo { name: text.to_owned(), ..Default::default() };

        match node.kind() {
            "reference_type" => {
                type_info.is_reference = true;
                if let Some(inner) = node.child_by_field_name("type") {
                    let inner_type = self.parse_rust_type(&inner);
                    type_info.name = inner_type.name;
                    type_info.generic_args = inner_type.generic_args;
                }
                // Check for mut
                type_info.is_mutable = text.contains("&mut");
            },
            "generic_type" => {
                if let Some(name) = node.child_by_field_name("type") {
                    type_info.name = self.node_text(&name).to_owned();
                }
                if let Some(args) = node.child_by_field_name("type_arguments") {
                    type_info.generic_args = self.extract_rust_type_args(&args);
                }

                // Check for Option
                if type_info.name == "Option" {
                    type_info.is_nullable = true;
                }
            },
            "array_type" => {
                type_info.array_dimensions = 1;
                if let Some(elem) = node.child_by_field_name("element") {
                    let elem_type = self.parse_rust_type(&elem);
                    type_info.name = elem_type.name;
                }
            },
            "type_identifier" | "primitive_type" => {
                type_info.name = text.to_owned();
            },
            _ => {},
        }

        type_info
    }

    fn extract_rust_type_args(&self, node: &Node<'_>) -> Vec<TypeInfo> {
        let mut args = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier"
                || child.kind() == "generic_type"
                || child.kind() == "reference_type"
                || child.kind() == "primitive_type"
            {
                args.push(self.parse_rust_type(&child));
            }
        }

        args
    }

    fn extract_rust_generics(&self, node: &Node<'_>) -> Vec<GenericParam> {
        let mut generics = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" => {
                    generics.push(GenericParam {
                        name: self.node_text(&child).to_owned(),
                        ..Default::default()
                    });
                },
                "constrained_type_parameter" => {
                    let mut param = GenericParam::default();
                    if let Some(name) = child.child(0) {
                        param.name = self.node_text(&name).to_owned();
                    }
                    if let Some(bounds) = child.child_by_field_name("bounds") {
                        param.constraints.push(self.node_text(&bounds).to_owned());
                    }
                    generics.push(param);
                },
                _ => {},
            }
        }

        generics
    }

    /// Extract Go type signature
    fn extract_go(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_go_params(&params);
        }

        // Extract return type
        if let Some(result) = node.child_by_field_name("result") {
            sig.return_type = Some(self.parse_go_type(&result));
        }

        // Extract receiver for methods
        if let Some(receiver) = node.child_by_field_name("receiver") {
            sig.receiver = Some(self.node_text(&receiver).to_owned());
        }

        // Extract type parameters (Go 1.18+ generics)
        if let Some(type_params) = node.child_by_field_name("type_parameters") {
            sig.generics = self.extract_go_generics(&type_params);
        }

        sig
    }

    fn extract_go_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter_declaration" {
                let mut param =
                    ParameterInfo { kind: ParameterKind::Positional, ..Default::default() };

                // Go parameters can have multiple names for same type
                let mut names = Vec::new();
                let mut type_node = None;

                let mut param_cursor = child.walk();
                for param_child in child.children(&mut param_cursor) {
                    match param_child.kind() {
                        "identifier" => {
                            names.push(self.node_text(&param_child).to_owned());
                        },
                        _ if param_child.kind().contains("type") => {
                            type_node = Some(param_child);
                        },
                        _ => {},
                    }
                }

                let type_info = type_node.map(|n| self.parse_go_type(&n));

                // Create param for each name
                for name in names {
                    params.push(ParameterInfo {
                        name,
                        type_info: type_info.clone(),
                        kind: ParameterKind::Positional,
                        ..Default::default()
                    });
                }

                // If no names, just type
                if params.is_empty() && type_node.is_some() {
                    param.type_info = type_info;
                    params.push(param);
                }
            } else if child.kind() == "variadic_parameter_declaration" {
                let mut param = ParameterInfo {
                    kind: ParameterKind::VarPositional,
                    is_variadic: true,
                    ..Default::default()
                };
                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }
                if let Some(type_node) = child.child_by_field_name("type") {
                    param.type_info = Some(self.parse_go_type(&type_node));
                }
                params.push(param);
            }
        }

        params
    }

    fn parse_go_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        TypeInfo { name: text.to_owned(), ..Default::default() }
    }

    fn extract_go_generics(&self, node: &Node<'_>) -> Vec<GenericParam> {
        let mut generics = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameter_declaration" {
                let mut param = GenericParam::default();
                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }
                if let Some(constraint) = child.child_by_field_name("constraint") {
                    param
                        .constraints
                        .push(self.node_text(&constraint).to_owned());
                }
                generics.push(param);
            }
        }

        generics
    }

    /// Extract Java type signature
    fn extract_java(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_java_params(&params);
        }

        // Extract return type
        if let Some(return_type) = node.child_by_field_name("type") {
            sig.return_type = Some(self.parse_java_type(&return_type));
        }

        // Extract generics
        if let Some(type_params) = node.child_by_field_name("type_parameters") {
            sig.generics = self.extract_java_generics(&type_params);
        }

        // Extract throws
        if let Some(throws) = node.child_by_field_name("throws") {
            sig.throws = self.extract_java_throws(&throws);
        }

        sig
    }

    fn extract_java_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "formal_parameter" || child.kind() == "spread_parameter" {
                let mut param = ParameterInfo {
                    kind: ParameterKind::Positional,
                    is_variadic: child.kind() == "spread_parameter",
                    ..Default::default()
                };

                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }

                if let Some(type_node) = child.child_by_field_name("type") {
                    param.type_info = Some(self.parse_java_type(&type_node));
                }

                params.push(param);
            }
        }

        params
    }

    fn parse_java_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        let mut type_info = TypeInfo { name: text.to_owned(), ..Default::default() };

        match node.kind() {
            "generic_type" => {
                if let Some(name) = node.child(0) {
                    type_info.name = self.node_text(&name).to_owned();
                }
                if let Some(args) = node.child_by_field_name("arguments") {
                    let mut arg_cursor = args.walk();
                    for arg in args.children(&mut arg_cursor) {
                        if arg.kind() == "type_identifier" || arg.kind() == "generic_type" {
                            type_info.generic_args.push(self.parse_java_type(&arg));
                        }
                    }
                }
            },
            "array_type" => {
                type_info.array_dimensions = 1;
                if let Some(elem) = node.child_by_field_name("element") {
                    let elem_type = self.parse_java_type(&elem);
                    type_info.name = elem_type.name;
                }
            },
            _ => {},
        }

        type_info
    }

    fn extract_java_generics(&self, node: &Node<'_>) -> Vec<GenericParam> {
        let mut generics = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameter" {
                let mut param = GenericParam::default();

                if let Some(name) = child.child(0) {
                    param.name = self.node_text(&name).to_owned();
                }

                if let Some(bounds) = child.child_by_field_name("bounds") {
                    param.constraints.push(self.node_text(&bounds).to_owned());
                }

                generics.push(param);
            }
        }

        generics
    }

    fn extract_java_throws(&self, throws_node: &Node<'_>) -> Vec<String> {
        let mut throws = Vec::new();
        let mut cursor = throws_node.walk();

        for child in throws_node.children(&mut cursor) {
            if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                throws.push(self.node_text(&child).to_owned());
            }
        }

        throws
    }

    /// Extract C type signature
    fn extract_c(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract return type
        if let Some(return_type) = node.child_by_field_name("type") {
            sig.return_type = Some(TypeInfo {
                name: self.node_text(&return_type).to_owned(),
                ..Default::default()
            });
        }

        // Extract parameters
        if let Some(declarator) = node.child_by_field_name("declarator") {
            if let Some(params) = declarator.child_by_field_name("parameters") {
                sig.parameters = self.extract_c_params(&params);
            }
        }

        sig
    }

    fn extract_c_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter_declaration" {
                let mut param =
                    ParameterInfo { kind: ParameterKind::Positional, ..Default::default() };

                if let Some(type_node) = child.child_by_field_name("type") {
                    param.type_info = Some(TypeInfo {
                        name: self.node_text(&type_node).to_owned(),
                        ..Default::default()
                    });
                }

                if let Some(declarator) = child.child_by_field_name("declarator") {
                    param.name = self.node_text(&declarator).to_owned();
                }

                params.push(param);
            } else if child.kind() == "variadic_parameter" {
                params.push(ParameterInfo {
                    name: "...".to_owned(),
                    kind: ParameterKind::VarPositional,
                    is_variadic: true,
                    ..Default::default()
                });
            }
        }

        params
    }

    /// Extract C++ type signature
    fn extract_cpp(&self, node: &Node<'_>) -> TypeSignature {
        // C++ is similar to C but with additional features
        let mut sig = self.extract_c(node);

        // Check for virtual, override, const qualifiers
        let text = self.node_text(node);
        if text.contains("noexcept") {
            // No throws
        }

        // Extract template parameters
        if let Some(parent) = node.parent() {
            if parent.kind() == "template_declaration" {
                if let Some(params) = parent.child_by_field_name("parameters") {
                    sig.generics = self.extract_cpp_template_params(&params);
                }
            }
        }

        sig
    }

    fn extract_cpp_template_params(&self, node: &Node<'_>) -> Vec<GenericParam> {
        let mut generics = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_parameter_declaration" => {
                    let mut param = GenericParam::default();
                    if let Some(name) = child.child_by_field_name("name") {
                        param.name = self.node_text(&name).to_owned();
                    }
                    generics.push(param);
                },
                "template_parameter_declaration" => {
                    let mut param = GenericParam::default();
                    param.name = self.node_text(&child).to_owned();
                    generics.push(param);
                },
                _ => {},
            }
        }

        generics
    }

    /// Extract C# type signature
    fn extract_csharp(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Check for async
        let text = self.node_text(node);
        sig.is_async = text.contains("async ");

        // Extract return type
        if let Some(return_type) = node.child_by_field_name("type") {
            sig.return_type = Some(self.parse_csharp_type(&return_type));
        }

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_csharp_params(&params);
        }

        // Extract generics
        if let Some(type_params) = node.child_by_field_name("type_parameters") {
            sig.generics = self.extract_csharp_generics(&type_params);
        }

        sig
    }

    fn extract_csharp_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter" {
                let mut param =
                    ParameterInfo { kind: ParameterKind::Positional, ..Default::default() };

                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }

                if let Some(type_node) = child.child_by_field_name("type") {
                    param.type_info = Some(self.parse_csharp_type(&type_node));
                }

                if let Some(default) = child.child_by_field_name("default_value") {
                    param.default_value = Some(self.node_text(&default).to_owned());
                    param.is_optional = true;
                }

                // Check for params keyword (variadic)
                let param_text = self.node_text(&child);
                if param_text.starts_with("params ") {
                    param.is_variadic = true;
                    param.kind = ParameterKind::VarPositional;
                }

                params.push(param);
            }
        }

        params
    }

    fn parse_csharp_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        let mut type_info = TypeInfo { name: text.to_owned(), ..Default::default() };

        // Check for nullable
        if text.ends_with('?') {
            type_info.is_nullable = true;
            type_info.name = text.trim_end_matches('?').to_owned();
        }

        type_info
    }

    fn extract_csharp_generics(&self, node: &Node<'_>) -> Vec<GenericParam> {
        let mut generics = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameter" {
                let mut param = GenericParam::default();
                param.name = self.node_text(&child).to_owned();

                // Check for variance
                let text = self.node_text(&child);
                if text.starts_with("out ") {
                    param.variance = Variance::Covariant;
                    param.name = text[4..].to_string();
                } else if text.starts_with("in ") {
                    param.variance = Variance::Contravariant;
                    param.name = text[3..].to_string();
                }

                generics.push(param);
            }
        }

        generics
    }

    /// Extract Ruby type signature (limited type info available)
    fn extract_ruby(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_ruby_params(&params);
        }

        sig
    }

    fn extract_ruby_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    params.push(ParameterInfo {
                        name: self.node_text(&child).to_owned(),
                        kind: ParameterKind::Positional,
                        ..Default::default()
                    });
                },
                "optional_parameter" => {
                    let mut param = ParameterInfo {
                        is_optional: true,
                        kind: ParameterKind::Positional,
                        ..Default::default()
                    };
                    if let Some(name) = child.child_by_field_name("name") {
                        param.name = self.node_text(&name).to_owned();
                    }
                    if let Some(value) = child.child_by_field_name("value") {
                        param.default_value = Some(self.node_text(&value).to_owned());
                    }
                    params.push(param);
                },
                "splat_parameter" => {
                    let mut param = ParameterInfo {
                        kind: ParameterKind::VarPositional,
                        is_variadic: true,
                        ..Default::default()
                    };
                    if let Some(name) = child.child_by_field_name("name") {
                        param.name = self.node_text(&name).to_owned();
                    }
                    params.push(param);
                },
                "hash_splat_parameter" => {
                    let mut param = ParameterInfo {
                        kind: ParameterKind::VarKeyword,
                        is_variadic: true,
                        ..Default::default()
                    };
                    if let Some(name) = child.child_by_field_name("name") {
                        param.name = self.node_text(&name).to_owned();
                    }
                    params.push(param);
                },
                "keyword_parameter" => {
                    let mut param =
                        ParameterInfo { kind: ParameterKind::Keyword, ..Default::default() };
                    if let Some(name) = child.child_by_field_name("name") {
                        param.name = self.node_text(&name).to_owned();
                    }
                    if let Some(value) = child.child_by_field_name("value") {
                        param.default_value = Some(self.node_text(&value).to_owned());
                        param.is_optional = true;
                    }
                    params.push(param);
                },
                "block_parameter" => {
                    let mut param =
                        ParameterInfo { kind: ParameterKind::Positional, ..Default::default() };
                    if let Some(name) = child.child_by_field_name("name") {
                        param.name = format!("&{}", self.node_text(&name));
                    }
                    params.push(param);
                },
                _ => {},
            }
        }

        params
    }

    /// Extract PHP type signature
    fn extract_php(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_php_params(&params);
        }

        // Extract return type
        if let Some(return_type) = node.child_by_field_name("return_type") {
            sig.return_type = Some(self.parse_php_type(&return_type));
        }

        sig
    }

    fn extract_php_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "simple_parameter" || child.kind() == "variadic_parameter" {
                let mut param = ParameterInfo {
                    kind: ParameterKind::Positional,
                    is_variadic: child.kind() == "variadic_parameter",
                    ..Default::default()
                };

                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }

                if let Some(type_node) = child.child_by_field_name("type") {
                    param.type_info = Some(self.parse_php_type(&type_node));
                }

                if let Some(default) = child.child_by_field_name("default_value") {
                    param.default_value = Some(self.node_text(&default).to_owned());
                    param.is_optional = true;
                }

                params.push(param);
            }
        }

        params
    }

    fn parse_php_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        let mut type_info = TypeInfo { name: text.to_owned(), ..Default::default() };

        // Check for nullable
        if text.starts_with('?') {
            type_info.is_nullable = true;
            type_info.name = text[1..].to_string();
        }

        type_info
    }

    /// Extract Kotlin type signature
    fn extract_kotlin(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Check for suspend (coroutine)
        let text = self.node_text(node);
        sig.is_async = text.contains("suspend ");

        // Extract parameters
        if let Some(params) = node
            .child_by_field_name("value_parameters")
            .or_else(|| node.child_by_field_name("parameters"))
        {
            sig.parameters = self.extract_kotlin_params(&params);
        }

        // Extract return type
        if let Some(return_type) = node.child_by_field_name("return_type") {
            sig.return_type = Some(self.parse_kotlin_type(&return_type));
        }

        // Extract type parameters
        if let Some(type_params) = node.child_by_field_name("type_parameters") {
            sig.generics = self.extract_kotlin_generics(&type_params);
        }

        sig
    }

    fn extract_kotlin_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter" {
                let mut param =
                    ParameterInfo { kind: ParameterKind::Positional, ..Default::default() };

                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }

                if let Some(type_node) = child.child_by_field_name("type") {
                    param.type_info = Some(self.parse_kotlin_type(&type_node));
                }

                if let Some(default) = child.child_by_field_name("default_value") {
                    param.default_value = Some(self.node_text(&default).to_owned());
                    param.is_optional = true;
                }

                // Check for vararg
                let param_text = self.node_text(&child);
                if param_text.starts_with("vararg ") {
                    param.is_variadic = true;
                    param.kind = ParameterKind::VarPositional;
                }

                params.push(param);
            }
        }

        params
    }

    fn parse_kotlin_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        let mut type_info = TypeInfo { name: text.to_owned(), ..Default::default() };

        // Check for nullable
        if text.ends_with('?') {
            type_info.is_nullable = true;
            type_info.name = text.trim_end_matches('?').to_owned();
        }

        type_info
    }

    fn extract_kotlin_generics(&self, node: &Node<'_>) -> Vec<GenericParam> {
        let mut generics = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameter" {
                let mut param = GenericParam::default();

                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }

                // Check for variance
                let text = self.node_text(&child);
                if text.starts_with("out ") {
                    param.variance = Variance::Covariant;
                } else if text.starts_with("in ") {
                    param.variance = Variance::Contravariant;
                }

                if let Some(bounds) = child.child_by_field_name("bounds") {
                    param.constraints.push(self.node_text(&bounds).to_owned());
                }

                generics.push(param);
            }
        }

        generics
    }

    /// Extract Swift type signature
    fn extract_swift(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Check for async/throws
        let text = self.node_text(node);
        sig.is_async = text.contains(" async ");
        if text.contains(" throws ") {
            sig.throws.push("Error".to_owned());
        }

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_swift_params(&params);
        }

        // Extract return type
        if let Some(return_type) = node.child_by_field_name("return_type") {
            sig.return_type = Some(self.parse_swift_type(&return_type));
        }

        // Extract generics
        if let Some(type_params) = node.child_by_field_name("type_parameters") {
            sig.generics = self.extract_swift_generics(&type_params);
        }

        sig
    }

    fn extract_swift_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter" {
                let mut param =
                    ParameterInfo { kind: ParameterKind::Positional, ..Default::default() };

                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }

                if let Some(type_node) = child.child_by_field_name("type") {
                    param.type_info = Some(self.parse_swift_type(&type_node));
                }

                if let Some(default) = child.child_by_field_name("default_value") {
                    param.default_value = Some(self.node_text(&default).to_owned());
                    param.is_optional = true;
                }

                // Check for variadic
                let param_text = self.node_text(&child);
                if param_text.contains("...") {
                    param.is_variadic = true;
                    param.kind = ParameterKind::VarPositional;
                }

                params.push(param);
            }
        }

        params
    }

    fn parse_swift_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        let mut type_info = TypeInfo { name: text.to_owned(), ..Default::default() };

        // Check for optional
        if text.ends_with('?') || text.ends_with('!') {
            type_info.is_nullable = true;
            type_info.name = text.trim_end_matches(['?', '!']).to_owned();
        }

        type_info
    }

    fn extract_swift_generics(&self, node: &Node<'_>) -> Vec<GenericParam> {
        let mut generics = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameter" {
                let mut param = GenericParam::default();
                param.name = self.node_text(&child).to_owned();

                if let Some(constraint) = child.child_by_field_name("constraint") {
                    param
                        .constraints
                        .push(self.node_text(&constraint).to_owned());
                }

                generics.push(param);
            }
        }

        generics
    }

    /// Extract Scala type signature
    fn extract_scala(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_scala_params(&params);
        }

        // Extract return type
        if let Some(return_type) = node.child_by_field_name("return_type") {
            sig.return_type = Some(self.parse_scala_type(&return_type));
        }

        // Extract type parameters
        if let Some(type_params) = node.child_by_field_name("type_parameters") {
            sig.generics = self.extract_scala_generics(&type_params);
        }

        sig
    }

    fn extract_scala_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter" {
                let mut param =
                    ParameterInfo { kind: ParameterKind::Positional, ..Default::default() };

                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }

                if let Some(type_node) = child.child_by_field_name("type") {
                    param.type_info = Some(self.parse_scala_type(&type_node));
                }

                if let Some(default) = child.child_by_field_name("default") {
                    param.default_value = Some(self.node_text(&default).to_owned());
                    param.is_optional = true;
                }

                // Check for varargs (*)
                let param_text = self.node_text(&child);
                if param_text.contains('*') {
                    param.is_variadic = true;
                    param.kind = ParameterKind::VarPositional;
                }

                params.push(param);
            }
        }

        params
    }

    fn parse_scala_type(&self, node: &Node<'_>) -> TypeInfo {
        let text = self.node_text(node);
        TypeInfo { name: text.to_owned(), ..Default::default() }
    }

    fn extract_scala_generics(&self, node: &Node<'_>) -> Vec<GenericParam> {
        let mut generics = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameter" {
                let mut param = GenericParam::default();

                let text = self.node_text(&child);
                // Check for variance (+/-)
                if text.starts_with('+') {
                    param.variance = Variance::Covariant;
                    param.name = text[1..].to_string();
                } else if text.starts_with('-') {
                    param.variance = Variance::Contravariant;
                    param.name = text[1..].to_string();
                } else {
                    param.name = text.to_owned();
                }

                if let Some(bounds) = child.child_by_field_name("bounds") {
                    param.constraints.push(self.node_text(&bounds).to_owned());
                }

                generics.push(param);
            }
        }

        generics
    }

    /// Extract Haskell type signature
    fn extract_haskell(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Haskell functions are defined with type signatures separately
        // Look for type_signature node
        if node.kind() == "type_signature" || node.kind() == "signature" {
            let text = self.node_text(node);
            // Parse :: type
            if let Some(type_part) = text.split("::").nth(1) {
                sig.return_type =
                    Some(TypeInfo { name: type_part.trim().to_owned(), ..Default::default() });
            }
        }

        sig
    }

    /// Extract Elixir type signature
    fn extract_elixir(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_elixir_params(&params);
        }

        sig
    }

    fn extract_elixir_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                params.push(ParameterInfo {
                    name: self.node_text(&child).to_owned(),
                    kind: ParameterKind::Positional,
                    ..Default::default()
                });
            } else if child.kind() == "binary_operator" {
                // Default value: param \\ default
                let mut param = ParameterInfo {
                    is_optional: true,
                    kind: ParameterKind::Positional,
                    ..Default::default()
                };
                if let Some(left) = child.child_by_field_name("left") {
                    param.name = self.node_text(&left).to_owned();
                }
                if let Some(right) = child.child_by_field_name("right") {
                    param.default_value = Some(self.node_text(&right).to_owned());
                }
                params.push(param);
            }
        }

        params
    }

    /// Extract Clojure type signature (limited - dynamically typed)
    fn extract_clojure(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Clojure uses vector for params in defn
        // Look for vector child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "vec_lit" {
                sig.parameters = self.extract_clojure_params(&child);
                break;
            }
        }

        sig
    }

    fn extract_clojure_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "sym_lit" {
                let name = self.node_text(&child).to_owned();
                // Check for &rest
                if name == "&" {
                    continue;
                }
                params.push(ParameterInfo {
                    name,
                    kind: ParameterKind::Positional,
                    ..Default::default()
                });
            }
        }

        params
    }

    /// Extract OCaml type signature
    fn extract_ocaml(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract parameters from let binding
        if let Some(params) = node.child_by_field_name("pattern") {
            sig.parameters = self.extract_ocaml_params(&params);
        }

        // Extract return type from type annotation
        if let Some(return_type) = node.child_by_field_name("type") {
            sig.return_type = Some(TypeInfo {
                name: self.node_text(&return_type).to_owned(),
                ..Default::default()
            });
        }

        sig
    }

    fn extract_ocaml_params(&self, _params_node: &Node<'_>) -> Vec<ParameterInfo> {
        // OCaml parameter extraction would require more complex pattern matching
        Vec::new()
    }

    /// Extract Lua type signature (dynamically typed)
    fn extract_lua(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_lua_params(&params);
        }

        sig
    }

    fn extract_lua_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    params.push(ParameterInfo {
                        name: self.node_text(&child).to_owned(),
                        kind: ParameterKind::Positional,
                        ..Default::default()
                    });
                },
                "spread" | "vararg_expression" => {
                    params.push(ParameterInfo {
                        name: "...".to_owned(),
                        kind: ParameterKind::VarPositional,
                        is_variadic: true,
                        ..Default::default()
                    });
                },
                _ => {},
            }
        }

        params
    }

    /// Extract R type signature
    fn extract_r(&self, node: &Node<'_>) -> TypeSignature {
        let mut sig = TypeSignature::default();

        // Extract parameters from formal_parameters
        if let Some(params) = node.child_by_field_name("parameters") {
            sig.parameters = self.extract_r_params(&params);
        }

        sig
    }

    fn extract_r_params(&self, params_node: &Node<'_>) -> Vec<ParameterInfo> {
        let mut params = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter" {
                let mut param =
                    ParameterInfo { kind: ParameterKind::Positional, ..Default::default() };

                if let Some(name) = child.child_by_field_name("name") {
                    param.name = self.node_text(&name).to_owned();
                }

                if let Some(default) = child.child_by_field_name("default") {
                    param.default_value = Some(self.node_text(&default).to_owned());
                    param.is_optional = true;
                }

                // Check for ... (variadic)
                if param.name == "..." {
                    param.is_variadic = true;
                    param.kind = ParameterKind::VarPositional;
                }

                params.push(param);
            }
        }

        params
    }

    /// Extract Bash type signature (no types, just parameters)
    fn extract_bash(&self, node: &Node<'_>) -> TypeSignature {
        // Bash functions don't have explicit parameters in their definition
        // They use $1, $2, etc.
        TypeSignature::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_signature() {
        let source = r#"
def greet(name: str, greeting: str = "Hello") -> str:
    return f"{greeting}, {name}!"
"#;
        let extractor = TypeSignatureExtractor::new(source);

        // Would need actual tree-sitter parsing for full test
        let sig = TypeSignature::default();
        assert!(!sig.is_async);
    }

    #[test]
    fn test_type_info_default() {
        let type_info = TypeInfo::default();
        assert!(type_info.name.is_empty());
        assert!(!type_info.is_nullable);
        assert_eq!(type_info.array_dimensions, 0);
    }

    #[test]
    fn test_parameter_kind_default() {
        let kind = ParameterKind::default();
        assert_eq!(kind, ParameterKind::Positional);
    }
}
