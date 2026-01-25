"""
Pydantic models for Infiniloom Python bindings.

This module provides runtime validation with Pydantic v2 for all option types
and result types returned by the Infiniloom API.

Usage
-----

Validating options from external sources (API calls, config files, etc.):

    >>> from infiniloom.models import PackOptions, OutputFormat
    >>> from infiniloom import pack
    >>>
    >>> # Validate options from user input
    >>> options = PackOptions(
    ...     format="xml",
    ...     model="claude",
    ...     compression="balanced"
    ... )
    >>>
    >>> # Use validated options
    >>> context = pack("/path/to/repo", **options.model_dump())

Using Literal types for strict enum validation:

    >>> from infiniloom.models import OutputFormat, TokenizerModel
    >>>
    >>> # These are Literal types - validated at runtime
    >>> format: OutputFormat = "xml"  # OK
    >>> format: OutputFormat = "invalid"  # ValidationError

Converting dict results to typed models:

    >>> from infiniloom.models import ScanStats, SecurityFinding
    >>> from infiniloom import scan, scan_security
    >>>
    >>> # Convert raw dict to typed model
    >>> stats_dict = scan("/path/to/repo")
    >>> stats = ScanStats.model_validate(stats_dict)
    >>> print(f"Files: {stats.total_files}")
    >>>
    >>> findings_list = scan_security("/path/to/repo")
    >>> findings = [SecurityFinding.model_validate(f) for f in findings_list]
"""

from __future__ import annotations

from enum import Enum
from typing import Annotated, Any, Literal, TypeAlias

import re
from pydantic import BaseModel, ConfigDict, Field, field_validator, model_validator


# ============================================================================
# Enum Types as Literal Unions
# ============================================================================

OutputFormat: TypeAlias = Literal["xml", "markdown", "json", "yaml", "toon", "plain"]
"""Output format for pack/chunk operations."""

TokenizerModel: TypeAlias = Literal[
    # OpenAI models (exact tokenization via tiktoken)
    "gpt-5.2",
    "gpt-5.2-pro",
    "gpt-5.1",
    "gpt-5.1-mini",
    "gpt-5.1-codex",
    "gpt-5",
    "gpt-5-mini",
    "gpt-5-nano",
    "o4-mini",
    "o3",
    "o3-mini",
    "o1",
    "o1-mini",
    "o1-preview",
    "gpt-4o",
    "gpt-4o-mini",
    "gpt-4",
    "gpt-4-turbo",
    "gpt-3.5-turbo",
    # Anthropic
    "claude",
    # Google
    "gemini",
    "gemini-1.5",
    "gemini-2.0",
    # Meta
    "llama",
    "llama-3",
    "llama-3.1",
    "llama-3.2",
    "codellama",
    # Mistral
    "mistral",
    "mixtral",
    # DeepSeek
    "deepseek",
    "deepseek-v3",
    # Alibaba
    "qwen",
    "qwen-2.5",
    # Cohere
    "cohere",
    "command-r",
    # xAI
    "grok",
]
"""Supported LLM tokenizer models."""

CompressionLevel: TypeAlias = Literal[
    "none",
    "minimal",
    "balanced",
    "aggressive",
    "extreme",
    "focused",
    "semantic",
]
"""Compression level for output."""

SecuritySeverity: TypeAlias = Literal["critical", "high", "medium", "low", "info"]
"""Security severity levels."""

GitStatus: TypeAlias = Literal[
    "Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown"
]
"""Git file status."""

SymbolKind: TypeAlias = Literal[
    "function",
    "method",
    "class",
    "struct",
    "interface",
    "trait",
    "enum",
    "constant",
    "variable",
    "import",
    "export",
    "type",
    "module",
    "macro",
]
"""Symbol kinds."""

Visibility: TypeAlias = Literal["public", "private", "protected", "internal"]
"""Symbol visibility."""

ImpactType: TypeAlias = Literal["direct", "caller", "callee", "dependent"]
"""Impact type for affected symbols."""

ImpactLevel: TypeAlias = Literal["low", "medium", "high", "critical"]
"""Impact level for analysis results."""

ChangeType: TypeAlias = Literal["added", "modified", "deleted"]
"""Change type for symbols."""

ChunkStrategy: TypeAlias = Literal[
    "fixed", "file", "module", "symbol", "semantic", "dependency"
]
"""Chunking strategy."""

EmbedChunkKind: TypeAlias = Literal[
    "function",
    "method",
    "class",
    "struct",
    "enum",
    "interface",
    "trait",
    "module",
    "constant",
    "variable",
    "imports",
    "top_level",
    "function_part",
    "class_part",
]
"""Embed chunk kind."""

