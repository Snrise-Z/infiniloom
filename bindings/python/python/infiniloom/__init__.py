"""
Infiniloom Python Bindings
==========================

Infiniloom is a repository context engine for Large Language Models (LLMs).
It analyzes codebases and generates optimized context for AI assistants.

Basic Usage
-----------

Functional API (quick and simple):

    >>> import infiniloom
    >>>
    >>> # Pack a repository into Claude-optimized XML
    >>> context = infiniloom.pack("/path/to/repo", format="xml", model="claude")
    >>>
    >>> # Scan a repository and get statistics
    >>> stats = infiniloom.scan("/path/to/repo")
    >>> print(f"Files: {stats['total_files']}")
    >>> print(f"Lines: {stats['total_lines']}")
    >>>
    >>> # Count tokens in text
    >>> tokens = infiniloom.count_tokens("Hello, world!", model="claude")
    >>> print(f"Tokens: {tokens}")

Object-Oriented API (more control):

    >>> from infiniloom import Infiniloom
    >>>
    >>> # Create an Infiniloom instance
    >>> loom = Infiniloom("/path/to/repo")
    >>>
    >>> # Get statistics
    >>> stats = loom.stats()
    >>> print(stats)
    >>>
    >>> # Pack the repository
    >>> context = loom.pack(format="xml", model="claude")
    >>>
    >>> # Get repository map with key symbols
    >>> repo_map = loom.map(map_budget=2000, max_symbols=50)
    >>> for symbol in repo_map['key_symbols']:
    ...     print(f"{symbol['name']} ({symbol['kind']}) - {symbol['file']}")
    >>>
    >>> # Scan for security issues
    >>> findings = loom.scan_security()
    >>> for finding in findings:
    ...     print(f"{finding['severity']}: {finding['message']}")
    >>>
    >>> # List all files
    >>> files = loom.files()
    >>> for file in files:
    ...     print(f"{file['path']} - {file['language']}")

Git Operations:

    >>> from infiniloom import GitRepo, is_git_repo
    >>>
    >>> # Check if path is a git repo
    >>> if is_git_repo("/path/to/repo"):
    ...     repo = GitRepo("/path/to/repo")
    ...     print(f"Branch: {repo.current_branch()}")
    ...     print(f"Commit: {repo.current_commit()}")
    ...
    ...     # Get recent commits
    ...     for commit in repo.log(count=5):
    ...         print(f"{commit['short_hash']}: {commit['message']}")
    ...
    ...     # Get file history
    ...     for commit in repo.file_log("src/main.py", count=5):
    ...         print(f"{commit['date']}: {commit['message']}")
    ...
    ...     # Get blame info
    ...     for line in repo.blame("src/main.py"):
    ...         print(f"Line {line['line_number']}: {line['author']}")

Semantic Compression:

    >>> import infiniloom
    >>>
    >>> # Compress long text while preserving meaning
    >>> long_text = "..." # Your long text
    >>> compressed = infiniloom.semantic_compress(
    ...     long_text,
    ...     similarity_threshold=0.7,  # How similar chunks need to be to group
    ...     budget_ratio=0.5           # Target 50% of original size
    ... )
    >>> print(f"Reduced from {len(long_text)} to {len(compressed)} chars")

Available Formats
-----------------

- **xml**: Claude-optimized XML format (default)
- **markdown**: GPT-optimized Markdown format
- **json**: Generic JSON format
- **yaml**: Gemini-optimized YAML format
- **toon**: Most token-efficient (~40% smaller than JSON)

Supported Models
----------------

- **claude**: Anthropic Claude (default)
- **gpt-5**, **gpt-5.1**, **gpt-5.2**: OpenAI GPT-5 series
- **gpt-4o**, **gpt-4o-mini**: OpenAI GPT-4o
- **o3**, **o1**: OpenAI reasoning models
- **gpt-4**: OpenAI GPT-4 (legacy)
- **gemini**: Google Gemini
- **llama**: Meta Llama

Compression Levels
------------------

- **none**: No compression
- **minimal**: Remove empty lines, trim whitespace (15% reduction)
- **balanced**: Remove comments, normalize whitespace (35% reduction, default)
- **aggressive**: Remove docstrings, keep signatures only (60% reduction)
- **extreme**: Key symbols only (80% reduction)
- **semantic**: AI-powered semantic compression (90% reduction)

Examples
--------

Generate context for different models:

    >>> import infiniloom
    >>>
    >>> # Claude (XML format)
    >>> claude_ctx = infiniloom.pack("/path/to/repo", format="xml", model="claude")
    >>>
    >>> # GPT (Markdown format)
    >>> gpt_ctx = infiniloom.pack("/path/to/repo", format="markdown", model="gpt")
    >>>
    >>> # Gemini (YAML format)
    >>> gemini_ctx = infiniloom.pack("/path/to/repo", format="yaml", model="gemini")
    >>>
    >>> # Most token-efficient (TOON format)
    >>> toon_ctx = infiniloom.pack("/path/to/repo", format="toon")

Advanced repository analysis:

    >>> from infiniloom import Infiniloom
    >>>
    >>> loom = Infiniloom("/path/to/my-project")
    >>>
    >>> # Get detailed statistics
    >>> stats = loom.stats()
    >>> print(f"Repository: {stats['name']}")
    >>> print(f"Total files: {stats['total_files']}")
    >>> print(f"Total lines: {stats['total_lines']}")
    >>> print(f"Claude tokens: {stats['tokens']['claude']}")
    >>>
    >>> # Get repository map with important symbols
    >>> repo_map = loom.map(map_budget=3000, max_symbols=100)
    >>> print(repo_map['summary'])
    >>>
    >>> # Find security issues
    >>> findings = loom.scan_security()
    >>> critical = [f for f in findings if f['severity'] == 'Critical']
    >>> print(f"Found {len(critical)} critical security issues")

Integration with LLM APIs:

    >>> import infiniloom
    >>> import anthropic  # or openai, etc.
    >>>
    >>> # Generate repository context
    >>> context = infiniloom.pack(
    ...     "/path/to/repo",
    ...     format="xml",
    ...     model="claude",
    ...     compression="balanced"
    ... )
    >>>
    >>> # Send to Claude
    >>> client = anthropic.Anthropic()
    >>> response = client.messages.create(
    ...     model="claude-sonnet-4-20250514",
    ...     max_tokens=4096,
    ...     messages=[{
    ...         "role": "user",
    ...         "content": f"{context}\\n\\nQuestion: Explain the architecture of this codebase."
    ...     }]
    ... )
    >>> print(response.content[0].text)
"""

