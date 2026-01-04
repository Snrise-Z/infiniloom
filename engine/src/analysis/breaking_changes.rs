//! Breaking change detection between two versions of code
//!
//! Compares symbols between git refs to detect API breaking changes
//! such as removed symbols, signature changes, visibility changes, etc.

use crate::analysis::types::{
    BreakingChange, BreakingChangeReport, BreakingChangeSummary, BreakingChangeType, ChangeSeverity,
};
use crate::types::{Symbol, SymbolKind, Visibility};
use std::collections::HashMap;

/// Detects breaking changes between two versions
pub struct BreakingChangeDetector {
    /// Symbols from old version (key: qualified name or name)
    old_symbols: HashMap<String, SymbolSnapshot>,
    /// Symbols from new version
    new_symbols: HashMap<String, SymbolSnapshot>,
    /// Old git ref
    old_ref: String,
    /// New git ref
    new_ref: String,
}

/// Snapshot of a symbol for comparison
#[derive(Debug, Clone)]
struct SymbolSnapshot {
    name: String,
    qualified_name: String,
    kind: SymbolKind,
    signature: Option<String>,
    visibility: Visibility,
    file_path: String,
    line: u32,
    extends: Option<String>,
    implements: Vec<String>,
    is_async: bool,
    parameter_count: usize,
    parameters: Vec<String>,
    return_type: Option<String>,
    generic_count: usize,
}

impl BreakingChangeDetector {
    /// Create a new detector
    pub fn new(old_ref: impl Into<String>, new_ref: impl Into<String>) -> Self {
        Self {
            old_symbols: HashMap::new(),
            new_symbols: HashMap::new(),
            old_ref: old_ref.into(),
            new_ref: new_ref.into(),
        }
    }

    /// Add symbols from old version
    pub fn add_old_symbols(&mut self, file_path: &str, symbols: &[Symbol]) {
        for symbol in symbols {
            let snapshot = self.symbol_to_snapshot(symbol, file_path);
            self.old_symbols
                .insert(snapshot.qualified_name.clone(), snapshot);
        }
    }

    /// Add symbols from new version
    pub fn add_new_symbols(&mut self, file_path: &str, symbols: &[Symbol]) {
        for symbol in symbols {
            let snapshot = self.symbol_to_snapshot(symbol, file_path);
            self.new_symbols
                .insert(snapshot.qualified_name.clone(), snapshot);
        }
    }

    /// Convert Symbol to SymbolSnapshot
    fn symbol_to_snapshot(&self, symbol: &Symbol, file_path: &str) -> SymbolSnapshot {
        let qualified_name = if let Some(ref parent) = symbol.parent {
            format!("{}::{}", parent, symbol.name)
        } else {
            symbol.name.clone()
        };

        // Parse signature to extract parameters, return type, etc.
        let (parameters, return_type, is_async, generic_count) =
            self.parse_signature(&symbol.signature);

        SymbolSnapshot {
            name: symbol.name.clone(),
            qualified_name,
            kind: symbol.kind,
            signature: symbol.signature.clone(),
            visibility: symbol.visibility,
            file_path: file_path.to_owned(),
            line: symbol.start_line,
            extends: symbol.extends.clone(),
            implements: symbol.implements.clone(),
            is_async,
            parameter_count: parameters.len(),
            parameters,
            return_type,
            generic_count,
        }
    }

    /// Parse signature to extract components
    fn parse_signature(
        &self,
        signature: &Option<String>,
    ) -> (Vec<String>, Option<String>, bool, usize) {
        let mut parameters = Vec::new();
        let mut return_type = None;
        let mut is_async = false;
        let mut generic_count = 0;

        if let Some(sig) = signature {
            // Check for async
            is_async = sig.contains("async ");

            // Count generics (simplified: count < and >)
            generic_count = sig.matches('<').count();

            // Try to extract parameters (between ( and ))
            if let Some(start) = sig.find('(') {
                if let Some(end) = sig.rfind(')') {
                    let params_str = &sig[start + 1..end];
                    if !params_str.trim().is_empty() {
                        // Split by comma, but respect nested brackets
                        parameters = self.split_parameters(params_str);
                    }
                }
            }

            // Try to extract return type (after -> or :)
            if let Some(arrow_pos) = sig.find("->") {
                return_type = Some(sig[arrow_pos + 2..].trim().to_owned());
            } else if let Some(colon_pos) = sig.rfind(':') {
                // Check if this colon is for return type (not in parameter type)
                let after = &sig[colon_pos + 1..];
                if !after.contains(',') && !after.contains('(') {
                    return_type = Some(after.trim().to_owned());
                }
            }
        }

        (parameters, return_type, is_async, generic_count)
    }