ChangeSeverity: TypeAlias = Literal["critical", "high", "medium", "low"]
"""Breaking change severity."""

ReferenceKind: TypeAlias = Literal["call", "import", "inherit", "implement"]
"""Reference kind."""

ContextReason: TypeAlias = Literal["changed", "caller", "callee", "dependent"]
"""Context symbol reason."""


# ============================================================================
# Depth and Related Depth Constrained Types
# ============================================================================

Depth: TypeAlias = Literal[1, 2, 3]
"""Depth for dependency traversal (1-3)."""


# ============================================================================
# Validation Constants and Helpers
# ============================================================================

# Upper bounds for numeric fields (matching TypeScript/Zod schemas)
MAX_TOKEN_BUDGET = 10_000_000
MAX_TOKENS_PER_CHUNK = 100_000
MAX_SYMBOLS = 10_000
MAX_OVERLAP = 10_000
MAX_CONTEXT_LINES = 1_000
MAX_FILE_SIZE = 100 * 1024 * 1024  # 100MB
MAX_GRAPH_NODES = 100_000
MAX_GRAPH_EDGES = 100_000
MAX_CHUNK_SIZE = 100_000

# String limits
MAX_GLOB_PATTERN_LENGTH = 256
MAX_GLOB_PATTERNS = 100
MAX_PATH_LENGTH = 4096

# Git SHA pattern
GIT_SHA_PATTERN = re.compile(r"^[a-fA-F0-9]{7,40}$")


def _validate_glob_patterns(patterns: list[str] | None) -> list[str] | None:
    """Validate glob patterns list."""
    if patterns is None:
        return None
    if len(patterns) > MAX_GLOB_PATTERNS:
        raise ValueError(f"Too many patterns (max {MAX_GLOB_PATTERNS})")
    for p in patterns:
        if len(p) > MAX_GLOB_PATTERN_LENGTH:
            raise ValueError(f"Pattern too long (max {MAX_GLOB_PATTERN_LENGTH})")
    return patterns


def _validate_path_no_traversal(path: str | None) -> str | None:
    """Validate path doesn't contain traversal sequences."""
    if path is None:
        return None
    if len(path) > MAX_PATH_LENGTH:
        raise ValueError(f"Path too long (max {MAX_PATH_LENGTH})")
    if ".." in path:
        raise ValueError("Path traversal not allowed")
    return path


def _validate_git_sha(sha: str | None) -> str | None:
    """Validate git SHA format."""
    if sha is None:
        return None
    if not GIT_SHA_PATTERN.match(sha):
        raise ValueError("Invalid git SHA format (expected 7-40 hex characters)")
    return sha


# ============================================================================
# Option Models
# ============================================================================


class PackOptions(BaseModel):
    """Options for packing a repository."""

    model_config = ConfigDict(strict=True, extra="forbid")

    format: OutputFormat | None = Field(
        default=None,
        description='Output format: "xml", "markdown", "json", "yaml", "toon", or "plain"',
    )
    model: TokenizerModel | None = Field(
        default=None,
        description="Target LLM model for token counting",
    )
    compression: CompressionLevel | None = Field(
        default=None,
        description="Compression level for output",
    )
    map_budget: Annotated[int, Field(ge=0, le=MAX_TOKEN_BUDGET)] | None = Field(
        default=None,
        description="Token budget for repository map",
    )
    max_symbols: Annotated[int, Field(gt=0, le=MAX_SYMBOLS)] | None = Field(
        default=None,
        description="Maximum number of symbols in map",
    )
    skip_security: bool | None = Field(
        default=None,
        description="Skip security scanning",
    )
    redact_secrets: bool | None = Field(
        default=None,
        description="Redact detected secrets in output",
    )
    skip_symbols: bool | None = Field(
        default=None,
        description="Skip symbol extraction for faster scanning",
    )
    include: list[str] | None = Field(
        default=None,
        description="Glob patterns to include",
    )
    exclude: list[str] | None = Field(
        default=None,
        description="Glob patterns to exclude",
    )
    include_tests: bool | None = Field(
        default=None,
        description="Include test files",
    )
    security_threshold: SecuritySeverity | None = Field(
        default=None,
        description="Minimum security severity to block on",
    )
    token_budget: Annotated[int, Field(ge=0, le=MAX_TOKEN_BUDGET)] | None = Field(
        default=None,
        description="Token budget for total output (0 = no limit)",
    )
    changed_only: bool | None = Field(
        default=None,
        description="Only include files changed in git",
    )
    base_sha: str | None = Field(
        default=None,
        description="Base SHA/ref for diff comparison",
    )
    head_sha: str | None = Field(
        default=None,
        description="Head SHA/ref for diff comparison",
    )
    staged_only: bool | None = Field(
        default=None,
        description="Include staged changes only",
    )
    include_related: bool | None = Field(
        default=None,
        description="Include related files",
    )
    related_depth: Depth | None = Field(
        default=None,
        description="Depth for related file traversal (1-3)",
    )

    # Validators
    _validate_include = field_validator("include")(_validate_glob_patterns)
    _validate_exclude = field_validator("exclude")(_validate_glob_patterns)
    _validate_base_sha = field_validator("base_sha")(_validate_git_sha)
    _validate_head_sha = field_validator("head_sha")(_validate_git_sha)


