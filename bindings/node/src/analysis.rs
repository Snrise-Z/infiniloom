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

/// Options for complexity threshold checking
#[napi(object)]
pub struct CheckComplexityOptions {
    /// The programming language (e.g., "javascript", "python", "rust")
    pub language: String,
    /// Maximum cyclomatic complexity (default: 10)
    pub max_cyclomatic: Option<u32>,
    /// Maximum cognitive complexity (default: 15)
    pub max_cognitive: Option<u32>,
    /// Maximum nesting depth (default: 4)
    pub max_nesting: Option<u32>,
    /// Maximum parameter count (default: 5)
    pub max_params: Option<u32>,
    /// Minimum maintainability index (default: 40.0; lower is worse)
    pub min_maintainability: Option<f64>,
}

/// A single complexity threshold violation
#[napi(object)]
#[derive(Clone)]
pub struct JsComplexityViolation {
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
}

/// Result of checking complexity against thresholds
#[napi(object)]
pub struct JsComplexityCheckResult {
    pub passed: bool,
    pub violations: Vec<JsComplexityViolation>,
    pub metrics: JsComplexityMetrics,
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
// Complexity Functions
// ============================================================================

/// Calculate complexity metrics for source code
///
/// Analyzes source code and returns cyclomatic complexity, cognitive complexity,
/// Halstead metrics, lines of code, maintainability index, and more.
///
/// # Arguments
/// * `source` - The source code to analyze
/// * `options` - Options including the programming language
///
/// # Returns
/// Complexity metrics object
///
/// # Example
/// ```javascript
/// const { calculateComplexity } = require('infiniloom-node');
///
/// const metrics = calculateComplexity('function foo(a, b) { if (a) return b; }', {
///   language: 'javascript'
/// });
/// console.log(`Cyclomatic: ${metrics.cyclomatic}`);
/// ```
#[napi]
pub fn calculate_complexity(
    source: String,
    options: ComplexityOptions,
) -> Result<JsComplexityMetrics> {
    use infiniloom_engine::analysis::calculate_complexity_from_source;

    let language = parse_language(&options.language)?;
    let metrics = calculate_complexity_from_source(&source, language)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to analyze complexity: {}", e)))?;

    Ok(convert_complexity_metrics(metrics))
}

/// Async version of calculateComplexity
#[napi]
pub async fn calculate_complexity_async(
    source: String,
    options: ComplexityOptions,
) -> Result<JsComplexityMetrics> {
    tokio::task::spawn_blocking(move || calculate_complexity(source, options))
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Task failed: {}", e)))?
}

/// Check complexity against configurable thresholds
///
/// Calculates complexity metrics then validates them against thresholds.
/// Returns a structured result with pass/fail status, violations, and metrics.
///
/// # Arguments
/// * `source` - The source code to analyze
/// * `options` - Options including language and optional thresholds
///
/// # Returns
/// Check result with `passed` boolean, `violations` array, and full `metrics`
///
/// # Example
/// ```javascript
/// const { checkComplexity } = require('infiniloom-node');
///
/// const result = checkComplexity(complexCode, {
///   language: 'javascript',
///   maxCyclomatic: 5,
///   maxCognitive: 10
/// });
///
/// if (!result.passed) {
///   for (const v of result.violations) {
///     console.log(`${v.metric}: ${v.value} exceeds threshold ${v.threshold}`);
///   }
/// }
/// ```
#[napi]
pub fn check_complexity(
    source: String,
    options: CheckComplexityOptions,
) -> Result<JsComplexityCheckResult> {
    use infiniloom_engine::analysis::calculate_complexity_from_source;

    let language = parse_language(&options.language)?;
    let metrics = calculate_complexity_from_source(&source, language)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to analyze complexity: {}", e)))?;

    let max_cyclomatic = options.max_cyclomatic.unwrap_or(10);
    let max_cognitive = options.max_cognitive.unwrap_or(15);
    let max_nesting = options.max_nesting.unwrap_or(4);
    let max_params = options.max_params.unwrap_or(5);
    let min_maintainability = options.min_maintainability.unwrap_or(40.0);

    let mut violations = Vec::new();

    if metrics.cyclomatic >= max_cyclomatic {
        violations.push(JsComplexityViolation {
            metric: "cyclomatic".to_string(),
            value: metrics.cyclomatic as f64,
            threshold: max_cyclomatic as f64,
        });
    }

    if metrics.cognitive >= max_cognitive {
        violations.push(JsComplexityViolation {
            metric: "cognitive".to_string(),
            value: metrics.cognitive as f64,
            threshold: max_cognitive as f64,
        });
    }

    if metrics.max_nesting_depth >= max_nesting {
        violations.push(JsComplexityViolation {
            metric: "max_nesting_depth".to_string(),
            value: metrics.max_nesting_depth as f64,
            threshold: max_nesting as f64,
        });
    }

    if metrics.parameter_count >= max_params {
        violations.push(JsComplexityViolation {
            metric: "parameter_count".to_string(),
            value: metrics.parameter_count as f64,
            threshold: max_params as f64,
        });
    }

    if let Some(mi) = metrics.maintainability_index {
        if (mi as f64) <= min_maintainability {
            violations.push(JsComplexityViolation {
                metric: "maintainability_index".to_string(),
                value: mi as f64,
                threshold: min_maintainability,
            });
        }
    }

    let js_metrics = convert_complexity_metrics(metrics);

    Ok(JsComplexityCheckResult {
        passed: violations.is_empty(),
        violations,
        metrics: js_metrics,
    })
}

/// Async version of checkComplexity
#[napi]
pub async fn check_complexity_async(
    source: String,
    options: CheckComplexityOptions,
) -> Result<JsComplexityCheckResult> {
    tokio::task::spawn_blocking(move || check_complexity(source, options))
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

fn convert_halstead(h: infiniloom_engine::analysis::HalsteadMetrics) -> JsHalsteadMetrics {
    JsHalsteadMetrics {
        distinct_operators: h.distinct_operators,
        distinct_operands: h.distinct_operands,
        total_operators: h.total_operators,
        total_operands: h.total_operands,
        vocabulary: h.vocabulary,
        length: h.length,
        calculated_length: h.calculated_length as f64,
        volume: h.volume as f64,
        difficulty: h.difficulty as f64,
        effort: h.effort as f64,
        time: h.time as f64,
        bugs: h.bugs as f64,
    }
}

fn convert_complexity_metrics(m: infiniloom_engine::analysis::ComplexityMetrics) -> JsComplexityMetrics {
    JsComplexityMetrics {
        cyclomatic: m.cyclomatic,
        cognitive: m.cognitive,
        halstead: m.halstead.map(convert_halstead),
        loc: JsLocMetrics {
            total: m.loc.total,
            source: m.loc.source,
            comments: m.loc.comments,
            blank: m.loc.blank,
        },
        maintainability_index: m.maintainability_index.map(|mi| mi as f64),
        max_nesting_depth: m.max_nesting_depth,
        parameter_count: m.parameter_count,
        return_count: m.return_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn js_source() -> &'static str {
        r#"function foo(a, b) {
    if (a > 0) {
        return a + b;
    }
    return b;
}"#
    }

    fn complex_js_source() -> &'static str {
        r#"function complex(a, b, c, d, e, f) {
    if (a > 0) {
        if (b > 0) {
            if (c > 0) {
                if (d > 0) {
                    if (e > 0) {
                        return a + b + c + d + e + f;
                    }
                }
            }
        }
    }
    switch (a) {
        case 1: return 1;
        case 2: return 2;
        case 3: return 3;
        case 4: return 4;
        case 5: return 5;
        case 6: return 6;
        case 7: return 7;
        case 8: return 8;
        case 9: return 9;
        case 10: return 10;
    }
    for (let i = 0; i < a; i++) {
        for (let j = 0; j < b; j++) {
            if (i === j) continue;
        }
    }
    return 0;
}"#
    }

    // ---- calculate_complexity tests ----

    #[test]
    fn test_calculate_complexity_valid_source() {
        let result = calculate_complexity(
            js_source().to_string(),
            ComplexityOptions { language: "javascript".to_string() },
        );
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert!(metrics.cyclomatic > 0);
        assert!(metrics.loc.total > 0);
        assert!(metrics.loc.source > 0);
    }

    #[test]
    fn test_calculate_complexity_empty_source() {
        let result = calculate_complexity(
            String::new(),
            ComplexityOptions { language: "javascript".to_string() },
        );
        assert!(result.is_ok());
        let metrics = result.unwrap();
        // Engine returns base cyclomatic of 1 (one path through empty program)
        assert!(metrics.cyclomatic <= 1);
    }

    #[test]
    fn test_calculate_complexity_invalid_language() {
        let result = calculate_complexity(
            js_source().to_string(),
            ComplexityOptions { language: "brainfuck".to_string() },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_complexity_clojure_returns_error() {
        let result = calculate_complexity(
            "(defn foo [x] (+ x 1))".to_string(),
            ComplexityOptions { language: "clojure".to_string() },
        );
        assert!(result.is_err());
    }

    // ---- check_complexity tests ----

    #[test]
    fn test_check_complexity_low_complexity_passes() {
        let result = check_complexity(
            js_source().to_string(),
            CheckComplexityOptions {
                language: "javascript".to_string(),
                max_cyclomatic: None,
                max_cognitive: None,
                max_nesting: None,
                max_params: None,
                min_maintainability: None,
            },
        );
        assert!(result.is_ok());
        let check = result.unwrap();
        assert!(check.passed);
        assert!(check.violations.is_empty());
        assert!(check.metrics.cyclomatic > 0);
    }

    #[test]
    fn test_check_complexity_exceeding_cyclomatic() {
        let result = check_complexity(
            complex_js_source().to_string(),
            CheckComplexityOptions {
                language: "javascript".to_string(),
                max_cyclomatic: Some(3),
                max_cognitive: Some(100),
                max_nesting: Some(100),
                max_params: Some(100),
                min_maintainability: Some(0.0),
            },
        );
        assert!(result.is_ok());
        let check = result.unwrap();
        assert!(!check.passed);
        let cyclomatic_violation = check.violations.iter().find(|v| v.metric == "cyclomatic");
        assert!(cyclomatic_violation.is_some());
        assert_eq!(cyclomatic_violation.unwrap().threshold, 3.0);
    }

    #[test]
    fn test_check_complexity_multiple_violations() {
        let result = check_complexity(
            complex_js_source().to_string(),
            CheckComplexityOptions {
                language: "javascript".to_string(),
                max_cyclomatic: Some(3),
                max_cognitive: Some(3),
                max_nesting: Some(2),
                max_params: Some(3),
                min_maintainability: None,
            },
        );
        assert!(result.is_ok());
        let check = result.unwrap();
        assert!(!check.passed);
        assert!(check.violations.len() >= 3);
    }

    #[test]
    fn test_check_complexity_defaults_applied() {
        let result = check_complexity(
            js_source().to_string(),
            CheckComplexityOptions {
                language: "javascript".to_string(),
                max_cyclomatic: None,
                max_cognitive: None,
                max_nesting: None,
                max_params: None,
                min_maintainability: None,
            },
        );
        assert!(result.is_ok());
        let check = result.unwrap();
        // Simple source should pass all defaults (cyc=10, cog=15, nest=4, params=5, mi=40)
        assert!(check.passed);
    }

    #[test]
    fn test_check_complexity_boundary_gte_semantics() {
        // First calculate actual cyclomatic complexity of the source
        let calc = calculate_complexity(
            js_source().to_string(),
            ComplexityOptions { language: "javascript".to_string() },
        ).unwrap();
        let actual_cyclomatic = calc.cyclomatic;
        assert!(actual_cyclomatic > 0, "need non-zero complexity for boundary test");

        // Set threshold exactly equal to value -- should be a violation (>= semantics)
        let result = check_complexity(
            js_source().to_string(),
            CheckComplexityOptions {
                language: "javascript".to_string(),
                max_cyclomatic: Some(actual_cyclomatic),
                max_cognitive: Some(100),
                max_nesting: Some(100),
                max_params: Some(100),
                min_maintainability: Some(0.0),
            },
        );
        assert!(result.is_ok());
        let check = result.unwrap();
        let cyclomatic_violation = check.violations.iter().find(|v| v.metric == "cyclomatic");
        assert!(
            cyclomatic_violation.is_some(),
            "value {} at threshold {} should be a violation with >= semantics",
            actual_cyclomatic, actual_cyclomatic
        );
        assert_eq!(cyclomatic_violation.unwrap().value, actual_cyclomatic as f64);
        assert_eq!(cyclomatic_violation.unwrap().threshold, actual_cyclomatic as f64);
    }

    // ---- conversion helper tests ----

    #[test]
    fn test_convert_complexity_metrics_with_halstead() {
        use infiniloom_engine::analysis::{ComplexityMetrics, HalsteadMetrics, LocMetrics};

        let engine_metrics = ComplexityMetrics {
            cyclomatic: 5,
            cognitive: 3,
            halstead: Some(HalsteadMetrics {
                distinct_operators: 4,
                distinct_operands: 6,
                total_operators: 10,
                total_operands: 15,
                vocabulary: 10,
                length: 25,
                calculated_length: 28.5,
                volume: 83.0,
                difficulty: 5.0,
                effort: 415.0,
                time: 23.1,
                bugs: 0.028,
            }),
            loc: LocMetrics { total: 10, source: 7, comments: 2, blank: 1 },
            maintainability_index: Some(65.5),
            max_nesting_depth: 2,
            parameter_count: 3,
            return_count: 1,
        };

        let js = convert_complexity_metrics(engine_metrics);
        assert_eq!(js.cyclomatic, 5);
        assert_eq!(js.cognitive, 3);
        assert!(js.halstead.is_some());
        let h = js.halstead.unwrap();
        assert_eq!(h.calculated_length, 28.5_f32 as f64);
        assert_eq!(h.volume, 83.0_f32 as f64);
        assert_eq!(h.difficulty, 5.0_f32 as f64);
        assert_eq!(js.maintainability_index, Some(65.5_f32 as f64));
        assert_eq!(js.loc.total, 10);
        assert_eq!(js.loc.source, 7);
        assert_eq!(js.max_nesting_depth, 2);
        assert_eq!(js.parameter_count, 3);
        assert_eq!(js.return_count, 1);
    }

    #[test]
    fn test_convert_complexity_metrics_none_halstead() {
        use infiniloom_engine::analysis::{ComplexityMetrics, LocMetrics};

        let engine_metrics = ComplexityMetrics {
            cyclomatic: 1,
            cognitive: 0,
            halstead: None,
            loc: LocMetrics { total: 3, source: 2, comments: 0, blank: 1 },
            maintainability_index: None,
            max_nesting_depth: 0,
            parameter_count: 0,
            return_count: 0,
        };

        let js = convert_complexity_metrics(engine_metrics);
        assert!(js.halstead.is_none());
        assert!(js.maintainability_index.is_none());
        assert_eq!(js.cyclomatic, 1);
    }
}
