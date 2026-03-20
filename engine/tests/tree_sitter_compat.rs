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
    // tree-sitter-clojure removed: incompatible with tree-sitter 0.26
    // assert_abi_compatible("clojure", tree_sitter_clojure::LANGUAGE.into());
    assert_abi_compatible("ocaml", tree_sitter_ocaml::LANGUAGE_OCAML.into());
    assert_abi_compatible("lua", tree_sitter_lua::LANGUAGE.into());
    assert_abi_compatible("r", tree_sitter_r::LANGUAGE.into());
    assert_abi_compatible("hcl", tree_sitter_hcl::LANGUAGE.into());
    assert_abi_compatible("puppet", tree_sitter_puppet::LANGUAGE.into());
    assert_abi_compatible("yaml", tree_sitter_yaml::LANGUAGE.into());
    // Dockerfile uses tree-sitter 0.20 internally; we access it via the raw
    // C symbol + tree-sitter-language bridge (see language.rs).
    assert_abi_compatible(
        "dockerfile",
        infiniloom_engine::parser::language::dockerfile_ts_language(),
    );
}

#[test]
fn test_single_tree_sitter_version_in_lockfile() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let lock_path = Path::new(manifest_dir).join("..").join("Cargo.lock");
    let contents =
        std::fs::read_to_string(&lock_path).expect("Cargo.lock should be readable in workspace");
    let count = contents.matches("name = \"tree-sitter\"").count();
    // We expect 2 tree-sitter versions: 0.26 (primary) and 0.20 (transitive
    // dep of tree-sitter-dockerfile 0.2.0, which hasn't been updated).
    // We bypass dockerfile's Rust bindings via extern "C" + tree-sitter-language.
    assert!(
        count <= 2,
        "Expected at most 2 tree-sitter versions in Cargo.lock (0.26 + 0.20 for dockerfile), found {}",
        count
    );
}
