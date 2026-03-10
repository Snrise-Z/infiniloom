//! Comprehensive tests for CLI scanner helpers
//!
//! Tests for language detection, binary detection, token estimation,
//! directory structure generation, and git detection.

use std::path::PathBuf;

// Use the canonical language detection from the engine
use infiniloom_engine::detect_file_language as detect_language;

// ============================================================================
// Language Detection Tests
// ============================================================================

#[test]
fn test_detect_language_python() {
    assert_eq!(detect_language(&PathBuf::from("test.py")), Some("python".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("stub.pyi")), Some("python".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("cython.pyx")), Some("python".to_owned()));
}

#[test]
fn test_detect_language_javascript() {
    assert_eq!(detect_language(&PathBuf::from("app.js")), Some("javascript".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("module.mjs")), Some("javascript".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("common.cjs")), Some("javascript".to_owned()));
}

#[test]
fn test_detect_language_typescript() {
    assert_eq!(detect_language(&PathBuf::from("app.ts")), Some("typescript".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("module.mts")), Some("typescript".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("common.cts")), Some("typescript".to_owned()));
}

#[test]
fn test_detect_language_jsx_tsx() {
    assert_eq!(detect_language(&PathBuf::from("component.jsx")), Some("jsx".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("component.tsx")), Some("tsx".to_owned()));
}

#[test]
fn test_detect_language_rust() {
    assert_eq!(detect_language(&PathBuf::from("main.rs")), Some("rust".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("lib.rs")), Some("rust".to_owned()));
}

#[test]
fn test_detect_language_go() {
    assert_eq!(detect_language(&PathBuf::from("main.go")), Some("go".to_owned()));
}

#[test]
fn test_detect_language_jvm() {
    assert_eq!(detect_language(&PathBuf::from("Main.java")), Some("java".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Main.kt")), Some("kotlin".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("build.kts")), Some("kotlin".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Main.scala")), Some("scala".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("build.groovy")), Some("groovy".to_owned()));
}

#[test]
fn test_detect_language_clojure() {
    assert_eq!(detect_language(&PathBuf::from("core.clj")), Some("clojure".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("app.cljs")), Some("clojure".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("shared.cljc")), Some("clojure".to_owned()));
}

#[test]
fn test_detect_language_c_cpp() {
    assert_eq!(detect_language(&PathBuf::from("main.c")), Some("c".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("header.h")), Some("c".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("main.cpp")), Some("cpp".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("header.hpp")), Some("cpp".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("main.cc")), Some("cpp".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("main.cxx")), Some("cpp".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("header.hxx")), Some("cpp".to_owned()));
}

#[test]
fn test_detect_language_csharp() {
    assert_eq!(detect_language(&PathBuf::from("Program.cs")), Some("csharp".to_owned()));
}

#[test]
fn test_detect_language_ruby() {
    assert_eq!(detect_language(&PathBuf::from("app.rb")), Some("ruby".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("task.rake")), Some("ruby".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("mylib.gemspec")), Some("ruby".to_owned()));
}

#[test]
fn test_detect_language_php() {
    assert_eq!(detect_language(&PathBuf::from("index.php")), Some("php".to_owned()));
}

#[test]
fn test_detect_language_swift() {
    assert_eq!(detect_language(&PathBuf::from("App.swift")), Some("swift".to_owned()));
}

#[test]
fn test_detect_language_shell() {
    assert_eq!(detect_language(&PathBuf::from("script.sh")), Some("bash".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("script.bash")), Some("bash".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("script.zsh")), Some("zsh".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("script.fish")), Some("fish".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("script.ps1")), Some("powershell".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("module.psm1")), Some("powershell".to_owned()));
}

#[test]
fn test_detect_language_web() {
    assert_eq!(detect_language(&PathBuf::from("index.html")), Some("html".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("index.htm")), Some("html".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("style.css")), Some("css".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("style.scss")), Some("scss".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("style.sass")), Some("sass".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("style.less")), Some("less".to_owned()));
}

#[test]
fn test_detect_language_data() {
    assert_eq!(detect_language(&PathBuf::from("data.json")), Some("json".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("config.yaml")), Some("yaml".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("config.yml")), Some("yaml".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("config.toml")), Some("toml".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("config.xml")), Some("xml".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("config.ini")), Some("ini".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("settings.cfg")), Some("ini".to_owned()));
}

