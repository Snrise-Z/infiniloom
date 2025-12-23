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
- **gpt**: OpenAI GPT-4
- **gpt-4o**: OpenAI GPT-4o
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
    pack,
    scan,
    count_tokens,
    scan_security,
    Infiniloom,
    InfiniloomError,
    __version__,
)

__all__ = [
    # Functions
    "pack",
    "scan",
    "count_tokens",
    "scan_security",

    # Classes
    "Infiniloom",

    # Exceptions
    "InfiniloomError",

    # Version
    "__version__",
]