class ScanOptions(BaseModel):
    """Options for scanning a repository."""

    model_config = ConfigDict(strict=True, extra="forbid")

    model: TokenizerModel | None = Field(
        default=None,
        description="Target model for token counting",
    )
    include: list[str] | None = Field(
        default=None,
        description="Glob patterns to include",
    )
    exclude: list[str] | None = Field(
        default=None,
        description="Glob patterns to exclude",
    )
    include_tests: bool | None = Field(
        default=None,
        description="Include test files",
    )
    apply_default_ignores: bool | None = Field(
        default=None,
        description="Apply default ignores",
    )

    # Validators
    _validate_include = field_validator("include")(_validate_glob_patterns)
    _validate_exclude = field_validator("exclude")(_validate_glob_patterns)


class ChunkOptions(BaseModel):
    """Options for chunking a repository."""

    model_config = ConfigDict(strict=True, extra="forbid")

    strategy: ChunkStrategy | None = Field(
        default=None,
        description="Chunking strategy",
    )
    max_tokens: Annotated[int, Field(gt=0, le=MAX_TOKENS_PER_CHUNK)] | None = Field(
        default=None,
        description="Maximum tokens per chunk",
    )
    overlap: Annotated[int, Field(ge=0, le=MAX_OVERLAP)] | None = Field(
        default=None,
        description="Token overlap between chunks",
    )
    model: TokenizerModel | None = Field(
        default=None,
        description="Target model for token counting",
    )
    format: OutputFormat | None = Field(
        default=None,
        description="Output format",
    )
    priority_first: bool | None = Field(
        default=None,
        description="Sort chunks by priority",
    )
    exclude: list[str] | None = Field(
        default=None,
        description="Directories/patterns to exclude",
    )

    # Validators
    _validate_exclude = field_validator("exclude")(_validate_glob_patterns)


class IndexOptions(BaseModel):
    """Options for building an index."""

    model_config = ConfigDict(strict=True, extra="forbid")

    force: bool | None = Field(
        default=None,
        description="Force full rebuild",
    )
    include_tests: bool | None = Field(
        default=None,
        description="Include test files",
    )
    max_file_size: Annotated[int, Field(gt=0, le=MAX_FILE_SIZE)] | None = Field(
        default=None,
        description="Maximum file size to index (bytes)",
    )
    exclude: list[str] | None = Field(
        default=None,
        description="Directories/patterns to exclude",
    )
    incremental: bool | None = Field(
        default=None,
        description="Incremental update",
    )

    # Validators
    _validate_exclude = field_validator("exclude")(_validate_glob_patterns)


class DiffContextOptions(BaseModel):
    """Options for diff context."""

    model_config = ConfigDict(strict=True, extra="forbid")

    depth: Depth | None = Field(
        default=None,
        description="Depth of context expansion (1-3)",
    )
    budget: Annotated[int, Field(gt=0, le=MAX_TOKEN_BUDGET)] | None = Field(
        default=None,
        description="Token budget for context",
    )
    include_diff: bool | None = Field(
        default=None,
        description="Include the actual diff content",
    )
    format: OutputFormat | None = Field(
        default=None,
        description="Output format",
    )
    model: TokenizerModel | None = Field(
        default=None,
        description="Target model for token counting",
    )
    exclude: list[str] | None = Field(
        default=None,
        description="Glob patterns to exclude",
    )
    include: list[str] | None = Field(
        default=None,
        description="Glob patterns to include",
    )

    # Validators
    _validate_include = field_validator("include")(_validate_glob_patterns)
    _validate_exclude = field_validator("exclude")(_validate_glob_patterns)


