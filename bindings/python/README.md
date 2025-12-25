# Infiniloom Python Bindings

Python bindings for [Infiniloom](https://github.com/Topos-Labs/infiniloom) - a repository context engine for Large Language Models.

## Installation

```bash
pip install infiniloom
```

### Building from Source

```bash
git clone https://github.com/Topos-Labs/infiniloom.git
cd infiniloom/bindings/python
pip install maturin
maturin develop  # For development
maturin build --release  # For production wheel
```

## Quick Start

### Functional API

```python
import infiniloom

# Pack a repository into Claude-optimized XML
context = infiniloom.pack("/path/to/repo", format="xml", model="claude")
print(context)

# Scan repository and get statistics
stats = infiniloom.scan("/path/to/repo")
print(f"Files: {stats['total_files']}")
print(f"Languages: {stats['languages']}")

# Count tokens for a specific model
tokens = infiniloom.count_tokens("Hello, world!", model="claude")
print(f"Tokens: {tokens}")
```

### Object-Oriented API

```python
from infiniloom import Infiniloom

# Create an Infiniloom instance
loom = Infiniloom("/path/to/repo")

# Get repository statistics
stats = loom.stats()
print(stats)

# Generate repository context
context = loom.pack(format="xml", model="claude", compression="balanced")

# Get repository map with important symbols
repo_map = loom.map(map_budget=2000, max_symbols=50)
for symbol in repo_map['key_symbols']:
    print(f"{symbol['name']} ({symbol['kind']}) in {symbol['file']}")

# Scan for security issues
findings = loom.scan_security()
for finding in findings:
    print(f"{finding['severity']}: {finding['message']} at {finding['file']}:{finding['line']}")

# List all files
files = loom.files()
for file in files:
    print(f"{file['path']} - {file['language']} ({file['tokens']} tokens)")
```

## API Reference

### Functions

#### `pack(path, format="xml", model="claude", compression="balanced", map_budget=2000, max_symbols=50)`

Pack a repository into an LLM-optimized format.

**Parameters:**
- `path` (str): Path to the repository
- `format` (str): Output format - "xml", "markdown", "json", "yaml", "toon", or "plain"
- `model` (str): Target model for token counting. Supports:
  - OpenAI GPT-5.x: "gpt-5.2", "gpt-5.2-pro", "gpt-5.1", "gpt-5.1-mini", "gpt-5.1-codex", "gpt-5", "gpt-5-mini", "gpt-5-nano"
  - OpenAI O-series: "o4-mini", "o3", "o3-mini", "o1", "o1-mini", "o1-preview"
  - OpenAI GPT-4: "gpt-4o", "gpt-4o-mini", "gpt-4", "gpt-3.5-turbo"
  - Anthropic: "claude" (default)
  - Google: "gemini"
  - Meta: "llama", "codellama"
  - Others: "deepseek", "mistral", "qwen", "cohere", "grok"
- `compression` (str): Compression level - "none", "minimal", "balanced", "aggressive", "extreme", "focused", "semantic"
- `map_budget` (int): Token budget for repository map (default: 2000)
- `max_symbols` (int): Maximum symbols to include (default: 50)

**Returns:** str - Formatted repository context

#### `scan(path, include_hidden=False, respect_gitignore=True)`

Scan a repository and return statistics.

**Parameters:**
- `path` (str): Path to the repository
- `include_hidden` (bool): Include hidden files (default: False)
- `respect_gitignore` (bool): Respect .gitignore files (default: True)

**Returns:** dict - Repository statistics including:
- `name`: Repository name
- `path`: Absolute path
- `total_files`: Number of files
- `total_lines`: Total lines of code
- `total_tokens`: Token counts for each model
- `languages`: Language breakdown
- `branch`: Git branch (if available)
- `commit`: Git commit hash (if available)

#### `count_tokens(text, model="claude")`

Count tokens in text for a specific model.

**Parameters:**
- `text` (str): Text to count tokens for
- `model` (str): Target model. Supports all models listed above in `pack()`, including GPT-5.x series

**Returns:** int - Number of tokens (exact for OpenAI models via tiktoken, calibrated estimates for others)

#### `semantic_compress(text, similarity_threshold=0.7, budget_ratio=0.5)`

Compress text using semantic compression while preserving important content.

**Parameters:**
- `text` (str): Text to compress
- `similarity_threshold` (float): Threshold for grouping similar chunks (0.0-1.0, default: 0.7). Note: Only affects output when built with "embeddings" feature.
- `budget_ratio` (float): Target size as ratio of original (0.0-1.0, default: 0.5). Lower values = more aggressive compression.

**Returns:** str - Compressed text

```python
import infiniloom

long_text = "... your long text content ..."
compressed = infiniloom.semantic_compress(long_text, budget_ratio=0.3)
print(compressed)
```

#### `scan_security(path)`

Scan repository for security issues.

**Parameters:**
- `path` (str): Path to the repository

**Returns:** list[dict] - List of security findings with:
- `file`: File path
- `line`: Line number
- `severity`: Severity level ("Critical", "High", "Medium", "Low", "Info")
- `kind`: Type of finding (e.g., "aws_access_key", "github_token")
- `pattern`: The matched pattern

#### `is_git_repo(path)`

Check if a path is a git repository.

**Parameters:**
- `path` (str): Path to check

**Returns:** bool - True if path is a git repository, False otherwise

```python
from infiniloom import is_git_repo

if is_git_repo("/path/to/repo"):
    print("This is a git repository")
```

### Call Graph API

Query caller/callee relationships and navigate your codebase programmatically.

#### `build_index(path, force=False, include_tests=False, max_file_size=None)`

Build or update the symbol index for a repository (required for call graph queries).

**Parameters:**
- `path` (str): Path to repository root
- `force` (bool): Force full rebuild even if index exists (default: False)
- `include_tests` (bool): Include test files in index (default: False)
- `max_file_size` (int): Maximum file size to index in bytes (default: 10MB)

**Returns:** dict - Index status with `exists`, `file_count`, `symbol_count`, `last_built` (ISO 8601 timestamp), `version`

```python
import infiniloom

status = infiniloom.build_index("/path/to/repo")
print(f"Indexed {status['symbol_count']} symbols")
```

#### `find_symbol(path, name)`

Find symbols by name in the index.

**Parameters:**
- `path` (str): Path to repository root
- `name` (str): Symbol name to search for

**Returns:** list[dict] - List of matching symbols with `id`, `name`, `kind`, `file`, `line`, `end_line`, `signature`, `visibility`

```python
import infiniloom

infiniloom.build_index("/path/to/repo")
symbols = infiniloom.find_symbol("/path/to/repo", "process_request")
for s in symbols:
    print(f"{s['name']} ({s['kind']}) at {s['file']}:{s['line']}")
```

#### `get_callers(path, symbol_name)`

Get all functions/methods that call the target symbol.

**Parameters:**
- `path` (str): Path to repository root
- `symbol_name` (str): Name of the symbol to find callers for

**Returns:** list[dict] - List of symbols that call the target

```python
import infiniloom

infiniloom.build_index("/path/to/repo")
callers = infiniloom.get_callers("/path/to/repo", "authenticate")
print(f"authenticate is called by {len(callers)} functions")
for c in callers:
    print(f"  {c['name']} at {c['file']}:{c['line']}")
```

#### `get_callees(path, symbol_name)`

Get all functions/methods that the target symbol calls.

**Parameters:**
- `path` (str): Path to repository root
- `symbol_name` (str): Name of the symbol to find callees for

**Returns:** list[dict] - List of symbols that the target calls

```python
import infiniloom

infiniloom.build_index("/path/to/repo")
callees = infiniloom.get_callees("/path/to/repo", "main")
print(f"main calls {len(callees)} functions")
```

#### `get_references(path, symbol_name)`

Get all references to a symbol (calls, imports, inheritance).

**Parameters:**
- `path` (str): Path to repository root
- `symbol_name` (str): Name of the symbol to find references for

**Returns:** list[dict] - List of references with `symbol` (dict) and `kind` (str)

```python
import infiniloom

infiniloom.build_index("/path/to/repo")
refs = infiniloom.get_references("/path/to/repo", "UserService")
for r in refs:
    print(f"{r['kind']}: {r['symbol']['name']} at {r['symbol']['file']}:{r['symbol']['line']}")
```

#### `get_call_graph(path, max_nodes=None, max_edges=None)`

Get the complete call graph with all symbols and call relationships.

**Parameters:**
- `path` (str): Path to repository root
- `max_nodes` (int): Maximum number of nodes to return (default: unlimited)
- `max_edges` (int): Maximum number of edges to return (default: unlimited)

**Returns:** dict - Call graph with `nodes` (list), `edges` (list), `stats` (dict)

```python
import infiniloom

infiniloom.build_index("/path/to/repo")
graph = infiniloom.get_call_graph("/path/to/repo")
print(f"{graph['stats']['total_symbols']} symbols, {graph['stats']['total_calls']} calls")

# Find most called functions
from collections import Counter
call_counts = Counter(edge['callee'] for edge in graph['edges'])
print("Most called:", call_counts.most_common(5))
```

#### Async versions

All call graph functions have async versions:
- `find_symbol_async(path, name)`
- `get_callers_async(path, symbol_name)`
- `get_callees_async(path, symbol_name)`
- `get_references_async(path, symbol_name)`
- `get_call_graph_async(path, max_nodes=None, max_edges=None)`

```python
import asyncio
import infiniloom

async def analyze_codebase():
    await infiniloom.build_index_async("/path/to/repo")
    callers = await infiniloom.get_callers_async("/path/to/repo", "authenticate")
    print(f"Found {len(callers)} callers")

asyncio.run(analyze_codebase())
```

#### `index_status(path)`

Get the status of an existing index.

**Parameters:**
- `path` (str): Path to repository root

**Returns:** dict - Index status with `exists`, `file_count`, `symbol_count`, `last_built` (ISO 8601 timestamp), `version`

```python
import infiniloom

status = infiniloom.index_status("/path/to/repo")
if status["exists"]:
    print(f"Index has {status['symbol_count']} symbols")
else:
    print("No index found - run build_index first")
```

### Chunking API

Split large repositories into manageable chunks for multi-turn LLM conversations.

#### `chunk(path, strategy="module", max_tokens=8000, overlap=0, model="claude", priority_first=False)`

Split repository into chunks for processing with limited context windows.

**Parameters:**
- `path` (str): Path to repository root
- `strategy` (str): Chunking strategy - "fixed", "file", "module", "symbol", "semantic", "dependency"
- `max_tokens` (int): Maximum tokens per chunk (default: 8000)
- `overlap` (int): Token overlap between chunks (default: 0)
- `model` (str): Target model for token counting (default: "claude")
- `priority_first` (bool): Sort chunks by file priority (default: False)

**Returns:** list[dict] - List of chunks with `index`, `total`, `focus`, `tokens`, `files`, `content`

```python
import infiniloom

# Split large repo into manageable chunks
chunks = infiniloom.chunk("/path/to/large-repo", strategy="module", max_tokens=50000)

for c in chunks:
    print(f"Chunk {c['index']+1}/{c['total']}: {c['focus']} ({c['tokens']} tokens)")
    # Send c['content'] to LLM for analysis

# Use dependency-aware chunking for better context
chunks = infiniloom.chunk("/path/to/repo", strategy="dependency", priority_first=True)
```

**Strategies:**
- `fixed`: Split at fixed token boundaries
- `file`: One file per chunk
- `module`: Group by module/directory
- `symbol`: Group by symbol (function/class)
- `semantic`: Group by semantic similarity
- `dependency`: Group by dependency relationships

### Impact Analysis API

Analyze the impact of changes to understand what code is affected.

#### `analyze_impact(path, files, depth=2, include_tests=False, model=None, exclude=None, include=None)`

Analyze the impact of changes to files or symbols.

**Parameters:**
- `path` (str): Path to repository root
- `files` (list[str]): List of files to analyze
- `depth` (int): Depth of dependency traversal (1-3, default: 2)
- `include_tests` (bool): Include test files in analysis (default: False)
- `model` (str, optional): Target model for token counting (default: "claude")
- `exclude` (list[str], optional): Glob patterns to exclude (e.g., ["**/*.test.py", "dist/**"])
- `include` (list[str], optional): Glob patterns to include (e.g., ["src/**/*.py"])

**Returns:** dict - Impact analysis with:
- `changed_files`: List of files being analyzed
- `dependent_files`: Files that depend on changed files
- `test_files`: Related test files
- `affected_symbols`: List of affected symbols with name, kind, file, line, impact_type
- `impact_level`: Impact severity ("low", "medium", "high", "critical")
- `summary`: Human-readable summary

```python
import infiniloom

# Build index first
infiniloom.build_index("/path/to/repo")

# Analyze impact of changing a file
impact = infiniloom.analyze_impact("/path/to/repo", ["src/auth.py"])
print(f"Impact level: {impact['impact_level']}")
print(f"Summary: {impact['summary']}")

# See what else needs updating
for dep in impact['dependent_files']:
    print(f"  Dependent: {dep}")

for sym in impact['affected_symbols']:
    print(f"  {sym['impact_type']}: {sym['name']} in {sym['file']}")
```

### Diff Context API

Get semantic context around code changes for AI-powered code review.

#### `get_diff_context(path, from_ref="", to_ref="HEAD", depth=2, budget=50000, include_diff=False, model=None, exclude=None, include=None)`

Get context-aware diff with surrounding symbols and dependencies.

**Parameters:**
- `path` (str): Path to repository root
- `from_ref` (str): Starting ref - "" for unstaged, "HEAD" for staged, commit hash, branch name
- `to_ref` (str): Ending ref - "HEAD", commit hash, branch name
- `depth` (int): Context expansion depth (1-3, default: 2)
- `budget` (int): Token budget for context (default: 50000)
- `include_diff` (bool): Include actual diff content (default: False)
- `model` (str, optional): Target model for token counting (default: "claude")
- `exclude` (list[str], optional): Glob patterns to exclude (e.g., ["**/*.test.py", "dist/**"])
- `include` (list[str], optional): Glob patterns to include (e.g., ["src/**/*.py"])

**Returns:** dict - Diff context with:
- `changed_files`: List of changed files with path, change_type, additions, deletions, diff (if requested)
- `context_symbols`: Related symbols with name, kind, file, line, reason, signature
- `related_tests`: List of related test file paths
- `total_tokens`: Estimated token count

```python
import infiniloom

# Build index for full context (optional but recommended)
infiniloom.build_index("/path/to/repo")

# Get context for uncommitted changes
context = infiniloom.get_diff_context("/path/to/repo")
print(f"Changed: {len(context['changed_files'])} files")

# Get context for last commit with diff content
context = infiniloom.get_diff_context(
    "/path/to/repo",
    from_ref="HEAD~1",
    to_ref="HEAD",
    include_diff=True
)
for f in context['changed_files']:
    print(f"{f['change_type']}: {f['path']}")
    if 'diff' in f:
        print(f['diff'])

# Get context for a PR (branch comparison)
context = infiniloom.get_diff_context(
    "/path/to/repo",
    from_ref="main",
    to_ref="feature-branch",
    depth=3
)
print(f"Related symbols: {len(context['context_symbols'])}")
print(f"Related tests: {len(context['related_tests'])}")
```

### Classes

#### `Infiniloom(path)`

Object-oriented interface for repository analysis.

**Methods:**

##### `load(include_hidden=False, respect_gitignore=True)`

Load the repository into memory.

##### `stats()`

Get repository statistics. Returns same structure as `scan()` function.

##### `pack(format="xml", model="claude", compression="balanced", map_budget=2000)`

Pack the repository. Returns formatted string.

##### `map(map_budget=2000, max_symbols=50)`

Get repository map with key symbols. Returns dict with:
- `summary`: Text summary
- `token_count`: Estimated tokens
- `key_symbols`: List of important symbols

##### `scan_security()`

Scan for security issues. Returns list of findings.

##### `files()`

Get list of all files. Returns list of dicts with file metadata.

#### `GitRepo(path)`

Git repository wrapper for accessing git operations like status, diff, log, and blame.

**Constructor:**
- `path` (str): Path to the git repository

**Raises:** `InfiniloomError` if path is not a git repository

**Methods:**

##### `current_branch()`

Get the current branch name.

**Returns:** str - Current branch name (e.g., "main", "feature/xyz")

##### `current_commit()`

Get the current commit hash.

**Returns:** str - Full SHA-1 hash of HEAD commit (40 characters)

##### `status()`

Get working tree status (both staged and unstaged changes).

**Returns:** list[dict] - List of file status objects with:
- `path`: File path
- `status`: Status type ("Added", "Modified", "Deleted", "Renamed", "Copied", "Unknown")
- `old_path`: Old path for renames (optional)

##### `log(count=10)`

Get recent commits.

**Parameters:**
- `count` (int): Maximum number of commits to return (default: 10)

**Returns:** list[dict] - List of commit objects with:
- `hash`: Full commit hash
- `short_hash`: Short commit hash (7 characters)
- `author`: Author name
- `email`: Author email
- `date`: Commit date (ISO 8601 format)
- `message`: Commit message (first line)

##### `file_log(path, count=10)`

Get commits that modified a specific file.

**Parameters:**
- `path` (str): File path relative to repo root
- `count` (int): Maximum number of commits to return (default: 10)

**Returns:** list[dict] - List of commits that modified the file

##### `blame(path)`

Get blame information for a file.

**Parameters:**
- `path` (str): File path relative to repo root

**Returns:** list[dict] - List of blame line objects with:
- `commit`: Commit hash that introduced the line
- `author`: Author who wrote the line
- `date`: Date when line was written
- `line_number`: Line number (1-indexed)

##### `ls_files()`

Get list of files tracked by git.

**Returns:** list[str] - Array of file paths tracked by git

##### `diff_files(from_ref, to_ref)`

Get files changed between two commits.

**Parameters:**
- `from_ref` (str): Starting commit/branch/tag
- `to_ref` (str): Ending commit/branch/tag

**Returns:** list[dict] - List of changed files with:
- `path`: File path
- `status`: Status ("Added", "Modified", "Deleted", "Renamed", "Copied")
- `additions`: Number of lines added
- `deletions`: Number of lines deleted

##### `uncommitted_diff(path)`

Get diff content for uncommitted changes in a file.

**Parameters:**
- `path` (str): File path relative to repo root

**Returns:** str - Unified diff content

##### `all_uncommitted_diffs()`

Get diff for all uncommitted changes.

**Returns:** str - Combined unified diff for all changed files

##### `has_changes(path)`

Check if a file has uncommitted changes.

**Parameters:**
- `path` (str): File path relative to repo root

**Returns:** bool - True if file has changes

##### `last_modified_commit(path)`

Get the last commit that modified a file.

**Parameters:**
- `path` (str): File path relative to repo root

**Returns:** dict - Commit information object

##### `file_change_frequency(path, days=30)`

Get file change frequency in recent days.

**Parameters:**
- `path` (str): File path relative to repo root
- `days` (int): Number of days to look back (default: 30)

**Returns:** int - Number of commits that modified the file in the period

##### `file_at_ref(path, git_ref)`

Get file content at a specific git ref (commit, branch, tag).

**Parameters:**
- `path` (str): File path relative to repo root
- `git_ref` (str): Git ref (commit hash, branch name, tag, HEAD~n, etc.)

**Returns:** str - File content

```python
repo = GitRepo("/path/to/repo")
old_version = repo.file_at_ref("src/main.py", "HEAD~5")
main_version = repo.file_at_ref("src/main.py", "main")
```

##### `diff_hunks(from_ref, to_ref, path=None)`

Parse diff between two refs into structured hunks with line-level changes.
Useful for PR review tools that need to post comments at specific lines.

**Parameters:**
- `from_ref` (str): Starting ref (e.g., "main", "HEAD~5", commit hash)
- `to_ref` (str): Ending ref (e.g., "HEAD", "feature-branch")
- `path` (str, optional): File path to filter to a single file

**Returns:** list[dict] - List of diff hunks with:
- `old_start`: Starting line in old file
- `old_count`: Number of lines in old file
- `new_start`: Starting line in new file
- `new_count`: Number of lines in new file
- `header`: Hunk header
- `lines`: List of line dicts with `change_type`, `old_line`, `new_line`, `content`

```python
repo = GitRepo("/path/to/repo")
hunks = repo.diff_hunks("main", "HEAD", "src/index.py")
for hunk in hunks:
    print(f"Hunk at old:{hunk['old_start']} new:{hunk['new_start']}")
    for line in hunk['lines']:
        print(f"{line['change_type']}: {line['content']}")
```

##### `uncommitted_hunks(path=None)`

Parse uncommitted changes (working tree vs HEAD) into structured hunks.

**Parameters:**
- `path` (str, optional): File path to filter to a single file

**Returns:** list[dict] - List of diff hunks for uncommitted changes

##### `staged_hunks(path=None)`

Parse staged changes into structured hunks.

**Parameters:**
- `path` (str, optional): File path to filter to a single file

**Returns:** list[dict] - List of diff hunks for staged changes only

**Example:**

```python
from infiniloom import GitRepo, is_git_repo

# Check if path is a git repo first
if is_git_repo("/path/to/repo"):
    repo = GitRepo("/path/to/repo")

    # Get current state
    print(f"Branch: {repo.current_branch()}")
    print(f"Commit: {repo.current_commit()}")

    # Get recent commits
    for commit in repo.log(count=5):
        print(f"{commit['short_hash']}: {commit['message']}")

    # Get file history
    for commit in repo.file_log("src/main.py", count=3):
        print(f"{commit['date']}: {commit['message']}")

    # Get blame information
    for line in repo.blame("src/main.py")[:10]:
        print(f"Line {line['line_number']}: {line['author']}")

    # Check for uncommitted changes
    if repo.has_changes("src/main.py"):
        diff = repo.uncommitted_diff("src/main.py")
        print(diff)
```

### Async Functions

Infiniloom provides async versions of the main functions for use in async/await contexts.
These use a thread pool executor to avoid blocking the event loop.

```python
import asyncio
import infiniloom

async def main():
    # Pack repository asynchronously
    context = await infiniloom.pack_async("/path/to/repo", format="xml", model="claude")

    # Scan repository asynchronously
    stats = await infiniloom.scan_async("/path/to/repo")

    # Count tokens asynchronously
    tokens = await infiniloom.count_tokens_async("Hello, world!", model="claude")

    # Scan security asynchronously
    findings = await infiniloom.scan_security_async("/path/to/repo")

    # Semantic compress asynchronously
    compressed = await infiniloom.semantic_compress_async(long_text, budget_ratio=0.3)

asyncio.run(main())
```

#### Available Async Functions

- `pack_async(path, format="xml", model="claude", compression="balanced", ...)` - Async pack
- `scan_async(path, include_hidden=False, respect_gitignore=True)` - Async scan
- `count_tokens_async(text, model="claude")` - Async token counting
- `scan_security_async(path)` - Async security scanning
- `semantic_compress_async(text, similarity_threshold=0.7, budget_ratio=0.5)` - Async compression

## Formats

### XML (Claude-optimized)

Best for Claude models. Uses XML structure that Claude understands well.

```python
context = infiniloom.pack("/path/to/repo", format="xml", model="claude")
```

### Markdown (GPT-optimized)

Best for GPT models. Uses Markdown with clear hierarchical structure.

```python
context = infiniloom.pack("/path/to/repo", format="markdown", model="gpt")
```

### JSON

Generic JSON format for programmatic processing.

```python
context = infiniloom.pack("/path/to/repo", format="json")
```

### YAML (Gemini-optimized)

Best for Gemini. Query should be placed at the end.

```python
context = infiniloom.pack("/path/to/repo", format="yaml", model="gemini")
```

### TOON (Token-Efficient)

Most token-efficient format (~40% smaller than JSON). Best for limited context windows.

```python
context = infiniloom.pack("/path/to/repo", format="toon")
```

## Compression Levels

- **none**: No compression (0% reduction)
- **minimal**: Remove empty lines, trim whitespace (15% reduction)
- **balanced**: Remove comments, normalize whitespace (35% reduction) - Default
- **aggressive**: Remove docstrings, keep signatures only (60% reduction)
- **extreme**: Key symbols only (80% reduction)
- **focused**: Key symbols with small context (75% reduction)
- **semantic**: Heuristic semantic compression (~60-70% reduction)

## Integration Examples

### With Anthropic Claude

```python
import infiniloom
import anthropic

# Generate context
context = infiniloom.pack(
    "/path/to/repo",
    format="xml",
    model="claude",
    compression="balanced"
)

# Send to Claude
client = anthropic.Anthropic()
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=4096,
    messages=[{
        "role": "user",
        "content": f"{context}\n\nExplain the architecture of this codebase."
    }]
)
print(response.content[0].text)
```

### With OpenAI GPT

```python
import infiniloom
import openai

context = infiniloom.pack("/path/to/repo", format="markdown", model="gpt")

client = openai.OpenAI()
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{
        "role": "user",
        "content": f"{context}\n\nWhat are the main components?"
    }]
)
print(response.choices[0].message.content)
```

### With Google Gemini

```python
import infiniloom
import google.generativeai as genai

context = infiniloom.pack("/path/to/repo", format="yaml", model="gemini")

genai.configure(api_key="YOUR_API_KEY")
model = genai.GenerativeModel("gemini-1.5-pro")
response = model.generate_content(f"{context}\n\nSummarize this codebase")
print(response.text)
```

## Advanced Usage

### Custom Token Budget

```python
from infiniloom import Infiniloom

loom = Infiniloom("/large/repo")

# Generate smaller context for models with limited context windows
compact_map = loom.map(map_budget=1000, max_symbols=25)

# Generate larger context for models with large context windows
detailed_map = loom.map(map_budget=5000, max_symbols=200)
```

### Security Scanning

```python
from infiniloom import Infiniloom

loom = Infiniloom("/path/to/repo")
findings = loom.scan_security()

# Filter by severity
critical = [f for f in findings if f['severity'] == 'Critical']
high = [f for f in findings if f['severity'] == 'High']

print(f"Critical: {len(critical)}, High: {len(high)}")

for finding in critical:
    print(f"{finding['file']}:{finding['line']}")
    print(f"  {finding['category']}: {finding['message']}")
```

### File Filtering

```python
from infiniloom import Infiniloom

loom = Infiniloom("/path/to/repo")
files = loom.files()

# Get Python files only
python_files = [f for f in files if f['language'] == 'python']

# Get high-importance files
important_files = [f for f in files if f['importance'] > 0.7]

# Get large files
large_files = [f for f in files if f['tokens'] > 1000]
```

## Performance

Infiniloom is built in Rust for maximum performance:

- **Fast scanning**: Parallel file processing with ignore patterns
- **Memory efficient**: Streaming processing, optional content loading
- **Native speed**: No Python overhead for core operations

## Requirements

- Python 3.8+
- Rust 1.91+ (for building from source)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

## Links

- [GitHub](https://github.com/Topos-Labs/infiniloom)
- [Documentation](https://toposlabs.ai/infiniloom/)
- [PyPI](https://pypi.org/project/infiniloom)
