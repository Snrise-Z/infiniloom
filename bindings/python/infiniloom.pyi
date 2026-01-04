"""Type stubs for infiniloom Python bindings.

This module provides high-performance repository context generation for LLMs.
Built in Rust with PyO3 bindings for maximum performance.
"""

from typing import Any, Dict, List, Optional, TypedDict

__version__: str

# ============================================================================
# Type Definitions
# ============================================================================

class TokenCounts(TypedDict):
    """Token counts for different LLM models."""
    o200k: int
    cl100k: int
    claude: int
    gemini: int
    llama: int
    mistral: int
    deepseek: int
    qwen: int
    cohere: int
    grok: int

class LanguageStats(TypedDict):
    """Statistics for a programming language."""
    language: str
    files: int
    lines: int
    percentage: float

class ScanResult(TypedDict):
    """Result of scanning a repository."""
    name: str
    path: str
    total_files: int
    total_lines: int
    total_tokens: TokenCounts
    languages: List[LanguageStats]
    branch: Optional[str]
    commit: Optional[str]
    framework: Optional[str]

class SecurityFinding(TypedDict):
    """A security finding from secret scanning."""
    file: str
    line: int
    severity: str
    kind: str
    pattern: str

class SymbolInfo(TypedDict):
    """Information about a code symbol."""
    id: int
    name: str
    kind: str
    file: str
    line: int
    end_line: int
    signature: Optional[str]
    visibility: str

class ReferenceInfo(TypedDict):
    """Information about a symbol reference."""
    symbol: SymbolInfo
    kind: str

class CallGraphEdge(TypedDict):
    """An edge in the call graph."""
    caller_id: int
    callee_id: int
    caller: str
    callee: str
    file: str
    line: int

class CallGraphStats(TypedDict):
    """Statistics about a call graph."""
    total_symbols: int
    total_calls: int
    functions: int
    classes: int

class CallGraph(TypedDict):
    """A complete call graph."""
    nodes: List[SymbolInfo]
    edges: List[CallGraphEdge]
    stats: CallGraphStats

class IndexStatus(TypedDict):
    """Status of a symbol index."""
    exists: bool
    file_count: int
    symbol_count: int
    last_built: Optional[str]
    version: Optional[str]
    files_updated: Optional[int]
    incremental: bool

class DiffLine(TypedDict):
    """A line in a diff hunk."""
    change_type: str
    old_line: Optional[int]
    new_line: Optional[int]
    content: str

class DiffHunk(TypedDict):
    """A hunk in a diff."""
    old_start: int
    old_count: int
    new_start: int
    new_count: int
    header: str
    lines: List[DiffLine]

class Commit(TypedDict):
    """Git commit information."""
    hash: str
    short_hash: str
    author: str
    email: str
    date: str
    message: str

class ChangedFileInfo(TypedDict):
    """Information about a changed file."""
    path: str
    old_path: Optional[str]
    status: str
    additions: int
    deletions: int
    diff: Optional[str]

class ContextSymbol(TypedDict):
    """A symbol in diff context."""
    name: str
    kind: str
    file: str
    line: int
    reason: str
    signature: Optional[str]

class DiffContext(TypedDict):
    """Context-aware diff result."""
    changed_files: List[ChangedFileInfo]
    context_symbols: List[ContextSymbol]
    related_tests: List[str]
    total_tokens: int

class ImpactResult(TypedDict):
    """Result of impact analysis."""
    changed_files: List[str]
    dependent_files: List[str]
    test_files: List[str]
    affected_symbols: List[Dict[str, Any]]
    impact_level: str
    summary: str

class ChunkResult(TypedDict):
    """A repository chunk."""
    index: int
    total: int
    focus: str
    tokens: int
    files: List[str]
    content: str

class TransitiveCaller(TypedDict):
    """A transitive caller result."""
    name: str
    kind: str
    file: str
    line: int
    depth: int
    call_path: List[str]

class CallSite(TypedDict):
    """A call site with context."""
    caller: str
    callee: str
    file: str
    line: int
    column: Optional[int]
    caller_id: int
    callee_id: int
    context: Optional[str]
    context_start_line: Optional[int]
    context_end_line: Optional[int]

class ChangedSymbol(TypedDict):
    """A symbol changed in a diff."""
    id: int
    name: str
    kind: str
    file: str
    line: int
    end_line: int
    signature: Optional[str]
    visibility: str
    change_type: str