class ImpactOptions(BaseModel):
    """Options for impact analysis."""

    model_config = ConfigDict(strict=True, extra="forbid")

    depth: Depth | None = Field(
        default=None,
        description="Depth of dependency traversal (1-3)",
    )
    include_tests: bool | None = Field(
        default=None,
        description="Include test files in analysis",
    )
    model: TokenizerModel | None = Field(
        default=None,
        description="Target model for token counting",
    )
    exclude: list[str] | None = Field(
        default=None,
        description="Glob patterns to exclude",
    )
    include: list[str] | None = Field(
        default=None,
        description="Glob patterns to include",
    )

    # Validators
    _validate_include = field_validator("include")(_validate_glob_patterns)
    _validate_exclude = field_validator("exclude")(_validate_glob_patterns)


class EmbedOptions(BaseModel):
    """Options for embedding chunk generation."""

    model_config = ConfigDict(strict=True, extra="forbid")

    max_tokens: Annotated[int, Field(gt=0, le=MAX_TOKENS_PER_CHUNK)] | None = Field(
        default=None,
        description="Maximum tokens per chunk",
    )
    min_tokens: Annotated[int, Field(ge=0, le=MAX_OVERLAP)] | None = Field(
        default=None,
        description="Minimum tokens for a chunk",
    )
    context_lines: Annotated[int, Field(ge=0, le=MAX_CONTEXT_LINES)] | None = Field(
        default=None,
        description="Lines of context around symbols",
    )
    include_imports: bool | None = Field(
        default=None,
        description="Include imports in chunks",
    )
    include_top_level: bool | None = Field(
        default=None,
        description="Include top-level code",
    )
    include_tests: bool | None = Field(
        default=None,
        description="Include test files",
    )
    security_scan: bool | None = Field(
        default=None,
        description="Enable secret scanning",
    )
    include_patterns: list[str] | None = Field(
        default=None,
        description="Include patterns (glob)",
    )
    exclude_patterns: list[str] | None = Field(
        default=None,
        description="Exclude patterns (glob)",
    )
    manifest_path: str | None = Field(
        default=None,
        description="Path to manifest file",
    )
    diff_only: bool | None = Field(
        default=None,
        description="Only return changed chunks (diff mode)",
    )

    # Validators
    _validate_include_patterns = field_validator("include_patterns")(_validate_glob_patterns)
    _validate_exclude_patterns = field_validator("exclude_patterns")(_validate_glob_patterns)
    _validate_manifest_path = field_validator("manifest_path")(_validate_path_no_traversal)


class QueryFilter(BaseModel):
    """Filter for symbol queries."""

    model_config = ConfigDict(strict=True, extra="forbid")

    kinds: list[SymbolKind] | None = Field(
        default=None,
        description="Filter by symbol kinds",
    )
    exclude_kinds: list[SymbolKind] | None = Field(
        default=None,
        description="Exclude specific kinds",
    )


class SymbolFilter(BaseModel):
    """Filter for symbols."""

    model_config = ConfigDict(strict=True, extra="forbid")

    kind: SymbolKind | None = Field(
        default=None,
        description="Filter by symbol kind",
    )
    visibility: Visibility | None = Field(
        default=None,
        description="Filter by visibility",
    )


class CallGraphOptions(BaseModel):
    """Options for call graph queries."""

    model_config = ConfigDict(strict=True, extra="forbid")

    max_nodes: Annotated[int, Field(gt=0, le=MAX_GRAPH_NODES)] | None = Field(
        default=None,
        description="Maximum number of nodes to return",
    )
    max_edges: Annotated[int, Field(gt=0, le=MAX_GRAPH_EDGES)] | None = Field(
        default=None,
        description="Maximum number of edges to return",
    )


class SemanticCompressOptions(BaseModel):
    """Options for semantic compression."""

    model_config = ConfigDict(strict=True, extra="forbid")

    similarity_threshold: Annotated[float, Field(ge=0.0, le=1.0)] | None = Field(
        default=None,
        description="Threshold for grouping similar chunks (0.0-1.0)",
    )
    budget_ratio: Annotated[float, Field(ge=0.0, le=1.0)] | None = Field(
        default=None,
        description="Target size as ratio of original (0.0-1.0)",
    )
    min_chunk_size: Annotated[int, Field(gt=0, le=MAX_CHUNK_SIZE)] | None = Field(
        default=None,
        description="Minimum chunk size in characters",
    )
    max_chunk_size: Annotated[int, Field(gt=0, le=MAX_CHUNK_SIZE)] | None = Field(
        default=None,
        description="Maximum chunk size in characters",
    )


# ============================================================================
# Result Models
# ============================================================================


