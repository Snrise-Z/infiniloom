//! Type hierarchy navigation for all supported languages
//!
//! Navigates extends/implements chains to find ancestors and descendants.
//! Supports all 21 languages with their respective inheritance models.

use crate::analysis::types::{AncestorInfo, AncestorKind, TypeHierarchy};
use crate::parser::Language;
use crate::types::Symbol;
use std::collections::{HashMap, HashSet, VecDeque};

/// Builds and navigates type hierarchies from symbol data
pub struct TypeHierarchyBuilder {
    /// Map of symbol name to its inheritance info
    symbols: HashMap<String, SymbolInheritance>,
    /// Reverse map: parent -> children
    children_map: HashMap<String, Vec<String>>,
}

/// Inheritance information for a single symbol
#[derive(Debug, Clone, Default)]
struct SymbolInheritance {
    /// Symbol name
    name: String,
    /// Parent class/struct
    extends: Option<String>,
    /// Interfaces/traits implemented
    implements: Vec<String>,
    /// Mixins included
    mixins: Vec<String>,
    /// Kind of this symbol
    kind: AncestorKind,
    /// File path
    file_path: Option<String>,
}

impl TypeHierarchyBuilder {
    /// Create a new empty builder
    pub fn new() -> Self {
        Self { symbols: HashMap::new(), children_map: HashMap::new() }
    }

    /// Add symbols from parsed files
    pub fn add_symbols(&mut self, symbols: &[Symbol], file_path: &str, language: Language) {
        for symbol in symbols {
            if !self.is_type_symbol(symbol) {
                continue;
            }

            let inheritance = SymbolInheritance {
                name: symbol.name.clone(),
                extends: symbol.extends.clone(),
                implements: symbol.implements.clone(),
                mixins: self.extract_mixins(symbol, language),
                kind: self.symbol_to_ancestor_kind(symbol, language),
                file_path: Some(file_path.to_owned()),
            };

            // Update children map
            if let Some(ref parent) = inheritance.extends {
                self.children_map
                    .entry(parent.clone())
                    .or_default()
                    .push(symbol.name.clone());
            }
            for iface in &inheritance.implements {
                self.children_map
                    .entry(iface.clone())
                    .or_default()
                    .push(symbol.name.clone());
            }

            self.symbols.insert(symbol.name.clone(), inheritance);
        }
    }

    /// Check if symbol is a type (class, struct, interface, trait, etc.)
    fn is_type_symbol(&self, symbol: &Symbol) -> bool {
        use crate::types::SymbolKind;
        matches!(
            symbol.kind,
            SymbolKind::Class
                | SymbolKind::Struct
                | SymbolKind::Interface
                | SymbolKind::Trait
                | SymbolKind::Enum
                | SymbolKind::TypeAlias
        )
    }

    /// Extract mixins from symbol based on language
    fn extract_mixins(&self, symbol: &Symbol, language: Language) -> Vec<String> {
        match language {
            // Ruby uses modules as mixins via include/extend
            Language::Ruby => {
                // Would need to analyze include/extend statements
                Vec::new()
            },
            // Scala has traits that can be mixed in
            Language::Scala => {
                // "with Trait1 with Trait2" would be in implements
                Vec::new()
            },
            _ => Vec::new(),
        }
    }

    /// Convert symbol kind to ancestor kind
    fn symbol_to_ancestor_kind(&self, symbol: &Symbol, language: Language) -> AncestorKind {
        use crate::types::SymbolKind;

        match symbol.kind {
            SymbolKind::Class => {
                // Check if abstract based on signature or name conventions
                if symbol
                    .signature
                    .as_ref()
                    .is_some_and(|s| s.contains("abstract"))
                {
                    AncestorKind::AbstractClass
                } else {
                    AncestorKind::Class
                }
            },
            SymbolKind::Interface => match language {
                Language::Swift => AncestorKind::Protocol,
                Language::Rust => AncestorKind::Trait,
                _ => AncestorKind::Interface,
            },
            SymbolKind::Trait => AncestorKind::Trait,
            _ => AncestorKind::Class,
        }
    }

    /// Get the full type hierarchy for a symbol
    pub fn get_hierarchy(&self, symbol_name: &str) -> TypeHierarchy {
        let mut hierarchy =
            TypeHierarchy { symbol_name: symbol_name.to_owned(), ..Default::default() };

        if let Some(info) = self.symbols.get(symbol_name) {
            hierarchy.extends = info.extends.clone();
            hierarchy.implements = info.implements.clone();
            hierarchy.mixins = info.mixins.clone();
        }

        // Build ancestor chain
        hierarchy.ancestors = self.get_ancestors(symbol_name);

        // Build descendant list
        hierarchy.descendants = self.get_descendants(symbol_name);

        hierarchy
    }

