//! Language definitions and support traits
//!
//! This module defines the supported programming languages and provides
//! a uniform interface for language-specific operations.

use super::core::ParserError;
use super::queries;
use tree_sitter::{Language as TSLanguage, Parser as TSParser, Query};

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Rust,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Bash,
    Php,
    Kotlin,
    Swift,
    Scala,
    Haskell,
    Elixir,
    Clojure,
    OCaml,
    FSharp,
    Lua,
    R,
}

impl Language {
    /// Detect language from file extension
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "py" | "pyw" => Some(Self::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" => Some(Self::TypeScript),
            "rs" => Some(Self::Rust),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "c" | "h" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Some(Self::Cpp),
            "cs" => Some(Self::CSharp),
            "rb" | "rake" | "gemspec" => Some(Self::Ruby),
            "sh" | "bash" | "zsh" | "fish" => Some(Self::Bash),
            "php" | "phtml" | "php3" | "php4" | "php5" | "phps" => Some(Self::Php),
            "kt" | "kts" => Some(Self::Kotlin),
            "swift" => Some(Self::Swift),
            "scala" | "sc" => Some(Self::Scala),
            "hs" | "lhs" => Some(Self::Haskell),
            "ex" | "exs" | "eex" | "heex" | "leex" => Some(Self::Elixir),
            "clj" | "cljs" | "cljc" | "edn" => Some(Self::Clojure),
            "ml" | "mli" => Some(Self::OCaml),
            "fs" | "fsi" | "fsx" | "fsscript" => Some(Self::FSharp),
            "lua" => Some(Self::Lua),
            "r" | "rmd" => Some(Self::R),
            _ => None,
        }
    }

    /// Get language name as string
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Rust => "rust",
            Self::Go => "go",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Ruby => "ruby",
            Self::Bash => "bash",
            Self::Php => "php",
            Self::Kotlin => "kotlin",
            Self::Swift => "swift",
            Self::Scala => "scala",
            Self::Haskell => "haskell",
            Self::Elixir => "elixir",
            Self::Clojure => "clojure",
            Self::OCaml => "ocaml",
            Self::FSharp => "fsharp",
            Self::Lua => "lua",
            Self::R => "r",
        }
    }

    /// Get display name for pretty printing
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Rust => "Rust",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::CSharp => "C#",
            Self::Ruby => "Ruby",
            Self::Bash => "Bash",
            Self::Php => "PHP",
            Self::Kotlin => "Kotlin",
            Self::Swift => "Swift",
            Self::Scala => "Scala",
            Self::Haskell => "Haskell",
            Self::Elixir => "Elixir",
            Self::Clojure => "Clojure",
            Self::OCaml => "OCaml",
            Self::FSharp => "F#",
            Self::Lua => "Lua",
            Self::R => "R",
        }
    }

    /// Check if this language has full tree-sitter support
    #[must_use]
    pub const fn has_parser_support(self) -> bool {
        !matches!(self, Self::FSharp)
    }

    /// Get the tree-sitter language for this language
    pub fn tree_sitter_language(self) -> Option<TSLanguage> {
        Some(match self {
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Self::Bash => tree_sitter_bash::LANGUAGE.into(),
            Self::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Self::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Self::Swift => tree_sitter_swift::LANGUAGE.into(),
            Self::Scala => tree_sitter_scala::LANGUAGE.into(),
            Self::Haskell => tree_sitter_haskell::LANGUAGE.into(),
            Self::Elixir => tree_sitter_elixir::LANGUAGE.into(),
            Self::Clojure => tree_sitter_clojure::LANGUAGE.into(),
            Self::OCaml => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
            Self::Lua => tree_sitter_lua::LANGUAGE.into(),
            Self::R => tree_sitter_r::LANGUAGE.into(),
            Self::FSharp => return None,
        })
    }

    /// Get the query string for symbol extraction
    #[must_use]
    pub const fn query_string(self) -> Option<&'static str> {
        Some(match self {
            Self::Python => queries::PYTHON,
            Self::JavaScript => queries::JAVASCRIPT,
            Self::TypeScript => queries::TYPESCRIPT,
            Self::Rust => queries::RUST,
            Self::Go => queries::GO,
            Self::Java => queries::JAVA,
            Self::C => queries::C,
            Self::Cpp => queries::CPP,
            Self::CSharp => queries::CSHARP,
            Self::Ruby => queries::RUBY,
            Self::Bash => queries::BASH,
            Self::Php => queries::PHP,
            Self::Kotlin => queries::KOTLIN,
            Self::Swift => queries::SWIFT,
            Self::Scala => queries::SCALA,
            Self::Haskell => queries::HASKELL,
            Self::Elixir => queries::ELIXIR,
            Self::Clojure => queries::CLOJURE,
            Self::OCaml => queries::OCAML,
            Self::Lua => queries::LUA,
            Self::R => queries::R,
            Self::FSharp => return None,
        })
    }

    /// Initialize a tree-sitter parser for this language
    pub fn init_parser(self) -> Result<TSParser, ParserError> {
        let ts_lang = self.tree_sitter_language().ok_or_else(|| {
            ParserError::UnsupportedLanguage(format!("{} has no parser support", self.name()))
        })?;

        let mut parser = TSParser::new();
        parser
            .set_language(&ts_lang)
            .map_err(|e| ParserError::ParseError(e.to_string()))?;
        Ok(parser)
    }

    /// Create a tree-sitter query for symbol extraction
    pub fn create_query(self) -> Result<Query, ParserError> {
        let ts_lang = self.tree_sitter_language().ok_or_else(|| {
            ParserError::UnsupportedLanguage(format!("{} has no parser support", self.name()))
        })?;

        let query_str = self.query_string().ok_or_else(|| {
            ParserError::UnsupportedLanguage(format!("{} has no query defined", self.name()))
        })?;

        Query::new(&ts_lang, query_str).map_err(|e| ParserError::QueryError(e.to_string()))
    }

    /// Get all supported languages
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Python,
            Self::JavaScript,
            Self::TypeScript,
            Self::Rust,
            Self::Go,
            Self::Java,
            Self::C,
            Self::Cpp,
            Self::CSharp,
            Self::Ruby,
            Self::Bash,
            Self::Php,
            Self::Kotlin,
            Self::Swift,
            Self::Scala,
            Self::Haskell,
            Self::Elixir,
            Self::Clojure,
            Self::OCaml,
            Self::FSharp,
            Self::Lua,
            Self::R,
        ]
    }

    /// Get all languages with full parser support
    #[must_use]
    pub fn all_with_parser_support() -> Vec<Self> {
        Self::all()
            .iter()
            .copied()
            .filter(|l| l.has_parser_support())
            .collect()
    }

    /// Check if this language uses indentation for blocks (like Python)
    #[must_use]
    pub const fn uses_indentation_blocks(self) -> bool {
        matches!(self, Self::Python | Self::Haskell)
    }

    /// Check if this is a C-family language (uses braces for blocks)
    #[must_use]
    pub const fn is_c_family(self) -> bool {
        matches!(
            self,
            Self::C
                | Self::Cpp
                | Self::CSharp
                | Self::Java
                | Self::JavaScript
                | Self::TypeScript
                | Self::Go
                | Self::Rust
                | Self::Kotlin
                | Self::Swift
                | Self::Scala
                | Self::Php
        )
    }

    /// Check if this is a functional language
    #[must_use]
    pub const fn is_functional(self) -> bool {
        matches!(self, Self::Haskell | Self::OCaml | Self::Elixir | Self::Clojure | Self::Scala)
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Detect programming language from file path (filename or extension).
/// Returns a language name as a string, supporting many more formats than the `Language` enum.
/// This is useful for display purposes and handling non-parseable file types.
#[must_use]
pub fn detect_file_language(path: &std::path::Path) -> Option<String> {
    // First, check for well-known filenames without extensions
    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
        let lower = filename.to_lowercase();
        let lang =
            match lower.as_str() {
                // Docker
                "dockerfile" | "dockerfile.dev" | "dockerfile.prod" | "dockerfile.test" => {
                    Some("dockerfile")
                },
                // Make
                "makefile" | "gnumakefile" | "bsdmakefile" => Some("make"),
                // Ruby
                "gemfile" | "rakefile" | "guardfile" | "vagrantfile" | "berksfile" | "podfile"
                | "fastfile" | "appfile" | "matchfile" | "deliverfile" | "snapfile"
                | "brewfile" => Some("ruby"),
                // Shell
                ".bashrc" | ".bash_profile" | ".zshrc" | ".zprofile" | ".profile"
                | ".bash_aliases" => Some("shell"),
                // Git
                ".gitignore" | ".gitattributes" | ".gitmodules" => Some("gitignore"),
                // Editor config
                ".editorconfig" => Some("editorconfig"),
                // Procfile (Heroku)
                "procfile" => Some("procfile"),
                // Justfile
                "justfile" => Some("just"),
                // Caddyfile
                "caddyfile" => Some("caddyfile"),
                _ => None,
            };
        if lang.is_some() {
            return lang.map(|s| s.to_owned());
        }
        // Check for patterns like Dockerfile.something
        if lower.starts_with("dockerfile") {
            return Some("dockerfile".to_owned());
        }
        if lower.starts_with("makefile") {
            return Some("make".to_owned());
        }
    }

    // Then check extensions
    let ext = path.extension()?.to_str()?;
    let lang = match ext.to_lowercase().as_str() {
        // Python
        "py" | "pyi" | "pyx" => "python",
        // JavaScript/TypeScript
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        // Rust
        "rs" => "rust",
        // Go
        "go" => "go",
        // Java/JVM
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "groovy" => "groovy",
        "clj" | "cljs" | "cljc" => "clojure",
        // C/C++
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "cxx" | "hxx" => "cpp",
        // C#
        "cs" => "csharp",
        // Ruby
        "rb" | "rake" | "gemspec" => "ruby",
        // PHP
        "php" => "php",
        // Swift
        "swift" => "swift",
        // Shell
        "sh" | "bash" => "bash",
        "zsh" => "zsh",
        "fish" => "fish",
        "ps1" | "psm1" => "powershell",
        // Web
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "sass" => "sass",
        "less" => "less",
        // Data/Config
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "ini" | "cfg" => "ini",
        // Documentation
        "md" | "markdown" => "markdown",
        "mdx" => "mdx",
        "rst" => "rst",
        "txt" => "text",
        // Zig
        "zig" => "zig",
        // Lua
        "lua" => "lua",
        // SQL
        "sql" => "sql",
        // Elixir/Erlang
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        // Haskell
        "hs" | "lhs" => "haskell",
        // OCaml/F#
        "ml" | "mli" => "ocaml",
        "fs" | "fsi" | "fsx" => "fsharp",
        // Vue/Svelte
        "vue" => "vue",
        "svelte" => "svelte",
        // Docker
        "dockerfile" => "dockerfile",
        // Terraform
        "tf" | "tfvars" => "terraform",
        // Makefile-like
        "makefile" | "mk" => "make",
        "cmake" => "cmake",
        // Nix
        "nix" => "nix",
        // Julia
        "jl" => "julia",
        // R
        "r" | "rmd" => "r",
        // Dart
        "dart" => "dart",
        // Nim
        "nim" => "nim",
        // V
        "v" => "vlang",
        // Crystal
        "cr" => "crystal",
        _ => return None,
    };

    Some(lang.to_owned())
}

