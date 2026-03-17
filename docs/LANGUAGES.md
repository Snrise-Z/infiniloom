# Supported Languages

Infiniloom supports **23 programming languages** with full AST parsing via [Tree-sitter](https://tree-sitter.github.io/tree-sitter/), plus 2 additional languages with file detection only.

## Language Support Tiers

### Tier 1: Full Support (8 languages)

Complete symbol extraction, signatures, docstrings, and import detection.

| Language | Extensions | Symbol Types | Notes |
|----------|------------|--------------|-------|
| **Python** | `.py`, `.pyi` | Functions, Classes, Methods, Imports | Docstrings, type hints |
| **JavaScript** | `.js`, `.mjs`, `.cjs` | Functions, Classes, Methods, Imports | JSDoc, arrow functions |
| **TypeScript** | `.ts`, `.tsx` | Functions, Classes, Interfaces, Enums, Imports | Full type support, JSX/TSX |
| **Rust** | `.rs` | Functions, Structs, Enums, Traits, Impls, Imports | Doc comments, visibility |
| **Go** | `.go` | Functions, Structs, Interfaces, Methods, Imports | Receiver methods |
| **Java** | `.java` | Classes, Interfaces, Methods, Enums, Imports | JavaDoc |
| **C** | `.c`, `.h` | Functions, Structs, Enums | Header parsing |
| **C++** | `.cpp`, `.hpp`, `.cc`, `.cxx` | Classes, Functions, Templates, Namespaces | Full template support |

### Tier 2: Good Support (9 languages)

Reliable symbol extraction for common patterns.

| Language | Extensions | Symbol Types |
|----------|------------|--------------|
| **C#** | `.cs` | Classes, Interfaces, Methods, Properties |
| **Ruby** | `.rb` | Classes, Modules, Methods |
| **PHP** | `.php` | Classes, Functions, Methods |
| **Kotlin** | `.kt`, `.kts` | Classes, Functions, Objects |
| **Swift** | `.swift` | Classes, Structs, Protocols, Functions |
| **Scala** | `.scala` | Classes, Traits, Objects, Defs |
| **Dart** | `.dart` | Classes, Functions, Methods |
| **Zig** | `.zig`, `.zon` | Functions, Structs, Enums |
| **Bash** | `.sh`, `.bash` | Functions |

### Tier 3: Basic Support (6 languages)

Symbol extraction for primary constructs.

| Language | Extensions | Symbol Types |
|----------|------------|--------------|
| **Haskell** | `.hs` | Functions, Types, Classes |
| **Elixir** | `.ex`, `.exs` | Modules, Functions |
| **OCaml** | `.ml`, `.mli` | Functions, Types, Modules |
| **Lua** | `.lua` | Functions |
| **R** | `.r`, `.R` | Functions |
| **HCL/Terraform** | `.tf`, `.hcl` | Resources, Variables, Outputs |

### Detected Only (no AST parsing)

These languages are recognized by file extension but do not have Tree-sitter grammar support. Files are included in output with content but without symbol extraction.

| Language | Extensions | Status |
|----------|------------|--------|
| **Clojure** | `.clj`, `.cljs` | Deprecated in v0.7.0 (tree-sitter-clojure incompatible with tree-sitter 0.26) |
| **F#** | `.fs`, `.fsi`, `.fsx` | Recognized but not parsed (no compatible tree-sitter grammar) |

## Additional File Types

Beyond programming languages, Infiniloom also processes:

| Type | Extensions | Treatment |
|------|------------|-----------|
| **Data/Config** | `.yaml`, `.yml`, `.toml`, `.json` | Included in output, no symbol extraction |
| **Markdown** | `.md`, `.markdown` | Included in output (or use `ingest` command for structured processing) |
| **Documents** | `.html`, `.csv`, `.docx`, `.xlsx` | Use the `ingest` command for structured processing |

## Symbol Types Extracted

| Symbol Kind | Description |
|-------------|-------------|
| `Function` | Standalone functions |
| `Method` | Class/struct methods |
| `Class` | Class definitions |
| `Interface` | Interface/protocol definitions |
| `Struct` | Struct definitions |
| `Enum` | Enum definitions |
| `Trait` | Trait definitions (Rust, Scala) |
| `Import` | Import/use/require statements |
| `Constant` | Constant definitions |
| `Variable` | Variable declarations |

## How Language Detection Works

Infiniloom detects languages by file extension using `Language::from_extension()`. No configuration is needed - it works automatically. Binary files are detected by inspecting the first 8KB and are skipped.

## See Also

- [Parser Documentation](../engine/PARSER_README.md) - Technical details of Tree-sitter integration
- [embed command](commands/embed.md) - Language-specific chunking for vector databases
- [pack command](commands/pack.md) - Language-aware context generation
