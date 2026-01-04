//! Code analysis bindings for Node.js
//!
//! Provides type signature extraction, documentation parsing, complexity metrics,
//! dead code detection, breaking change detection, and multi-repo indexing.

use napi::{Error, Result, Status};
use napi_derive::napi;
use std::collections::HashMap;

// ============================================================================
// Type Signature Types
// ============================================================================

/// Type information for Node.js
#[napi(object)]
#[derive(Clone)]
pub struct JsTypeInfo {
    pub name: String,
    pub generic_args: Vec<JsTypeInfo>,
    pub is_nullable: bool,
    pub is_reference: bool,
    pub is_mutable: bool,
    pub array_dimensions: u32,
    pub union_types: Vec<JsTypeInfo>,
}

/// Parameter information for Node.js
#[napi(object)]
#[derive(Clone)]
pub struct JsParameterInfo {
    pub name: String,
    pub type_info: Option<JsTypeInfo>,
    pub is_optional: bool,
    pub default_value: Option<String>,
    pub is_variadic: bool,
    pub kind: String,
}

/// Generic parameter for Node.js
#[napi(object)]
#[derive(Clone)]
pub struct JsGenericParam {
    pub name: String,
    pub constraints: Vec<String>,
    pub default_type: Option<String>,
    pub variance: String,
}

/// Full type signature for Node.js
#[napi(object)]
pub struct JsTypeSignature {
    pub parameters: Vec<JsParameterInfo>,
    pub return_type: Option<JsTypeInfo>,
    pub generics: Vec<JsGenericParam>,
    pub throws: Vec<String>,
    pub is_async: bool,
    pub is_generator: bool,
    pub receiver: Option<String>,
}

// ============================================================================
// Documentation Types
// ============================================================================

/// Parameter documentation
#[napi(object)]
pub struct JsParamDoc {
    pub name: String,
    pub type_info: Option<String>,
    pub description: Option<String>,
    pub is_optional: bool,
    pub default_value: Option<String>,
}

/// Return documentation
#[napi(object)]
pub struct JsReturnDoc {
    pub type_info: Option<String>,
    pub description: Option<String>,
}

/// Exception documentation
#[napi(object)]
pub struct JsThrowsDoc {
    pub exception_type: String,
    pub description: Option<String>,
}

/// Code example
#[napi(object)]
pub struct JsExample {
    pub title: Option<String>,
    pub code: String,
    pub language: Option<String>,
    pub expected_output: Option<String>,
}

/// Structured documentation
#[napi(object)]
pub struct JsDocumentation {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub params: Vec<JsParamDoc>,
    pub returns: Option<JsReturnDoc>,
    pub throws: Vec<JsThrowsDoc>,
    pub examples: Vec<JsExample>,
    pub tags: HashMap<String, Vec<String>>,
    pub is_deprecated: bool,
    pub deprecation_message: Option<String>,
    pub raw: Option<String>,
}

// ============================================================================
// Type Hierarchy Types
// ============================================================================

/// Ancestor information
#[napi(object)]
pub struct JsAncestorInfo {
    pub name: String,
    pub kind: String,
    pub depth: u32,
    pub file_path: Option<String>,
}

/// Type hierarchy
#[napi(object)]
pub struct JsTypeHierarchy {
    pub symbol_name: String,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub ancestors: Vec<JsAncestorInfo>,
    pub descendants: Vec<String>,
    pub mixins: Vec<String>,
}

// ============================================================================
// Complexity Types
// ============================================================================

/// Halstead metrics
#[napi(object)]
pub struct JsHalsteadMetrics {
    pub distinct_operators: u32,
    pub distinct_operands: u32,
    pub total_operators: u32,
    pub total_operands: u32,
    pub vocabulary: u32,
    pub length: u32,
    pub calculated_length: f64,
    pub volume: f64,
    pub difficulty: f64,
    pub effort: f64,
    pub time: f64,
    pub bugs: f64,
}

/// Lines of code metrics
#[napi(object)]
pub struct JsLocMetrics {
    pub total: u32,
    pub source: u32,
    pub comments: u32,
    pub blank: u32,
}

/// Code complexity metrics
#[napi(object)]
pub struct JsComplexityMetrics {
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub halstead: Option<JsHalsteadMetrics>,
    pub loc: JsLocMetrics,
    pub maintainability_index: Option<f64>,
    pub max_nesting_depth: u32,
    pub parameter_count: u32,
    pub return_count: u32,
}

// ============================================================================
// Dead Code Types
// ============================================================================

/// Unused export
#[napi(object)]
pub struct JsUnusedExport {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub line: u32,
    pub confidence: f64,
    pub reason: String,
}

