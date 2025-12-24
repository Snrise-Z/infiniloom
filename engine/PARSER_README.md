# Tree-sitter Parser Module

A comprehensive code parsing module for the Infiniloom engine that extracts symbols from source files across 21 programming languages.

## Features

- **Multi-language Support**: 21 languages with full AST parsing
  - **Tier 1 (Full Support)**: Python, JavaScript, TypeScript, Rust, Go, Java, C, C++
  - **Tier 2 (Good Support)**: C#, Ruby, PHP, Kotlin, Swift, Scala, Bash
  - **Tier 3 (Basic Support)**: Haskell, Elixir, Clojure, OCaml, Lua, R
- **Symbol Extraction**: Functions, classes, methods, structs, enums, interfaces, traits
- **Metadata Capture**:
  - Symbol names and types
  - Function/method signatures
  - Docstrings and comments
  - Line numbers (start/end)
  - Parent relationships (for methods)
- **Import Detection**: Automatically identifies import/use statements
- **Zero Configuration**: Works out of the box with sensible defaults

## Installation

The parser module is already included in the Infiniloom engine. The following dependencies are in `Cargo.toml`:

```toml
# Tree-sitter for AST parsing (21 languages)
tree-sitter = "0.25"
tree-sitter-python = "0.23"
tree-sitter-javascript = "0.23"
tree-sitter-typescript = "0.23"
tree-sitter-rust = "0.23"
tree-sitter-go = "0.23"
tree-sitter-java = "0.23"
tree-sitter-c = "0.23"
tree-sitter-cpp = "0.23"
tree-sitter-c-sharp = "0.23"
tree-sitter-ruby = "0.23"
tree-sitter-bash = "0.23"
tree-sitter-php = "0.23"
tree-sitter-kotlin-ng = "1.1"
tree-sitter-swift = "0.6"
tree-sitter-scala = "0.23"
tree-sitter-haskell = "0.23"
tree-sitter-elixir = "0.3"
tree-sitter-clojure = "0.1"
tree-sitter-ocaml = "0.23"
tree-sitter-lua = "0.2"
tree-sitter-r = "1"
```

## Usage

### Basic Parsing

```rust
use infiniloom_engine::parser::{Parser, Language};

// Create a parser instance
let mut parser = Parser::new();

// Parse Python code
let source_code = r#"
def hello(name: str) -> str:
    """Greet someone by name."""
    return f"Hello, {name}!"
"#;

let symbols = parser.parse(source_code, Language::Python)?;

// Access extracted symbols
for symbol in symbols {
    println!("{}: {} (lines {}-{})",
        symbol.kind.name(),
        symbol.name,
        symbol.start_line,
        symbol.end_line
    );
}
```

### Language Detection

```rust
use infiniloom_engine::parser::Language;

// Detect language from file extension
let lang = Language::from_extension("py");
assert_eq!(lang, Some(Language::Python));

let lang = Language::from_extension("rs");
assert_eq!(lang, Some(Language::Rust));

let lang = Language::from_extension("kt");
assert_eq!(lang, Some(Language::Kotlin));
```

### Accessing Symbol Metadata

```rust
let symbols = parser.parse(source_code, Language::Python)?;

for symbol in symbols {
    // Basic info
    println!("Name: {}", symbol.name);
    println!("Kind: {}", symbol.kind.name());
    println!("Lines: {}-{}", symbol.start_line, symbol.end_line);

    // Optional metadata
    if let Some(signature) = &symbol.signature {
        println!("Signature: {}", signature);
    }

    if let Some(docstring) = &symbol.docstring {
        println!("Documentation: {}", docstring);
    }

    if let Some(parent) = &symbol.parent {
        println!("Parent: {}", parent);
    }
}
```

## Symbol Types

The parser extracts the following symbol types (defined in `SymbolKind`):