class LanguageStat(BaseModel):
    """Statistics for a single language."""

    model_config = ConfigDict(strict=True)

    language: str = Field(description="Language name")
    files: Annotated[int, Field(ge=0)] = Field(description="Number of files")
    lines: Annotated[int, Field(ge=0)] = Field(description="Total lines")
    percentage: Annotated[float, Field(ge=0, le=100)] = Field(
        description="Percentage of codebase"
    )


class ScanStats(BaseModel):
    """Statistics from scanning a repository."""

    model_config = ConfigDict(strict=True)

    name: str = Field(description="Repository name")
    total_files: Annotated[int, Field(ge=0)] = Field(description="Total number of files")
    total_lines: Annotated[int, Field(ge=0)] = Field(description="Total lines of code")
    total_tokens: Annotated[int, Field(ge=0)] = Field(
        description="Total tokens for target model"
    )
    primary_language: str | None = Field(
        default=None, description="Primary language"
    )
    languages: list[LanguageStat] = Field(description="Language breakdown")
    security_findings: Annotated[int, Field(ge=0)] = Field(
        description="Number of security findings"
    )


class SecurityFinding(BaseModel):
    """A security finding."""

    model_config = ConfigDict(strict=True)

    file: str = Field(description="File where the finding was detected")
    line: Annotated[int, Field(gt=0)] = Field(description="Line number (1-indexed)")
    severity: str = Field(description="Severity level")
    kind: str = Field(description="Type of finding")
    pattern: str = Field(description="Matched pattern")


class IndexStatus(BaseModel):
    """Index status information."""

    model_config = ConfigDict(strict=True)

    exists: bool = Field(description="Whether an index exists")
    file_count: Annotated[int, Field(ge=0)] = Field(description="Number of files indexed")
    symbol_count: Annotated[int, Field(ge=0)] = Field(
        description="Number of symbols indexed"
    )
    last_built: str | None = Field(
        default=None, description="Last build timestamp (ISO 8601)"
    )
    version: str | None = Field(default=None, description="Index version")
    files_updated: Annotated[int, Field(ge=0)] | None = Field(
        default=None, description="Number of files updated in incremental build"
    )
    incremental: bool | None = Field(
        default=None, description="Whether this was an incremental update"
    )


class SymbolInfo(BaseModel):
    """Information about a symbol in the call graph."""

    model_config = ConfigDict(strict=True)

    id: Annotated[int, Field(ge=0)] = Field(description="Symbol ID")
    name: str = Field(description="Symbol name")
    kind: str = Field(description="Symbol kind (function, class, method, etc.)")
    file: str = Field(description="File path containing the symbol")
    line: Annotated[int, Field(gt=0)] = Field(
        description="Start line number (1-indexed)"
    )
    end_line: Annotated[int, Field(gt=0)] = Field(
        description="End line number (1-indexed)"
    )
    signature: str | None = Field(
        default=None, description="Function/method signature"
    )
    visibility: str = Field(description="Visibility (public, private, etc.)")


class ReferenceInfo(BaseModel):
    """A reference to a symbol with context."""

    model_config = ConfigDict(strict=True)

    symbol: SymbolInfo = Field(description="Symbol making the reference")
    kind: str = Field(description="Reference kind (call, import, inherit, implement)")
    file: str = Field(description="File path containing the reference")
    line: Annotated[int, Field(gt=0)] = Field(description="Line number of the reference")


class CallGraphEdge(BaseModel):
    """An edge in the call graph."""

    model_config = ConfigDict(strict=True)

    caller_id: Annotated[int, Field(ge=0)] = Field(description="Caller symbol ID")
    callee_id: Annotated[int, Field(ge=0)] = Field(description="Callee symbol ID")
    caller: str = Field(description="Caller symbol name")
    callee: str = Field(description="Callee symbol name")
    file: str = Field(description="File containing the call site")
    line: Annotated[int, Field(gt=0)] = Field(description="Line number of the call")


class CallGraphStats(BaseModel):
    """Call graph statistics."""

    model_config = ConfigDict(strict=True)

    total_symbols: Annotated[int, Field(ge=0)] = Field(
        description="Total number of symbols"
    )
    total_calls: Annotated[int, Field(ge=0)] = Field(
        description="Total number of call edges"
    )
    functions: Annotated[int, Field(ge=0)] = Field(
        description="Number of functions/methods"
    )
    classes: Annotated[int, Field(ge=0)] = Field(
        description="Number of classes/structs"
    )