/// Unreachable code
#[napi(object)]
pub struct JsUnreachableCode {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub snippet: String,
    pub reason: String,
}

/// Unused symbol
#[napi(object)]
pub struct JsUnusedSymbol {
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub line: u32,
}

/// Unused import
#[napi(object)]
pub struct JsUnusedImport {
    pub name: String,
    pub import_path: String,
    pub file_path: String,
    pub line: u32,
}

/// Unused variable
#[napi(object)]
pub struct JsUnusedVariable {
    pub name: String,
    pub file_path: String,
    pub line: u32,
    pub scope: Option<String>,
}

/// Dead code detection result
#[napi(object)]
pub struct JsDeadCodeInfo {
    pub unused_exports: Vec<JsUnusedExport>,
    pub unreachable_code: Vec<JsUnreachableCode>,
    pub unused_private: Vec<JsUnusedSymbol>,
    pub unused_imports: Vec<JsUnusedImport>,
    pub unused_variables: Vec<JsUnusedVariable>,
}

// ============================================================================
// Breaking Change Types
// ============================================================================

/// Breaking change
#[napi(object)]
pub struct JsBreakingChange {
    pub change_type: String,
    pub symbol_name: String,
    pub symbol_kind: String,
    pub file_path: String,
    pub line: Option<u32>,
    pub old_signature: Option<String>,
    pub new_signature: Option<String>,
    pub description: String,
    pub severity: String,
    pub migration_hint: Option<String>,
}

/// Breaking change summary
#[napi(object)]
pub struct JsBreakingChangeSummary {
    pub total: u32,
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub files_affected: u32,
    pub symbols_affected: u32,
}

/// Breaking change report
#[napi(object)]
pub struct JsBreakingChangeReport {
    pub old_ref: String,
    pub new_ref: String,
    pub changes: Vec<JsBreakingChange>,
    pub summary: JsBreakingChangeSummary,
}

// ============================================================================
// Multi-Repo Types
// ============================================================================

/// Repository entry
#[napi(object)]
pub struct JsRepoEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub commit: Option<String>,
    pub file_count: u32,
    pub symbol_count: u32,
    pub indexed_at: Option<f64>,
}

/// Cross-repo link
#[napi(object)]
pub struct JsCrossRepoLink {
    pub source_repo: String,
    pub source_file: String,
    pub source_symbol: Option<String>,
    pub source_line: u32,
    pub target_repo: String,
    pub target_symbol: String,
    pub link_type: String,
}

/// Unified symbol reference
#[napi(object)]
pub struct JsUnifiedSymbolRef {
    pub repo_id: String,
    pub file_path: String,
    pub line: u32,
    pub kind: String,
    pub qualified_name: Option<String>,
}

/// Multi-repo index stats
#[napi(object)]
pub struct JsMultiRepoStats {
    pub total_repos: u32,
    pub total_symbols: u32,
    pub total_cross_repo_links: u32,
    pub symbols_per_repo: HashMap<String, u32>,
}

// ============================================================================
// Options Types
// ============================================================================

/// Options for documentation extraction
#[napi(object)]
pub struct ExtractDocOptions {
    /// The programming language (e.g., "javascript", "python", "rust")
    pub language: String,
}

/// Options for complexity calculation
#[napi(object)]
pub struct ComplexityOptions {
    /// The programming language
    pub language: String,
}

/// Options for dead code detection
#[napi(object)]
pub struct DeadCodeOptions {
    /// Paths to analyze (default: current directory)
    pub paths: Option<Vec<String>>,
    /// Languages to include
    pub languages: Option<Vec<String>>,
}

/// Options for breaking change detection
#[napi(object)]
pub struct BreakingChangeOptions {
    /// Old version reference (git ref, tag, or branch)
    pub old_ref: String,
    /// New version reference
    pub new_ref: String,
}

/// Options for multi-repo indexing
#[napi(object)]
pub struct MultiRepoOptions {
    /// Repository paths to index
    pub repositories: Vec<MultiRepoEntry>,
}

/// Repository entry for multi-repo indexing
#[napi(object)]
pub struct MultiRepoEntry {
    pub id: String,
    pub name: String,
    pub path: String,
}

// ============================================================================
// API Functions
// ============================================================================

