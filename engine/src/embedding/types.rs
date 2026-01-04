//! Core types for embedding chunk generation
//!
//! This module defines the data structures used throughout the embedding system,
//! including chunks, settings, and metadata types.

use serde::{Deserialize, Serialize};

use super::error::EmbedError;

/// Repository identifier for multi-tenant RAG systems
///
/// This enables embedding multiple codebases into a single vector database
/// while maintaining clear isolation and traceability. Essential for:
/// - Multi-repository search with proper attribution
/// - Access control based on repository ownership
/// - Cross-repository dependency tracking
/// - Audit trails for compliance (SOC2, GDPR)
///
/// # Example
///
/// ```
/// use infiniloom_engine::embedding::RepoIdentifier;
///
/// let repo = RepoIdentifier {
///     namespace: "github.com/myorg".to_string(),
///     name: "auth-service".to_string(),
///     version: Some("v2.1.0".to_string()),
///     branch: Some("main".to_string()),
///     commit: Some("abc123def".to_string()),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct RepoIdentifier {
    /// Namespace/organization (e.g., "github.com/myorg", "gitlab.com/team")
    /// Used for grouping and access control
    pub namespace: String,

    /// Repository name (e.g., "auth-service", "frontend")
    pub name: String,

    /// Semantic version or tag (e.g., "v2.1.0", "release-2024.01")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Branch name (e.g., "main", "feature/new-auth")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,

    /// Git commit hash (short or full)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

impl RepoIdentifier {
    /// Create a new repository identifier
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
            version: None,
            branch: None,
            commit: None,
        }
    }

    /// Create with full details including version and commit
    pub fn full(
        namespace: impl Into<String>,
        name: impl Into<String>,
        version: Option<String>,
        branch: Option<String>,
        commit: Option<String>,
    ) -> Self {
        Self { namespace: namespace.into(), name: name.into(), version, branch, commit }
    }

    /// Get fully qualified repository name (namespace/name)
    pub fn qualified_name(&self) -> String {
        if self.namespace.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", self.namespace, self.name)
        }
    }

    /// Check if this identifier represents the same repository (ignores version/commit)
    pub fn same_repo(&self, other: &Self) -> bool {
        self.namespace == other.namespace && self.name == other.name
    }
}

/// A single embedding chunk with stable, content-addressable ID
///
/// Each chunk represents a semantic unit of code (function, class, etc.) with
/// a deterministic ID derived from its normalized content. This enables:
/// - Cross-repository deduplication (same code = same ID)
/// - Incremental updates (compare IDs to detect changes)
/// - Stable references for vector databases
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedChunk {
    /// Content-addressable ID: BLAKE3 hash of normalized content
    /// Format: "ec_" + 32 hex chars (128 bits) - collision-resistant for enterprise scale
    pub id: String,

    /// Full 256-bit hash for collision verification
    pub full_hash: String,

    /// The actual code content (normalized)
    pub content: String,

    /// Token count for the target model
    pub tokens: u32,

    /// Symbol kind
    pub kind: ChunkKind,

    /// Source location metadata
    pub source: ChunkSource,

    /// Enriched context for better retrieval
    pub context: ChunkContext,

    /// For split chunks: part N of M
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<ChunkPart>,
}

/// Source location metadata for a chunk
///
/// This metadata helps identify where the chunk originated, but importantly
/// does NOT affect the chunk ID (which is based solely on content).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkSource {
    /// Repository identifier for multi-tenant RAG
    /// Essential for distinguishing chunks from different codebases
    #[serde(default, skip_serializing_if = "is_default_repo")]
    pub repo: RepoIdentifier,

    /// Relative file path (from repo root, never absolute)
    pub file: String,

    /// Line range (1-indexed, inclusive)
    pub lines: (u32, u32),

    /// Symbol name
    pub symbol: String,

    /// Fully qualified name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fqn: Option<String>,

    /// Programming language
    pub language: String,

    /// Parent symbol (for methods inside classes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    /// Visibility modifier
    pub visibility: Visibility,

    /// Whether this is test code
    pub is_test: bool,
}