class CallGraph(BaseModel):
    """Complete call graph with nodes and edges."""

    model_config = ConfigDict(strict=True)

    nodes: list[SymbolInfo] = Field(description="All symbols (nodes)")
    edges: list[CallGraphEdge] = Field(description="Call relationships (edges)")
    stats: CallGraphStats = Field(description="Summary statistics")


class RepoChunk(BaseModel):
    """A chunk of repository content."""

    model_config = ConfigDict(strict=True)

    index: Annotated[int, Field(ge=0)] = Field(description="Chunk index (0-based)")
    total: Annotated[int, Field(gt=0)] = Field(description="Total number of chunks")
    focus: str = Field(description="Primary focus/topic of this chunk")
    tokens: Annotated[int, Field(ge=0)] = Field(description="Estimated token count")
    files: list[str] = Field(description="Files included in this chunk")
    content: str = Field(description="Formatted content of the chunk")


class AffectedSymbol(BaseModel):
    """Symbol affected by a change."""

    model_config = ConfigDict(strict=True)

    name: str = Field(description="Symbol name")
    kind: str = Field(description="Symbol kind (function, class, etc.)")
    file: str = Field(description="File containing the symbol")
    line: Annotated[int, Field(gt=0)] = Field(description="Line number")
    impact_type: str = Field(
        description='How the symbol is affected: "direct", "caller", "callee", "dependent"'
    )


class ImpactResult(BaseModel):
    """Impact analysis result."""

    model_config = ConfigDict(strict=True)

    changed_files: list[str] = Field(description="Files directly changed")
    dependent_files: list[str] = Field(
        description="Files that depend on changed files"
    )
    test_files: list[str] = Field(description="Related test files")
    affected_symbols: list[AffectedSymbol] = Field(
        description="Symbols affected by the changes"
    )
    impact_level: str = Field(
        description='Overall impact level: "low", "medium", "high", "critical"'
    )
    summary: str = Field(description="Summary of the impact")


class DiffFileContext(BaseModel):
    """A changed file with surrounding context."""

    model_config = ConfigDict(strict=True)

    path: str = Field(description="File path")
    change_type: str = Field(
        description='Change type: "Added", "Modified", "Deleted", "Renamed"'
    )
    additions: Annotated[int, Field(ge=0)] = Field(description="Lines added")
    deletions: Annotated[int, Field(ge=0)] = Field(description="Lines deleted")
    diff: str | None = Field(
        default=None, description="Unified diff content (if include_diff is true)"
    )
    context_snippets: list[str] = Field(
        description="Relevant code context around changes"
    )


class ContextSymbolInfo(BaseModel):
    """Symbol context information."""

    model_config = ConfigDict(strict=True)

    name: str = Field(description="Symbol name")
    kind: str = Field(description="Symbol kind")
    file: str = Field(description="File containing symbol")
    line: Annotated[int, Field(gt=0)] = Field(description="Line number")
    reason: str = Field(
        description='Why this symbol is included: "changed", "caller", "callee", "dependent"'
    )
    signature: str | None = Field(
        default=None, description="Symbol signature/definition"
    )


class DiffContextResult(BaseModel):
    """Context-aware diff result."""

    model_config = ConfigDict(strict=True)

    changed_files: list[DiffFileContext] = Field(
        description="Changed files with context"
    )
    context_symbols: list[ContextSymbolInfo] = Field(
        description="Related symbols and their context"
    )
    related_tests: list[str] = Field(description="Related test files")
    formatted_output: str | None = Field(
        default=None, description="Formatted output (if format specified)"
    )
    total_tokens: Annotated[int, Field(ge=0)] = Field(description="Total token count")


class EmbedChunkSource(BaseModel):
    """Source information for an embed chunk."""

    model_config = ConfigDict(strict=True)

    file: str = Field(description="File path")
    lines_start: Annotated[int, Field(gt=0)] = Field(
        description="Start line (1-indexed)"
    )
    lines_end: Annotated[int, Field(gt=0)] = Field(description="End line (1-indexed)")
    symbol: str = Field(description="Symbol name")
    fqn: str | None = Field(default=None, description="Fully qualified name")
    language: str = Field(description="Programming language")
    parent: str | None = Field(default=None, description="Parent symbol (if any)")
    visibility: str = Field(description="Visibility")
    is_test: bool = Field(description="Whether this is test code")


