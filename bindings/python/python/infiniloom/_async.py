"""
Async wrappers for Infiniloom functions.

This module provides async versions of the main Infiniloom functions,
allowing them to be used in async/await contexts without blocking
the event loop.

Usage
-----

    >>> import asyncio
    >>> import infiniloom
    >>>
    >>> async def main():
    ...     # Pack repository asynchronously
    ...     context = await infiniloom.pack_async("/path/to/repo", format="xml")
    ...
    ...     # Scan repository asynchronously
    ...     stats = await infiniloom.scan_async("/path/to/repo")
    ...
    ...     # Count tokens asynchronously
    ...     tokens = await infiniloom.count_tokens_async("Hello, world!")
    ...
    ...     print(f"Context length: {len(context)}")
    ...     print(f"Total files: {stats['total_files']}")
    ...     print(f"Tokens: {tokens}")
    >>>
    >>> asyncio.run(main())

All async functions use a thread pool executor to run the synchronous
Rust bindings without blocking the event loop.
"""

import asyncio
from concurrent.futures import ThreadPoolExecutor
from functools import partial
from typing import Optional, Dict, Any, List

from ._infiniloom import (
    pack,
    scan,
    count_tokens,
    scan_security,
    semantic_compress,
    build_index,
    chunk,
    analyze_impact,
    get_diff_context,
)

# Thread pool for running blocking operations
_executor = ThreadPoolExecutor(max_workers=4)


async def pack_async(
    path: str,
    format: str = "xml",
    model: str = "claude",
    compression: str = "balanced",
    map_budget: int = 2000,
    max_symbols: int = 50,
    redact_secrets: bool = True,
    skip_symbols: bool = False,
) -> str:
    """
    Pack a repository into an LLM-optimized format (async version).

    This is the async version of :func:`pack`. It runs the operation
    in a thread pool to avoid blocking the event loop.

    Args:
        path: Path to the repository
        format: Output format ("xml", "markdown", "json", "yaml", "toon", "plain")
        model: Target LLM model ("claude", "gpt-4o", "gpt-5", "gemini", etc.)
        compression: Compression level ("none", "minimal", "balanced", "aggressive", "extreme", "semantic")
        map_budget: Token budget for repository map (default: 2000)
        max_symbols: Maximum number of symbols to include (default: 50)
        redact_secrets: Redact detected secrets in output (default: True)
        skip_symbols: Skip symbol extraction for faster scanning (default: False)

    Returns:
        Formatted repository context as a string

    Example:
        >>> import asyncio
        >>> import infiniloom
        >>>
        >>> async def main():
        ...     context = await infiniloom.pack_async(
        ...         "/path/to/repo",
        ...         format="xml",
        ...         model="claude"
        ...     )
        ...     print(f"Generated {len(context)} characters")
        >>>
        >>> asyncio.run(main())
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(
        _executor,
        partial(
            pack,
            path,
            format=format,
            model=model,
            compression=compression,
            map_budget=map_budget,
            max_symbols=max_symbols,
            redact_secrets=redact_secrets,
            skip_symbols=skip_symbols,
        ),
    )


async def scan_async(
    path: str,
    include_hidden: bool = False,
    respect_gitignore: bool = True,
) -> Dict[str, Any]:
    """
    Scan a repository and return statistics (async version).

    This is the async version of :func:`scan`. It runs the operation
    in a thread pool to avoid blocking the event loop.

    Args:
        path: Path to the repository
        include_hidden: Include hidden files (default: False)
        respect_gitignore: Respect .gitignore files (default: True)

    Returns:
        Dictionary with repository statistics including:
        - name: Repository name
        - path: Repository path
        - total_files: Total number of files
        - total_lines: Total lines of code
        - tokens: Dict of token counts for different models

    Example:
        >>> import asyncio
        >>> import infiniloom
        >>>
        >>> async def main():
        ...     stats = await infiniloom.scan_async("/path/to/repo")
        ...     print(f"Files: {stats['total_files']}")
        ...     print(f"Lines: {stats['total_lines']}")
        >>>
        >>> asyncio.run(main())
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(
        _executor,
        partial(scan, path, include_hidden=include_hidden, respect_gitignore=respect_gitignore),
    )


