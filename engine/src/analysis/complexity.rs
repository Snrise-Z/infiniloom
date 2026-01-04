//! Code complexity metrics calculation for all supported languages
//!
//! Computes cyclomatic complexity, cognitive complexity, Halstead metrics,
//! and maintainability index for functions/methods.

use crate::analysis::types::{ComplexityMetrics, HalsteadMetrics, LocMetrics};
use crate::parser::Language;
use std::collections::HashSet;
use tree_sitter::Node;

/// Calculates complexity metrics from AST nodes
pub struct ComplexityCalculator {
    /// Source code being analyzed
    source: String,
}

impl ComplexityCalculator {
    /// Create a new calculator with the given source code
    pub fn new(source: impl Into<String>) -> Self {
        Self { source: source.into() }
    }

    /// Get text for a node
    fn node_text(&self, node: &Node<'_>) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    /// Calculate all complexity metrics for a function node
    pub fn calculate(&self, node: &Node<'_>, language: Language) -> ComplexityMetrics {
        let cyclomatic = self.cyclomatic_complexity(node, language);
        let cognitive = self.cognitive_complexity(node, language);
        let halstead = self.halstead_metrics(node, language);
        let loc = self.loc_metrics(node);
        let max_nesting_depth = self.max_nesting_depth(node, language);
        let parameter_count = self.parameter_count(node, language);
        let return_count = self.return_count(node, language);

        // Calculate maintainability index (MI)
        // Formula: MI = 171 - 5.2 * ln(V) - 0.23 * CC - 16.2 * ln(LOC)
        // Where V = Halstead Volume, CC = Cyclomatic Complexity, LOC = Lines of Code
        let maintainability_index = halstead.as_ref().map(|h| {
            let v = h.volume.max(1.0);
            let cc = cyclomatic as f32;
            let loc = loc.source.max(1) as f32;

            let mi = 171.0 - 5.2 * v.ln() - 0.23 * cc - 16.2 * loc.ln();
            // Normalize to 0-100 scale
            (mi.max(0.0) * 100.0 / 171.0).min(100.0)
        });

        ComplexityMetrics {
            cyclomatic,
            cognitive,
            halstead,
            loc,
            maintainability_index,
            max_nesting_depth,
            parameter_count,
            return_count,
        }
    }

    /// Calculate cyclomatic complexity (McCabe's complexity)
    ///
    /// CC = E - N + 2P (for a single function, P=1)
    /// Simplified: CC = 1 + number of decision points
    ///
    /// Decision points: if, else if, while, for, case, catch, &&, ||, ?:
    pub fn cyclomatic_complexity(&self, node: &Node<'_>, language: Language) -> u32 {
        let mut complexity = 1; // Base complexity

        self.walk_tree(node, &mut |child| {
            if self.is_decision_point(child, language) {
                complexity += 1;
            }
        });

        complexity
    }

    /// Check if a node is a decision point (contributes to cyclomatic complexity)
    fn is_decision_point(&self, node: &Node<'_>, language: Language) -> bool {
        let kind = node.kind();

        // Language-agnostic decision points
        let common_decisions = [
            "if_statement",
            "if_expression",
            "if",
            "else_if",
            "elif",
            "elsif",
            "while_statement",
            "while_expression",
            "while",
            "for_statement",
            "for_expression",
            "for",
            "for_in_statement",
            "foreach",
            "case",
            "when",
            "match_arm",
            "catch_clause",
            "except_clause",
            "rescue",
            "conditional_expression", // ternary
            "ternary_expression",
            "binary_expression",
            "logical_and",
            "logical_or",
        ];

        if common_decisions.contains(&kind) {
            return true;
        }

        // Check for && and || operators in binary expressions
        if kind == "binary_expression" || kind == "binary_operator" {
            let text = self.node_text(node);
            if text.contains("&&")
                || text.contains("||")
                || text.contains(" and ")
                || text.contains(" or ")
            {
                return true;
            }
        }

        // Language-specific decision points
        match language {
            Language::Rust => {
                matches!(kind, "match_expression" | "if_let_expression" | "while_let_expression")
            },
            Language::Go => matches!(kind, "select_statement" | "type_switch_statement"),
            Language::Swift => matches!(kind, "guard_statement" | "switch_statement"),
            Language::Kotlin => matches!(kind, "when_expression"),
            Language::Haskell => matches!(kind, "case_expression" | "guard"),
            Language::Elixir => matches!(kind, "case" | "cond" | "with"),
            Language::Clojure => matches!(kind, "cond" | "case"),
            Language::OCaml => matches!(kind, "match_expression"),
            _ => false,
        }
    }

    /// Calculate cognitive complexity
    ///
    /// Cognitive complexity measures how hard code is to understand.
    /// It penalizes nesting, breaks in linear flow, and complex control structures.
    pub fn cognitive_complexity(&self, node: &Node<'_>, language: Language) -> u32 {
        let mut complexity = 0;
        self.cognitive_walk(node, language, 0, &mut complexity);
        complexity
    }

    fn cognitive_walk(
        &self,
        node: &Node<'_>,
        language: Language,
        nesting: u32,
        complexity: &mut u32,
    ) {
        let kind = node.kind();

        // Increment for control flow structures
        let is_control_flow = self.is_control_flow(kind, language);
        if is_control_flow {
            // Base increment
            *complexity += 1;
            // Nesting increment
            *complexity += nesting;
        }

        // Increment for breaks in linear flow
        if self.is_flow_break(kind, language) {
            *complexity += 1;
        }

        // Recursion penalty
        if self.is_recursion(node, language) {
            *complexity += 1;
        }

        // Walk children with updated nesting
        let new_nesting = if is_control_flow || self.is_nesting_structure(kind, language) {
            nesting + 1
        } else {
            nesting
        };

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.cognitive_walk(&child, language, new_nesting, complexity);
        }
    }

