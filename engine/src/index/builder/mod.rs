//! Index builder module.
//!
//! Contains the IndexBuilder for building symbol indices from repositories,
//! dependency graph construction, and PageRank computation.

mod core;
mod graph;
mod types;

// Re-exports
pub use core::IndexBuilder;
pub use types::{BuildError, BuildOptions};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::types::Language;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_build_simple_index() {
        let tmp = TempDir::new().unwrap();

        // Create test files directly in tmp root (simpler test)
        fs::write(
            tmp.path().join("main.rs"),
            r#"fn main() {
    println!("Hello, world!");
    helper();
}

fn helper() {
    // Do something
}
"#,
        )
        .unwrap();

        fs::write(
            tmp.path().join("lib.rs"),
            r#"pub mod utils;

pub fn public_fn() {}
"#,
        )
        .unwrap();

        // Build index
        let builder = IndexBuilder::new(tmp.path());
        let (index, graph) = builder.build().unwrap();

        // Verify index found the files
        assert_eq!(
            index.files.len(),
            2,
            "Expected 2 files, found {:?}",
            index.files.iter().map(|f| &f.path).collect::<Vec<_>>()
        );

        // Verify symbols were extracted
        assert!(
            index.symbols.len() >= 3,
            "Expected at least 3 symbols, got {}",
            index.symbols.len()
        );

        // Verify lookups work
        assert!(index.get_file("main.rs").is_some(), "main.rs not found in index");
        assert!(index.get_file("lib.rs").is_some(), "lib.rs not found in index");

        // Verify PageRank was computed
        assert_eq!(graph.file_pagerank.len(), 2);
    }

    #[test]
    fn test_symbol_reference_edges() {
        let tmp = TempDir::new().unwrap();

        fs::write(
            tmp.path().join("lib.rs"),
            r#"pub struct Foo;
"#,
        )
        .unwrap();

        fs::write(
            tmp.path().join("main.rs"),
            r#"mod lib;

fn main() {
    let _value: Foo;
}
"#,
        )
        .unwrap();

        let builder = IndexBuilder::new(tmp.path());
        let (index, graph) = builder.build().unwrap();

        let foo = index.find_symbols("Foo");
        let main = index.find_symbols("main");
        assert!(!foo.is_empty(), "Expected Foo symbol");
        assert!(!main.is_empty(), "Expected main symbol");

        let foo_id = foo[0].id.as_u32();
        let main_id = main[0].id.as_u32();

        let referencers = graph.get_referencers(foo_id);
        assert!(referencers.contains(&main_id), "Expected main to reference Foo");
    }

    #[test]
    fn test_language_detection() {
        // Original languages
        assert_eq!(Language::from_extension("rs"), Language::Rust);
        assert_eq!(Language::from_extension("py"), Language::Python);
        assert_eq!(Language::from_extension("ts"), Language::TypeScript);
        assert_eq!(Language::from_extension("tsx"), Language::TypeScript);
        assert_eq!(Language::from_extension("go"), Language::Go);
        assert_eq!(Language::from_extension("java"), Language::Java);
        assert_eq!(Language::from_extension("js"), Language::JavaScript);
        assert_eq!(Language::from_extension("c"), Language::C);
        assert_eq!(Language::from_extension("cpp"), Language::Cpp);
        assert_eq!(Language::from_extension("cs"), Language::CSharp);
        assert_eq!(Language::from_extension("rb"), Language::Ruby);
        assert_eq!(Language::from_extension("sh"), Language::Bash);
        // New languages
        assert_eq!(Language::from_extension("php"), Language::Php);
        assert_eq!(Language::from_extension("kt"), Language::Kotlin);
        assert_eq!(Language::from_extension("swift"), Language::Swift);
        assert_eq!(Language::from_extension("scala"), Language::Scala);
        assert_eq!(Language::from_extension("hs"), Language::Haskell);
        assert_eq!(Language::from_extension("ex"), Language::Elixir);
        assert_eq!(Language::from_extension("clj"), Language::Clojure);
        assert_eq!(Language::from_extension("ml"), Language::OCaml);
        assert_eq!(Language::from_extension("lua"), Language::Lua);
        assert_eq!(Language::from_extension("r"), Language::R);
    }
}