    /// Split parameters respecting nested brackets
    fn split_parameters(&self, params_str: &str) -> Vec<String> {
        let mut params = Vec::new();
        let mut current = String::new();
        let mut depth = 0;

        for c in params_str.chars() {
            match c {
                '<' | '(' | '[' | '{' => {
                    depth += 1;
                    current.push(c);
                },
                '>' | ')' | ']' | '}' => {
                    depth -= 1;
                    current.push(c);
                },
                ',' if depth == 0 => {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        params.push(trimmed.to_owned());
                    }
                    current.clear();
                },
                _ => current.push(c),
            }
        }

        let trimmed = current.trim();
        if !trimmed.is_empty() {
            params.push(trimmed.to_owned());
        }

        params
    }

    /// Detect all breaking changes
    pub fn detect(&self) -> BreakingChangeReport {
        let mut changes = Vec::new();

        // Check for removed symbols
        for (name, old) in &self.old_symbols {
            // Only check public API
            if !matches!(old.visibility, Visibility::Public) {
                continue;
            }

            if let Some(new) = self.new_symbols.get(name) {
                // Symbol exists in both - check for changes
                changes.extend(self.compare_symbols(old, new));
            } else {
                // Symbol was removed
                changes.push(BreakingChange {
                    change_type: BreakingChangeType::Removed,
                    symbol_name: old.name.clone(),
                    symbol_kind: format!("{:?}", old.kind),
                    file_path: old.file_path.clone(),
                    line: None,
                    old_signature: old.signature.clone(),
                    new_signature: None,
                    description: format!(
                        "Public {} '{}' was removed",
                        format!("{:?}", old.kind).to_lowercase(),
                        old.name
                    ),
                    severity: ChangeSeverity::Critical,
                    migration_hint: Some(format!(
                        "Remove usage of '{}' or find an alternative",
                        old.name
                    )),
                });
            }
        }

        // Check for moved symbols (new location)
        for (name, new) in &self.new_symbols {
            if let Some(old) = self.old_symbols.get(name) {
                if old.file_path != new.file_path
                    && matches!(old.visibility, Visibility::Public)
                    && matches!(new.visibility, Visibility::Public)
                {
                    changes.push(BreakingChange {
                        change_type: BreakingChangeType::Moved,
                        symbol_name: old.name.clone(),
                        symbol_kind: format!("{:?}", old.kind),
                        file_path: new.file_path.clone(),
                        line: Some(new.line),
                        old_signature: Some(old.file_path.clone()),
                        new_signature: Some(new.file_path.clone()),
                        description: format!(
                            "'{}' moved from '{}' to '{}'",
                            old.name, old.file_path, new.file_path
                        ),
                        severity: ChangeSeverity::Medium,
                        migration_hint: Some(format!(
                            "Update import path from '{}' to '{}'",
                            old.file_path, new.file_path
                        )),
                    });
                }
            }
        }

        // Build summary
        let summary = self.build_summary(&changes);

        BreakingChangeReport {
            old_ref: self.old_ref.clone(),
            new_ref: self.new_ref.clone(),
            changes,
            summary,
        }
    }

    /// Compare two versions of the same symbol
    fn compare_symbols(&self, old: &SymbolSnapshot, new: &SymbolSnapshot) -> Vec<BreakingChange> {
        let mut changes = Vec::new();

        // Check visibility reduction
        if self.is_visibility_reduced(&old.visibility, &new.visibility) {
            changes.push(BreakingChange {
                change_type: BreakingChangeType::VisibilityReduced,
                symbol_name: old.name.clone(),
                symbol_kind: format!("{:?}", old.kind),
                file_path: new.file_path.clone(),
                line: Some(new.line),
                old_signature: Some(format!("{:?}", old.visibility)),
                new_signature: Some(format!("{:?}", new.visibility)),
                description: format!(
                    "Visibility of '{}' reduced from {:?} to {:?}",
                    old.name, old.visibility, new.visibility
                ),
                severity: ChangeSeverity::Critical,
                migration_hint: Some(
                    "This symbol may no longer be accessible from your code".to_owned(),
                ),
            });
        }

        // Check return type change
        if old.return_type != new.return_type {
            if let (Some(old_ret), Some(new_ret)) = (&old.return_type, &new.return_type) {
                changes.push(BreakingChange {
                    change_type: BreakingChangeType::ReturnTypeChanged,
                    symbol_name: old.name.clone(),
                    symbol_kind: format!("{:?}", old.kind),
                    file_path: new.file_path.clone(),
                    line: Some(new.line),
                    old_signature: Some(old_ret.clone()),
                    new_signature: Some(new_ret.clone()),
                    description: format!(
                        "Return type of '{}' changed from '{}' to '{}'",
                        old.name, old_ret, new_ret
                    ),
                    severity: ChangeSeverity::High,
                    migration_hint: Some(format!(
                        "Update code that uses return value of '{}' to handle new type '{}'",
                        old.name, new_ret
                    )),
                });
            }
        }

        // Check parameter changes
        let param_changes = self.compare_parameters(old, new);
        changes.extend(param_changes);

        // Check async/sync change
        if old.is_async != new.is_async {
            changes.push(BreakingChange {
                change_type: BreakingChangeType::AsyncChanged,
                symbol_name: old.name.clone(),
                symbol_kind: format!("{:?}", old.kind),
                file_path: new.file_path.clone(),
                line: Some(new.line),
                old_signature: Some(if old.is_async { "async" } else { "sync" }.to_owned()),
                new_signature: Some(if new.is_async { "async" } else { "sync" }.to_owned()),
                description: format!(
                    "'{}' changed from {} to {}",
                    old.name,
                    if old.is_async { "async" } else { "sync" },
                    if new.is_async { "async" } else { "sync" }
                ),
                severity: ChangeSeverity::High,
                migration_hint: Some(format!(
                    "Update call sites of '{}' to {} the result",
                    old.name,
                    if new.is_async { "await" } else { "not await" }
                )),
            });
        }

        // Check generic parameter changes
        if old.generic_count != new.generic_count {
            changes.push(BreakingChange {
                change_type: BreakingChangeType::GenericChanged,
                symbol_name: old.name.clone(),
                symbol_kind: format!("{:?}", old.kind),
                file_path: new.file_path.clone(),
                line: Some(new.line),
                old_signature: Some(format!("{} type parameters", old.generic_count)),
                new_signature: Some(format!("{} type parameters", new.generic_count)),
                description: format!(
                    "Generic type parameters of '{}' changed from {} to {}",
                    old.name, old.generic_count, new.generic_count
                ),
                severity: ChangeSeverity::High,
                migration_hint: Some("Update type arguments at call sites".to_owned()),
            });
        }

        // Check extends/implements changes
        if old.extends != new.extends {
            changes.push(BreakingChange {
                change_type: BreakingChangeType::TypeConstraintChanged,
                symbol_name: old.name.clone(),
                symbol_kind: format!("{:?}", old.kind),
                file_path: new.file_path.clone(),
                line: Some(new.line),
                old_signature: old.extends.clone(),
                new_signature: new.extends.clone(),
                description: format!(
                    "Base class of '{}' changed from {:?} to {:?}",
                    old.name, old.extends, new.extends
                ),
                severity: ChangeSeverity::Medium,
                migration_hint: None,
            });
        }

        changes
    }

    /// Compare parameters between old and new versions
    fn compare_parameters(
        &self,
        old: &SymbolSnapshot,
        new: &SymbolSnapshot,
    ) -> Vec<BreakingChange> {
        let mut changes = Vec::new();

        // Check if required parameters were added
        if new.parameter_count > old.parameter_count {
            // New parameters added - check if they might be required
            let added_count = new.parameter_count - old.parameter_count;
            changes.push(BreakingChange {
                change_type: BreakingChangeType::ParameterAdded,
                symbol_name: old.name.clone(),
                symbol_kind: format!("{:?}", old.kind),
                file_path: new.file_path.clone(),
                line: Some(new.line),
                old_signature: old.signature.clone(),
                new_signature: new.signature.clone(),
                description: format!("'{}' has {} new parameter(s)", old.name, added_count),
                severity: ChangeSeverity::High,
                migration_hint: Some(format!(
                    "Add {} new argument(s) to calls to '{}'",
                    added_count, old.name
                )),
            });
        }

        // Check if parameters were removed
        if new.parameter_count < old.parameter_count {
            let removed_count = old.parameter_count - new.parameter_count;
            changes.push(BreakingChange {
                change_type: BreakingChangeType::ParameterRemoved,
                symbol_name: old.name.clone(),
                symbol_kind: format!("{:?}", old.kind),
                file_path: new.file_path.clone(),
                line: Some(new.line),
                old_signature: old.signature.clone(),
                new_signature: new.signature.clone(),
                description: format!("'{}' has {} fewer parameter(s)", old.name, removed_count),
                severity: ChangeSeverity::High,
                migration_hint: Some(format!(
                    "Remove {} argument(s) from calls to '{}'",
                    removed_count, old.name
                )),
            });
        }

        // Check for parameter type changes (simplified: compare parameter strings)
        let min_len = old.parameters.len().min(new.parameters.len());
        for i in 0..min_len {
            if old.parameters[i] != new.parameters[i] {
                changes.push(BreakingChange {
                    change_type: BreakingChangeType::ParameterTypeChanged,
                    symbol_name: old.name.clone(),
                    symbol_kind: format!("{:?}", old.kind),
                    file_path: new.file_path.clone(),
                    line: Some(new.line),
                    old_signature: Some(old.parameters[i].clone()),
                    new_signature: Some(new.parameters[i].clone()),
                    description: format!(
                        "Parameter {} of '{}' changed from '{}' to '{}'",
                        i + 1,
                        old.name,
                        old.parameters[i],
                        new.parameters[i]
                    ),
                    severity: ChangeSeverity::High,
                    migration_hint: Some(format!(
                        "Update argument {} in calls to '{}'",
                        i + 1,
                        old.name
                    )),
                });
            }
        }

        changes
    }

    /// Check if visibility was reduced (breaking)
    fn is_visibility_reduced(&self, old: &Visibility, new: &Visibility) -> bool {
        let visibility_level = |v: &Visibility| match v {
            Visibility::Public => 3,
            Visibility::Protected => 2,
            Visibility::Internal => 1,
            Visibility::Private => 0,
        };

        visibility_level(new) < visibility_level(old)
    }

    /// Build summary statistics
    fn build_summary(&self, changes: &[BreakingChange]) -> BreakingChangeSummary {
        let mut summary =
            BreakingChangeSummary { total: changes.len() as u32, ..Default::default() };

        let mut affected_files = std::collections::HashSet::new();
        let mut affected_symbols = std::collections::HashSet::new();

        for change in changes {
            match change.severity {
                ChangeSeverity::Critical => summary.critical += 1,
                ChangeSeverity::High => summary.high += 1,
                ChangeSeverity::Medium => summary.medium += 1,
                ChangeSeverity::Low => summary.low += 1,
            }

            affected_files.insert(&change.file_path);
            affected_symbols.insert(&change.symbol_name);
        }

        summary.files_affected = affected_files.len() as u32;
        summary.symbols_affected = affected_symbols.len() as u32;

        summary
    }
}