    fn is_control_flow(&self, kind: &str, language: Language) -> bool {
        let common_control = [
            "if_statement",
            "if_expression",
            "while_statement",
            "while_expression",
            "for_statement",
            "for_expression",
            "for_in_statement",
            "switch_statement",
            "match_expression",
            "try_statement",
        ];

        if common_control.contains(&kind) {
            return true;
        }

        match language {
            Language::Rust => matches!(kind, "if_let_expression" | "while_let_expression"),
            Language::Go => matches!(kind, "select_statement"),
            Language::Swift => matches!(kind, "guard_statement"),
            _ => false,
        }
    }

    fn is_flow_break(&self, kind: &str, _language: Language) -> bool {
        matches!(
            kind,
            "break_statement"
                | "continue_statement"
                | "goto_statement"
                | "return_statement"
                | "throw_statement"
                | "raise"
        )
    }

    fn is_nesting_structure(&self, kind: &str, _language: Language) -> bool {
        matches!(
            kind,
            "lambda_expression"
                | "anonymous_function"
                | "closure_expression"
                | "block"
                | "arrow_function"
                | "function_expression"
        )
    }

    fn is_recursion(&self, node: &Node<'_>, _language: Language) -> bool {
        // Check if this node is a function call to the current function
        // This is a simplified check - full recursion detection would need function context
        if node.kind() == "call_expression" || node.kind() == "function_call" {
            // Would need to compare called function name with enclosing function name
            // For now, return false - this would need more context
        }
        false
    }

    /// Calculate Halstead complexity metrics
    pub fn halstead_metrics(&self, node: &Node<'_>, language: Language) -> Option<HalsteadMetrics> {
        let mut operators = HashSet::new();
        let mut operands = HashSet::new();
        let mut total_operators = 0u32;
        let mut total_operands = 0u32;

        self.walk_tree(node, &mut |child| {
            let kind = child.kind();
            let text = self.node_text(child);

            if self.is_operator(kind, language) {
                operators.insert(text.to_owned());
                total_operators += 1;
            } else if self.is_operand(kind, language) {
                operands.insert(text.to_owned());
                total_operands += 1;
            }
        });

        let n1 = operators.len() as u32; // distinct operators
        let n2 = operands.len() as u32; // distinct operands
        let nn1 = total_operators; // total operators
        let nn2 = total_operands; // total operands

        if n1 == 0 || n2 == 0 {
            return None;
        }

        let vocabulary = n1 + n2;
        let length = nn1 + nn2;

        // Calculated length: n1 * log2(n1) + n2 * log2(n2)
        let calculated_length = (n1 as f32) * (n1 as f32).log2() + (n2 as f32) * (n2 as f32).log2();

        // Volume: N * log2(n)
        let volume = (length as f32) * (vocabulary as f32).log2();

        // Difficulty: (n1/2) * (N2/n2)
        let difficulty = ((n1 as f32) / 2.0) * ((nn2 as f32) / (n2 as f32).max(1.0));

        // Effort: D * V
        let effort = difficulty * volume;

        // Time to program: E / 18 (seconds)
        let time = effort / 18.0;

        // Estimated bugs: V / 3000
        let bugs = volume / 3000.0;

        Some(HalsteadMetrics {
            distinct_operators: n1,
            distinct_operands: n2,
            total_operators: nn1,
            total_operands: nn2,
            vocabulary,
            length,
            calculated_length,
            volume,
            difficulty,
            effort,
            time,
            bugs,
        })
    }