async def count_tokens_async(
    text: str,
    model: str = "claude",
) -> int:
    """
    Count tokens in text for a specific model (async version).

    This is the async version of :func:`count_tokens`. It runs the operation
    in a thread pool to avoid blocking the event loop.

    Args:
        text: Text to tokenize
        model: Model name for tokenization (default: "claude")

    Returns:
        Number of tokens

    Example:
        >>> import asyncio
        >>> import infiniloom
        >>>
        >>> async def main():
        ...     tokens = await infiniloom.count_tokens_async("Hello, world!")
        ...     print(f"Tokens: {tokens}")
        >>>
        >>> asyncio.run(main())
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(
        _executor,
        partial(count_tokens, text, model=model),
    )


async def scan_security_async(path: str) -> List[Dict[str, Any]]:
    """
    Scan a repository for security issues (async version).

    This is the async version of :func:`scan_security`. It runs the operation
    in a thread pool to avoid blocking the event loop.

    Args:
        path: Path to the repository

    Returns:
        List of security findings, each with:
        - file: File path
        - line: Line number
        - severity: Severity level
        - kind: Type of finding
        - pattern: Matched pattern

    Example:
        >>> import asyncio
        >>> import infiniloom
        >>>
        >>> async def main():
        ...     findings = await infiniloom.scan_security_async("/path/to/repo")
        ...     for finding in findings:
        ...         print(f"{finding['severity']}: {finding['kind']} in {finding['file']}")
        >>>
        >>> asyncio.run(main())
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(_executor, partial(scan_security, path))


async def semantic_compress_async(
    text: str,
    similarity_threshold: float = 0.7,
    budget_ratio: float = 0.5,
) -> str:
    """
    Compress text using semantic compression (async version).

    This is the async version of :func:`semantic_compress`. It runs the operation
    in a thread pool to avoid blocking the event loop.

    Args:
        text: Text to compress
        similarity_threshold: Threshold for grouping similar chunks (0.0-1.0, default: 0.7)
        budget_ratio: Target size as ratio of original (0.0-1.0, default: 0.5)

    Returns:
        Compressed text

    Example:
        >>> import asyncio
        >>> import infiniloom
        >>>
        >>> async def main():
        ...     long_text = "..." # Your long text
        ...     compressed = await infiniloom.semantic_compress_async(
        ...         long_text,
        ...         similarity_threshold=0.7,
        ...         budget_ratio=0.3
        ...     )
        ...     print(f"Reduced to {len(compressed)} chars")
        >>>
        >>> asyncio.run(main())
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(
        _executor,
        partial(
            semantic_compress,
            text,
            similarity_threshold=similarity_threshold,
            budget_ratio=budget_ratio,
        ),
    )


async def build_index_async(
    path: str,
    force: bool = False,
    include_tests: bool = False,
    max_file_size: Optional[int] = None,
) -> Dict[str, Any]:
    """
    Build or update the symbol index for a repository (async version).

    This is the async version of :func:`build_index`. It runs the operation
    in a thread pool to avoid blocking the event loop.

    Args:
        path: Path to repository root
        force: Force full rebuild even if index exists (default: False)
        include_tests: Include test files in index (default: False)
        max_file_size: Maximum file size to index in bytes (default: 10MB)

    Returns:
        Dictionary with index status: exists, file_count, symbol_count, last_built, version

    Example:
        >>> import asyncio
        >>> import infiniloom
        >>>
        >>> async def main():
        ...     status = await infiniloom.build_index_async("/path/to/repo")
        ...     print(f"Indexed {status['symbol_count']} symbols")
        >>>
        >>> asyncio.run(main())
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(
        _executor,
        partial(
            build_index,
            path,
            force=force,
            include_tests=include_tests,
            max_file_size=max_file_size,
        ),
    )


