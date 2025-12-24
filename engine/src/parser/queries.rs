//! Tree-sitter query strings for symbol extraction
//!
//! This module contains the tree-sitter queries used to extract symbols
//! from source code in various programming languages.

/// Python symbol extraction query
pub const PYTHON: &str = r#"
    (function_definition
      name: (identifier) @name) @function

    (class_definition
      name: (identifier) @name) @class

    (class_definition
      body: (block
        (function_definition
          name: (identifier) @name))) @method
"#;

/// JavaScript/JSX symbol extraction query
pub const JAVASCRIPT: &str = r#"
    (function_declaration
      name: (_) @name) @function

    (class_declaration
      name: (identifier) @name) @class

    (method_definition
      name: (property_identifier) @name) @method

    (arrow_function) @function

    (variable_declarator
      name: (identifier) @name
      value: (arrow_function)) @function

    (export_statement
      (function_declaration
        name: (identifier) @name)) @function
"#;

/// TypeScript symbol extraction query
pub const TYPESCRIPT: &str = r#"
    (function_declaration
      name: (identifier) @name) @function

    (class_declaration
      name: (type_identifier) @name) @class

    (method_definition
      name: (property_identifier) @name) @method

    (interface_declaration
      name: (type_identifier) @name) @interface

    (type_alias_declaration
      name: (type_identifier) @name) @type

    (arrow_function) @function

    (variable_declarator
      name: (identifier) @name
      value: (arrow_function)) @function
"#;

/// Rust symbol extraction query
pub const RUST: &str = r#"
    (function_item
      name: (identifier) @name) @function

    (struct_item
      name: (type_identifier) @name) @struct

    (enum_item
      name: (type_identifier) @name) @enum

    (impl_item
      type: (type_identifier) @name) @impl

    (trait_item
      name: (type_identifier) @name) @trait

    (mod_item
      name: (identifier) @name) @module

    (macro_definition
      name: (identifier) @name) @macro
"#;

/// Go symbol extraction query
pub const GO: &str = r#"
    (function_declaration
      name: (identifier) @name) @function

    (method_declaration
      name: (field_identifier) @name) @method

    (type_declaration
      (type_spec
        name: (type_identifier) @name)) @type

    (type_declaration
      (type_spec
        name: (type_identifier) @name
        type: (struct_type))) @struct

    (type_declaration
      (type_spec
        name: (type_identifier) @name
        type: (interface_type))) @interface
"#;

/// Java symbol extraction query
pub const JAVA: &str = r#"
    (method_declaration
      name: (identifier) @name) @method

    (class_declaration
      name: (identifier) @name) @class

    (interface_declaration
      name: (identifier) @name) @interface

    (enum_declaration
      name: (identifier) @name) @enum

    (constructor_declaration
      name: (identifier) @name) @constructor
"#;

/// C symbol extraction query
pub const C: &str = r#"
    (function_definition
      declarator: (function_declarator
        declarator: (identifier) @name)) @function

    (declaration
      declarator: (function_declarator
        declarator: (identifier) @name)) @function

    (struct_specifier
      name: (type_identifier) @name) @struct

    (enum_specifier
      name: (type_identifier) @name) @enum

    (type_definition
      declarator: (type_identifier) @name) @type
"#;

/// C++ symbol extraction query
pub const CPP: &str = r#"
    (function_definition
      declarator: (function_declarator
        declarator: (identifier) @name)) @function

    (function_definition
      declarator: (function_declarator
        declarator: (qualified_identifier
          name: (identifier) @name))) @function

    (class_specifier
      name: (type_identifier) @name) @class

    (struct_specifier
      name: (type_identifier) @name) @struct

    (enum_specifier
      name: (type_identifier) @name) @enum

    (namespace_definition
      name: (identifier) @name) @namespace

    (template_declaration
      (function_definition
        declarator: (function_declarator
          declarator: (identifier) @name))) @function
"#;

/// C# symbol extraction query
pub const CSHARP: &str = r#"
    (method_declaration
      name: (identifier) @name) @method

    (class_declaration
      name: (identifier) @name) @class

    (interface_declaration
      name: (identifier) @name) @interface

    (struct_declaration
      name: (identifier) @name) @struct

    (enum_declaration
      name: (identifier) @name) @enum

    (property_declaration
      name: (identifier) @name) @property

    (namespace_declaration
      name: (identifier) @name) @namespace
"#;

/// Ruby symbol extraction query
pub const RUBY: &str = r#"
    (method
      name: (identifier) @name) @method

    (singleton_method
      name: (identifier) @name) @method

    (class
      name: (constant) @name) @class

    (module
      name: (constant) @name) @module

    (alias
      name: (identifier) @name) @alias