/// Helper for skip_serializing_if - skip if repo is default (empty)
fn is_default_repo(repo: &RepoIdentifier) -> bool {
    repo.namespace.is_empty() && repo.name.is_empty()
}

/// Context information extracted from the chunk for better retrieval
///
/// This metadata improves RAG recall by providing natural language descriptions,
/// signatures for type matching, and relationship information.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkContext {
    /// Extracted docstring (for natural language retrieval)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docstring: Option<String>,

    /// Extracted comments within the chunk
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub comments: Vec<String>,

    /// Function/class signature (always included, even in split parts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    /// Symbols this chunk calls
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub calls: Vec<String>,

    /// Symbols that call this chunk
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub called_by: Vec<String>,

    /// Import dependencies
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub imports: Vec<String>,

    /// Auto-generated semantic tags
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<String>,

    // === Complexity Metrics ===
    // These enable filtering by code complexity in RAG applications
    /// Lines of code in this chunk (excluding blank lines and comments)
    /// Useful for filtering out trivial one-liners vs substantial implementations
    #[serde(skip_serializing_if = "is_zero", default)]
    pub lines_of_code: u32,

    /// Maximum nesting depth (control flow, blocks)
    /// Higher values indicate more complex logic; useful for prioritizing review
    #[serde(skip_serializing_if = "is_zero", default)]
    pub max_nesting_depth: u32,
}

/// Helper for serde skip_serializing_if
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Default value for hierarchy_min_children (for serde)
fn default_hierarchy_min_children() -> usize {
    2
}

/// Kind of code symbol represented by a chunk
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChunkKind {
    #[default]
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Module,
    Constant,
    Variable,
    Imports,
    TopLevel,
    FunctionPart,
    ClassPart,
}

impl ChunkKind {
    /// Get human-readable name for the chunk kind
    pub fn name(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::Variable => "variable",
            Self::Imports => "imports",
            Self::TopLevel => "top_level",
            Self::FunctionPart => "function_part",
            Self::ClassPart => "class_part",
        }
    }

    /// Check if this is a partial chunk (split from a larger symbol)
    pub fn is_part(&self) -> bool {
        matches!(self, Self::FunctionPart | Self::ClassPart)
    }
}

/// Visibility modifier for symbols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    #[default]
    Public,
    Private,
    Protected,
    Internal,
}

impl Visibility {
    /// Get the visibility name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Protected => "protected",
            Self::Internal => "internal",
        }
    }
}

/// Information about a chunk that was split from a larger symbol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkPart {
    /// Part number (1-indexed)
    pub part: u32,

    /// Total number of parts
    pub of: u32,

    /// ID of the logical parent (full symbol hash)
    pub parent_id: String,

    /// Signature repeated for context
    pub parent_signature: String,

    /// Number of overlapping lines from the previous chunk (for context continuity)
    /// This is 0 for the first part, and > 0 for subsequent parts when overlap is enabled.
    #[serde(skip_serializing_if = "is_zero", default)]
    pub overlap_lines: u32,
}