impl std::str::FromStr for Language {
    type Err = ParserError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "python" | "py" => Ok(Self::Python),
            "javascript" | "js" => Ok(Self::JavaScript),
            "typescript" | "ts" => Ok(Self::TypeScript),
            "rust" | "rs" => Ok(Self::Rust),
            "go" | "golang" => Ok(Self::Go),
            "java" => Ok(Self::Java),
            "c" => Ok(Self::C),
            "cpp" | "c++" | "cxx" => Ok(Self::Cpp),
            "csharp" | "c#" | "cs" => Ok(Self::CSharp),
            "ruby" | "rb" => Ok(Self::Ruby),
            "bash" | "shell" | "sh" => Ok(Self::Bash),
            "php" => Ok(Self::Php),
            "kotlin" | "kt" => Ok(Self::Kotlin),
            "swift" => Ok(Self::Swift),
            "scala" => Ok(Self::Scala),
            "haskell" | "hs" => Ok(Self::Haskell),
            "elixir" | "ex" => Ok(Self::Elixir),
            "clojure" | "clj" => Ok(Self::Clojure),
            "ocaml" | "ml" => Ok(Self::OCaml),
            "fsharp" | "f#" | "fs" => Ok(Self::FSharp),
            "lua" => Ok(Self::Lua),
            "r" => Ok(Self::R),
            _ => Err(ParserError::UnsupportedLanguage(s.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_from_extension() {
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("tsx"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("unknown"), None);
    }

    #[test]
    fn test_language_name() {
        assert_eq!(Language::Python.name(), "python");
        assert_eq!(Language::Rust.name(), "rust");
        assert_eq!(Language::TypeScript.name(), "typescript");
    }

    #[test]
    fn test_language_display_name() {
        assert_eq!(Language::Python.display_name(), "Python");
        assert_eq!(Language::Cpp.display_name(), "C++");
        assert_eq!(Language::CSharp.display_name(), "C#");
    }

    #[test]
    fn test_parser_support() {
        assert!(Language::Python.has_parser_support());
        assert!(Language::Rust.has_parser_support());
        assert!(!Language::FSharp.has_parser_support());
    }

    #[test]
    fn test_language_from_str() {
        assert_eq!("python".parse::<Language>().unwrap(), Language::Python);
        assert_eq!("c++".parse::<Language>().unwrap(), Language::Cpp);
        assert_eq!("c#".parse::<Language>().unwrap(), Language::CSharp);
        assert!("invalid".parse::<Language>().is_err());
    }

    #[test]
    fn test_all_languages() {
        let all = Language::all();
        assert_eq!(all.len(), 22);
        assert!(all.contains(&Language::Python));
        assert!(all.contains(&Language::Rust));
    }

    #[test]
    fn test_tree_sitter_language() {
        assert!(Language::Python.tree_sitter_language().is_some());
        assert!(Language::Rust.tree_sitter_language().is_some());
        assert!(Language::FSharp.tree_sitter_language().is_none());
    }

    #[test]
    fn test_query_string() {
        assert!(Language::Python.query_string().is_some());
        assert!(Language::Rust.query_string().is_some());
        assert!(Language::FSharp.query_string().is_none());
    }

    #[test]
    fn test_init_parser() {
        assert!(Language::Python.init_parser().is_ok());
        assert!(Language::Rust.init_parser().is_ok());
        assert!(Language::FSharp.init_parser().is_err());
    }

    #[test]
    fn test_create_query() {
        assert!(Language::Python.create_query().is_ok());
        assert!(Language::Rust.create_query().is_ok());
        assert!(Language::FSharp.create_query().is_err());
    }

    #[test]
    fn test_language_categories() {
        assert!(Language::Python.uses_indentation_blocks());
        assert!(!Language::Rust.uses_indentation_blocks());

        assert!(Language::Rust.is_c_family());
        assert!(!Language::Python.is_c_family());

        assert!(Language::Haskell.is_functional());
        assert!(!Language::Python.is_functional());
    }
}