"#;

/// Bash/Shell symbol extraction query
pub const BASH: &str = r#"
    (function_definition
      name: (word) @name) @function
"#;

/// PHP symbol extraction query
pub const PHP: &str = r#"
    (function_definition
      name: (name) @name) @function

    (method_declaration
      name: (name) @name) @method

    (class_declaration
      name: (name) @name) @class

    (interface_declaration
      name: (name) @name) @interface

    (trait_declaration
      name: (name) @name) @trait
"#;

/// Kotlin symbol extraction query
pub const KOTLIN: &str = r#"
    (function_declaration
      (simple_identifier) @name) @function

    (class_declaration
      (type_identifier) @name) @class

    (object_declaration
      (type_identifier) @name) @class

    (interface_declaration
      (type_identifier) @name) @interface
"#;

/// Swift symbol extraction query
pub const SWIFT: &str = r#"
    (function_declaration
      name: (simple_identifier) @name) @function

    (class_declaration
      name: (type_identifier) @name) @class

    (struct_declaration
      name: (type_identifier) @name) @struct

    (enum_declaration
      name: (type_identifier) @name) @enum

    (protocol_declaration
      name: (type_identifier) @name) @protocol
"#;

/// Scala symbol extraction query
pub const SCALA: &str = r#"
    (function_definition
      name: (identifier) @name) @function

    (class_definition
      name: (identifier) @name) @class

    (object_definition
      name: (identifier) @name) @object

    (trait_definition
      name: (identifier) @name) @trait
"#;

/// Haskell symbol extraction query
pub const HASKELL: &str = r#"
    (function
      name: (variable) @name) @function

    (signature
      name: (variable) @name) @signature

    (type_synomym
      name: (type) @name) @type

    (newtype
      name: (type) @name) @newtype

    (adt
      name: (type) @name) @data

    (class
      name: (type) @name) @class
"#;

/// Elixir symbol extraction query
pub const ELIXIR: &str = r#"
    (call
      target: (identifier) @keyword
      (arguments
        (call
          target: (identifier) @name)))

    (call
      target: (identifier) @keyword
      (arguments
        (identifier) @name))

    (call
      target: (identifier) @keyword
      (arguments
        (alias) @name))
"#;

/// Clojure symbol extraction query
pub const CLOJURE: &str = r#"
    (list_lit
      (sym_lit) @keyword
      (sym_lit) @name)
"#;

/// OCaml symbol extraction query
pub const OCAML: &str = r#"
    (value_definition
      (let_binding
        pattern: (value_name) @name)) @function

    (type_definition
      (type_binding
        name: (type_constructor) @name)) @type

    (module_definition
      (module_binding
        name: (module_name) @name)) @module
"#;

/// Lua symbol extraction query
pub const LUA: &str = r#"
    (function_declaration
      name: (identifier) @name) @function

    (function_declaration
      name: (dot_index_expression
        field: (identifier) @name)) @function

    (local_function_declaration
      name: (identifier) @name) @function

    (function_definition_statement
      name: (identifier) @name) @function
"#;

/// R symbol extraction query
pub const R: &str = r#"
    (left_assignment
      name: (identifier) @name
      value: (function_definition)) @function

    (equals_assignment
      name: (identifier) @name
      value: (function_definition)) @function
"#;

#[cfg(test)]
mod tests {
    use super::*;

    // These are compile-time constant strings; verifying they're non-empty
    // documents the expectation even if clippy optimizes it away.
    #[allow(clippy::const_is_empty)]
    #[test]
    fn test_query_strings_not_empty() {
        assert!(!PYTHON.is_empty());
        assert!(!JAVASCRIPT.is_empty());
        assert!(!TYPESCRIPT.is_empty());
        assert!(!RUST.is_empty());
        assert!(!GO.is_empty());
        assert!(!JAVA.is_empty());
        assert!(!C.is_empty());
        assert!(!CPP.is_empty());
        assert!(!CSHARP.is_empty());
        assert!(!RUBY.is_empty());
        assert!(!BASH.is_empty());
        assert!(!PHP.is_empty());
        assert!(!KOTLIN.is_empty());
        assert!(!SWIFT.is_empty());
        assert!(!SCALA.is_empty());
        assert!(!HASKELL.is_empty());
        assert!(!ELIXIR.is_empty());
        assert!(!CLOJURE.is_empty());
        assert!(!OCAML.is_empty());
        assert!(!LUA.is_empty());
        assert!(!R.is_empty());
    }
}