/// Settings that control chunk generation
///
/// These settings affect the output of chunk generation. Changing settings
/// will result in different chunk IDs, so the manifest tracks settings
/// to detect when a full rebuild is needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedSettings {
    /// Maximum tokens per chunk (default: 1000 for code models)
    pub max_tokens: u32,

    /// Minimum tokens per chunk (smaller merged, default: 50)
    pub min_tokens: u32,

    /// Overlap tokens between sequential chunks (default: 100)
    pub overlap_tokens: u32,

    /// Lines of context around symbols (default: 5)
    pub context_lines: u32,

    /// Include import statements as separate chunks
    pub include_imports: bool,

    /// Include top-level code outside symbols
    pub include_top_level: bool,

    /// Token counting model
    pub token_model: String,

    /// Version of chunking algorithm (for compatibility)
    pub algorithm_version: u32,

    /// Enable secret scanning
    pub scan_secrets: bool,

    /// Fail if secrets detected (CI mode)
    pub fail_on_secrets: bool,

    /// Redact detected secrets
    pub redact_secrets: bool,

    /// Include glob patterns (e.g., ["*.rs", "src/**"])
    /// Note: skip_serializing_if removed for bincode compatibility (requires all fields)
    #[serde(default)]
    pub include_patterns: Vec<String>,

    /// Exclude glob patterns (e.g., ["tests/*", "*.test.*"])
    /// Note: skip_serializing_if removed for bincode compatibility (requires all fields)
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// Include test files (default: false)
    #[serde(default)]
    pub include_tests: bool,

    /// Enable hierarchical chunking for improved RAG recall
    ///
    /// When enabled, generates summary chunks for container types (classes, structs)
    /// that list their children with signatures and brief descriptions. This enables
    /// RAG systems to retrieve both high-level overviews and specific implementations.
    ///
    /// Recommended for object-oriented codebases (Java, Python, TypeScript).
    #[serde(default)]
    pub enable_hierarchy: bool,

    /// Minimum number of children required to generate a summary chunk
    /// (default: 2, only relevant when enable_hierarchy is true)
    #[serde(default = "default_hierarchy_min_children")]
    pub hierarchy_min_children: usize,
}

impl Default for EmbedSettings {
    fn default() -> Self {
        Self {
            max_tokens: 1000,        // Optimized for code embedding models
            min_tokens: 50,          // Minimum meaningful chunk size
            overlap_tokens: 100,     // Context continuity between chunks
            context_lines: 5,        // Capture docstrings above functions
            include_imports: true,   // Track dependencies
            include_top_level: true, // Include module-level code
            token_model: "claude".to_owned(),
            algorithm_version: 1,
            scan_secrets: true, // Safe default
            fail_on_secrets: false,
            redact_secrets: true, // Safe default
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            include_tests: false,
            enable_hierarchy: false, // Off by default for backward compatibility
            hierarchy_min_children: 2, // Minimum children for summary generation
        }
    }
}

impl EmbedSettings {
    /// Current algorithm version
    pub const CURRENT_ALGORITHM_VERSION: u32 = 1;

    /// Maximum tokens limit (DoS protection)
    pub const MAX_TOKENS_LIMIT: u32 = 100_000;

    /// Get recommended settings for specific embedding model
    ///
    /// Different embedding models have different optimal chunk sizes:
    /// - voyage-code-2/3: 1500 tokens (large context window)
    /// - cohere-embed-v3: 400 tokens (smaller model)
    /// - openai-text-embedding-3: 800 tokens (balanced)
    /// - sentence-transformers: 384 tokens (BERT-based)
    pub fn for_embedding_model(model: &str) -> Self {
        let mut settings = Self::default();
        settings.max_tokens = match model.to_lowercase().as_str() {
            "voyage-code-2" | "voyage-code-3" => 1500,
            "cohere-embed-v3" | "cohere" => 400,
            "openai-text-embedding-3-small" | "openai-text-embedding-3-large" | "openai" => 800,
            "sentence-transformers" | "all-minilm" | "minilm" => 384,
            _ => 1000, // Default for most code models
        };
        settings
    }

    /// Validate settings, return error if invalid
    pub fn validate(&self) -> Result<(), EmbedError> {
        if self.max_tokens > Self::MAX_TOKENS_LIMIT {
            return Err(EmbedError::InvalidSettings {
                field: "max_tokens".to_owned(),
                reason: format!("exceeds limit of {}", Self::MAX_TOKENS_LIMIT),
            });
        }
        if self.min_tokens > self.max_tokens {
            return Err(EmbedError::InvalidSettings {
                field: "min_tokens".to_owned(),
                reason: "cannot exceed max_tokens".to_owned(),
            });
        }
        if self.algorithm_version > Self::CURRENT_ALGORITHM_VERSION {
            return Err(EmbedError::UnsupportedAlgorithmVersion {
                found: self.algorithm_version,
                max_supported: Self::CURRENT_ALGORITHM_VERSION,
            });
        }
        Ok(())
    }

