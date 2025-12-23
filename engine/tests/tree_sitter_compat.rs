use std::path::Path;
use tree_sitter::{
    Language as TsLanguage, Parser as TsParser, LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION,
};

fn assert_abi_compatible(name: &str, language: TsLanguage) {
    let version = language.abi_version();
    assert!(
        (MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&version),
        "tree-sitter ABI mismatch for {}: language ABI {}, supported range {}..={}",
        name,
        version,
        MIN_COMPATIBLE_LANGUAGE_VERSION,
        LANGUAGE_VERSION
    );

    let mut parser = TsParser::new();
    parser.set_language(&language).unwrap_or_else(|err| {
        panic!(
            "tree-sitter set_language failed for {}: {} (ABI {}, supported {}..={})",
            name, err, version, MIN_COMPATIBLE_LANGUAGE_VERSION, LANGUAGE_VERSION
        )
    });
}

#[test]
fn test_tree_sitter_abi_compatibility() {
    assert_abi_compatible("python", tree_sitter_python::LANGUAGE.into());
    assert_abi_compatible("javascript", tree_sitter_javascript::LANGUAGE.into());
    assert_abi_compatible("typescript", tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
    assert_abi_compatible("rust", tree_sitter_rust::LANGUAGE.into());
    assert_abi_compatible("go", tree_sitter_go::LANGUAGE.into());
    assert_abi_compatible("java", tree_sitter_java::LANGUAGE.into());
    assert_abi_compatible("c", tree_sitter_c::LANGUAGE.into());
    assert_abi_compatible("cpp", tree_sitter_cpp::LANGUAGE.into());
    assert_abi_compatible("csharp", tree_sitter_c_sharp::LANGUAGE.into());
    assert_abi_compatible("ruby", tree_sitter_ruby::LANGUAGE.into());
    assert_abi_compatible("bash", tree_sitter_bash::LANGUAGE.into());
    assert_abi_compatible("php", tree_sitter_php::LANGUAGE_PHP.into());
    assert_abi_compatible("kotlin", tree_sitter_kotlin_ng::LANGUAGE.into());
    assert_abi_compatible("swift", tree_sitter_swift::LANGUAGE.into());
    assert_abi_compatible("scala", tree_sitter_scala::LANGUAGE.into());
    assert_abi_compatible("haskell", tree_sitter_haskell::LANGUAGE.into());
    assert_abi_compatible("elixir", tree_sitter_elixir::LANGUAGE.into());
    assert_abi_compatible("clojure", tree_sitter_clojure::LANGUAGE.into());
    assert_abi_compatible("ocaml", tree_sitter_ocaml::LANGUAGE_OCAML.into());
    assert_abi_compatible("lua", tree_sitter_lua::LANGUAGE.into());
    assert_abi_compatible("r", tree_sitter_r::LANGUAGE.into());
}

#[test]
fn test_single_tree_sitter_version_in_lockfile() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let lock_path = Path::new(manifest_dir).join("..").join("Cargo.lock");
    let contents =
        std::fs::read_to_string(&lock_path).expect("Cargo.lock should be readable in workspace");
    let count = contents.matches("name = \"tree-sitter\"").count();
    assert_eq!(count, 1, "Expected a single tree-sitter version in Cargo.lock, found {}", count);
}