    /// Get all ancestors (parents, grandparents, etc.)
    fn get_ancestors(&self, symbol_name: &str) -> Vec<AncestorInfo> {
        let mut ancestors = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start with direct parent
        if let Some(info) = self.symbols.get(symbol_name) {
            if let Some(ref parent) = info.extends {
                queue.push_back((parent.clone(), 1u32));
            }
            for iface in &info.implements {
                queue.push_back((iface.clone(), 1));
            }
        }

        // BFS to find all ancestors
        while let Some((name, depth)) = queue.pop_front() {
            if visited.contains(&name) {
                continue;
            }
            visited.insert(name.clone());

            let kind = self
                .symbols
                .get(&name)
                .map(|i| i.kind.clone())
                .unwrap_or_default();

            let file_path = self.symbols.get(&name).and_then(|i| i.file_path.clone());

            ancestors.push(AncestorInfo { name: name.clone(), kind, depth, file_path });

            // Add this ancestor's parents
            if let Some(info) = self.symbols.get(&name) {
                if let Some(ref parent) = info.extends {
                    queue.push_back((parent.clone(), depth + 1));
                }
                for iface in &info.implements {
                    queue.push_back((iface.clone(), depth + 1));
                }
            }
        }

        // Sort by depth
        ancestors.sort_by_key(|a| a.depth);
        ancestors
    }

    /// Get all descendants (children, grandchildren, etc.)
    fn get_descendants(&self, symbol_name: &str) -> Vec<String> {
        let mut descendants = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start with direct children
        if let Some(children) = self.children_map.get(symbol_name) {
            for child in children {
                queue.push_back(child.clone());
            }
        }

        // BFS to find all descendants
        while let Some(name) = queue.pop_front() {
            if visited.contains(&name) {
                continue;
            }
            visited.insert(name.clone());
            descendants.push(name.clone());

            // Add this descendant's children
            if let Some(children) = self.children_map.get(&name) {
                for child in children {
                    queue.push_back(child.clone());
                }
            }
        }

        descendants
    }

    /// Get all root types (types with no parent)
    pub fn get_roots(&self) -> Vec<String> {
        self.symbols
            .values()
            .filter(|info| info.extends.is_none() && info.implements.is_empty())
            .map(|info| info.name.clone())
            .collect()
    }

    /// Get all leaf types (types with no children)
    pub fn get_leaves(&self) -> Vec<String> {
        let parents: HashSet<_> = self.children_map.keys().collect();

        self.symbols
            .keys()
            .filter(|name| !parents.contains(name))
            .cloned()
            .collect()
    }

    /// Check if one type is an ancestor of another
    pub fn is_ancestor_of(&self, potential_ancestor: &str, symbol_name: &str) -> bool {
        let ancestors = self.get_ancestors(symbol_name);
        ancestors.iter().any(|a| a.name == potential_ancestor)
    }

    /// Check if one type is a descendant of another
    pub fn is_descendant_of(&self, potential_descendant: &str, symbol_name: &str) -> bool {
        let descendants = self.get_descendants(symbol_name);
        descendants.contains(&potential_descendant.to_owned())
    }

    /// Get the depth of a type in the hierarchy
    pub fn get_depth(&self, symbol_name: &str) -> u32 {
        let ancestors = self.get_ancestors(symbol_name);
        ancestors.iter().map(|a| a.depth).max().unwrap_or(0)
    }

    /// Get all types that implement a specific interface/trait
    pub fn get_implementors(&self, interface_name: &str) -> Vec<String> {
        self.symbols
            .values()
            .filter(|info| info.implements.contains(&interface_name.to_owned()))
            .map(|info| info.name.clone())
            .collect()
    }

    /// Get the common ancestor of two types (if any)
    pub fn get_common_ancestor(&self, type1: &str, type2: &str) -> Option<String> {
        let ancestors1: HashSet<_> = self
            .get_ancestors(type1)
            .into_iter()
            .map(|a| a.name)
            .collect();

        let ancestors2 = self.get_ancestors(type2);

        // Find the first ancestor of type2 that is also an ancestor of type1
        for ancestor in ancestors2 {
            if ancestors1.contains(&ancestor.name) {
                return Some(ancestor.name);
            }
        }

        None
    }
}