class EmbedChunkContext(BaseModel):
    """Context information for an embed chunk."""

    model_config = ConfigDict(strict=True)

    docstring: str | None = Field(
        default=None, description="Extracted docstring for natural language retrieval"
    )
    comments: list[str] = Field(description="Extracted comments within the chunk")
    signature: str | None = Field(
        default=None, description="Function/class signature"
    )
    calls: list[str] = Field(description="Symbols this chunk calls")
    called_by: list[str] = Field(description="Symbols that call this chunk")
    imports: list[str] = Field(description="Imports in this chunk")
    tags: list[str] = Field(
        description="Auto-generated semantic tags (async, security, database, etc.)"
    )
    lines_of_code: Annotated[int, Field(ge=0)] = Field(
        description="Lines of code (excluding blank lines and comments)"
    )
    max_nesting_depth: Annotated[int, Field(ge=0)] = Field(
        description="Maximum nesting depth (control flow, blocks)"
    )


class EmbedChunkPart(BaseModel):
    """Chunk part info for split chunks."""

    model_config = ConfigDict(strict=True)

    part: Annotated[int, Field(gt=0)] = Field(description="Part number (1-indexed)")
    of: Annotated[int, Field(gt=0)] = Field(description="Total number of parts")
    parent_id: str = Field(description="ID of the logical parent (full symbol hash)")
    parent_signature: str | None = Field(
        default=None, description="Signature repeated for context"
    )


class EmbedChunk(BaseModel):
    """A single embedding chunk."""

    model_config = ConfigDict(strict=True)

    id: str = Field(
        description="Content-addressable chunk ID (ec_ prefix + 32 hex chars)",
        pattern=r"^ec_[a-f0-9]{32}$",
    )
    full_hash: str = Field(description="Full content hash for collision detection")
    content: str = Field(description="Chunk content (code)")
    tokens: Annotated[int, Field(ge=0)] = Field(description="Token count")
    kind: str = Field(description="Chunk kind")
    source: EmbedChunkSource = Field(description="Source information")
    context: EmbedChunkContext = Field(description="Context information")
    part: EmbedChunkPart | None = Field(
        default=None, description="Part info (for multi-part chunks)"
    )


class EmbedDiffSummary(BaseModel):
    """Diff summary statistics for embedding."""

    model_config = ConfigDict(strict=True)

    added: Annotated[int, Field(ge=0)] = Field(description="Number of added chunks")
    modified: Annotated[int, Field(ge=0)] = Field(
        description="Number of modified chunks"
    )
    removed: Annotated[int, Field(ge=0)] = Field(description="Number of removed chunks")
    unchanged: Annotated[int, Field(ge=0)] = Field(
        description="Number of unchanged chunks"
    )
    total_chunks: Annotated[int, Field(ge=0)] = Field(
        description="Total chunks in current state"
    )


class EmbedResult(BaseModel):
    """Result from embedding operation."""

    model_config = ConfigDict(strict=True)

    chunks: list[EmbedChunk] = Field(description="Generated chunks")
    diff: EmbedDiffSummary | None = Field(
        default=None, description="Diff summary (if manifest existed)"
    )
    manifest_version: Annotated[int, Field(ge=0)] = Field(description="Manifest version")
    elapsed_ms: Annotated[int, Field(ge=0)] = Field(
        description="Processing time in milliseconds"
    )


# ============================================================================
# Git Models
# ============================================================================


class GitFileStatus(BaseModel):
    """File status information."""

    model_config = ConfigDict(strict=True)

    path: str = Field(description="File path")
    old_path: str | None = Field(default=None, description="Old path (for renames)")
    status: str = Field(
        description='Status: "Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown"'
    )


class GitChangedFile(BaseModel):
    """Changed file with diff stats."""

    model_config = ConfigDict(strict=True)

    path: str = Field(description="File path")
    old_path: str | None = Field(default=None, description="Old path (for renames)")
    status: str = Field(
        description='Status: "Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown"'
    )
    additions: Annotated[int, Field(ge=0)] = Field(description="Number of lines added")
    deletions: Annotated[int, Field(ge=0)] = Field(
        description="Number of lines deleted"
    )


class GitCommit(BaseModel):
    """Commit information."""

    model_config = ConfigDict(strict=True)

    hash: str = Field(description="Full commit hash")
    short_hash: str = Field(description="Short commit hash (7 characters)")
    author: str = Field(description="Author name")
    email: str = Field(description="Author email")
    date: str = Field(description="Commit date (ISO 8601 format)")
    message: str = Field(description="Commit message (first line)")


class GitBlameLine(BaseModel):
    """Blame line information."""

    model_config = ConfigDict(strict=True)

    commit: str = Field(description="Commit hash that introduced the line")
    author: str = Field(description="Author who wrote the line")
    date: str = Field(description="Date when line was written")
    line_number: Annotated[int, Field(gt=0)] = Field(
        description="Line number (1-indexed)"
    )