class EmbedSource(TypedDict):
    """Source information for an embedding chunk."""
    file: str
    lines: tuple[int, int]
    symbol: str
    fqn: Optional[str]
    language: str
    parent: Optional[str]
    visibility: str
    is_test: bool

class EmbedContext(TypedDict):
    """Context information for an embedding chunk."""
    docstring: Optional[str]
    comments: Optional[List[str]]
    signature: Optional[str]
    calls: Optional[List[str]]
    called_by: Optional[List[str]]
    imports: Optional[List[str]]
    tags: Optional[List[str]]

class EmbedPart(TypedDict):
    """Part information for split chunks."""
    part: int
    of: int
    parent_id: str
    parent_signature: str

class EmbedChunk(TypedDict):
    """An embedding chunk."""
    id: str
    full_hash: str
    content: str
    tokens: int
    kind: str
    source: EmbedSource
    context: EmbedContext
    part: Optional[EmbedPart]

class ModifiedChunk(TypedDict):
    """A modified embedding chunk."""
    id: str
    full_hash: str
    content: str
    tokens: int
    kind: str
    source: EmbedSource
    context: EmbedContext
    part: Optional[EmbedPart]
    old_id: str

class RemovedChunk(TypedDict):
    """A removed embedding chunk."""
    id: str
    location_key: str

class EmbedDiff(TypedDict):
    """Diff information for embedding chunks."""
    added: List[EmbedChunk]
    modified: List[ModifiedChunk]
    removed: List[RemovedChunk]
    unchanged_count: int

class EmbedSettings(TypedDict):
    """Settings for embedding generation."""
    max_tokens: int
    min_tokens: int
    context_lines: int
    include_imports: bool
    include_top_level: bool
    include_tests: bool
    security_scan: bool

class EmbedSummary(TypedDict):
    """Summary of embedding generation."""
    total_chunks: int
    total_tokens: int
    added: Optional[int]
    modified: Optional[int]
    removed: Optional[int]
    unchanged: Optional[int]
    has_changes: Optional[bool]

class EmbedResult(TypedDict):
    """Result of embedding generation."""
    version: int
    settings: EmbedSettings
    chunks: List[EmbedChunk]
    summary: EmbedSummary
    diff: Optional[EmbedDiff]

class ManifestInfo(TypedDict):
    """Information about an embedding manifest."""
    version: int
    repo_path: str
    chunk_count: int
    commit_hash: Optional[str]
    updated_at: Optional[int]
    checksum: Optional[str]
    settings: EmbedSettings

class MapSymbol(TypedDict):
    """A symbol in the repository map."""
    name: str
    kind: str
    file: str
    line: int
    rank: int
    importance: float
    signature: Optional[str]

class RepoMap(TypedDict):
    """Repository map result."""
    summary: str
    token_count: int
    key_symbols: List[MapSymbol]

class FileInfo(TypedDict):
    """File information."""
    path: str
    language: Optional[str]
    size_bytes: int
    tokens: int
    importance: float

class BlameInfo(TypedDict):
    """Git blame information."""
    commit: str
    author: str
    date: str
    line_number: int

# ============================================================================
# Analysis Types
# ============================================================================

class ParamDoc(TypedDict):
    """Parameter documentation."""
    name: str
    type_info: Optional[str]
    description: Optional[str]
    is_optional: bool
    default_value: Optional[str]

class ReturnDoc(TypedDict):
    """Return value documentation."""
    type_info: Optional[str]
    description: Optional[str]

class ThrowsDoc(TypedDict):
    """Exception documentation."""
    exception_type: str
    description: Optional[str]

class Example(TypedDict):
    """Code example from documentation."""
    code: str
    title: Optional[str]
    language: Optional[str]

class Documentation(TypedDict):
    """Structured documentation extracted from code."""
    summary: Optional[str]
    description: Optional[str]
    params: List[ParamDoc]
    returns: Optional[ReturnDoc]
    throws: List[ThrowsDoc]
    examples: List[Example]
    is_deprecated: bool
    deprecation_message: Optional[str]
    since: Optional[str]
    see_also: List[str]
    tags: Dict[str, List[str]]
    raw: Optional[str]

