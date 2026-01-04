//! Dead code detection for all supported languages
//!
//! Detects unused exports, unreachable code, unused private symbols,
//! unused imports, and unused variables.

use crate::analysis::types::{
    DeadCodeInfo, UnreachableCode, UnusedExport, UnusedImport, UnusedSymbol, UnusedVariable,
};
use crate::parser::Language;
use crate::types::{Symbol, SymbolKind, Visibility};
use std::collections::{HashMap, HashSet};

/// Detects dead code across a codebase
pub struct DeadCodeDetector {
    /// Map of symbol name to its definition info
    definitions: HashMap<String, DefinitionInfo>,
    /// Set of referenced symbols
    references: HashSet<String>,
    /// Map of import name to import info
    imports: HashMap<String, ImportInfo>,
    /// Map of variable name to variable info (per scope)
    variables: HashMap<String, HashMap<String, VariableInfo>>,
    /// Files being analyzed
    files: Vec<FileInfo>,
}

#[derive(Debug, Clone)]
struct DefinitionInfo {
    name: String,
    kind: SymbolKind,
    visibility: Visibility,
    file_path: String,
    line: u32,
    is_entry_point: bool,
}

#[derive(Debug, Clone)]
struct ImportInfo {
    name: String,
    import_path: String,
    file_path: String,
    line: u32,
    is_used: bool,
}

#[derive(Debug, Clone)]
struct VariableInfo {
    name: String,
    file_path: String,
    line: u32,
    scope: String,
    is_used: bool,
}

#[derive(Debug, Clone)]
struct FileInfo {
    path: String,
    language: Language,
    symbols: Vec<Symbol>,
}