| Symbol Kind | Description | Languages |
|-------------|-------------|-----------|
| `Function` | Standalone function | All |
| `Method` | Class/struct method | All |
| `Class` | Class definition | Python, JS, TS, Java, C++, C#, Ruby, PHP, Kotlin, Swift, Scala |
| `Interface` | Interface definition | TypeScript, Go, Java, C#, Kotlin |
| `Struct` | Struct definition | Rust, Go, C, C++, Swift |
| `Enum` | Enum definition | Rust, TypeScript, Java, C#, Kotlin, Swift |
| `Trait` | Trait definition | Rust, Scala |
| `Import` | Import/use statement | All |

## Supported Languages

### Tier 1: Full Support

| Language | Extensions | Symbol Types | Notes |
|----------|------------|--------------|-------|
| Python | `.py`, `.pyi` | Functions, Classes, Methods, Imports | Docstrings, type hints |
| JavaScript | `.js`, `.mjs`, `.cjs` | Functions, Classes, Methods, Imports | JSDoc, arrow functions |
| TypeScript | `.ts`, `.tsx` | Functions, Classes, Interfaces, Enums | Full type support |
| Rust | `.rs` | Functions, Structs, Enums, Traits, Impls | Doc comments |
| Go | `.go` | Functions, Structs, Interfaces, Methods | Receiver methods |
| Java | `.java` | Classes, Interfaces, Methods, Enums | JavaDoc |
| C | `.c`, `.h` | Functions, Structs, Enums | Header parsing |
| C++ | `.cpp`, `.hpp`, `.cc` | Classes, Functions, Templates | Namespaces |

### Tier 2: Good Support

| Language | Extensions | Symbol Types |
|----------|------------|--------------|
| C# | `.cs` | Classes, Interfaces, Methods, Properties |
| Ruby | `.rb` | Classes, Modules, Methods |
| PHP | `.php` | Classes, Functions, Methods |
| Kotlin | `.kt`, `.kts` | Classes, Functions, Objects |
| Swift | `.swift` | Classes, Structs, Protocols, Functions |
| Scala | `.scala` | Classes, Traits, Objects, Defs |
| Bash | `.sh`, `.bash` | Functions |

### Tier 3: Basic Support

| Language | Extensions | Symbol Types |
|----------|------------|--------------|
| Haskell | `.hs` | Functions, Types, Classes |
| Elixir | `.ex`, `.exs` | Modules, Functions |
| Clojure | `.clj`, `.cljs` | Defns, Defs |
| OCaml | `.ml`, `.mli` | Functions, Types, Modules |
| Lua | `.lua` | Functions |
| R | `.r`, `.R` | Functions |

### Known Limitations

| Language | Extensions | Status |
|----------|------------|--------|
| F# | `.fs`, `.fsi`, `.fsx` | **Recognized but not parsed** - File extension detected but no tree-sitter grammar is available. Returns `UnsupportedLanguage` error. |

> **Note**: F# files are detected via `Language::from_extension()` but calling `Parser::parse()` on F# code will return `ParserError::UnsupportedLanguage("F# not yet supported (no tree-sitter grammar available)")`. This allows graceful handling of F# files in mixed-language codebases while clearly indicating parsing is not supported.

## Architecture

### Module Structure

```
parser/
├── mod.rs           # Public API, re-exports
├── core.rs          # Parser struct, main entry point
├── language.rs      # Language enum (21 variants)
├── extraction.rs    # Symbol extraction from AST nodes
├── queries.rs       # Tree-sitter query strings per language
├── query_builder.rs # Super-query construction
└── init.rs          # Tree-sitter parser initialization
```

### Parser Structure

```rust
pub struct Parser {
    parsers: HashMap<Language, TSParser>,
    queries: HashMap<Language, Query>,
}
```

The `Parser` maintains:
- A tree-sitter parser instance for each language
- Pre-compiled queries for efficient symbol extraction

### Query System

Each language has custom tree-sitter queries optimized for symbol extraction:

```rust
// Example: Python query
(function_definition
  name: (identifier) @name) @function

(class_definition
  name: (identifier) @name) @class
```

Queries identify:
- Symbol types (function, class, method, etc.)
- Symbol names
- Symbol boundaries (start/end lines)

### Symbol Extraction Pipeline