    fn is_operator(&self, kind: &str, _language: Language) -> bool {
        matches!(
            kind,
            "binary_operator"
                | "unary_operator"
                | "assignment_operator"
                | "comparison_operator"
                | "arithmetic_operator"
                | "logical_operator"
                | "bitwise_operator"
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "="
                | "=="
                | "!="
                | "<"
                | ">"
                | "<="
                | ">="
                | "&&"
                | "||"
                | "!"
                | "&"
                | "|"
                | "^"
                | "~"
                | "<<"
                | ">>"
                | "+="
                | "-="
                | "*="
                | "/="
                | "."
                | "->"
                | "::"
                | "?"
                | ":"
        )
    }

    fn is_operand(&self, kind: &str, _language: Language) -> bool {
        matches!(
            kind,
            "identifier"
                | "number"
                | "integer"
                | "float"
                | "string"
                | "string_literal"
                | "number_literal"
                | "integer_literal"
                | "float_literal"
                | "boolean"
                | "true"
                | "false"
                | "nil"
                | "null"
                | "none"
        )
    }

    /// Calculate lines of code metrics
    pub fn loc_metrics(&self, node: &Node<'_>) -> LocMetrics {
        let text = self.node_text(node);
        let lines: Vec<&str> = text.lines().collect();

        let mut source = 0u32;
        let mut comments = 0u32;
        let mut blank = 0u32;

        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                blank += 1;
            } else if self.is_comment_line(trimmed) {
                comments += 1;
            } else {
                source += 1;
            }
        }

        LocMetrics { total: lines.len() as u32, source, comments, blank }
    }

    fn is_comment_line(&self, line: &str) -> bool {
        line.starts_with("//")
            || line.starts_with('#')
            || line.starts_with("/*")
            || line.starts_with('*')
            || line.starts_with("*/")
            || line.starts_with("--")
            || line.starts_with(";;")
            || line.starts_with("\"\"\"")
            || line.starts_with("'''")
    }

    /// Calculate maximum nesting depth
    pub fn max_nesting_depth(&self, node: &Node<'_>, language: Language) -> u32 {
        let mut max_depth = 0;
        self.nesting_walk(node, language, 0, &mut max_depth);
        max_depth
    }

    fn nesting_walk(&self, node: &Node<'_>, language: Language, depth: u32, max_depth: &mut u32) {
        let kind = node.kind();

        let is_nesting =
            self.is_control_flow(kind, language) || self.is_nesting_structure(kind, language);

        let new_depth = if is_nesting { depth + 1 } else { depth };

        if new_depth > *max_depth {
            *max_depth = new_depth;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.nesting_walk(&child, language, new_depth, max_depth);
        }
    }

    /// Count number of parameters
    pub fn parameter_count(&self, node: &Node<'_>, _language: Language) -> u32 {
        let mut count = 0;

        // Find parameters node
        if let Some(params) = node.child_by_field_name("parameters") {
            let mut cursor = params.walk();
            for child in params.children(&mut cursor) {
                let kind = child.kind();
                if kind.contains("parameter")
                    || kind == "identifier"
                    || kind == "typed_parameter"
                    || kind == "formal_parameter"
                {
                    count += 1;
                }
            }
        }

        count
    }

    /// Count number of return statements
    pub fn return_count(&self, node: &Node<'_>, _language: Language) -> u32 {
        let mut count = 0;

        self.walk_tree(node, &mut |child| {
            if child.kind() == "return_statement" || child.kind() == "return" {
                count += 1;
            }
        });

        // If no explicit return but function has expression body, count as 1
        if count == 0 {
            count = 1;
        }

        count
    }

    /// Walk tree and apply callback to each node
    fn walk_tree<F>(&self, node: &Node<'_>, callback: &mut F)
    where
        F: FnMut(&Node<'_>),
    {
        callback(node);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_tree(&child, callback);
        }
    }
}