impl DeadCodeDetector {
    /// Create a new detector
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
            references: HashSet::new(),
            imports: HashMap::new(),
            variables: HashMap::new(),
            files: Vec::new(),
        }
    }

    /// Add a file's symbols for analysis
    pub fn add_file(&mut self, file_path: &str, symbols: &[Symbol], language: Language) {
        self.files.push(FileInfo {
            path: file_path.to_owned(),
            language,
            symbols: symbols.to_vec(),
        });

        for symbol in symbols {
            // Track definition
            let is_entry_point = self.is_entry_point(symbol, language);

            self.definitions.insert(
                symbol.name.clone(),
                DefinitionInfo {
                    name: symbol.name.clone(),
                    kind: symbol.kind,
                    visibility: symbol.visibility,
                    file_path: file_path.to_owned(),
                    line: symbol.start_line,
                    is_entry_point,
                },
            );

            // Track references (calls, extends, implements)
            for call in &symbol.calls {
                self.references.insert(call.clone());
            }

            if let Some(ref extends) = symbol.extends {
                self.references.insert(extends.clone());
            }

            for implements in &symbol.implements {
                self.references.insert(implements.clone());
            }

            // Track parent reference
            if let Some(ref parent) = symbol.parent {
                self.references.insert(parent.clone());
            }
        }
    }

    /// Add import information
    pub fn add_import(&mut self, name: &str, import_path: &str, file_path: &str, line: u32) {
        self.imports.insert(
            format!("{}:{}", file_path, name),
            ImportInfo {
                name: name.to_owned(),
                import_path: import_path.to_owned(),
                file_path: file_path.to_owned(),
                line,
                is_used: false,
            },
        );
    }

    /// Add variable information
    pub fn add_variable(&mut self, name: &str, file_path: &str, line: u32, scope: &str) {
        let scope_vars = self.variables.entry(scope.to_owned()).or_default();
        scope_vars.insert(
            name.to_owned(),
            VariableInfo {
                name: name.to_owned(),
                file_path: file_path.to_owned(),
                line,
                scope: scope.to_owned(),
                is_used: false,
            },
        );
    }

    /// Mark an import as used
    pub fn mark_import_used(&mut self, name: &str, file_path: &str) {
        let key = format!("{}:{}", file_path, name);
        if let Some(import) = self.imports.get_mut(&key) {
            import.is_used = true;
        }
    }

    /// Mark a variable as used
    pub fn mark_variable_used(&mut self, name: &str, scope: &str) {
        if let Some(scope_vars) = self.variables.get_mut(scope) {
            if let Some(var) = scope_vars.get_mut(name) {
                var.is_used = true;
            }
        }
    }

    /// Check if a symbol is an entry point (should not be flagged as unused)
    fn is_entry_point(&self, symbol: &Symbol, language: Language) -> bool {
        let name = &symbol.name;

        // Common entry point patterns
        if name == "main" || name == "init" || name == "__init__" {
            return true;
        }

        // Test functions
        if name.starts_with("test_")
            || name.starts_with("Test")
            || name.ends_with("_test")
            || name.ends_with("Test")
        {
            return true;
        }

        // Language-specific entry points
        match language {
            Language::Python => {
                // __all__ exports, __main__, decorators like @app.route
                name.starts_with("__") && name.ends_with("__")
            },
            Language::JavaScript | Language::TypeScript => {
                // Module exports, React components, etc.
                name.chars().next().is_some_and(|c| c.is_uppercase())
                    && matches!(symbol.kind, SymbolKind::Class | SymbolKind::Function)
            },
            Language::Rust => {
                // pub items, #[test], #[no_mangle]
                matches!(symbol.visibility, Visibility::Public)
            },
            Language::Go => {
                // Exported (capitalized) names
                name.chars().next().is_some_and(|c| c.is_uppercase())
            },
            Language::Java | Language::Kotlin => {
                // public static void main, @Test, @Bean, etc.
                name == "main"
                    || matches!(symbol.visibility, Visibility::Public)
                        && matches!(symbol.kind, SymbolKind::Method)
            },
            Language::Ruby => {
                // initialize, Rails callbacks
                name == "initialize" || name.starts_with("before_") || name.starts_with("after_")
            },
            Language::Php => {
                // __construct, __destruct, magic methods
                name.starts_with("__")
            },
            Language::Swift => {
                // @main, viewDidLoad, etc.
                name == "viewDidLoad" || name == "applicationDidFinishLaunching"
            },
            Language::Elixir => {
                // start, init, handle_* callbacks
                name == "start" || name.starts_with("handle_") || name == "child_spec"
            },
            _ => false,
        }
    }

    /// Detect all dead code
    pub fn detect(&self) -> DeadCodeInfo {
        DeadCodeInfo {
            unused_exports: self.find_unused_exports(),
            unreachable_code: Vec::new(), // Requires AST analysis
            unused_private: self.find_unused_private(),
            unused_imports: self.find_unused_imports(),
            unused_variables: self.find_unused_variables(),
        }
    }

    /// Find unused public exports
    fn find_unused_exports(&self) -> Vec<UnusedExport> {
        let mut unused = Vec::new();

        for (name, def) in &self.definitions {
            // Skip if not public
            if !matches!(def.visibility, Visibility::Public) {
                continue;
            }

            // Skip entry points
            if def.is_entry_point {
                continue;
            }

            // Skip if referenced
            if self.references.contains(name) {
                continue;
            }

            // Calculate confidence based on analysis scope
            let confidence = self.calculate_confidence(def);

            unused.push(UnusedExport {
                name: name.clone(),
                kind: format!("{:?}", def.kind),
                file_path: def.file_path.clone(),
                line: def.line,
                confidence,
                reason: "No references found in analyzed codebase".to_owned(),
            });
        }

        unused
    }

    /// Find unused private symbols
    fn find_unused_private(&self) -> Vec<UnusedSymbol> {
        let mut unused = Vec::new();

        for (name, def) in &self.definitions {
            // Only check private symbols
            if matches!(def.visibility, Visibility::Public) {
                continue;
            }

            // Skip entry points
            if def.is_entry_point {
                continue;
            }

            // Skip if referenced
            if self.references.contains(name) {
                continue;
            }

            unused.push(UnusedSymbol {
                name: name.clone(),
                kind: format!("{:?}", def.kind),
                file_path: def.file_path.clone(),
                line: def.line,
            });
        }

        unused
    }

    /// Find unused imports
    fn find_unused_imports(&self) -> Vec<UnusedImport> {
        self.imports
            .values()
            .filter(|import| !import.is_used)
            .map(|import| UnusedImport {
                name: import.name.clone(),
                import_path: import.import_path.clone(),
                file_path: import.file_path.clone(),
                line: import.line,
            })
            .collect()
    }

    /// Find unused variables
    fn find_unused_variables(&self) -> Vec<UnusedVariable> {
        let mut unused = Vec::new();

        for (scope, vars) in &self.variables {
            for var in vars.values() {
                if !var.is_used {
                    // Skip underscore-prefixed variables (intentionally unused)
                    if var.name.starts_with('_') {
                        continue;
                    }

                    unused.push(UnusedVariable {
                        name: var.name.clone(),
                        file_path: var.file_path.clone(),
                        line: var.line,
                        scope: Some(scope.clone()),
                    });
                }
            }
        }

        unused
    }

    /// Calculate confidence for unused export detection
    fn calculate_confidence(&self, def: &DefinitionInfo) -> f32 {
        let mut confidence: f32 = 0.5; // Base confidence

        // Higher confidence for private/internal visibility
        if matches!(def.visibility, Visibility::Private | Visibility::Internal) {
            confidence += 0.3;
        }

        // Higher confidence for certain symbol kinds
        match def.kind {
            SymbolKind::Function | SymbolKind::Method => confidence += 0.1,
            SymbolKind::Class | SymbolKind::Struct => confidence += 0.05,
            SymbolKind::Variable | SymbolKind::Constant => confidence += 0.15,
            _ => {},
        }

        // Cap at 0.95 since we can't be 100% sure without full program analysis
        confidence.min(0.95)
    }
}