1. **Parse**: Source code → AST (Abstract Syntax Tree)
2. **Query**: Run language-specific queries on AST
3. **Extract**: Convert query matches to `Symbol` objects
4. **Enhance**: Add signatures, docstrings, parent relationships
5. **Import Detection**: Traverse AST for import statements

## Performance

- **Fast**: Tree-sitter is a high-performance parser (C library)
- **Incremental**: Supports incremental parsing (not yet exposed)
- **Memory Efficient**: Streams over AST without full tree in memory
- **Cached Queries**: Queries compiled once per language
- **Thread-Local Parsers**: Lock-free parallel parsing via thread-local storage

Typical performance:
- Small files (<1000 lines): <1ms
- Medium files (1000-5000 lines): 1-10ms
- Large files (>5000 lines): 10-50ms

## Error Handling

The parser returns `Result<Vec<Symbol>, ParserError>` with the following error types:

- `UnsupportedLanguage`: Language not supported
- `ParseError`: Failed to parse source code
- `QueryError`: Query compilation failed
- `InvalidUtf8`: Source contains invalid UTF-8

```rust
match parser.parse(source, language) {
    Ok(symbols) => {
        // Process symbols
    }
    Err(ParserError::UnsupportedLanguage(lang)) => {
        eprintln!("Language not supported: {}", lang);
    }
    Err(e) => {
        eprintln!("Parse error: {}", e);
    }
}
```

## Testing

Run the comprehensive test suite:

```bash
cargo test -p infiniloom-engine parser
cargo test -p infiniloom-engine --test parser_all_languages
```

Tests cover:
- Language detection
- Symbol extraction for all 21 languages
- Metadata capture (signatures, docstrings)
- Import detection
- Edge cases

Run the interactive demo:

```bash
cargo run --example parser_demo
```

## Integration with Infiniloom

The parser integrates seamlessly with the existing Infiniloom types:

```rust
use infiniloom_engine::{RepoFile, Parser, Language};

let mut file = RepoFile::new("/path/to/file.py", "file.py");
let content = std::fs::read_to_string(&file.path)?;

// Detect language
if let Some(lang) = Language::from_extension(file.extension().unwrap()) {
    let mut parser = Parser::new();
    file.symbols = parser.parse(&content, lang)?;
}

// Now file.symbols contains all extracted symbols
for symbol in &file.symbols {
    println!("{}: {}", symbol.kind.name(), symbol.name);
}
```

## Future Enhancements (Phase 2 - not implemented intentionally)

Potential improvements:

1. **Incremental Parsing**: Reparse only changed regions
2. **Reference Resolution**: Track symbol references and call graphs
3. **Type Information**: Extract and track type definitions
4. **Scope Analysis**: Identify local vs global symbols
5. **Semantic Queries**: "Find all functions that return Promise"
6. **Custom Queries**: Allow users to define custom extraction rules

## Contributing

To add support for a new language:

1. Add the tree-sitter crate to `Cargo.toml`:
   ```toml
   tree-sitter-newlang = "0.23"
   ```

2. Add language variant to `Language` enum:
   ```rust
   pub enum Language {
       // ...
       NewLang,
   }
   ```

3. Implement parser initialization:
   ```rust
   fn init_newlang_parser() -> Result<TSParser, ParserError> {
       let mut parser = TSParser::new();
       parser.set_language(tree_sitter_newlang::language())?;
       Ok(parser)
   }
   ```

4. Create tree-sitter query:
   ```rust
   fn newlang_query() -> Result<Query, ParserError> {
       let query_string = r#"
           (function_definition
             name: (identifier) @name) @function
       "#;
       Query::new(tree_sitter_newlang::language(), query_string)
   }
   ```

5. Add to `Parser::new()` initialization
6. Add tests in `engine/tests/parser_all_languages.rs`
7. Update documentation

## Resources

- [Tree-sitter Documentation](https://tree-sitter.github.io/tree-sitter/)
- [Tree-sitter Playground](https://tree-sitter.github.io/tree-sitter/playground)
- [Infiniloom GitHub](https://github.com/Topos-Labs/infiniloom)

## License

MIT License - see LICENSE file for details.