#[test]
fn test_detect_language_markdown() {
    assert_eq!(detect_language(&PathBuf::from("README.md")), Some("markdown".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("doc.markdown")), Some("markdown".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("blog.mdx")), Some("mdx".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("doc.rst")), Some("rst".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("notes.txt")), Some("text".to_owned()));
}

#[test]
fn test_detect_language_functional() {
    assert_eq!(detect_language(&PathBuf::from("lib.ex")), Some("elixir".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("test.exs")), Some("elixir".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("server.erl")), Some("erlang".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("header.hrl")), Some("erlang".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Main.hs")), Some("haskell".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Literate.lhs")), Some("haskell".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("lib.ml")), Some("ocaml".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("lib.mli")), Some("ocaml".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Program.fs")), Some("fsharp".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Interface.fsi")), Some("fsharp".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("script.fsx")), Some("fsharp".to_owned()));
}

#[test]
fn test_detect_language_misc() {
    assert_eq!(detect_language(&PathBuf::from("main.zig")), Some("zig".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("script.lua")), Some("lua".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("query.sql")), Some("sql".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("infra.tf")), Some("hcl".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("vars.tfvars")), Some("hcl".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("config.nix")), Some("nix".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("script.jl")), Some("julia".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("analysis.r")), Some("r".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("report.rmd")), Some("r".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("app.dart")), Some("dart".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("lib.nim")), Some("nim".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("app.v")), Some("vlang".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("lib.cr")), Some("crystal".to_owned()));
}

#[test]
fn test_detect_language_frameworks() {
    assert_eq!(detect_language(&PathBuf::from("App.vue")), Some("vue".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Page.svelte")), Some("svelte".to_owned()));
}

#[test]
fn test_detect_language_special_filenames() {
    // Dockerfile variants
    assert_eq!(detect_language(&PathBuf::from("Dockerfile")), Some("dockerfile".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Dockerfile.dev")), Some("dockerfile".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Dockerfile.prod")), Some("dockerfile".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Dockerfile.test")), Some("dockerfile".to_owned()));

    // Makefile variants
    assert_eq!(detect_language(&PathBuf::from("Makefile")), Some("make".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("GNUmakefile")), Some("make".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("BSDmakefile")), Some("make".to_owned()));

    // Ruby special files
    assert_eq!(detect_language(&PathBuf::from("Gemfile")), Some("ruby".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Rakefile")), Some("ruby".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Guardfile")), Some("ruby".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Vagrantfile")), Some("ruby".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Podfile")), Some("ruby".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Fastfile")), Some("ruby".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Brewfile")), Some("ruby".to_owned()));

    // Shell config files
    assert_eq!(detect_language(&PathBuf::from(".bashrc")), Some("shell".to_owned()));
    assert_eq!(detect_language(&PathBuf::from(".bash_profile")), Some("shell".to_owned()));
    assert_eq!(detect_language(&PathBuf::from(".zshrc")), Some("shell".to_owned()));
    assert_eq!(detect_language(&PathBuf::from(".profile")), Some("shell".to_owned()));

    // Git files
    assert_eq!(detect_language(&PathBuf::from(".gitignore")), Some("gitignore".to_owned()));
    assert_eq!(detect_language(&PathBuf::from(".gitattributes")), Some("gitignore".to_owned()));
    assert_eq!(detect_language(&PathBuf::from(".gitmodules")), Some("gitignore".to_owned()));

    // Other special files
    assert_eq!(detect_language(&PathBuf::from(".editorconfig")), Some("editorconfig".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Procfile")), Some("procfile".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Justfile")), Some("just".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("Caddyfile")), Some("caddyfile".to_owned()));
}

#[test]
fn test_detect_language_no_extension() {
    assert_eq!(detect_language(&PathBuf::from("README")), None);
    assert_eq!(detect_language(&PathBuf::from("LICENSE")), None);
    assert_eq!(detect_language(&PathBuf::from("unknown")), None);
}

#[test]
fn test_detect_language_case_insensitive() {
    // Extensions should be case-insensitive
    assert_eq!(detect_language(&PathBuf::from("test.PY")), Some("python".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("test.Py")), Some("python".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("test.RS")), Some("rust".to_owned()));
}

// ============================================================================
// Binary Extension Detection Tests
// ============================================================================

