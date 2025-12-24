//! Shared regex patterns for import extraction across index modules.
//!
//! These patterns are used by both the full index builder and the lazy context builder
//! to extract import statements from source code.

use once_cell::sync::Lazy;
use regex::Regex;

/// Python: `import module` or `import module.submodule`
pub static PYTHON_IMPORT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*import\s+(\S+)").expect("PYTHON_IMPORT: invalid regex pattern"));

/// Python: `from module import name`
pub static PYTHON_FROM_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*from\s+(\S+)\s+import").expect("PYTHON_FROM_IMPORT: invalid regex pattern")
});

/// JavaScript/TypeScript: `import ... from 'module'` or `import ... from "module"`
pub static JS_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"import\s+.*\s+from\s+['"]([^'"]+)['"]"#)
        .expect("JS_IMPORT: invalid regex pattern")
});

/// JavaScript/TypeScript: multi-line import statements
pub static JS_IMPORT_MULTILINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?s)import\s+.*?\s+from\s+['"]([^'"]+)['"]"#)
        .expect("JS_IMPORT_MULTILINE: invalid regex pattern")
});

/// JavaScript/TypeScript: `require('module')` or `require("module")`
pub static JS_REQUIRE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#)
        .expect("JS_REQUIRE: invalid regex pattern")
});

/// Rust: `use crate::module;` or `use std::collections::HashMap;`
pub static RUST_USE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*use\s+([^;]+);").expect("RUST_USE: invalid regex pattern"));

/// Go: `import "module"` or `import alias "module"` or `import ( "module" )`
pub static GO_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"import\s+(?:\(\s*)?(?:[\w.]+\s+)?["']([^"']+)["']"#)
        .expect("GO_IMPORT: invalid regex pattern")
});

/// Java: `import package.Class;`
pub static JAVA_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*import\s+([\w.]+);").expect("JAVA_IMPORT: invalid regex pattern")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_import() {
        assert!(PYTHON_IMPORT.is_match("import os"));
        assert!(PYTHON_IMPORT.is_match("import os.path"));
        assert!(PYTHON_IMPORT.is_match("  import json"));
    }

    #[test]
    fn test_python_from_import() {
        assert!(PYTHON_FROM_IMPORT.is_match("from os import path"));
        assert!(PYTHON_FROM_IMPORT.is_match("from typing import List"));
    }

    #[test]
    fn test_js_import() {
        assert!(JS_IMPORT.is_match("import { foo } from 'module'"));
        assert!(JS_IMPORT.is_match("import foo from \"module\""));
    }

    #[test]
    fn test_js_require() {
        assert!(JS_REQUIRE.is_match("require('module')"));
        assert!(JS_REQUIRE.is_match("const x = require(\"module\")"));
    }

    #[test]
    fn test_rust_use() {
        assert!(RUST_USE.is_match("use std::collections::HashMap;"));
        assert!(RUST_USE.is_match("  use crate::module;"));
    }

    #[test]
    fn test_go_import() {
        assert!(GO_IMPORT.is_match("import \"fmt\""));
        assert!(GO_IMPORT.is_match("import f \"fmt\""));
    }

    #[test]
    fn test_java_import() {
        assert!(JAVA_IMPORT.is_match("import java.util.List;"));
        assert!(JAVA_IMPORT.is_match("import com.example.MyClass;"));
    }
}