/// Calculate complexity for a function given its source code
pub fn calculate_complexity(
    source: &str,
    node: &Node<'_>,
    language: Language,
) -> ComplexityMetrics {
    let calculator = ComplexityCalculator::new(source);
    calculator.calculate(node, language)
}

/// Calculate complexity for source code without needing a tree-sitter node
///
/// This is a convenience function that handles the parsing internally.
/// Returns an error if the source cannot be parsed.
pub fn calculate_complexity_from_source(
    source: &str,
    language: Language,
) -> Result<ComplexityMetrics, String> {
    // Get tree-sitter language for parsing
    let ts_language = match language {
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::Go => tree_sitter_go::LANGUAGE.into(),
        Language::Java => tree_sitter_java::LANGUAGE.into(),
        Language::C => tree_sitter_c::LANGUAGE.into(),
        Language::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        Language::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
        Language::Ruby => tree_sitter_ruby::LANGUAGE.into(),
        Language::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        Language::Swift => tree_sitter_swift::LANGUAGE.into(),
        Language::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
        Language::Scala => tree_sitter_scala::LANGUAGE.into(),
        Language::Haskell => tree_sitter_haskell::LANGUAGE.into(),
        Language::Elixir => tree_sitter_elixir::LANGUAGE.into(),
        Language::Clojure => tree_sitter_clojure::LANGUAGE.into(),
        Language::OCaml => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
        Language::Lua => tree_sitter_lua::LANGUAGE.into(),
        Language::R => tree_sitter_r::LANGUAGE.into(),
        Language::Bash => tree_sitter_bash::LANGUAGE.into(),
        // FSharp doesn't have tree-sitter support yet
        Language::FSharp => {
            return Err(
                "F# complexity analysis not yet supported (no tree-sitter parser available)"
                    .to_owned(),
            )
        },
    };

    let mut ts_parser = tree_sitter::Parser::new();
    ts_parser
        .set_language(&ts_language)
        .map_err(|e| format!("Failed to set language: {}", e))?;

    let tree = ts_parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse source code".to_owned())?;

    let calculator = ComplexityCalculator::new(source);
    Ok(calculator.calculate(&tree.root_node(), language))
}

/// Thresholds for complexity warnings
#[derive(Debug, Clone, Copy)]
pub struct ComplexityThresholds {
    /// Cyclomatic complexity warning threshold
    pub cyclomatic_warn: u32,
    /// Cyclomatic complexity error threshold
    pub cyclomatic_error: u32,
    /// Cognitive complexity warning threshold
    pub cognitive_warn: u32,
    /// Cognitive complexity error threshold
    pub cognitive_error: u32,
    /// Max nesting depth warning threshold
    pub nesting_warn: u32,
    /// Max nesting depth error threshold
    pub nesting_error: u32,
    /// Max parameter count warning threshold
    pub params_warn: u32,
    /// Max parameter count error threshold
    pub params_error: u32,
    /// Maintainability index warning threshold (below this)
    pub maintainability_warn: f32,
    /// Maintainability index error threshold (below this)
    pub maintainability_error: f32,
}

impl Default for ComplexityThresholds {
    fn default() -> Self {
        Self {
            cyclomatic_warn: 10,
            cyclomatic_error: 20,
            cognitive_warn: 15,
            cognitive_error: 30,
            nesting_warn: 4,
            nesting_error: 6,
            params_warn: 5,
            params_error: 8,
            maintainability_warn: 40.0,
            maintainability_error: 20.0,
        }
    }
}

/// Severity of a complexity issue
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexitySeverity {
    Ok,
    Warning,
    Error,
}