class HalsteadMetrics(TypedDict):
    """Halstead software complexity metrics."""
    distinct_operators: int
    distinct_operands: int
    total_operators: int
    total_operands: int
    vocabulary: int
    length: int
    calculated_length: float
    volume: float
    difficulty: float
    effort: float
    time: float
    bugs: float

class LocMetrics(TypedDict):
    """Lines of code metrics."""
    total: int
    source: int
    comments: int
    blank: int

class ComplexityMetrics(TypedDict):
    """Code complexity metrics."""
    cyclomatic: int
    cognitive: int
    halstead: Optional[HalsteadMetrics]
    loc: LocMetrics
    maintainability_index: Optional[float]
    max_nesting_depth: int
    parameter_count: int
    return_count: int

class ComplexityIssue(TypedDict):
    """A complexity issue found in code."""
    message: str
    severity: str

class UnusedExport(TypedDict):
    """An unused exported symbol."""
    name: str
    kind: str
    file_path: str
    line: int
    confidence: float
    reason: str

class UnusedSymbol(TypedDict):
    """An unused private symbol."""
    name: str
    kind: str
    file_path: str
    line: int

class UnusedImport(TypedDict):
    """An unused import."""
    name: str
    import_path: str
    file_path: str
    line: int

class UnusedVariable(TypedDict):
    """An unused variable."""
    name: str
    file_path: str
    line: int
    scope: str

class UnreachableCode(TypedDict):
    """Unreachable code location."""
    file_path: str
    start_line: int
    end_line: int
    reason: str
    snippet: str

class DeadCodeInfo(TypedDict):
    """Dead code detection results."""
    unused_exports: List[UnusedExport]
    unreachable_code: List[UnreachableCode]
    unused_private: List[UnusedSymbol]
    unused_imports: List[UnusedImport]
    unused_variables: List[UnusedVariable]

class BreakingChange(TypedDict):
    """A breaking change between two versions."""
    symbol_name: str
    change_type: str
    severity: str
    file_path: str
    old_line: Optional[int]
    new_line: Optional[int]
    old_value: Optional[str]
    new_value: Optional[str]
    description: str
    suggestion: Optional[str]

class BreakingChangeSummary(TypedDict):
    """Summary of breaking changes."""
    total_changes: int
    by_severity: Dict[str, int]
    by_type: Dict[str, int]

class BreakingChangeReport(TypedDict):
    """Breaking change analysis report."""
    changes: List[BreakingChange]
    summary: BreakingChangeSummary
    old_ref: str
    new_ref: str

class AncestorInfo(TypedDict):
    """Information about a type ancestor."""
    name: str
    file: str
    line: int
    kind: str
    depth: int
    is_direct: bool

class TypeHierarchy(TypedDict):
    """Type hierarchy information."""
    name: str
    kind: str
    file: str
    line: int
    ancestors: List[AncestorInfo]
    descendants: List[AncestorInfo]
    interfaces: List[str]

# ============================================================================
# Core Functions
# ============================================================================

def pack(
    path: str,
    format: str = "xml",
    model: str = "claude",
    compression: str = "balanced",
    map_budget: int = 2000,
    max_symbols: int = 50,
    redact_secrets: bool = True,
    skip_symbols: bool = False,
) -> str:
    """Pack a repository into an LLM-optimized format.

    Args:
        path: Path to the repository.
        format: Output format ("xml", "markdown", "json", "yaml", "toon", "plain").
        model: Target LLM model ("gpt-5.2", "gpt-5.1", "gpt-5", "o3", "gpt-4o", "claude", "gemini", etc.).
        compression: Compression level ("none", "minimal", "balanced", "aggressive", "extreme").
        map_budget: Token budget for repository map.
        max_symbols: Maximum number of symbols to include.
        redact_secrets: Redact detected secrets in output.
        skip_symbols: Skip symbol extraction for faster scanning.

    Returns:
        Formatted repository context as a string.
    """
    ...

def scan(
    path: str,
    include_hidden: bool = False,
    respect_gitignore: bool = True,
    exclude: Optional[List[str]] = None,
) -> ScanResult:
    """Scan a repository and return statistics.

    Args:
        path: Path to the repository.
        include_hidden: Include hidden files.
        respect_gitignore: Respect .gitignore files.
        exclude: List of directories/patterns to exclude.

    Returns:
        Dictionary with repository statistics.
    """
    ...