impl Default for DeadCodeDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect unreachable code in a function body
pub fn detect_unreachable_code(
    source: &str,
    file_path: &str,
    _language: Language,
) -> Vec<UnreachableCode> {
    let mut unreachable = Vec::new();

    let lines: Vec<&str> = source.lines().collect();
    let mut after_terminator = false;
    let mut terminator_line = 0u32;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let line_num = (i + 1) as u32;

        // Check for terminators
        if is_terminator(trimmed) {
            after_terminator = true;
            terminator_line = line_num;
            continue;
        }

        // If we're after a terminator and this is code (not blank/comment/closing brace)
        if after_terminator {
            if trimmed.is_empty()
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || trimmed == "}"
                || trimmed == ")"
                || trimmed == "]"
            {
                continue;
            }

            // Check for control flow that makes it reachable
            if trimmed.starts_with("case ")
                || trimmed.starts_with("default:")
                || trimmed.starts_with("else")
                || trimmed.starts_with("catch")
                || trimmed.starts_with("except")
                || trimmed.starts_with("rescue")
                || trimmed.starts_with("finally")
            {
                after_terminator = false;
                continue;
            }

            // This is unreachable code
            unreachable.push(UnreachableCode {
                file_path: file_path.to_owned(),
                start_line: line_num,
                end_line: line_num,
                snippet: trimmed.to_owned(),
                reason: format!("Code after terminator on line {}", terminator_line),
            });

            after_terminator = false;
        }
    }

    unreachable
}

/// Check if a line is a terminator (return, throw, break, continue, etc.)
fn is_terminator(line: &str) -> bool {
    let terminators = [
        "return",
        "return;",
        "throw",
        "raise",
        "break",
        "break;",
        "continue",
        "continue;",
        "exit",
        "exit(",
        "die(",
        "panic!",
        "unreachable!",
    ];

    for term in &terminators {
        if line.starts_with(term) || line == *term {
            return true;
        }
    }

    // Language-specific patterns
    if line.starts_with("return ") && line.ends_with(';') {
        return true;
    }

    false
}