class GitDiffLine(BaseModel):
    """A single line change within a diff hunk."""

    model_config = ConfigDict(strict=True)

    change_type: str = Field(description='Type of change: "add", "remove", or "context"')
    old_line: Annotated[int, Field(gt=0)] | None = Field(
        default=None, description="Line number in the old file (null for additions)"
    )
    new_line: Annotated[int, Field(gt=0)] | None = Field(
        default=None, description="Line number in the new file (null for deletions)"
    )
    content: str = Field(description="The actual line content (without +/- prefix)")


class GitDiffHunk(BaseModel):
    """A diff hunk representing a contiguous block of changes."""

    model_config = ConfigDict(strict=True)

    old_start: Annotated[int, Field(gt=0)] = Field(
        description="Starting line in the old file"
    )
    old_count: Annotated[int, Field(ge=0)] = Field(
        description="Number of lines in the old file section"
    )
    new_start: Annotated[int, Field(gt=0)] = Field(
        description="Starting line in the new file"
    )
    new_count: Annotated[int, Field(ge=0)] = Field(
        description="Number of lines in the new file section"
    )
    header: str = Field(
        description='Header line (e.g., "@@ -1,5 +1,7 @@ function name")'
    )
    lines: list[GitDiffLine] = Field(
        description="Individual line changes within this hunk"
    )


# ============================================================================
# Validation Helpers
# ============================================================================


def validate_pack_options(data: dict[str, Any]) -> PackOptions:
    """Validate pack options from a dict.

    Args:
        data: Dictionary with pack options

    Returns:
        Validated PackOptions instance

    Raises:
        pydantic.ValidationError: If validation fails
    """
    return PackOptions.model_validate(data)


def validate_scan_options(data: dict[str, Any]) -> ScanOptions:
    """Validate scan options from a dict."""
    return ScanOptions.model_validate(data)


def validate_chunk_options(data: dict[str, Any]) -> ChunkOptions:
    """Validate chunk options from a dict."""
    return ChunkOptions.model_validate(data)


def validate_embed_options(data: dict[str, Any]) -> EmbedOptions:
    """Validate embed options from a dict."""
    return EmbedOptions.model_validate(data)


def validate_index_options(data: dict[str, Any]) -> IndexOptions:
    """Validate index options from a dict."""
    return IndexOptions.model_validate(data)


def validate_diff_context_options(data: dict[str, Any]) -> DiffContextOptions:
    """Validate diff context options from a dict."""
    return DiffContextOptions.model_validate(data)


def validate_impact_options(data: dict[str, Any]) -> ImpactOptions:
    """Validate impact options from a dict."""
    return ImpactOptions.model_validate(data)


__all__ = [
    # Type aliases
    "OutputFormat",
    "TokenizerModel",
    "CompressionLevel",
    "SecuritySeverity",
    "GitStatus",
    "SymbolKind",
    "Visibility",
    "ImpactType",
    "ImpactLevel",
    "ChangeType",
    "ChunkStrategy",
    "EmbedChunkKind",
    "ChangeSeverity",
    "ReferenceKind",
    "ContextReason",
    "Depth",
    # Option models
    "PackOptions",
    "ScanOptions",
    "ChunkOptions",
    "IndexOptions",
    "DiffContextOptions",
    "ImpactOptions",
    "EmbedOptions",
    "QueryFilter",
    "SymbolFilter",
    "CallGraphOptions",
    "SemanticCompressOptions",
    # Result models
    "LanguageStat",
    "ScanStats",
    "SecurityFinding",
    "IndexStatus",
    "SymbolInfo",
    "ReferenceInfo",
    "CallGraphEdge",
    "CallGraphStats",
    "CallGraph",
    "RepoChunk",
    "AffectedSymbol",
    "ImpactResult",
    "DiffFileContext",
    "ContextSymbolInfo",
    "DiffContextResult",
    "EmbedChunkSource",
    "EmbedChunkContext",
    "EmbedChunkPart",
    "EmbedChunk",
    "EmbedDiffSummary",
    "EmbedResult",
    # Git models
    "GitFileStatus",
    "GitChangedFile",
    "GitCommit",
    "GitBlameLine",
    "GitDiffLine",
    "GitDiffHunk",
    # Validation helpers
    "validate_pack_options",
    "validate_scan_options",
    "validate_chunk_options",
    "validate_embed_options",
    "validate_index_options",
    "validate_diff_context_options",
    "validate_impact_options",
]
