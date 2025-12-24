//! Context expander implementation.
//!
//! Given a diff (changed files and lines), this module expands the context
//! to include relevant dependent files, symbols, and call graphs.

use super::types::{
    CallChain, ChangeClassification, ChangeType, ContextFile, ContextSymbol, DiffChange,
    ExpandedContext, ImpactLevel, ImpactSummary,
};
use crate::index::types::{DepGraph, FileEntry, IndexSymbol, IndexSymbolKind, SymbolIndex};
use std::collections::{HashSet, VecDeque};

use super::types::ContextDepth;

/// Context expander
pub struct ContextExpander<'a> {
    index: &'a SymbolIndex,
    graph: &'a DepGraph,
}

impl<'a> ContextExpander<'a> {
    /// Create a new context expander
    pub fn new(index: &'a SymbolIndex, graph: &'a DepGraph) -> Self {
        Self { index, graph }
    }

    /// Classify a change for smart expansion
    ///
    /// Analyzes the diff content and symbol kinds to determine what type of change occurred,
    /// which affects how aggressively we expand context.
    pub fn classify_change(
        &self,
        change: &DiffChange,
        symbol: Option<&IndexSymbol>,
    ) -> ChangeClassification {
        // File-level classifications
        if change.change_type == ChangeType::Deleted {
            return ChangeClassification::Deletion;
        }
        if change.change_type == ChangeType::Renamed {
            return ChangeClassification::FileRename;
        }
        if change.change_type == ChangeType::Added {
            return ChangeClassification::NewCode;
        }

        // Analyze diff content if available for more detailed classification
        if let Some(diff) = &change.diff_content {
            // Check for signature changes (function/method definition lines changed)
            let signature_indicators = [
                "fn ",
                "def ",
                "function ",
                "func ",
                "pub fn ",
                "async fn ",
                "class ",
                "struct ",
                "enum ",
                "interface ",
                "type ",
                "trait ",
            ];
            let has_signature_change = diff.lines().any(|line| {
                let trimmed = line.trim_start_matches(['+', '-', ' ']);
                signature_indicators
                    .iter()
                    .any(|ind| trimmed.starts_with(ind))
            });

            if has_signature_change {
                // Check if it's a type definition
                let type_indicators =
                    ["class ", "struct ", "enum ", "interface ", "type ", "trait "];
                if diff.lines().any(|line| {
                    let trimmed = line.trim_start_matches(['+', '-', ' ']);
                    type_indicators.iter().any(|ind| trimmed.starts_with(ind))
                }) {
                    return ChangeClassification::TypeDefinitionChange;
                }
                return ChangeClassification::SignatureChange;
            }

            // Check for import changes
            let import_indicators = ["import ", "from ", "require(", "use ", "#include"];
            if diff.lines().any(|line| {
                let trimmed = line.trim_start_matches(['+', '-', ' ']);
                import_indicators.iter().any(|ind| trimmed.starts_with(ind))
            }) {
                return ChangeClassification::ImportChange;
            }

            // Check for documentation-only changes
            let doc_indicators = ["///", "//!", "/**", "/*", "#", "\"\"\"", "'''"];
            let all_doc_changes = diff
                .lines()
                .filter(|l| l.starts_with('+') || l.starts_with('-'))
                .filter(|l| l.len() > 1) // Skip empty diff markers
                .all(|line| {
                    let trimmed = line[1..].trim();
                    trimmed.is_empty()
                        || doc_indicators.iter().any(|ind| trimmed.starts_with(ind))
                });
            if all_doc_changes {
                return ChangeClassification::DocumentationOnly;
            }
        }

        // Symbol-based classification
        if let Some(sym) = symbol {
            match sym.kind {
                IndexSymbolKind::Class
                | IndexSymbolKind::Struct
                | IndexSymbolKind::Enum
                | IndexSymbolKind::Interface
                | IndexSymbolKind::Trait
                | IndexSymbolKind::TypeAlias => {
                    return ChangeClassification::TypeDefinitionChange;
                },
                IndexSymbolKind::Function | IndexSymbolKind::Method => {
                    // If we can't determine more, assume implementation change
                    return ChangeClassification::ImplementationChange;
                },
                _ => {},
            }
        }

        // Default to implementation change (safest assumption for modified code)
        ChangeClassification::ImplementationChange
    }