from ._infiniloom import (
    # Core functions
    pack,
    scan,
    count_tokens,
    scan_security,
    semantic_compress,
    is_git_repo,
    # Index API
    build_index,
    index_status,
    # Call Graph API
    find_symbol,
    get_callers,
    get_callees,
    get_references,
    get_call_graph,
    # Chunk API
    chunk,
    # Impact & Diff Context API
    analyze_impact,
    get_diff_context,
    # Classes
    Infiniloom,
    GitRepo,
    # Exceptions
    InfiniloomError,
    # Version
    __version__,
)

from ._async import (
    pack_async,
    scan_async,
    count_tokens_async,
    scan_security_async,
    semantic_compress_async,
    build_index_async,
    chunk_async,
    analyze_impact_async,
    get_diff_context_async,
    # Call Graph async
    find_symbol_async,
    get_callers_async,
    get_callees_async,
    get_references_async,
    get_call_graph_async,
)

__all__ = [
    # Core functions
    "pack",
    "scan",
    "count_tokens",
    "scan_security",
    "semantic_compress",
    "is_git_repo",
    # Index API
    "build_index",
    "index_status",
    # Call Graph API
    "find_symbol",
    "get_callers",
    "get_callees",
    "get_references",
    "get_call_graph",
    # Chunk API
    "chunk",
    # Impact & Diff Context API
    "analyze_impact",
    "get_diff_context",
    # Async functions
    "pack_async",
    "scan_async",
    "count_tokens_async",
    "scan_security_async",
    "semantic_compress_async",
    "build_index_async",
    "chunk_async",
    "analyze_impact_async",
    "get_diff_context_async",
    # Call Graph Async functions
    "find_symbol_async",
    "get_callers_async",
    "get_callees_async",
    "get_references_async",
    "get_call_graph_async",
    # Classes
    "Infiniloom",
    "GitRepo",
    # Exceptions
    "InfiniloomError",
    # Version
    "__version__",
]