/// Convenience function to detect breaking changes between file sets
pub fn detect_breaking_changes(
    old_ref: &str,
    old_files: &[(String, Vec<Symbol>)],
    new_ref: &str,
    new_files: &[(String, Vec<Symbol>)],
) -> BreakingChangeReport {
    let mut detector = BreakingChangeDetector::new(old_ref, new_ref);

    for (path, symbols) in old_files {
        detector.add_old_symbols(path, symbols);
    }

    for (path, symbols) in new_files {
        detector.add_new_symbols(path, symbols);
    }

    detector.detect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_symbol(
        name: &str,
        kind: SymbolKind,
        visibility: Visibility,
        signature: Option<&str>,
    ) -> Symbol {
        Symbol {
            name: name.to_owned(),
            kind,
            visibility,
            signature: signature.map(String::from),
            start_line: 1,
            end_line: 10,
            ..Default::default()
        }
    }

    #[test]
    fn test_removed_symbol() {
        let mut detector = BreakingChangeDetector::new("v1.0", "v2.0");

        let old_symbols = vec![
            make_symbol(
                "removed_func",
                SymbolKind::Function,
                Visibility::Public,
                Some("fn removed_func()"),
            ),
            make_symbol(
                "kept_func",
                SymbolKind::Function,
                Visibility::Public,
                Some("fn kept_func()"),
            ),
        ];

        let new_symbols = vec![make_symbol(
            "kept_func",
            SymbolKind::Function,
            Visibility::Public,
            Some("fn kept_func()"),
        )];

        detector.add_old_symbols("test.rs", &old_symbols);
        detector.add_new_symbols("test.rs", &new_symbols);

        let report = detector.detect();

        assert!(report.changes.iter().any(|c| {
            c.symbol_name == "removed_func" && c.change_type == BreakingChangeType::Removed
        }));
    }

    #[test]
    fn test_visibility_reduction() {
        let mut detector = BreakingChangeDetector::new("v1.0", "v2.0");

        let old_symbols = vec![make_symbol(
            "my_func",
            SymbolKind::Function,
            Visibility::Public,
            Some("fn my_func()"),
        )];

        let new_symbols = vec![make_symbol(
            "my_func",
            SymbolKind::Function,
            Visibility::Private,
            Some("fn my_func()"),
        )];

        detector.add_old_symbols("test.rs", &old_symbols);
        detector.add_new_symbols("test.rs", &new_symbols);

        let report = detector.detect();

        assert!(report.changes.iter().any(|c| {
            c.symbol_name == "my_func" && c.change_type == BreakingChangeType::VisibilityReduced
        }));
    }

    #[test]
    fn test_parameter_added() {
        let mut detector = BreakingChangeDetector::new("v1.0", "v2.0");

        let old_symbols = vec![make_symbol(
            "my_func",
            SymbolKind::Function,
            Visibility::Public,
            Some("fn my_func(a: i32)"),
        )];

        let new_symbols = vec![make_symbol(
            "my_func",
            SymbolKind::Function,
            Visibility::Public,
            Some("fn my_func(a: i32, b: i32)"),
        )];

        detector.add_old_symbols("test.rs", &old_symbols);
        detector.add_new_symbols("test.rs", &new_symbols);

        let report = detector.detect();

        assert!(report.changes.iter().any(|c| {
            c.symbol_name == "my_func" && c.change_type == BreakingChangeType::ParameterAdded
        }));
    }

    #[test]
    fn test_async_change() {
        let mut detector = BreakingChangeDetector::new("v1.0", "v2.0");

        let old_symbols = vec![make_symbol(
            "fetch",
            SymbolKind::Function,
            Visibility::Public,
            Some("fn fetch()"),
        )];

        let new_symbols = vec![make_symbol(
            "fetch",
            SymbolKind::Function,
            Visibility::Public,
            Some("async fn fetch()"),
        )];

        detector.add_old_symbols("test.rs", &old_symbols);
        detector.add_new_symbols("test.rs", &new_symbols);

        let report = detector.detect();

        assert!(report.changes.iter().any(|c| {
            c.symbol_name == "fetch" && c.change_type == BreakingChangeType::AsyncChanged
        }));
    }

    #[test]
    fn test_private_symbols_ignored() {
        let mut detector = BreakingChangeDetector::new("v1.0", "v2.0");

        let old_symbols = vec![make_symbol(
            "private_func",
            SymbolKind::Function,
            Visibility::Private,
            Some("fn private_func()"),
        )];

        let new_symbols: Vec<Symbol> = vec![];

        detector.add_old_symbols("test.rs", &old_symbols);
        detector.add_new_symbols("test.rs", &new_symbols);

        let report = detector.detect();

        // Private symbol removal should not be flagged as breaking
        assert!(report.changes.is_empty());
    }

    #[test]
    fn test_summary() {
        let mut detector = BreakingChangeDetector::new("v1.0", "v2.0");

        let old_symbols = vec![
            make_symbol("func1", SymbolKind::Function, Visibility::Public, Some("fn func1()")),
            make_symbol(
                "func2",
                SymbolKind::Function,
                Visibility::Public,
                Some("fn func2(a: i32)"),
            ),
        ];

        let new_symbols = vec![
            // func1 removed
            make_symbol(
                "func2",
                SymbolKind::Function,
                Visibility::Public,
                Some("fn func2(a: i32, b: i32)"),
            ),
        ];

        detector.add_old_symbols("test.rs", &old_symbols);
        detector.add_new_symbols("test.rs", &new_symbols);

        let report = detector.detect();

        assert!(report.summary.total >= 2);
        assert!(report.summary.files_affected >= 1);
        assert!(report.summary.symbols_affected >= 2);
    }
}