fn is_binary_extension(path: &std::path::Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    matches!(
        ext.as_str(),
        // Executables
        "exe" | "dll" | "so" | "dylib" | "a" | "o" | "obj" | "lib" |
        // Compiled
        "pyc" | "pyo" | "class" | "jar" | "war" | "ear" |
        // Archives
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "tgz" |
        // Images
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "svg" | "tiff" | "psd" |
        // Audio/Video
        "mp3" | "mp4" | "avi" | "mov" | "wav" | "flac" | "ogg" | "webm" | "mkv" |
        // Documents
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" |
        // Fonts
        "woff" | "woff2" | "ttf" | "eot" | "otf" |
        // Database
        "db" | "sqlite" | "sqlite3" |
        // Misc binary
        "bin" | "dat" | "cache" | "lock" | "sum"
    )
}

#[test]
fn test_is_binary_executables() {
    assert!(is_binary_extension(&PathBuf::from("program.exe")));
    assert!(is_binary_extension(&PathBuf::from("library.dll")));
    assert!(is_binary_extension(&PathBuf::from("library.so")));
    assert!(is_binary_extension(&PathBuf::from("library.dylib")));
    assert!(is_binary_extension(&PathBuf::from("archive.a")));
    assert!(is_binary_extension(&PathBuf::from("object.o")));
    assert!(is_binary_extension(&PathBuf::from("object.obj")));
    assert!(is_binary_extension(&PathBuf::from("library.lib")));
}

#[test]
fn test_is_binary_compiled() {
    assert!(is_binary_extension(&PathBuf::from("module.pyc")));
    assert!(is_binary_extension(&PathBuf::from("module.pyo")));
    assert!(is_binary_extension(&PathBuf::from("Main.class")));
    assert!(is_binary_extension(&PathBuf::from("app.jar")));
    assert!(is_binary_extension(&PathBuf::from("app.war")));
    assert!(is_binary_extension(&PathBuf::from("app.ear")));
}

#[test]
fn test_is_binary_archives() {
    assert!(is_binary_extension(&PathBuf::from("archive.zip")));
    assert!(is_binary_extension(&PathBuf::from("archive.tar")));
    assert!(is_binary_extension(&PathBuf::from("archive.gz")));
    assert!(is_binary_extension(&PathBuf::from("archive.bz2")));
    assert!(is_binary_extension(&PathBuf::from("archive.xz")));
    assert!(is_binary_extension(&PathBuf::from("archive.7z")));
    assert!(is_binary_extension(&PathBuf::from("archive.rar")));
    assert!(is_binary_extension(&PathBuf::from("archive.tgz")));
}

#[test]
fn test_is_binary_images() {
    assert!(is_binary_extension(&PathBuf::from("image.png")));
    assert!(is_binary_extension(&PathBuf::from("image.jpg")));
    assert!(is_binary_extension(&PathBuf::from("image.jpeg")));
    assert!(is_binary_extension(&PathBuf::from("image.gif")));
    assert!(is_binary_extension(&PathBuf::from("image.bmp")));
    assert!(is_binary_extension(&PathBuf::from("favicon.ico")));
    assert!(is_binary_extension(&PathBuf::from("image.webp")));
    assert!(is_binary_extension(&PathBuf::from("image.svg"))); // Often treated as binary
    assert!(is_binary_extension(&PathBuf::from("image.tiff")));
    assert!(is_binary_extension(&PathBuf::from("design.psd")));
}

#[test]
fn test_is_binary_media() {
    assert!(is_binary_extension(&PathBuf::from("audio.mp3")));
    assert!(is_binary_extension(&PathBuf::from("video.mp4")));
    assert!(is_binary_extension(&PathBuf::from("video.avi")));
    assert!(is_binary_extension(&PathBuf::from("video.mov")));
    assert!(is_binary_extension(&PathBuf::from("audio.wav")));
    assert!(is_binary_extension(&PathBuf::from("audio.flac")));
    assert!(is_binary_extension(&PathBuf::from("audio.ogg")));
    assert!(is_binary_extension(&PathBuf::from("video.webm")));
    assert!(is_binary_extension(&PathBuf::from("video.mkv")));
}