impl Default for TypeHierarchyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract type hierarchy from a collection of files
pub fn build_type_hierarchy(files: &[(String, Vec<Symbol>, Language)]) -> TypeHierarchyBuilder {
    let mut builder = TypeHierarchyBuilder::new();

    for (file_path, symbols, language) in files {
        builder.add_symbols(symbols, file_path, *language);
    }

    builder
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SymbolKind;

    fn make_symbol(name: &str, extends: Option<&str>, implements: Vec<&str>) -> Symbol {
        Symbol {
            name: name.to_owned(),
            kind: SymbolKind::Class,
            extends: extends.map(String::from),
            implements: implements.into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn test_simple_hierarchy() {
        let mut builder = TypeHierarchyBuilder::new();

        let symbols = vec![
            make_symbol("Animal", None, vec![]),
            make_symbol("Dog", Some("Animal"), vec![]),
            make_symbol("Cat", Some("Animal"), vec![]),
            make_symbol("Labrador", Some("Dog"), vec![]),
        ];

        builder.add_symbols(&symbols, "test.rs", Language::Rust);

        // Test hierarchy for Labrador
        let hierarchy = builder.get_hierarchy("Labrador");
        assert_eq!(hierarchy.extends, Some("Dog".to_owned()));
        assert_eq!(hierarchy.ancestors.len(), 2); // Dog, Animal
        assert_eq!(hierarchy.ancestors[0].name, "Dog");
        assert_eq!(hierarchy.ancestors[0].depth, 1);
        assert_eq!(hierarchy.ancestors[1].name, "Animal");
        assert_eq!(hierarchy.ancestors[1].depth, 2);

        // Test descendants of Animal
        let descendants = builder.get_descendants("Animal");
        assert!(descendants.contains(&"Dog".to_owned()));
        assert!(descendants.contains(&"Cat".to_owned()));
        assert!(descendants.contains(&"Labrador".to_owned()));
    }

    #[test]
    fn test_interface_implementation() {
        let mut builder = TypeHierarchyBuilder::new();

        let symbols = vec![
            Symbol {
                name: "Serializable".to_owned(),
                kind: SymbolKind::Interface,
                ..Default::default()
            },
            make_symbol("User", None, vec!["Serializable"]),
            make_symbol("Admin", Some("User"), vec!["Serializable"]),
        ];

        builder.add_symbols(&symbols, "test.java", Language::Java);

        let implementors = builder.get_implementors("Serializable");
        assert!(implementors.contains(&"User".to_owned()));
        assert!(implementors.contains(&"Admin".to_owned()));
    }

    #[test]
    fn test_is_ancestor_of() {
        let mut builder = TypeHierarchyBuilder::new();

        let symbols = vec![
            make_symbol("A", None, vec![]),
            make_symbol("B", Some("A"), vec![]),
            make_symbol("C", Some("B"), vec![]),
        ];

        builder.add_symbols(&symbols, "test.rs", Language::Rust);

        assert!(builder.is_ancestor_of("A", "C"));
        assert!(builder.is_ancestor_of("B", "C"));
        assert!(!builder.is_ancestor_of("C", "A"));
    }

    #[test]
    fn test_common_ancestor() {
        let mut builder = TypeHierarchyBuilder::new();

        let symbols = vec![
            make_symbol("Base", None, vec![]),
            make_symbol("Left", Some("Base"), vec![]),
            make_symbol("Right", Some("Base"), vec![]),
            make_symbol("LeftChild", Some("Left"), vec![]),
        ];

        builder.add_symbols(&symbols, "test.rs", Language::Rust);

        assert_eq!(builder.get_common_ancestor("LeftChild", "Right"), Some("Base".to_owned()));
        assert_eq!(builder.get_common_ancestor("Left", "Right"), Some("Base".to_owned()));
    }

    #[test]
    fn test_roots_and_leaves() {
        let mut builder = TypeHierarchyBuilder::new();

        let symbols = vec![
            make_symbol("Root1", None, vec![]),
            make_symbol("Root2", None, vec![]),
            make_symbol("Middle", Some("Root1"), vec![]),
            make_symbol("Leaf", Some("Middle"), vec![]),
        ];

        builder.add_symbols(&symbols, "test.rs", Language::Rust);

        let roots = builder.get_roots();
        assert!(roots.contains(&"Root1".to_owned()));
        assert!(roots.contains(&"Root2".to_owned()));
        assert!(!roots.contains(&"Middle".to_owned()));

        let leaves = builder.get_leaves();
        assert!(leaves.contains(&"Leaf".to_owned()));
        assert!(leaves.contains(&"Root2".to_owned()));
        assert!(!leaves.contains(&"Root1".to_owned()));
    }
}