def count_tokens(text: str, model: str = "claude") -> int:
    """Count tokens in text for a specific model.

    Args:
        text: Text to count tokens for.
        model: Target LLM model.

    Returns:
        Number of tokens.
    """
    ...

def scan_security(path: str) -> List[SecurityFinding]:
    """Scan repository for security issues.

    Args:
        path: Path to the repository.

    Returns:
        List of security findings.
    """
    ...

def semantic_compress(
    text: str,
    similarity_threshold: float = 0.7,
    budget_ratio: float = 0.5,
) -> str:
    """Compress text using semantic compression.

    Args:
        text: Text to compress.
        similarity_threshold: Threshold for grouping similar chunks.
        budget_ratio: Target size as ratio of original.

    Returns:
        Compressed text.
    """
    ...

def is_git_repo(path: str) -> bool:
    """Check if a path is a git repository.

    Args:
        path: Path to check.

    Returns:
        True if path is a git repository.
    """
    ...

# ============================================================================
# Index API
# ============================================================================

def build_index(
    path: str,
    force: bool = False,
    include_tests: bool = False,
    max_file_size: Optional[int] = None,
    exclude: Optional[List[str]] = None,
    incremental: bool = False,
) -> IndexStatus:
    """Build or update the symbol index for a repository.

    Args:
        path: Path to repository root.
        force: Force full rebuild even if index exists.
        include_tests: Include test files in index.
        max_file_size: Maximum file size to index in bytes.
        exclude: List of directories/patterns to exclude.
        incremental: Only re-index changed files.

    Returns:
        Dictionary with index status.
    """
    ...

def index_status(path: str) -> IndexStatus:
    """Get the status of an existing index.

    Args:
        path: Path to repository root.

    Returns:
        Dictionary with index status information.
    """
    ...

# ============================================================================
# Call Graph API
# ============================================================================

def find_symbol(path: str, name: str) -> List[SymbolInfo]:
    """Find a symbol by name.

    Args:
        path: Path to repository root.
        name: Symbol name to search for.

    Returns:
        List of matching symbols.
    """
    ...

def get_callers(path: str, symbol_name: str) -> List[SymbolInfo]:
    """Get all callers of a symbol.

    Args:
        path: Path to repository root.
        symbol_name: Name of the symbol.

    Returns:
        List of calling symbols.
    """
    ...

def get_callees(path: str, symbol_name: str) -> List[SymbolInfo]:
    """Get all callees of a symbol.

    Args:
        path: Path to repository root.
        symbol_name: Name of the symbol.

    Returns:
        List of called symbols.
    """
    ...

def get_references(path: str, symbol_name: str) -> List[ReferenceInfo]:
    """Get all references to a symbol.

    Args:
        path: Path to repository root.
        symbol_name: Name of the symbol.

    Returns:
        List of references.
    """
    ...

def get_call_graph(
    path: str,
    max_nodes: Optional[int] = None,
    max_edges: Optional[int] = None,
) -> CallGraph:
    """Get the complete call graph.

    Args:
        path: Path to repository root.
        max_nodes: Maximum number of nodes.
        max_edges: Maximum number of edges.

    Returns:
        The call graph.
    """
    ...

# ============================================================================
# Filtered Query API
# ============================================================================

def find_symbol_filtered(
    path: str,
    name: str,
    kinds: Optional[List[str]] = None,
    exclude_kinds: Optional[List[str]] = None,
) -> List[SymbolInfo]:
    """Find a symbol by name with filtering.

    Args:
        path: Path to repository root.
        name: Symbol name to search for.
        kinds: List of kinds to include.
        exclude_kinds: List of kinds to exclude.

    Returns:
        List of filtered symbols.
    """
    ...

def get_callers_filtered(
    path: str,
    symbol_name: str,
    kinds: Optional[List[str]] = None,
    exclude_kinds: Optional[List[str]] = None,
) -> List[SymbolInfo]:
    """Get all callers of a symbol with filtering.

    Args:
        path: Path to repository root.
        symbol_name: Name of the symbol.
        kinds: List of kinds to include.
        exclude_kinds: List of kinds to exclude.

    Returns:
        List of filtered calling symbols.
    """
    ...