/// Convenience function to detect dead code in a set of files
pub fn detect_dead_code(files: &[(String, Vec<Symbol>, Language)]) -> DeadCodeInfo {
    let mut detector = DeadCodeDetector::new();

    for (path, symbols, language) in files {
        detector.add_file(path, symbols, *language);
    }

    detector.detect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Visibility;

    fn make_symbol(name: &str, kind: SymbolKind, visibility: Visibility) -> Symbol {
        Symbol {
            name: name.to_owned(),
            kind,
            visibility,
            start_line: 1,
            end_line: 10,
            ..Default::default()
        }
    }

    #[test]
    fn test_unused_export_detection() {
        let mut detector = DeadCodeDetector::new();

        // Use C language where public visibility doesn't auto-mark as entry point
        let symbols = [
            make_symbol("used_func", SymbolKind::Function, Visibility::Public),
            make_symbol("unused_func", SymbolKind::Function, Visibility::Public),
        ];

        // Add a reference to used_func
        let caller = Symbol {
            name: "caller".to_owned(),
            kind: SymbolKind::Function,
            visibility: Visibility::Private,
            calls: vec!["used_func".to_owned()],
            ..Default::default()
        };

        detector.add_file("test.c", &[symbols[0].clone(), symbols[1].clone(), caller], Language::C);

        let result = detector.detect();

        // unused_func should be detected
        assert!(result
            .unused_exports
            .iter()
            .any(|e| e.name == "unused_func"));

        // used_func should not be detected
        assert!(!result.unused_exports.iter().any(|e| e.name == "used_func"));
    }

    #[test]
    fn test_entry_point_not_flagged() {
        let mut detector = DeadCodeDetector::new();

        let symbols = vec![
            make_symbol("main", SymbolKind::Function, Visibility::Public),
            make_symbol("test_something", SymbolKind::Function, Visibility::Public),
            make_symbol("__init__", SymbolKind::Method, Visibility::Public),
        ];

        detector.add_file("test.py", &symbols, Language::Python);

        let result = detector.detect();

        // Entry points should not be flagged
        assert!(!result.unused_exports.iter().any(|e| e.name == "main"));
        assert!(!result
            .unused_exports
            .iter()
            .any(|e| e.name == "test_something"));
        assert!(!result.unused_exports.iter().any(|e| e.name == "__init__"));
    }

    #[test]
    fn test_unreachable_code_detection() {
        let source = r#"
fn example() {
    let x = 1;
    return x;
    let y = 2; // unreachable
    println!("{}", y);
}
"#;

        let unreachable = detect_unreachable_code(source, "test.rs", Language::Rust);

        assert!(!unreachable.is_empty());
        assert!(unreachable.iter().any(|u| u.snippet.contains("let y")));
    }

    #[test]
    fn test_is_terminator() {
        assert!(is_terminator("return;"));
        assert!(is_terminator("return x"));
        assert!(is_terminator("throw new Error()"));
        assert!(is_terminator("break;"));
        assert!(is_terminator("continue;"));
        assert!(is_terminator("panic!(\"error\")"));

        assert!(!is_terminator("let x = 1;"));
        assert!(!is_terminator("if (x) {"));
        assert!(!is_terminator("// return"));
    }

    #[test]
    fn test_unused_imports() {
        let mut detector = DeadCodeDetector::new();

        detector.add_import("used_import", "some/path", "test.ts", 1);
        detector.add_import("unused_import", "other/path", "test.ts", 2);

        detector.mark_import_used("used_import", "test.ts");

        let result = detector.detect();

        assert_eq!(result.unused_imports.len(), 1);
        assert_eq!(result.unused_imports[0].name, "unused_import");
    }

    #[test]
    fn test_underscore_variables_ignored() {
        let mut detector = DeadCodeDetector::new();

        detector.add_variable("_unused", "test.rs", 1, "main");
        detector.add_variable("used", "test.rs", 2, "main");
        detector.add_variable("not_used", "test.rs", 3, "main");

        detector.mark_variable_used("used", "main");

        let result = detector.detect();

        // _unused should be ignored (intentionally unused)
        assert!(!result.unused_variables.iter().any(|v| v.name == "_unused"));

        // not_used should be flagged
        assert!(result.unused_variables.iter().any(|v| v.name == "not_used"));
    }
}