/// Check complexity metrics against thresholds
pub fn check_complexity(
    metrics: &ComplexityMetrics,
    thresholds: &ComplexityThresholds,
) -> Vec<(String, ComplexitySeverity)> {
    let mut issues = Vec::new();

    // Cyclomatic complexity
    if metrics.cyclomatic >= thresholds.cyclomatic_error {
        issues.push((
            format!(
                "Cyclomatic complexity {} exceeds error threshold {}",
                metrics.cyclomatic, thresholds.cyclomatic_error
            ),
            ComplexitySeverity::Error,
        ));
    } else if metrics.cyclomatic >= thresholds.cyclomatic_warn {
        issues.push((
            format!(
                "Cyclomatic complexity {} exceeds warning threshold {}",
                metrics.cyclomatic, thresholds.cyclomatic_warn
            ),
            ComplexitySeverity::Warning,
        ));
    }

    // Cognitive complexity
    if metrics.cognitive >= thresholds.cognitive_error {
        issues.push((
            format!(
                "Cognitive complexity {} exceeds error threshold {}",
                metrics.cognitive, thresholds.cognitive_error
            ),
            ComplexitySeverity::Error,
        ));
    } else if metrics.cognitive >= thresholds.cognitive_warn {
        issues.push((
            format!(
                "Cognitive complexity {} exceeds warning threshold {}",
                metrics.cognitive, thresholds.cognitive_warn
            ),
            ComplexitySeverity::Warning,
        ));
    }

    // Nesting depth
    if metrics.max_nesting_depth >= thresholds.nesting_error {
        issues.push((
            format!(
                "Nesting depth {} exceeds error threshold {}",
                metrics.max_nesting_depth, thresholds.nesting_error
            ),
            ComplexitySeverity::Error,
        ));
    } else if metrics.max_nesting_depth >= thresholds.nesting_warn {
        issues.push((
            format!(
                "Nesting depth {} exceeds warning threshold {}",
                metrics.max_nesting_depth, thresholds.nesting_warn
            ),
            ComplexitySeverity::Warning,
        ));
    }

    // Parameter count
    if metrics.parameter_count >= thresholds.params_error {
        issues.push((
            format!(
                "Parameter count {} exceeds error threshold {}",
                metrics.parameter_count, thresholds.params_error
            ),
            ComplexitySeverity::Error,
        ));
    } else if metrics.parameter_count >= thresholds.params_warn {
        issues.push((
            format!(
                "Parameter count {} exceeds warning threshold {}",
                metrics.parameter_count, thresholds.params_warn
            ),
            ComplexitySeverity::Warning,
        ));
    }

    // Maintainability index
    if let Some(mi) = metrics.maintainability_index {
        if mi <= thresholds.maintainability_error {
            issues.push((
                format!(
                    "Maintainability index {:.1} below error threshold {}",
                    mi, thresholds.maintainability_error
                ),
                ComplexitySeverity::Error,
            ));
        } else if mi <= thresholds.maintainability_warn {
            issues.push((
                format!(
                    "Maintainability index {:.1} below warning threshold {}",
                    mi, thresholds.maintainability_warn
                ),
                ComplexitySeverity::Warning,
            ));
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loc_metrics() {
        let source = r#"
fn example() {
    // Comment
    let x = 1;

    /* Multi-line
     * comment */
    let y = 2;
}
"#;
        let calculator = ComplexityCalculator::new(source);
        // Note: This would need actual tree-sitter node for proper test
        // For now, test the helper
        assert!(calculator.is_comment_line("// Comment"));
        assert!(calculator.is_comment_line("/* Multi-line"));
        assert!(!calculator.is_comment_line("let x = 1;"));
    }

    #[test]
    fn test_thresholds_default() {
        let thresholds = ComplexityThresholds::default();
        assert_eq!(thresholds.cyclomatic_warn, 10);
        assert_eq!(thresholds.cognitive_warn, 15);
    }

    #[test]
    fn test_check_complexity() {
        let metrics = ComplexityMetrics {
            cyclomatic: 25,
            cognitive: 35,
            max_nesting_depth: 7,
            parameter_count: 10,
            maintainability_index: Some(15.0),
            ..Default::default()
        };

        let thresholds = ComplexityThresholds::default();
        let issues = check_complexity(&metrics, &thresholds);

        // Should have errors for all metrics
        assert!(issues.len() >= 4);
        assert!(issues
            .iter()
            .any(|(msg, sev)| { msg.contains("Cyclomatic") && *sev == ComplexitySeverity::Error }));
    }
}