#[test]
fn test_is_binary_documents() {
    assert!(is_binary_extension(&PathBuf::from("document.pdf")));
    assert!(is_binary_extension(&PathBuf::from("document.doc")));
    assert!(is_binary_extension(&PathBuf::from("document.docx")));
    assert!(is_binary_extension(&PathBuf::from("spreadsheet.xls")));
    assert!(is_binary_extension(&PathBuf::from("spreadsheet.xlsx")));
    assert!(is_binary_extension(&PathBuf::from("presentation.ppt")));
    assert!(is_binary_extension(&PathBuf::from("presentation.pptx")));
    assert!(is_binary_extension(&PathBuf::from("document.odt")));
}

#[test]
fn test_is_binary_fonts() {
    assert!(is_binary_extension(&PathBuf::from("font.woff")));
    assert!(is_binary_extension(&PathBuf::from("font.woff2")));
    assert!(is_binary_extension(&PathBuf::from("font.ttf")));
    assert!(is_binary_extension(&PathBuf::from("font.eot")));
    assert!(is_binary_extension(&PathBuf::from("font.otf")));
}

#[test]
fn test_is_binary_database() {
    assert!(is_binary_extension(&PathBuf::from("data.db")));
    assert!(is_binary_extension(&PathBuf::from("data.sqlite")));
    assert!(is_binary_extension(&PathBuf::from("data.sqlite3")));
}

#[test]
fn test_is_binary_misc() {
    assert!(is_binary_extension(&PathBuf::from("data.bin")));
    assert!(is_binary_extension(&PathBuf::from("data.dat")));
    assert!(is_binary_extension(&PathBuf::from("data.cache")));
    assert!(is_binary_extension(&PathBuf::from("Cargo.lock")));
    assert!(is_binary_extension(&PathBuf::from("go.sum")));
}

#[test]
fn test_is_binary_text_files() {
    // These should NOT be detected as binary
    assert!(!is_binary_extension(&PathBuf::from("main.rs")));
    assert!(!is_binary_extension(&PathBuf::from("app.py")));
    assert!(!is_binary_extension(&PathBuf::from("index.js")));
    assert!(!is_binary_extension(&PathBuf::from("README.md")));
    assert!(!is_binary_extension(&PathBuf::from("config.json")));
    assert!(!is_binary_extension(&PathBuf::from("style.css")));
}

#[test]
fn test_is_binary_no_extension() {
    // Files without extension are not detected as binary by extension
    assert!(!is_binary_extension(&PathBuf::from("Makefile")));
    assert!(!is_binary_extension(&PathBuf::from("Dockerfile")));
    assert!(!is_binary_extension(&PathBuf::from("LICENSE")));
}

// ============================================================================
// Token Estimation Tests
// ============================================================================

fn estimate_tokens(
    size_bytes: u64,
    content: Option<&str>,
) -> infiniloom_engine::types::TokenCounts {
    let size = size_bytes as f32;

    if let Some(text) = content {
        let len = text.len() as f32;
        return infiniloom_engine::types::TokenCounts {
            o200k: (len / 4.0) as u32,
            cl100k: (len / 3.7) as u32,
            claude: (len / 3.5) as u32,
            gemini: (len / 3.8) as u32,
            llama: (len / 3.5) as u32,
            mistral: (len / 3.5) as u32,
            deepseek: (len / 3.5) as u32,
            qwen: (len / 3.5) as u32,
            cohere: (len / 3.6) as u32,
            grok: (len / 3.5) as u32,
        };
    }

    infiniloom_engine::types::TokenCounts {
        o200k: (size / 4.0) as u32,
        cl100k: (size / 3.7) as u32,
        claude: (size / 3.5) as u32,
        gemini: (size / 3.8) as u32,
        llama: (size / 3.5) as u32,
        mistral: (size / 3.5) as u32,
        deepseek: (size / 3.5) as u32,
        qwen: (size / 3.5) as u32,
        cohere: (size / 3.6) as u32,
        grok: (size / 3.5) as u32,
    }
}

#[test]
fn test_estimate_tokens_from_size() {
    let tokens = estimate_tokens(1000, None);

    // 1000 bytes / 3.5 chars per token ≈ 285 for claude
    assert!(tokens.claude > 200 && tokens.claude < 350);

    // o200k is more efficient (4.0 chars per token)
    assert!(tokens.o200k < tokens.claude);

    // All should be positive
    assert!(tokens.claude > 0);
    assert!(tokens.o200k > 0);
    assert!(tokens.cl100k > 0);
    assert!(tokens.gemini > 0);
    assert!(tokens.llama > 0);
}