def get_callees_filtered(
    path: str,
    symbol_name: str,
    kinds: Optional[List[str]] = None,
    exclude_kinds: Optional[List[str]] = None,
) -> List[SymbolInfo]:
    """Get all callees of a symbol with filtering.

    Args:
        path: Path to repository root.
        symbol_name: Name of the symbol.
        kinds: List of kinds to include.
        exclude_kinds: List of kinds to exclude.

    Returns:
        List of filtered called symbols.
    """
    ...

def get_references_filtered(
    path: str,
    symbol_name: str,
    kinds: Optional[List[str]] = None,
    exclude_kinds: Optional[List[str]] = None,
) -> List[ReferenceInfo]:
    """Get all references to a symbol with filtering.

    Args:
        path: Path to repository root.
        symbol_name: Name of the symbol.
        kinds: List of kinds to include.
        exclude_kinds: List of kinds to exclude.

    Returns:
        List of filtered references.
    """
    ...

# ============================================================================
# Chunk API
# ============================================================================

def chunk(
    path: str,
    strategy: str = "module",
    max_tokens: int = 8000,
    overlap: int = 0,
    model: str = "claude",
    priority_first: bool = False,
    exclude: Optional[List[str]] = None,
) -> List[ChunkResult]:
    """Split a repository into chunks for incremental processing.

    Args:
        path: Path to repository root.
        strategy: Chunking strategy ("fixed", "file", "module", "symbol", "semantic", "dependency").
        max_tokens: Maximum tokens per chunk.
        overlap: Token overlap between chunks.
        model: Target model for token counting.
        priority_first: Sort chunks by priority.
        exclude: Glob patterns to exclude.

    Returns:
        List of chunk dictionaries.
    """
    ...

# ============================================================================
# Impact & Diff API
# ============================================================================

def analyze_impact(
    path: str,
    files: List[str],
    depth: int = 2,
    include_tests: bool = False,
    model: Optional[str] = None,
    exclude: Optional[List[str]] = None,
    include: Optional[List[str]] = None,
) -> ImpactResult:
    """Analyze the impact of changes to files or symbols.

    Args:
        path: Path to repository root.
        files: List of files to analyze.
        depth: Depth of dependency traversal (1-3).
        include_tests: Include test files.
        model: Target model for token counting.
        exclude: Glob patterns to exclude.
        include: Glob patterns to include.

    Returns:
        Impact analysis result.
    """
    ...

def get_diff_context(
    path: str,
    from_ref: str = "",
    to_ref: str = "HEAD",
    depth: int = 2,
    budget: int = 50000,
    include_diff: bool = False,
    model: Optional[str] = None,
    exclude: Optional[List[str]] = None,
    include: Optional[List[str]] = None,
) -> DiffContext:
    """Get context-aware diff with surrounding symbols and dependencies.

    Args:
        path: Path to repository root.
        from_ref: Starting commit/branch.
        to_ref: Ending commit/branch.
        depth: Depth of context expansion (1-3).
        budget: Token budget for context.
        include_diff: Include the actual diff content.
        model: Target model for token counting.
        exclude: Glob patterns to exclude.
        include: Glob patterns to include.

    Returns:
        Context-aware diff result.
    """
    ...

def get_changed_symbols_filtered(
    path: str,
    from_ref: str = "",
    to_ref: str = "HEAD",
    kinds: Optional[List[str]] = None,
    exclude_kinds: Optional[List[str]] = None,
) -> List[ChangedSymbol]:
    """Get symbols changed in a diff with filtering.

    Args:
        path: Path to repository root.
        from_ref: Starting commit/branch.
        to_ref: Ending commit/branch.
        kinds: List of kinds to include.
        exclude_kinds: List of kinds to exclude.

    Returns:
        List of changed symbols.
    """
    ...

def get_transitive_callers(
    path: str,
    symbol_name: str,
    max_depth: int = 3,
    max_results: int = 100,
) -> List[TransitiveCaller]:
    """Get all functions that eventually call a symbol.

    Args:
        path: Path to repository root.
        symbol_name: Name of the symbol.
        max_depth: Maximum depth to traverse.
        max_results: Maximum number of results.

    Returns:
        List of transitive callers.
    """
    ...