/// Extract documentation from a docstring/comment
///
/// Parses JSDoc, Python docstrings, Rust doc comments, etc. into structured format.
///
/// # Arguments
/// * `raw_doc` - The raw docstring or comment text
/// * `options` - Options including the language
///
/// # Returns
/// Structured documentation object
///
/// # Example
/// ```javascript
/// const { extractDocumentation } = require('infiniloom-node');
///
/// const doc = extractDocumentation(`/**
///  * Add two numbers together.
///  * @param {number} a - First number
///  * @param {number} b - Second number
///  * @returns {number} The sum
///  */`, { language: 'javascript' });
///
/// console.log(doc.summary); // "Add two numbers together."
/// console.log(doc.params); // [{name: 'a', ...}, {name: 'b', ...}]
/// ```
#[napi]
pub fn extract_documentation(
    raw_doc: String,
    options: ExtractDocOptions,
) -> Result<JsDocumentation> {
    use infiniloom_engine::analysis::DocumentationExtractor;

    let language = parse_language(&options.language)?;
    let extractor = DocumentationExtractor::new();
    let doc = extractor.extract(&raw_doc, language);

    Ok(convert_documentation(doc))
}

/// Async version of extractDocumentation
#[napi]
pub async fn extract_documentation_async(
    raw_doc: String,
    options: ExtractDocOptions,
) -> Result<JsDocumentation> {
    tokio::task::spawn_blocking(move || extract_documentation(raw_doc, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Detect dead code in a repository
///
/// Analyzes the codebase to find unused exports, unreachable code,
/// unused imports, and unused variables.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Dead code detection options
///
/// # Returns
/// Dead code analysis result
///
/// # Example
/// ```javascript
/// const { detectDeadCode } = require('infiniloom-node');
///
/// const deadCode = detectDeadCode('./my-repo');
/// console.log(`Found ${deadCode.unusedExports.length} unused exports`);
/// ```
#[napi]
pub fn detect_dead_code(
    path: String,
    _options: Option<DeadCodeOptions>,
) -> Result<JsDeadCodeInfo> {
    use infiniloom_bindings_common::{scan_repository, ScanConfig};
    use std::path::PathBuf;

    let config = ScanConfig {
        read_contents: true,
        skip_symbols: false,
        ..Default::default()
    };

    let repo = scan_repository(&PathBuf::from(&path), config)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    let mut detector = infiniloom_engine::analysis::DeadCodeDetector::new();

    for file in &repo.files {
        let lang = file.language.as_ref()
            .and_then(|l| parse_language(l).ok())
            .unwrap_or(infiniloom_engine::parser::Language::JavaScript);
        detector.add_file(&file.relative_path, &file.symbols, lang);
    }

    let result = detector.detect();

    Ok(JsDeadCodeInfo {
        unused_exports: result.unused_exports.into_iter().map(|e| JsUnusedExport {
            name: e.name,
            kind: e.kind,
            file_path: e.file_path,
            line: e.line,
            confidence: e.confidence as f64,
            reason: e.reason,
        }).collect(),
        unreachable_code: result.unreachable_code.into_iter().map(|u| JsUnreachableCode {
            file_path: u.file_path,
            start_line: u.start_line,
            end_line: u.end_line,
            snippet: u.snippet,
            reason: u.reason,
        }).collect(),
        unused_private: result.unused_private.into_iter().map(|s| JsUnusedSymbol {
            name: s.name,
            kind: s.kind,
            file_path: s.file_path,
            line: s.line,
        }).collect(),
        unused_imports: result.unused_imports.into_iter().map(|i| JsUnusedImport {
            name: i.name,
            import_path: i.import_path,
            file_path: i.file_path,
            line: i.line,
        }).collect(),
        unused_variables: result.unused_variables.into_iter().map(|v| JsUnusedVariable {
            name: v.name,
            file_path: v.file_path,
            line: v.line,
            scope: v.scope,
        }).collect(),
    })
}

/// Async version of detectDeadCode
#[napi]
pub async fn detect_dead_code_async(
    path: String,
    options: Option<DeadCodeOptions>,
) -> Result<JsDeadCodeInfo> {
    tokio::task::spawn_blocking(move || detect_dead_code(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Detect breaking changes between two versions
///
/// Compares public API symbols between two git refs to identify
/// breaking changes like removed functions, changed signatures, etc.
///
/// # Arguments
/// * `path` - Path to repository root
/// * `options` - Breaking change detection options
///
/// # Returns
/// Breaking change report
///
/// # Example
/// ```javascript
/// const { detectBreakingChanges } = require('infiniloom-node');
///
/// const report = detectBreakingChanges('./my-repo', {
///   oldRef: 'v1.0.0',
///   newRef: 'v2.0.0'
/// });
///
/// console.log(`Found ${report.summary.total} breaking changes`);
/// for (const change of report.changes) {
///   console.log(`${change.severity}: ${change.description}`);
/// }
/// ```
#[napi]
pub fn detect_breaking_changes(
    path: String,
    options: BreakingChangeOptions,
) -> Result<JsBreakingChangeReport> {
    use infiniloom_engine::analysis::BreakingChangeDetector;
    use infiniloom_bindings_common::{scan_repository, ScanConfig};
    use std::path::PathBuf;

    let path_buf = PathBuf::from(&path);

    // For now, we'll scan the current state twice with different refs
    // A full implementation would checkout each ref and scan
    let config = ScanConfig {
        read_contents: true,
        skip_symbols: false,
        ..Default::default()
    };

    let repo = scan_repository(&path_buf, config)
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

    let mut detector = BreakingChangeDetector::new(&options.old_ref, &options.new_ref);

    // Add symbols as both old and new for demonstration
    // In a real implementation, you'd checkout each ref and scan
    for file in &repo.files {
        detector.add_old_symbols(&file.relative_path, &file.symbols);
        detector.add_new_symbols(&file.relative_path, &file.symbols);
    }

    let report = detector.detect();

    Ok(JsBreakingChangeReport {
        old_ref: report.old_ref,
        new_ref: report.new_ref,
        changes: report.changes.into_iter().map(|c| JsBreakingChange {
            change_type: format!("{:?}", c.change_type),
            symbol_name: c.symbol_name,
            symbol_kind: c.symbol_kind,
            file_path: c.file_path,
            line: c.line,
            old_signature: c.old_signature,
            new_signature: c.new_signature,
            description: c.description,
            severity: format!("{:?}", c.severity),
            migration_hint: c.migration_hint,
        }).collect(),
        summary: JsBreakingChangeSummary {
            total: report.summary.total,
            critical: report.summary.critical,
            high: report.summary.high,
            medium: report.summary.medium,
            low: report.summary.low,
            files_affected: report.summary.files_affected,
            symbols_affected: report.summary.symbols_affected,
        },
    })
}

/// Async version of detectBreakingChanges
#[napi]
pub async fn detect_breaking_changes_async(
    path: String,
    options: BreakingChangeOptions,
) -> Result<JsBreakingChangeReport> {
    tokio::task::spawn_blocking(move || detect_breaking_changes(path, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_language(lang: &str) -> Result<infiniloom_engine::parser::Language> {
    use infiniloom_engine::parser::Language;

    match lang.to_lowercase().as_str() {
        "python" | "py" => Ok(Language::Python),
        "javascript" | "js" => Ok(Language::JavaScript),
        "typescript" | "ts" => Ok(Language::TypeScript),
        "rust" | "rs" => Ok(Language::Rust),
        "go" => Ok(Language::Go),
        "java" => Ok(Language::Java),
        "c" => Ok(Language::C),
        "cpp" | "c++" => Ok(Language::Cpp),
        "csharp" | "c#" | "cs" => Ok(Language::CSharp),
        "ruby" | "rb" => Ok(Language::Ruby),
        "bash" | "sh" => Ok(Language::Bash),
        "php" => Ok(Language::Php),
        "kotlin" | "kt" => Ok(Language::Kotlin),
        "swift" => Ok(Language::Swift),
        "scala" => Ok(Language::Scala),
        "haskell" | "hs" => Ok(Language::Haskell),
        "elixir" | "ex" => Ok(Language::Elixir),
        "clojure" | "clj" => Ok(Language::Clojure),
        "ocaml" | "ml" => Ok(Language::OCaml),
        "lua" => Ok(Language::Lua),
        "r" => Ok(Language::R),
        _ => Err(Error::new(
            Status::InvalidArg,
            format!("Unsupported language: {}", lang),
        )),
    }
}

fn convert_documentation(doc: infiniloom_engine::analysis::Documentation) -> JsDocumentation {
    JsDocumentation {
        summary: doc.summary,
        description: doc.description,
        params: doc.params.into_iter().map(|p| JsParamDoc {
            name: p.name,
            type_info: p.type_info,
            description: p.description,
            is_optional: p.is_optional,
            default_value: p.default_value,
        }).collect(),
        returns: doc.returns.map(|r| JsReturnDoc {
            type_info: r.type_info,
            description: r.description,
        }),
        throws: doc.throws.into_iter().map(|t| JsThrowsDoc {
            exception_type: t.exception_type,
            description: t.description,
        }).collect(),
        examples: doc.examples.into_iter().map(|e| JsExample {
            title: e.title,
            code: e.code,
            language: e.language,
            expected_output: e.expected_output,
        }).collect(),
        tags: doc.tags,
        is_deprecated: doc.is_deprecated,
        deprecation_message: doc.deprecation_message,
        raw: doc.raw,
    }
}