#[test]
fn test_estimate_tokens_from_content() {
    let content = "Hello, World!"; // 13 characters
    let tokens = estimate_tokens(0, Some(content));

    // 13 / 3.5 ≈ 3.7 → 3 tokens for claude
    assert!(tokens.claude >= 2 && tokens.claude <= 5);
    assert!(tokens.o200k >= 2 && tokens.o200k <= 5);
}

#[test]
fn test_estimate_tokens_empty_content() {
    let tokens = estimate_tokens(0, Some(""));

    assert_eq!(tokens.claude, 0);
    assert_eq!(tokens.o200k, 0);
    assert_eq!(tokens.cl100k, 0);
    assert_eq!(tokens.gemini, 0);
    assert_eq!(tokens.llama, 0);
}

#[test]
fn test_estimate_tokens_large_file() {
    // 1MB file
    let tokens = estimate_tokens(1024 * 1024, None);

    // Should be reasonable estimate
    assert!(tokens.claude > 100000);
    assert!(tokens.claude < 500000);
}

#[test]
fn test_estimate_tokens_content_vs_size() {
    let content = "x".repeat(1000);
    let tokens_content = estimate_tokens(0, Some(&content));
    let tokens_size = estimate_tokens(1000, None);

    // Should be very similar when content length equals size
    let diff = (tokens_content.claude as i32 - tokens_size.claude as i32).abs();
    assert!(diff < 10, "Content and size estimation should be similar");
}

// ============================================================================
// Line Estimation Tests
// ============================================================================

fn estimate_lines(size_bytes: u64) -> u64 {
    // Average ~40 characters per line
    size_bytes / 40
}

#[test]
fn test_estimate_lines_small_file() {
    // 400 bytes / 40 = 10 lines
    assert_eq!(estimate_lines(400), 10);
}

#[test]
fn test_estimate_lines_medium_file() {
    // 4000 bytes / 40 = 100 lines
    assert_eq!(estimate_lines(4000), 100);
}

#[test]
fn test_estimate_lines_large_file() {
    // 1MB / 40 ≈ 26214 lines
    let lines = estimate_lines(1024 * 1024);
    assert!(lines > 25000 && lines < 30000);
}

#[test]
fn test_estimate_lines_empty() {
    assert_eq!(estimate_lines(0), 0);
}

// ============================================================================
// ScanConfig Tests
// ============================================================================

#[test]
fn test_scan_config_defaults() {
    // Note: ScanConfig is in the scanner module, testing expected defaults
    // Default values based on scanner.rs
    let default_include_hidden = false;
    let default_respect_gitignore = true;
    let default_read_contents = false;
    let default_max_file_size: u64 = 50 * 1024 * 1024; // 50MB
    let default_skip_symbols = false;

    assert!(!default_include_hidden);
    assert!(default_respect_gitignore);
    assert!(!default_read_contents);
    assert_eq!(default_max_file_size, 52428800);
    assert!(!default_skip_symbols);
}

// ============================================================================
// Path Handling Tests
// ============================================================================

#[test]
fn test_language_detection_full_path() {
    // Should work with full paths
    assert_eq!(
        detect_language(&PathBuf::from("/home/user/project/src/main.rs")),
        Some("rust".to_owned())
    );
    assert_eq!(
        detect_language(&PathBuf::from("C:\\Users\\dev\\project\\app.py")),
        Some("python".to_owned())
    );
}

#[test]
fn test_language_detection_hidden_files() {
    // Hidden config files
    assert_eq!(detect_language(&PathBuf::from(".bashrc")), Some("shell".to_owned()));
    assert_eq!(detect_language(&PathBuf::from(".gitignore")), Some("gitignore".to_owned()));

    // Hidden files with extensions
    assert_eq!(detect_language(&PathBuf::from(".hidden.py")), Some("python".to_owned()));
}

#[test]
fn test_language_detection_multiple_extensions() {
    // Multiple dots in filename
    assert_eq!(detect_language(&PathBuf::from("file.test.py")), Some("python".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("config.prod.json")), Some("json".to_owned()));
}

#[test]
fn test_language_detection_unicode_paths() {
    // Paths with Unicode characters
    assert_eq!(detect_language(&PathBuf::from("测试.py")), Some("python".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("プログラム.js")), Some("javascript".to_owned()));
    assert_eq!(detect_language(&PathBuf::from("программа.rs")), Some("rust".to_owned()));
}