def get_call_sites_with_context(
    path: str,
    symbol_name: str,
    lines_before: int = 3,
    lines_after: int = 3,
) -> List[CallSite]:
    """Get call sites with surrounding code context.

    Args:
        path: Path to repository root.
        symbol_name: Name of the symbol.
        lines_before: Lines of context before.
        lines_after: Lines of context after.

    Returns:
        List of call sites with context.
    """
    ...

# ============================================================================
# Embedding API
# ============================================================================

def embed(
    path: str,
    max_tokens: int = 1000,
    min_tokens: int = 50,
    context_lines: int = 5,
    include_imports: bool = True,
    include_top_level: bool = True,
    include_tests: bool = False,
    security_scan: bool = True,
    include_patterns: Optional[List[str]] = None,
    exclude_patterns: Optional[List[str]] = None,
    manifest_path: Optional[str] = None,
    diff_only: bool = False,
) -> EmbedResult:
    """Generate embedding chunks for a repository.

    Args:
        path: Path to the repository.
        max_tokens: Maximum tokens per chunk.
        min_tokens: Minimum tokens per chunk.
        context_lines: Lines of context around symbols.
        include_imports: Include import statements.
        include_top_level: Include top-level code.
        include_tests: Include test files.
        security_scan: Enable secret scanning.
        include_patterns: Glob patterns for files to include.
        exclude_patterns: Glob patterns for files to exclude.
        manifest_path: Path to manifest file.
        diff_only: Return only changed chunks.

    Returns:
        Embedding result with chunks and metadata.
    """
    ...

def load_embed_manifest(path: str) -> Optional[ManifestInfo]:
    """Load an embedding manifest.

    Args:
        path: Path to manifest file.

    Returns:
        Manifest information or None if not found.
    """
    ...

def delete_embed_manifest(path: str) -> bool:
    """Delete an embedding manifest.

    Args:
        path: Path to manifest file.

    Returns:
        True if deleted, False if not found.
    """
    ...

# ============================================================================
# Classes
# ============================================================================

class Infiniloom:
    """Infiniloom class for object-oriented interface."""

    def __init__(self, path: str) -> None:
        """Create a new Infiniloom instance.

        Args:
            path: Path to the repository.
        """
        ...

    def load(self, include_hidden: bool = False, respect_gitignore: bool = True) -> None:
        """Scan the repository and load it into memory."""
        ...

    def stats(self) -> ScanResult:
        """Get repository statistics."""
        ...

    def pack(
        self,
        format: str = "xml",
        model: str = "claude",
        compression: str = "balanced",
        map_budget: int = 2000,
        max_symbols: int = 50,
    ) -> str:
        """Pack the repository into an LLM-optimized format."""
        ...

    def map(self, map_budget: int = 2000, max_symbols: int = 50) -> RepoMap:
        """Get the repository map."""
        ...

    def scan_security(self) -> List[SecurityFinding]:
        """Scan for security issues."""
        ...

    def files(self) -> List[FileInfo]:
        """Get list of files in the repository."""
        ...

class GitRepo:
    """Git repository wrapper."""

    def __init__(self, path: str) -> None:
        """Open a git repository.

        Args:
            path: Path to the repository.

        Raises:
            InfiniloomError: If path is not a git repository.
        """
        ...

    def current_branch(self) -> str:
        """Get the current branch name."""
        ...

    def current_commit(self) -> str:
        """Get the current commit hash."""
        ...

    def status(self) -> List[ChangedFileInfo]:
        """Get working tree status."""
        ...

    def diff_files(self, from_ref: str, to_ref: str) -> List[ChangedFileInfo]:
        """Get files changed between two commits."""
        ...

    def log(self, count: int = 10) -> List[Commit]:
        """Get recent commits."""
        ...

    def file_log(self, path: str, count: int = 10) -> List[Commit]:
        """Get commits that modified a specific file."""
        ...

    def blame(self, path: str) -> List[BlameInfo]:
        """Get blame information for a file."""
        ...

    def ls_files(self) -> List[str]:
        """Get list of files tracked by git."""
        ...

    def diff_content(self, from_ref: str, to_ref: str, path: str) -> str:
        """Get diff content between two commits for a file."""
        ...

    def uncommitted_diff(self, path: str) -> str:
        """Get diff content for uncommitted changes in a file."""
        ...

    def all_uncommitted_diffs(self) -> str:
        """Get diff for all uncommitted changes."""
        ...

    def has_changes(self, path: str) -> bool:
        """Check if a file has uncommitted changes."""
        ...

    def last_modified_commit(self, path: str) -> Commit:
        """Get the last commit that modified a file."""
        ...

    def file_change_frequency(self, path: str, days: int = 30) -> int:
        """Get file change frequency in recent days."""
        ...

    def file_at_ref(self, path: str, git_ref: str) -> str:
        """Get file content at a specific git ref."""
        ...

    def diff_hunks(
        self,
        from_ref: str,
        to_ref: str,
        path: Optional[str] = None,
    ) -> List[DiffHunk]:
        """Parse diff between two refs into structured hunks."""
        ...

    def uncommitted_hunks(self, path: Optional[str] = None) -> List[DiffHunk]:
        """Parse uncommitted changes into structured hunks."""
        ...

    def staged_hunks(self, path: Optional[str] = None) -> List[DiffHunk]:
        """Parse staged changes into structured hunks."""
        ...