async def chunk_async(
    path: str,
    strategy: str = "module",
    max_tokens: int = 8000,
    overlap: int = 0,
    model: str = "claude",
    priority_first: bool = False,
) -> List[Dict[str, Any]]:
    """
    Split a repository into chunks for incremental processing (async version).

    This is the async version of :func:`chunk`. It runs the operation
    in a thread pool to avoid blocking the event loop.

    Args:
        path: Path to repository root
        strategy: Chunking strategy - "fixed", "file", "module", "symbol", "semantic", "dependency"
        max_tokens: Maximum tokens per chunk (default: 8000)
        overlap: Token overlap between chunks (default: 0)
        model: Target model for token counting (default: "claude")
        priority_first: Sort chunks by priority, core modules first (default: False)

    Returns:
        List of chunk dictionaries with: index, total, focus, tokens, files, content

    Example:
        >>> import asyncio
        >>> import infiniloom
        >>>
        >>> async def main():
        ...     chunks = await infiniloom.chunk_async("/path/to/repo", strategy="module")
        ...     for c in chunks:
        ...         print(f"Chunk {c['index']}/{c['total']}: {c['focus']}")
        >>>
        >>> asyncio.run(main())
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(
        _executor,
        partial(
            chunk,
            path,
            strategy=strategy,
            max_tokens=max_tokens,
            overlap=overlap,
            model=model,
            priority_first=priority_first,
        ),
    )


async def analyze_impact_async(
    path: str,
    files: List[str],
    depth: int = 2,
    include_tests: bool = False,
) -> Dict[str, Any]:
    """
    Analyze the impact of changes to files or symbols (async version).

    This is the async version of :func:`analyze_impact`. It runs the operation
    in a thread pool to avoid blocking the event loop.

    Requires an index to be built first (use build_index_async).

    Args:
        path: Path to repository root
        files: List of files to analyze
        depth: Depth of dependency traversal (1-3, default: 2)
        include_tests: Include test files in analysis (default: False)

    Returns:
        Dictionary with: changed_files, dependent_files, test_files, affected_symbols, impact_level, summary

    Example:
        >>> import asyncio
        >>> import infiniloom
        >>>
        >>> async def main():
        ...     await infiniloom.build_index_async("/path/to/repo")
        ...     impact = await infiniloom.analyze_impact_async("/path/to/repo", ["src/auth.py"])
        ...     print(f"Impact level: {impact['impact_level']}")
        >>>
        >>> asyncio.run(main())
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(
        _executor,
        partial(
            analyze_impact,
            path,
            files,
            depth=depth,
            include_tests=include_tests,
        ),
    )


async def get_diff_context_async(
    path: str,
    from_ref: str = "",
    to_ref: str = "HEAD",
    depth: int = 2,
    budget: int = 50000,
    include_diff: bool = False,
) -> Dict[str, Any]:
    """
    Get context-aware diff with surrounding symbols and dependencies (async version).

    This is the async version of :func:`get_diff_context`. It runs the operation
    in a thread pool to avoid blocking the event loop.

    Unlike basic git diff, this provides semantic context around changes.
    Requires an index for full functionality (will work with limited context without one).

    Args:
        path: Path to repository root
        from_ref: Starting commit/branch (use "" for unstaged changes)
        to_ref: Ending commit/branch (use "HEAD" for staged, "" for working tree)
        depth: Depth of context expansion (1-3, default: 2)
        budget: Token budget for context (default: 50000)
        include_diff: Include the actual diff content (default: False)

    Returns:
        Dictionary with: changed_files, context_symbols, related_tests, total_tokens

    Example:
        >>> import asyncio
        >>> import infiniloom
        >>>
        >>> async def main():
        ...     context = await infiniloom.get_diff_context_async("/path/to/repo", "HEAD~1", "HEAD")
        ...     print(f"Changed: {len(context['changed_files'])} files")
        >>>
        >>> asyncio.run(main())
    """
    loop = asyncio.get_event_loop()
    return await loop.run_in_executor(
        _executor,
        partial(
            get_diff_context,
            path,
            from_ref=from_ref,
            to_ref=to_ref,
            depth=depth,
            budget=budget,
            include_diff=include_diff,
        ),
    )


__all__ = [
    "pack_async",
    "scan_async",
    "count_tokens_async",
    "scan_security_async",
    "semantic_compress_async",
    "build_index_async",
    "chunk_async",
    "analyze_impact_async",
    "get_diff_context_async",
]
