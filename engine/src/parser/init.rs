//! Language-specific parser initializers
//!
//! This module contains functions to initialize tree-sitter parsers for each
//! supported programming language.

use super::core::ParserError;
use tree_sitter::Parser as TSParser;

/// Initialize Python parser
pub fn python() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize JavaScript parser
pub fn javascript() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize TypeScript parser
pub fn typescript() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Rust parser
pub fn rust() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Go parser
pub fn go() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Java parser
pub fn java() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize C parser
pub fn c() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_c::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize C++ parser
pub fn cpp() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_cpp::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize C# parser
pub fn csharp() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Ruby parser
pub fn ruby() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_ruby::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Bash parser
pub fn bash() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize PHP parser
pub fn php() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_php::LANGUAGE_PHP.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Kotlin parser
pub fn kotlin() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_kotlin_ng::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Swift parser
pub fn swift() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_swift::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Scala parser
pub fn scala() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_scala::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Haskell parser
pub fn haskell() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_haskell::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Elixir parser
pub fn elixir() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_elixir::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Clojure parser
pub fn clojure() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_clojure::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize OCaml parser
pub fn ocaml() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_ocaml::LANGUAGE_OCAML.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize Lua parser
pub fn lua() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_lua::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

/// Initialize R parser
pub fn r() -> Result<TSParser, ParserError> {
    let mut parser = TSParser::new();
    parser
        .set_language(&tree_sitter_r::LANGUAGE.into())
        .map_err(|e| ParserError::ParseError(e.to_string()))?;
    Ok(parser)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_init() {
        assert!(python().is_ok());
    }

    #[test]
    fn test_javascript_init() {
        assert!(javascript().is_ok());
    }

    #[test]
    fn test_typescript_init() {
        assert!(typescript().is_ok());
    }

    #[test]
    fn test_rust_init() {
        assert!(rust().is_ok());
    }

    #[test]
    fn test_go_init() {
        assert!(go().is_ok());
    }

    #[test]
    fn test_java_init() {
        assert!(java().is_ok());
    }

    #[test]
    fn test_c_init() {
        assert!(c().is_ok());
    }

    #[test]
    fn test_cpp_init() {
        assert!(cpp().is_ok());
    }

    #[test]
    fn test_csharp_init() {
        assert!(csharp().is_ok());
    }

    #[test]
    fn test_ruby_init() {
        assert!(ruby().is_ok());
    }

    #[test]
    fn test_bash_init() {
        assert!(bash().is_ok());
    }

    #[test]
    fn test_php_init() {
        assert!(php().is_ok());
    }

    #[test]
    fn test_kotlin_init() {
        assert!(kotlin().is_ok());
    }

    #[test]
    fn test_swift_init() {
        assert!(swift().is_ok());
    }

    #[test]
    fn test_scala_init() {
        assert!(scala().is_ok());
    }

    #[test]
    fn test_haskell_init() {
        assert!(haskell().is_ok());
    }

    #[test]
    fn test_elixir_init() {
        assert!(elixir().is_ok());
    }

    #[test]
    fn test_clojure_init() {
        assert!(clojure().is_ok());
    }

    #[test]
    fn test_ocaml_init() {
        assert!(ocaml().is_ok());
    }

    #[test]
    fn test_lua_init() {
        assert!(lua().is_ok());
    }

    #[test]
    fn test_r_init() {
        assert!(r().is_ok());
    }
}
