//! Shared conversion utilities for the index module.
//!
//! This module contains type conversions that are shared between
//! the full index builder and lazy context builder.

use super::types::{IndexSymbolKind, Visibility};
use crate::types::{SymbolKind, Visibility as CoreVisibility};

/// Convert from core SymbolKind to IndexSymbolKind
pub(super) fn convert_symbol_kind(kind: SymbolKind) -> IndexSymbolKind {
    match kind {
        SymbolKind::Function => IndexSymbolKind::Function,
        SymbolKind::Method => IndexSymbolKind::Method,
        SymbolKind::Class => IndexSymbolKind::Class,
        SymbolKind::Struct => IndexSymbolKind::Struct,
        SymbolKind::Interface => IndexSymbolKind::Interface,
        SymbolKind::Trait => IndexSymbolKind::Trait,
        SymbolKind::Enum => IndexSymbolKind::Enum,
        SymbolKind::Constant => IndexSymbolKind::Constant,
        SymbolKind::Variable => IndexSymbolKind::Variable,
        SymbolKind::Module => IndexSymbolKind::Module,
        SymbolKind::Import => IndexSymbolKind::Import,
        SymbolKind::Export => IndexSymbolKind::Export,
        SymbolKind::TypeAlias => IndexSymbolKind::TypeAlias,
        SymbolKind::Macro => IndexSymbolKind::Macro,
    }
}

/// Convert from core Visibility to index Visibility
pub(super) fn convert_visibility(visibility: CoreVisibility) -> Visibility {
    match visibility {
        CoreVisibility::Public => Visibility::Public,
        CoreVisibility::Private => Visibility::Private,
        CoreVisibility::Protected => Visibility::Protected,
        CoreVisibility::Internal => Visibility::Internal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_symbol_kind() {
        assert!(matches!(convert_symbol_kind(SymbolKind::Function), IndexSymbolKind::Function));
        assert!(matches!(convert_symbol_kind(SymbolKind::Class), IndexSymbolKind::Class));
        assert!(matches!(convert_symbol_kind(SymbolKind::Macro), IndexSymbolKind::Macro));
    }

    #[test]
    fn test_convert_visibility() {
        assert!(matches!(convert_visibility(CoreVisibility::Public), Visibility::Public));
        assert!(matches!(convert_visibility(CoreVisibility::Private), Visibility::Private));
    }
}