    /// Create settings optimized for CI/CD pipelines
    ///
    /// These settings fail on secrets and use stricter validation.
    pub fn for_ci() -> Self {
        Self {
            fail_on_secrets: true,
            scan_secrets: true,
            redact_secrets: false, // Fail instead of redact
            ..Self::default()
        }
    }
}

/// Convert from the parser's SymbolKind to our ChunkKind
impl From<crate::types::SymbolKind> for ChunkKind {
    fn from(kind: crate::types::SymbolKind) -> Self {
        match kind {
            crate::types::SymbolKind::Function => ChunkKind::Function,
            crate::types::SymbolKind::Method => ChunkKind::Method,
            crate::types::SymbolKind::Class => ChunkKind::Class,
            crate::types::SymbolKind::Struct => ChunkKind::Struct,
            crate::types::SymbolKind::Enum => ChunkKind::Enum,
            crate::types::SymbolKind::Interface => ChunkKind::Interface,
            crate::types::SymbolKind::Trait => ChunkKind::Trait,
            crate::types::SymbolKind::Import => ChunkKind::Imports,
            crate::types::SymbolKind::Constant => ChunkKind::Constant,
            crate::types::SymbolKind::Variable => ChunkKind::Variable,
            crate::types::SymbolKind::TypeAlias => ChunkKind::Struct, // Map type aliases to struct
            crate::types::SymbolKind::Export => ChunkKind::Imports,   // Map exports to imports
            crate::types::SymbolKind::Module => ChunkKind::Module,
            crate::types::SymbolKind::Macro => ChunkKind::Function, // Map macros to functions
        }
    }
}

/// Convert from the parser's Visibility to our Visibility
impl From<crate::types::Visibility> for Visibility {
    fn from(vis: crate::types::Visibility) -> Self {
        match vis {
            crate::types::Visibility::Public => Visibility::Public,
            crate::types::Visibility::Private => Visibility::Private,
            crate::types::Visibility::Protected => Visibility::Protected,
            crate::types::Visibility::Internal => Visibility::Internal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = EmbedSettings::default();
        assert_eq!(settings.max_tokens, 1000);
        assert_eq!(settings.min_tokens, 50);
        assert_eq!(settings.overlap_tokens, 100);
        assert!(settings.scan_secrets);
    }

    #[test]
    fn test_validate_settings() {
        let mut settings = EmbedSettings::default();
        assert!(settings.validate().is_ok());

        // Invalid: max_tokens too large
        settings.max_tokens = 200_000;
        assert!(settings.validate().is_err());

        // Invalid: min > max
        settings.max_tokens = 100;
        settings.min_tokens = 200;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_for_embedding_model() {
        let voyage = EmbedSettings::for_embedding_model("voyage-code-2");
        assert_eq!(voyage.max_tokens, 1500);

        let cohere = EmbedSettings::for_embedding_model("cohere");
        assert_eq!(cohere.max_tokens, 400);

        let unknown = EmbedSettings::for_embedding_model("unknown-model");
        assert_eq!(unknown.max_tokens, 1000);
    }

    #[test]
    fn test_chunk_kind_name() {
        assert_eq!(ChunkKind::Function.name(), "function");
        assert_eq!(ChunkKind::FunctionPart.name(), "function_part");
    }

    #[test]
    fn test_chunk_kind_is_part() {
        assert!(ChunkKind::FunctionPart.is_part());
        assert!(ChunkKind::ClassPart.is_part());
        assert!(!ChunkKind::Function.is_part());
    }

    #[test]
    fn test_visibility_name() {
        assert_eq!(Visibility::Public.name(), "public");
        assert_eq!(Visibility::Private.name(), "private");
    }

    #[test]
    fn test_settings_serialization() {
        let settings = EmbedSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: EmbedSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings, deserialized);
    }

    #[test]
    fn test_ci_settings() {
        let ci = EmbedSettings::for_ci();
        assert!(ci.fail_on_secrets);
        assert!(ci.scan_secrets);
        assert!(!ci.redact_secrets);
    }
}
