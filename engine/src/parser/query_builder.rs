//! Tree-sitter query builders for symbol extraction
//!
//! This module contains functions that create compiled tree-sitter queries
//! for extracting symbols from source code. Each language has two query types:
//!
//! - **Basic queries** (`*_query`): Extract only symbol definitions
//! - **Super-queries** (`*_super_query`): Extract symbols AND imports in one pass
//!
//! Super-queries are more efficient for full file analysis as they require
//! only a single AST traversal.

use super::core::ParserError;
use tree_sitter::Query;

// ==========================================================================
// Python
// ==========================================================================

pub fn python_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_definition
          name: (identifier) @name) @function

        (class_definition
          name: (identifier) @name) @class

        (class_definition
          body: (block
            (function_definition
              name: (identifier) @name))) @method
    "#;

    Query::new(&tree_sitter_python::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn python_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_definition
          name: (identifier) @name) @function

        ; Classes
        (class_definition
          name: (identifier) @name) @class

        ; Methods inside classes
        (class_definition
          body: (block
            (function_definition
              name: (identifier) @name))) @method

        ; Imports
        (import_statement) @import
        (import_from_statement) @import
    "#;

    Query::new(&tree_sitter_python::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// JavaScript
// ==========================================================================

pub fn javascript_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_declaration
          name: (_) @name) @function

        (class_declaration
          name: (_) @name) @class

        (method_definition
          name: (property_identifier) @name) @method

        (arrow_function) @function

        (function_expression) @function
    "#;

    Query::new(&tree_sitter_javascript::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn javascript_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_declaration
          name: (identifier) @name) @function

        ; Classes
        (class_declaration
          name: (identifier) @name) @class

        ; Methods
        (method_definition
          name: (property_identifier) @name) @method

        ; Arrow functions (named via variable)
        (lexical_declaration
          (variable_declarator
            name: (identifier) @name
            value: (arrow_function))) @function

        ; Imports
        (import_statement) @import
    "#;

    Query::new(&tree_sitter_javascript::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// TypeScript
// ==========================================================================

pub fn typescript_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_declaration
          name: (identifier) @name) @function

        (class_declaration
          name: (type_identifier) @name) @class

        (interface_declaration
          name: (type_identifier) @name) @interface

        (method_definition
          name: (property_identifier) @name) @method

        (enum_declaration
          name: (identifier) @name) @enum

        ; Arrow functions (named via variable) - Bug #1 fix
        (lexical_declaration
          (variable_declarator
            name: (identifier) @name
            value: (arrow_function))) @function
    "#;

    Query::new(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn typescript_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_declaration
          name: (identifier) @name) @function

        ; Classes
        (class_declaration
          name: (type_identifier) @name) @class

        ; Interfaces
        (interface_declaration
          name: (type_identifier) @name) @interface

        ; Methods
        (method_definition
          name: (property_identifier) @name) @method

        ; Enums
        (enum_declaration
          name: (identifier) @name) @enum

        ; Arrow functions (named via variable) - Bug #1 fix
        (lexical_declaration
          (variable_declarator
            name: (identifier) @name
            value: (arrow_function))) @function

        ; Arrow functions (exported)
        (export_statement
          declaration: (lexical_declaration
            (variable_declarator
              name: (identifier) @name
              value: (arrow_function)))) @function

        ; Type aliases
        (type_alias_declaration
          name: (type_identifier) @name) @struct

        ; Imports
        (import_statement) @import

        ; Exports (re-exports)
        (export_statement) @export
    "#;

    Query::new(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Rust
// ==========================================================================

pub fn rust_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_item
          name: (identifier) @name) @function

        (struct_item
          name: (type_identifier) @name) @struct

        (enum_item
          name: (type_identifier) @name) @enum

        (trait_item
          name: (type_identifier) @name) @trait
    "#;

    Query::new(&tree_sitter_rust::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn rust_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_item
          name: (identifier) @name) @function

        ; Structs
        (struct_item
          name: (type_identifier) @name) @struct

        ; Enums
        (enum_item
          name: (type_identifier) @name) @enum

        ; Traits
        (trait_item
          name: (type_identifier) @name) @trait

        ; Use statements (imports)
        (use_declaration) @import
    "#;

    Query::new(&tree_sitter_rust::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Go
// ==========================================================================

pub fn go_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_declaration
          name: (identifier) @name) @function

        (method_declaration
          name: (field_identifier) @name) @method

        (type_declaration
          (type_spec
            name: (type_identifier) @name
            type: (struct_type))) @struct

        (type_declaration
          (type_spec
            name: (type_identifier) @name
            type: (interface_type))) @interface
    "#;

    Query::new(&tree_sitter_go::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn go_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_declaration
          name: (identifier) @name) @function

        ; Methods
        (method_declaration
          name: (field_identifier) @name) @method

        ; Structs
        (type_declaration
          (type_spec
            name: (type_identifier) @name
            type: (struct_type))) @struct

        ; Interfaces
        (type_declaration
          (type_spec
            name: (type_identifier) @name
            type: (interface_type))) @interface

        ; Imports
        (import_declaration) @import
    "#;

    Query::new(&tree_sitter_go::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Java
// ==========================================================================

pub fn java_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (method_declaration
          name: (identifier) @name) @method

        (class_declaration
          name: (identifier) @name) @class

        (interface_declaration
          name: (identifier) @name) @interface

        (enum_declaration
          name: (identifier) @name) @enum
    "#;

    Query::new(&tree_sitter_java::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn java_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Methods
        (method_declaration
          name: (identifier) @name) @method

        ; Classes
        (class_declaration
          name: (identifier) @name) @class

        ; Interfaces
        (interface_declaration
          name: (identifier) @name) @interface

        ; Enums
        (enum_declaration
          name: (identifier) @name) @enum

        ; Imports
        (import_declaration) @import
    "#;

    Query::new(&tree_sitter_java::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// C
// ==========================================================================

pub fn c_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_definition
          declarator: (function_declarator
            declarator: (identifier) @name)) @function

        (struct_specifier
          name: (type_identifier) @name) @struct

        (enum_specifier
          name: (type_identifier) @name) @enum
    "#;

    Query::new(&tree_sitter_c::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn c_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_definition
          declarator: (function_declarator
            declarator: (identifier) @name)) @function

        ; Structs
        (struct_specifier
          name: (type_identifier) @name) @struct

        ; Enums
        (enum_specifier
          name: (type_identifier) @name) @enum

        ; Includes (imports)
        (preproc_include) @import
    "#;

    Query::new(&tree_sitter_c::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// C++
// ==========================================================================

pub fn cpp_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_definition
          declarator: (function_declarator
            declarator: (identifier) @name)) @function

        (class_specifier
          name: (type_identifier) @name) @class

        (struct_specifier
          name: (type_identifier) @name) @struct

        (enum_specifier
          name: (type_identifier) @name) @enum
    "#;

    Query::new(&tree_sitter_cpp::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn cpp_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_definition
          declarator: (function_declarator
            declarator: (identifier) @name)) @function

        ; Classes
        (class_specifier
          name: (type_identifier) @name) @class

        ; Structs
        (struct_specifier
          name: (type_identifier) @name) @struct

        ; Enums
        (enum_specifier
          name: (type_identifier) @name) @enum

        ; Includes (imports)
        (preproc_include) @import
    "#;

    Query::new(&tree_sitter_cpp::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// C#
// ==========================================================================

pub fn csharp_query() -> Result<Query, ParserError> {
    let query_string = r#"
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
    "#;

    Query::new(&tree_sitter_c_sharp::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn csharp_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Methods
        (method_declaration
          name: (identifier) @name) @method

        ; Classes
        (class_declaration
          name: (identifier) @name) @class

        ; Interfaces
        (interface_declaration
          name: (identifier) @name) @interface

        ; Structs
        (struct_declaration
          name: (identifier) @name) @struct

        ; Enums
        (enum_declaration
          name: (identifier) @name) @enum

        ; Imports (using directives)
        (using_directive) @import
    "#;

    Query::new(&tree_sitter_c_sharp::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Ruby
// ==========================================================================

pub fn ruby_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (method
          name: (identifier) @name) @function

        (class
          name: (constant) @name) @class

        (module
          name: (constant) @name) @class
    "#;

    Query::new(&tree_sitter_ruby::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn ruby_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Methods
        (method
          name: (identifier) @name) @function

        ; Classes
        (class
          name: (constant) @name) @class

        ; Modules
        (module
          name: (constant) @name) @class

        ; Requires (imports)
        (call
          method: (identifier) @_method
          (#match? @_method "^require")
          arguments: (argument_list)) @import
    "#;

    Query::new(&tree_sitter_ruby::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Bash
// ==========================================================================

pub fn bash_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_definition
          name: (word) @name) @function
    "#;

    Query::new(&tree_sitter_bash::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn bash_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_definition
          name: (word) @name) @function

        ; Source commands (imports)
        (command
          name: (command_name) @_cmd
          (#match? @_cmd "^(source|\\.)$")
          argument: (word)) @import
    "#;

    Query::new(&tree_sitter_bash::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// PHP
// ==========================================================================

pub fn php_query() -> Result<Query, ParserError> {
    let query_string = r#"
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

    Query::new(&tree_sitter_php::LANGUAGE_PHP.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn php_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_definition
          name: (name) @name) @function

        ; Methods
        (method_declaration
          name: (name) @name) @method

        ; Classes
        (class_declaration
          name: (name) @name) @class

        ; Interfaces
        (interface_declaration
          name: (name) @name) @interface

        ; Traits
        (trait_declaration
          name: (name) @name) @trait

        ; Use statements (imports)
        (namespace_use_declaration) @import
    "#;

    Query::new(&tree_sitter_php::LANGUAGE_PHP.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Kotlin
// ==========================================================================

pub fn kotlin_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_declaration
          name: (_) @name) @function

        (class_declaration
          name: (_) @name) @class

        (object_declaration
          name: (_) @name) @class
    "#;

    Query::new(&tree_sitter_kotlin_ng::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn kotlin_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_declaration
          name: (_) @name) @function

        ; Classes
        (class_declaration
          name: (_) @name) @class

        ; Objects
        (object_declaration
          name: (_) @name) @class

        ; Imports
        (import) @import
    "#;

    Query::new(&tree_sitter_kotlin_ng::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Swift
// ==========================================================================

pub fn swift_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_declaration
          name: (simple_identifier) @name) @function

        (class_declaration
          declaration_kind: "class"
          name: (type_identifier) @name) @class

        (protocol_declaration
          name: (type_identifier) @name) @interface

        (class_declaration
          declaration_kind: "struct"
          name: (type_identifier) @name) @struct

        (class_declaration
          declaration_kind: "enum"
          name: (type_identifier) @name) @enum
    "#;

    Query::new(&tree_sitter_swift::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn swift_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_declaration
          name: (simple_identifier) @name) @function

        ; Classes
        (class_declaration
          declaration_kind: "class"
          name: (type_identifier) @name) @class

        ; Protocols (interfaces)
        (protocol_declaration
          name: (type_identifier) @name) @interface

        ; Structs
        (class_declaration
          declaration_kind: "struct"
          name: (type_identifier) @name) @struct

        ; Enums
        (class_declaration
          declaration_kind: "enum"
          name: (type_identifier) @name) @enum

        ; Imports
        (import_declaration) @import
    "#;

    Query::new(&tree_sitter_swift::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Scala
// ==========================================================================

pub fn scala_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_definition
          name: (identifier) @name) @function

        (class_definition
          name: (identifier) @name) @class

        (object_definition
          name: (identifier) @name) @class

        (trait_definition
          name: (identifier) @name) @trait
    "#;

    Query::new(&tree_sitter_scala::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn scala_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function_definition
          name: (identifier) @name) @function

        ; Classes
        (class_definition
          name: (identifier) @name) @class

        ; Objects
        (object_definition
          name: (identifier) @name) @class

        ; Traits
        (trait_definition
          name: (identifier) @name) @trait

        ; Imports
        (import_declaration) @import
    "#;

    Query::new(&tree_sitter_scala::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Haskell
// ==========================================================================

pub fn haskell_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function
          name: (variable) @name) @function

        (signature
          name: (variable) @name) @function

        (function
          name: (prefix_id) @name) @function

        (signature
          name: (prefix_id) @name) @function

        (newtype
          name: (name) @name) @struct

        (type_synomym
          name: (name) @name) @struct

        (data_type
          name: (name) @name) @enum
    "#;

    Query::new(&tree_sitter_haskell::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn haskell_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (function
          name: (variable) @name) @function

        ; Type signatures
        (signature
          name: (variable) @name) @function

        ; Type aliases
        (function
          name: (prefix_id) @name) @function

        (signature
          name: (prefix_id) @name) @function

        ; Newtypes
        (newtype
          name: (name) @name) @struct

        ; ADTs (data declarations)
        (type_synomym
          name: (name) @name) @struct

        (data_type
          name: (name) @name) @enum

        ; Imports
        (import) @import
    "#;

    Query::new(&tree_sitter_haskell::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Elixir
// ==========================================================================

pub fn elixir_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (call
          target: (identifier) @_type
          (#match? @_type "^(def|defp|defmacro|defmacrop)$")
          (arguments
            (call
              target: (identifier) @name))) @function

        (call
          target: (identifier) @_type
          (#match? @_type "^defmodule$")
          (arguments
            (alias) @name)) @class
    "#;

    Query::new(&tree_sitter_elixir::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn elixir_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions (def, defp, defmacro)
        (call
          target: (identifier) @_type
          (#match? @_type "^(def|defp|defmacro|defmacrop)$")
          (arguments
            (call
              target: (identifier) @name))) @function

        ; Modules
        (call
          target: (identifier) @_type
          (#match? @_type "^defmodule$")
          (arguments
            (alias) @name)) @class

        ; Imports (alias, import, use, require)
        (call
          target: (identifier) @_type
          (#match? @_type "^(alias|import|use|require)$")) @import
    "#;

    Query::new(&tree_sitter_elixir::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Clojure
// ==========================================================================

pub fn clojure_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (list_lit
          (sym_lit) @_type
          (#match? @_type "^(defn|defn-|defmacro)$")
          (sym_lit) @name) @function

        (list_lit
          (sym_lit) @_type
          (#match? @_type "^(defrecord|deftype|defprotocol)$")
          (sym_lit) @name) @class
    "#;

    Query::new(&tree_sitter_clojure::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn clojure_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions
        (list_lit
          (sym_lit) @_type
          (#match? @_type "^(defn|defn-|defmacro)$")
          (sym_lit) @name) @function

        ; Records/Types/Protocols
        (list_lit
          (sym_lit) @_type
          (#match? @_type "^(defrecord|deftype|defprotocol)$")
          (sym_lit) @name) @class

        ; Namespace (imports)
        (list_lit
          (sym_lit) @_type
          (#match? @_type "^(ns|require|use|import)$")) @import
    "#;

    Query::new(&tree_sitter_clojure::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// OCaml
// ==========================================================================

pub fn ocaml_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (value_definition
          (let_binding
            pattern: (value_name) @name)) @function

        (type_definition
          (type_binding
            name: (type_constructor) @name)) @struct

        (module_definition
          (module_binding
            name: (module_name) @name)) @class
    "#;

    Query::new(&tree_sitter_ocaml::LANGUAGE_OCAML.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn ocaml_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions (let bindings)
        (value_definition
          (let_binding
            pattern: (value_name) @name)) @function

        ; Types
        (type_definition
          (type_binding
            name: (type_constructor) @name)) @struct

        ; Modules
        (module_definition
          (module_binding
            name: (module_name) @name)) @class

        ; Opens (imports)
        (open_module) @import
    "#;

    Query::new(&tree_sitter_ocaml::LANGUAGE_OCAML.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// Lua
// ==========================================================================

pub fn lua_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (function_declaration
          name: (identifier) @name) @function

        (function_declaration
          name: (dot_index_expression) @name) @method

        (function_declaration
          name: (method_index_expression) @name) @method
    "#;

    Query::new(&tree_sitter_lua::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn lua_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Global functions
        (function_declaration
          name: (identifier) @name) @function

        ; Method-like functions
        (function_declaration
          name: (dot_index_expression) @name) @method

        (function_declaration
          name: (method_index_expression) @name) @method

        ; Requires (imports)
        (function_call
          name: (variable
            (identifier) @_func)
          (#eq? @_func "require")
          arguments: (arguments)) @import
    "#;

    Query::new(&tree_sitter_lua::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

// ==========================================================================
// R
// ==========================================================================

pub fn r_query() -> Result<Query, ParserError> {
    let query_string = r#"
        (binary_operator
          lhs: (identifier) @name
          operator: "<-"
          rhs: (function_definition)) @function

        (binary_operator
          lhs: (identifier) @name
          operator: "="
          rhs: (function_definition)) @function
    "#;

    Query::new(&tree_sitter_r::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

pub fn r_super_query() -> Result<Query, ParserError> {
    let query_string = r#"
        ; Functions (left assignment)
        (binary_operator
          lhs: (identifier) @name
          operator: "<-"
          rhs: (function_definition)) @function

        ; Functions (equals assignment)
        (binary_operator
          lhs: (identifier) @name
          operator: "="
          rhs: (function_definition)) @function

        ; Library/require calls (imports)
        (call
          function: (identifier) @_func
          (#match? @_func "^(library|require|source)$")) @import
    "#;

    Query::new(&tree_sitter_r::LANGUAGE.into(), query_string)
        .map_err(|e| ParserError::QueryError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_queries() {
        assert!(python_query().is_ok());
        assert!(python_super_query().is_ok());
    }

    #[test]
    fn test_javascript_queries() {
        assert!(javascript_query().is_ok());
        assert!(javascript_super_query().is_ok());
    }

    #[test]
    fn test_typescript_queries() {
        assert!(typescript_query().is_ok());
        assert!(typescript_super_query().is_ok());
    }

    #[test]
    fn test_rust_queries() {
        assert!(rust_query().is_ok());
        assert!(rust_super_query().is_ok());
    }

    #[test]
    fn test_go_queries() {
        assert!(go_query().is_ok());
        assert!(go_super_query().is_ok());
    }

    #[test]
    fn test_java_queries() {
        assert!(java_query().is_ok());
        assert!(java_super_query().is_ok());
    }

    #[test]
    fn test_c_queries() {
        assert!(c_query().is_ok());
        assert!(c_super_query().is_ok());
    }

    #[test]
    fn test_cpp_queries() {
        assert!(cpp_query().is_ok());
        assert!(cpp_super_query().is_ok());
    }

    #[test]
    fn test_csharp_queries() {
        assert!(csharp_query().is_ok());
        assert!(csharp_super_query().is_ok());
    }

    #[test]
    fn test_ruby_queries() {
        assert!(ruby_query().is_ok());
        assert!(ruby_super_query().is_ok());
    }

    #[test]
    fn test_bash_queries() {
        assert!(bash_query().is_ok());
        assert!(bash_super_query().is_ok());
    }

    #[test]
    fn test_php_queries() {
        assert!(php_query().is_ok());
        assert!(php_super_query().is_ok());
    }

    #[test]
    fn test_kotlin_queries() {
        assert!(kotlin_query().is_ok());
        assert!(kotlin_super_query().is_ok());
    }

    #[test]
    fn test_swift_queries() {
        assert!(swift_query().is_ok());
        assert!(swift_super_query().is_ok());
    }

    #[test]
    fn test_scala_queries() {
        assert!(scala_query().is_ok());
        assert!(scala_super_query().is_ok());
    }

    #[test]
    fn test_haskell_queries() {
        assert!(haskell_query().is_ok());
        assert!(haskell_super_query().is_ok());
    }

    #[test]
    fn test_elixir_queries() {
        assert!(elixir_query().is_ok());
        assert!(elixir_super_query().is_ok());
    }

    #[test]
    fn test_clojure_queries() {
        assert!(clojure_query().is_ok());
        assert!(clojure_super_query().is_ok());
    }

    #[test]
    fn test_ocaml_queries() {
        assert!(ocaml_query().is_ok());
        assert!(ocaml_super_query().is_ok());
    }

    #[test]
    fn test_lua_queries() {
        assert!(lua_query().is_ok());
        assert!(lua_super_query().is_ok());
    }

    #[test]
    fn test_r_queries() {
        assert!(r_query().is_ok());
        assert!(r_super_query().is_ok());
    }
}