    /// Get relevance score multiplier based on change classification
    pub(crate) fn classification_score_multiplier(
        &self,
        classification: ChangeClassification,
    ) -> f32 {
        match classification {
            ChangeClassification::Deletion => 1.5, // Highest priority - callers will break
            ChangeClassification::SignatureChange => 1.3, // High priority - callers may need updates
            ChangeClassification::TypeDefinitionChange => 1.2, // High priority - usages may break
            ChangeClassification::FileRename => 1.1,      // Medium-high - importers need updates
            ChangeClassification::ImportChange => 0.9,    // Medium - may affect resolution
            ChangeClassification::NewCode => 0.8,         // Normal priority
            ChangeClassification::ImplementationChange => 0.7, // Lower priority - internal change
            ChangeClassification::DocumentationOnly => 0.3, // Minimal impact
        }
    }

    /// Get caller count for a symbol (for importance weighting)
    fn get_caller_count(&self, symbol_id: u32) -> usize {
        self.graph.get_callers(symbol_id).len() + self.graph.get_referencers(symbol_id).len()
    }

    /// Expand context for a diff
    pub fn expand(
        &self,
        changes: &[DiffChange],
        depth: ContextDepth,
        token_budget: u32,
    ) -> ExpandedContext {
        let mut changed_symbols = Vec::new();
        let mut changed_files = Vec::new();
        let mut dependent_symbols = Vec::new();
        let mut dependent_files = Vec::new();
        let mut related_tests = Vec::new();
        let mut call_chains = Vec::new();

        let mut seen_files: HashSet<u32> = HashSet::new();
        let mut seen_symbols: HashSet<u32> = HashSet::new();
        let mut change_classifications: Vec<ChangeClassification> = Vec::new();
        let mut high_impact_symbols: HashSet<u32> = HashSet::new(); // Symbols needing extra caller expansion

        // Phase 1: Map changes to symbols with classification
        let mut path_overrides: std::collections::HashMap<u32, String> =
            std::collections::HashMap::new();

        for change in changes {
            let (file, output_path) = if let Some(file) = self.index.get_file(&change.file_path) {
                (file, change.file_path.clone())
            } else if let Some(old_path) = &change.old_path {
                if let Some(file) = self.index.get_file(old_path) {
                    path_overrides.insert(file.id.as_u32(), change.file_path.clone());
                    (file, change.file_path.clone())
                } else {
                    continue;
                }
            } else {
                continue;
            };

            if !seen_files.contains(&file.id.as_u32()) {
                seen_files.insert(file.id.as_u32());
            }

            // Find symbols containing changed lines
            for (start, end) in &change.line_ranges {
                for line in *start..=*end {
                    if let Some(symbol) = self.index.find_symbol_at_line(file.id, line) {
                        if !seen_symbols.contains(&symbol.id.as_u32()) {
                            seen_symbols.insert(symbol.id.as_u32());

                            // Classify this change for smart expansion
                            let classification = self.classify_change(change, Some(symbol));
                            change_classifications.push(classification);

                            // Calculate relevance score based on:
                            // 1. Base score of 1.0 (directly modified)
                            // 2. Classification multiplier
                            // 3. Caller count bonus (more callers = higher impact)
                            let caller_count = self.get_caller_count(symbol.id.as_u32());
                            let caller_bonus = (caller_count as f32 * 0.05).min(0.3); // Max 0.3 bonus
                            let base_score = 1.0 + caller_bonus;

                            // Mark high-impact symbols for extra expansion
                            if matches!(
                                classification,
                                ChangeClassification::SignatureChange
                                    | ChangeClassification::TypeDefinitionChange
                                    | ChangeClassification::Deletion
                            ) || caller_count > 5
                            {
                                high_impact_symbols.insert(symbol.id.as_u32());
                            }

                            let reason = match classification {
                                ChangeClassification::SignatureChange => {
                                    format!("signature changed ({} callers)", caller_count)
                                },
                                ChangeClassification::TypeDefinitionChange => {
                                    format!("type definition changed ({} usages)", caller_count)
                                },
                                ChangeClassification::Deletion => {
                                    format!("deleted ({} callers will break)", caller_count)
                                },
                                _ => "directly modified".to_owned(),
                            };

                            changed_symbols.push(self.to_context_symbol(
                                symbol,
                                file,
                                &reason,
                                base_score,
                                path_overrides.get(&file.id.as_u32()).map(String::as_str),
                            ));
                        }
                    }
                }
            }

            // Classify file-level change
            let file_classification = self.classify_change(change, None);
            let file_multiplier = self.classification_score_multiplier(file_classification);

            changed_files.push(ContextFile {
                id: file.id.as_u32(),
                path: output_path,
                language: file.language.name().to_owned(),
                relevance_reason: format!("{:?} ({:?})", change.change_type, file_classification),
                relevance_score: file_multiplier,
                tokens: file.tokens,
                relevant_sections: change.line_ranges.clone(),
                diff_content: change.diff_content.clone(),
                snippets: Vec::new(),
            });
        }

        // Determine overall change impact for expansion decisions
        let has_high_impact_change = change_classifications.iter().any(|c| {
            matches!(
                c,
                ChangeClassification::SignatureChange
                    | ChangeClassification::TypeDefinitionChange
                    | ChangeClassification::Deletion
            )
        });

        // Phase 2: Expand to dependents based on depth (with smart expansion)
        if depth >= ContextDepth::L2 {
            let l2_files = self.expand_l2(&seen_files);
            for file_id in &l2_files {
                if !seen_files.contains(file_id) {
                    if let Some(file) = self.index.get_file_by_id(*file_id) {
                        seen_files.insert(*file_id);
                        // Higher score for high-impact changes
                        let score = if has_high_impact_change { 0.9 } else { 0.8 };
                        let reason = if has_high_impact_change {
                            "imports changed file (breaking change detected)".to_owned()
                        } else {
                            "imports changed file".to_owned()
                        };
                        dependent_files.push(ContextFile {
                            id: file.id.as_u32(),
                            path: file.path.clone(),
                            language: file.language.name().to_owned(),
                            relevance_reason: reason,
                            relevance_score: score,
                            tokens: file.tokens,
                            relevant_sections: vec![],
                            diff_content: None,
                            snippets: Vec::new(),
                        });
                    }
                }
            }

            // Expand symbols - with extra expansion for high-impact symbols
            let l2_symbols = self.expand_symbol_refs(&seen_symbols);
            for symbol_id in &l2_symbols {
                if !seen_symbols.contains(symbol_id) {
                    if let Some(symbol) = self.index.get_symbol(*symbol_id) {
                        if let Some(file) = self.index.get_file_by_id(symbol.file_id.as_u32()) {
                            seen_symbols.insert(*symbol_id);
                            // Determine if this is a caller of a high-impact symbol
                            let is_caller_of_high_impact = high_impact_symbols
                                .iter()
                                .any(|&hi_sym| self.graph.get_callers(hi_sym).contains(symbol_id));
                            let (reason, score) = if is_caller_of_high_impact {
                                ("calls changed symbol (may break)", 0.85)
                            } else {
                                ("references changed symbol", 0.7)
                            };
                            dependent_symbols.push(self.to_context_symbol(
                                symbol,
                                file,
                                reason,
                                score,
                                path_overrides.get(&file.id.as_u32()).map(String::as_str),
                            ));
                        }
                    }
                }
            }

            // For high-impact changes, also include ALL callers (not just direct refs)
            if has_high_impact_change {
                for &hi_sym_id in &high_impact_symbols {
                    let all_callers = self.graph.get_callers(hi_sym_id);
                    for caller_id in all_callers {
                        if !seen_symbols.contains(&caller_id) {
                            if let Some(caller) = self.index.get_symbol(caller_id) {
                                if let Some(file) =
                                    self.index.get_file_by_id(caller.file_id.as_u32())
                                {
                                    seen_symbols.insert(caller_id);
                                    dependent_symbols.push(self.to_context_symbol(
                                        caller,
                                        file,
                                        "calls modified symbol (potential breakage)",
                                        0.9, // High priority
                                        path_overrides.get(&file.id.as_u32()).map(String::as_str),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        if depth >= ContextDepth::L3 {
            let l3_files = self.expand_l3(&seen_files);
            for file_id in &l3_files {
                if !seen_files.contains(file_id) {
                    if let Some(file) = self.index.get_file_by_id(*file_id) {
                        seen_files.insert(*file_id);
                        dependent_files.push(ContextFile {
                            id: file.id.as_u32(),
                            path: file.path.clone(),
                            language: file.language.name().to_owned(),
                            relevance_reason: "transitively depends on changed file".to_owned(),
                            relevance_score: 0.5,
                            tokens: file.tokens,
                            relevant_sections: vec![],
                            diff_content: None,
                            snippets: Vec::new(),
                        });
                    }
                }
            }
        }

        // Phase 3: Find related tests (via imports AND naming conventions)
        let mut seen_test_ids: HashSet<u32> = HashSet::new();

        // 3a: Find tests via import analysis
        for file in &self.index.files {
            if self.is_test_file(&file.path) {
                let imports = self.graph.get_imports(file.id.as_u32());
                for &imported in &imports {
                    if seen_files.contains(&imported) && !seen_test_ids.contains(&file.id.as_u32())
                    {
                        seen_test_ids.insert(file.id.as_u32());
                        related_tests.push(ContextFile {
                            id: file.id.as_u32(),
                            path: file.path.clone(),
                            language: file.language.name().to_owned(),
                            relevance_reason: "imports changed file".to_owned(),
                            relevance_score: 0.95,
                            tokens: file.tokens,
                            relevant_sections: vec![],
                            diff_content: None,
                            snippets: Vec::new(),
                        });
                        break;
                    }
                }
            }
        }

        // 3b: Find tests via naming conventions
        for cf in &changed_files {
            for test_id in self.find_tests_by_naming(&cf.path) {
                if !seen_test_ids.contains(&test_id) {
                    if let Some(file) = self.index.get_file_by_id(test_id) {
                        seen_test_ids.insert(test_id);
                        related_tests.push(ContextFile {
                            id: file.id.as_u32(),
                            path: file.path.clone(),
                            language: file.language.name().to_owned(),
                            relevance_reason: "test for changed file (naming convention)"
                                .to_owned(),
                            relevance_score: 0.85,
                            tokens: file.tokens,
                            relevant_sections: vec![],
                            diff_content: None,
                            snippets: Vec::new(),
                        });
                    }
                }
            }
        }

        // Phase 4: Build call chains for changed symbols
        for sym in &changed_symbols {
            let chains = self.build_call_chains(sym.id, 3);
            call_chains.extend(chains);
        }

        // Phase 5: Compute impact summary
        let impact_summary = self.compute_impact_summary(
            &changed_files,
            &dependent_files,
            &changed_symbols,
            &dependent_symbols,
            &related_tests,
        );

        // Phase 6: Select within token budget
        // Sort by relevance and truncate if needed
        dependent_files.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        dependent_symbols.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        related_tests.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to budget - apply to all file collections
        let mut running_tokens = changed_files.iter().map(|f| f.tokens).sum::<u32>();

        // Truncate dependent files first (lower priority than changed files)
        dependent_files.retain(|f| {
            if running_tokens + f.tokens <= token_budget {
                running_tokens += f.tokens;
                true
            } else {
                false
            }
        });

        // Truncate related tests (lower priority than dependent files)
        related_tests.retain(|f| {
            if running_tokens + f.tokens <= token_budget {
                running_tokens += f.tokens;
                true
            } else {
                false
            }
        });

        ExpandedContext {
            changed_symbols,
            changed_files,
            dependent_symbols,
            dependent_files,
            related_tests,
            call_chains,
            impact_summary,
            total_tokens: running_tokens,
        }
    }

    /// Expand to L2 (direct dependents)
    fn expand_l2(&self, file_ids: &HashSet<u32>) -> Vec<u32> {
        let mut result = Vec::new();
        for &file_id in file_ids {
            result.extend(self.graph.get_importers(file_id));
        }
        result
    }

    /// Expand to L3 (transitive dependents)
    fn expand_l3(&self, file_ids: &HashSet<u32>) -> Vec<u32> {
        let mut result = Vec::new();
        let mut visited: HashSet<u32> = file_ids.iter().copied().collect();
        let mut queue: VecDeque<u32> = VecDeque::new();

        for &file_id in file_ids {
            for importer in self.graph.get_importers(file_id) {
                if visited.insert(importer) {
                    result.push(importer);
                    queue.push_back(importer);
                }
            }
        }

        while let Some(current) = queue.pop_front() {
            for importer in self.graph.get_importers(current) {
                if visited.insert(importer) {
                    result.push(importer);
                    queue.push_back(importer);
                }
            }
        }

        result
    }

    /// Expand symbol references
    fn expand_symbol_refs(&self, symbol_ids: &HashSet<u32>) -> Vec<u32> {
        let mut result = Vec::new();
        for &symbol_id in symbol_ids {
            result.extend(self.graph.get_referencers(symbol_id));
            result.extend(self.graph.get_callers(symbol_id));
        }
        result
    }

    /// Check if a file is a test file
    pub(crate) fn is_test_file(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();
        path_lower.contains("test")
            || path_lower.contains("spec")
            || path_lower.contains("__tests__")
            || path_lower.ends_with("_test.rs")
            || path_lower.ends_with("_test.go")
            || path_lower.ends_with("_test.py")
            || path_lower.ends_with(".test.ts")
            || path_lower.ends_with(".test.js")
            || path_lower.ends_with(".spec.ts")
            || path_lower.ends_with(".spec.js")
    }

    /// Find tests related to a source file by naming convention.
    ///
    /// Looks for common test file naming patterns like:
    /// - `foo.rs` -> `foo_test.rs`, `test_foo.rs`, `tests/foo.rs`
    /// - `src/foo.py` -> `tests/test_foo.py`, `src/foo_test.py`
    fn find_tests_by_naming(&self, source_path: &str) -> Vec<u32> {
        let path_lower = source_path.to_lowercase();
        let base_name = std::path::Path::new(&path_lower)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        let mut test_ids = Vec::new();

        if base_name.is_empty() {
            return test_ids;
        }

        // Common test file patterns
        let test_patterns = [
            format!("{}_test.", base_name),
            format!("test_{}", base_name),
            format!("{}.test.", base_name),
            format!("{}.spec.", base_name),
            format!("test/{}", base_name),
            format!("tests/{}", base_name),
            format!("__tests__/{}", base_name),
        ];

        for file in &self.index.files {
            let file_lower = file.path.to_lowercase();
            if self.is_test_file(&file.path) {
                for pattern in &test_patterns {
                    if file_lower.contains(pattern) {
                        test_ids.push(file.id.as_u32());
                        break;
                    }
                }
            }
        }

        test_ids
    }

    /// Convert symbol to context symbol
    fn to_context_symbol(
        &self,
        symbol: &IndexSymbol,
        file: &FileEntry,
        reason: &str,
        score: f32,
        path_override: Option<&str>,
    ) -> ContextSymbol {
        ContextSymbol {
            id: symbol.id.as_u32(),
            name: symbol.name.clone(),
            kind: symbol.kind.name().to_owned(),
            file_path: path_override.unwrap_or(&file.path).to_owned(),
            start_line: symbol.span.start_line,
            end_line: symbol.span.end_line,
            signature: symbol.signature.clone(),
            relevance_reason: reason.to_owned(),
            relevance_score: score,
        }
    }

    /// Build call chains for a symbol
    fn build_call_chains(&self, symbol_id: u32, max_depth: usize) -> Vec<CallChain> {
        let mut chains = Vec::new();

        // Build upstream chain (callers)
        let mut upstream = Vec::new();
        self.collect_callers(symbol_id, &mut upstream, max_depth, &mut HashSet::new());
        if !upstream.is_empty() {
            upstream.reverse();
            if let Some(sym) = self.index.get_symbol(symbol_id) {
                upstream.push(sym.name.clone());
            }
            chains.push(CallChain {
                symbols: upstream.clone(),
                files: self.get_files_for_symbols(&upstream),
            });
        }

        // Build downstream chain (callees)
        let mut downstream = Vec::new();
        if let Some(sym) = self.index.get_symbol(symbol_id) {
            downstream.push(sym.name.clone());
        }
        self.collect_callees(symbol_id, &mut downstream, max_depth, &mut HashSet::new());
        if downstream.len() > 1 {
            chains.push(CallChain {
                symbols: downstream.clone(),
                files: self.get_files_for_symbols(&downstream),
            });
        }

        chains
    }

    fn collect_callers(
        &self,
        symbol_id: u32,
        chain: &mut Vec<String>,
        depth: usize,
        visited: &mut HashSet<u32>,
    ) {
        if depth == 0 || visited.contains(&symbol_id) {
            return;
        }
        visited.insert(symbol_id);

        let callers = self.graph.get_callers(symbol_id);
        if let Some(&caller_id) = callers.first() {
            if let Some(sym) = self.index.get_symbol(caller_id) {
                chain.push(sym.name.clone());
                self.collect_callers(caller_id, chain, depth - 1, visited);
            }
        }
    }

    fn collect_callees(
        &self,
        symbol_id: u32,
        chain: &mut Vec<String>,
        depth: usize,
        visited: &mut HashSet<u32>,
    ) {
        if depth == 0 || visited.contains(&symbol_id) {
            return;
        }
        visited.insert(symbol_id);

        let callees = self.graph.get_callees(symbol_id);
        if let Some(&callee_id) = callees.first() {
            if let Some(sym) = self.index.get_symbol(callee_id) {
                chain.push(sym.name.clone());
                self.collect_callees(callee_id, chain, depth - 1, visited);
            }
        }
    }

    fn get_files_for_symbols(&self, symbol_names: &[String]) -> Vec<String> {
        let mut files = Vec::new();
        let mut seen = HashSet::new();
        for name in symbol_names {
            for sym in self.index.find_symbols(name) {
                if let Some(file) = self.index.get_file_by_id(sym.file_id.as_u32()) {
                    if seen.insert(file.id) {
                        files.push(file.path.clone());
                    }
                }
            }
        }
        files
    }

    /// Compute impact summary
    fn compute_impact_summary(
        &self,
        changed_files: &[ContextFile],
        dependent_files: &[ContextFile],
        changed_symbols: &[ContextSymbol],
        dependent_symbols: &[ContextSymbol],
        related_tests: &[ContextFile],
    ) -> ImpactSummary {
        let direct_files = changed_files.len();
        let transitive_files = dependent_files.len();
        let affected_symbols = changed_symbols.len() + dependent_symbols.len();
        let affected_tests = related_tests.len();

        // Determine impact level
        let level = if transitive_files > 20 || affected_symbols > 50 {
            ImpactLevel::Critical
        } else if transitive_files > 10 || affected_symbols > 20 {
            ImpactLevel::High
        } else if transitive_files > 3 || affected_symbols > 5 {
            ImpactLevel::Medium
        } else {
            ImpactLevel::Low
        };

        // Detect potential breaking changes
        // Only flag public/exported functions and methods - private internals aren't API
        // Note: We can't determine if signature actually changed (no old version), so flag as "potentially"
        let breaking_changes = changed_symbols
            .iter()
            .filter(|s| s.kind == "function" || s.kind == "method")
            .filter(|s| s.signature.is_some())
            // Only flag symbols that start with "pub" in their signature (public API)
            .filter(|s| {
                s.signature
                    .as_ref()
                    .is_some_and(|sig| sig.starts_with("pub ") || sig.starts_with("export "))
            })
            .map(|s| format!("{} public API signature may have changed", s.name))
            .collect();

        let description = format!(
            "Changed {} files affecting {} dependent files and {} symbols. {} tests may need updating.",
            direct_files, transitive_files, affected_symbols, affected_tests
        );

        ImpactSummary {
            level,
            direct_files,
            transitive_files,
            affected_symbols,
            affected_tests,
            breaking_changes,
            description,
        }
    }
}