# ============================================================================
# Exceptions
# ============================================================================

class InfiniloomError(Exception):
    """Exception raised by Infiniloom operations."""
    ...

# ============================================================================
# Analysis API
# ============================================================================

def extract_documentation(raw_doc: str, language: str) -> Documentation:
    """Extract structured documentation from a docstring/comment.

    Args:
        raw_doc: Raw documentation string (JSDoc, Python docstring, etc.).
        language: Language of the code ("javascript", "python", "rust", etc.).

    Returns:
        Structured documentation with summary, params, returns, etc.
    """
    ...

def detect_dead_code(
    path: str,
    languages: Optional[List[str]] = None,
) -> DeadCodeInfo:
    """Detect dead code in a repository.

    Args:
        path: Path to the repository.
        languages: Optional list of languages to analyze.

    Returns:
        Dead code detection results.
    """
    ...

def detect_breaking_changes(
    path: str,
    old_ref: str,
    new_ref: str,
) -> BreakingChangeReport:
    """Detect breaking changes between two versions.

    Args:
        path: Path to the repository.
        old_ref: Old git ref (commit, tag, branch).
        new_ref: New git ref (commit, tag, branch).

    Returns:
        Breaking change analysis report.
    """
    ...

def get_type_hierarchy(path: str, symbol_name: str) -> TypeHierarchy:
    """Get the type hierarchy for a class/interface.

    Args:
        path: Path to the repository.
        symbol_name: Name of the class/interface.

    Returns:
        Type hierarchy with ancestors and descendants.
    """
    ...

def get_type_ancestors(path: str, symbol_name: str) -> List[AncestorInfo]:
    """Get all ancestors (parent classes/interfaces) of a type.

    Args:
        path: Path to the repository.
        symbol_name: Name of the class/interface.

    Returns:
        List of ancestor types.
    """
    ...

def get_type_descendants(path: str, symbol_name: str) -> List[AncestorInfo]:
    """Get all descendants (child classes) of a type.

    Args:
        path: Path to the repository.
        symbol_name: Name of the class/interface.

    Returns:
        List of descendant types.
    """
    ...

def get_implementors(path: str, interface_name: str) -> List[SymbolInfo]:
    """Get all classes that implement an interface.

    Args:
        path: Path to the repository.
        interface_name: Name of the interface/trait.

    Returns:
        List of implementing classes.
    """
    ...

def calculate_complexity(source: str, language: str) -> ComplexityMetrics:
    """Calculate complexity metrics for source code.

    Args:
        source: Source code string.
        language: Language of the code ("python", "javascript", etc.).

    Returns:
        Complexity metrics including cyclomatic, cognitive, Halstead, etc.
    """
    ...

def check_complexity(
    source: str,
    language: str,
    max_cyclomatic: int = 10,
    max_cognitive: int = 15,
    max_nesting: int = 4,
    max_params: int = 5,
    min_maintainability: float = 40.0,
) -> List[ComplexityIssue]:
    """Check code complexity against thresholds.

    Args:
        source: Source code string.
        language: Language of the code.
        max_cyclomatic: Maximum cyclomatic complexity.
        max_cognitive: Maximum cognitive complexity.
        max_nesting: Maximum nesting depth.
        max_params: Maximum parameter count.
        min_maintainability: Minimum maintainability index.

    Returns:
        List of complexity issues found.
    """
    ...
