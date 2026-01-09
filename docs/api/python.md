# Python API Reference

**Infiniloom Python bindings for RAG pipelines and vector database integration.**

```bash
pip install infiniloom
```

---

## Quick Start

```python
import infiniloom

# Pack a repository into Claude-optimized XML
context = infiniloom.pack("/path/to/repo", format="xml", model="claude")

# Generate embedding chunks for vector databases
result = infiniloom.embed("/path/to/repo", max_tokens=1000)
for chunk in result["chunks"]:
    print(f"{chunk['id']}: {chunk['source']['symbol']}")

# Scan repository statistics
stats = infiniloom.scan("/path/to/repo")
print(f"Files: {stats['total_files']}, Tokens: {stats['total_tokens']['claude']}")

# Count tokens in text
tokens = infiniloom.count_tokens("Hello, world!", model="claude")
```

---

## Core Functions

### `pack()`

Pack a repository into an LLM-optimized format.

```python
def pack(
    path: str,
    format: str = "xml",
    model: str = "claude",
    compression: str = "balanced",
    map_budget: int = 2000,
    max_symbols: int = 50,
    redact_secrets: bool = True,
    skip_symbols: bool = False,
) -> str
```

**Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `path` | `str` | required | Path to the repository |
| `format` | `str` | `"xml"` | Output format: `"xml"`, `"markdown"`, `"json"`, `"yaml"`, `"toon"`, `"plain"` |
| `model` | `str` | `"claude"` | Target LLM for token counting |
| `compression` | `str` | `"balanced"` | Compression level: `"none"`, `"minimal"`, `"balanced"`, `"aggressive"`, `"extreme"` |
| `map_budget` | `int` | `2000` | Token budget for repository map |
| `max_symbols` | `int` | `50` | Maximum symbols in map |
| `redact_secrets` | `bool` | `True` | Redact detected secrets |
| `skip_symbols` | `bool` | `False` | Skip symbol extraction (faster) |

**Returns:** `str` - Formatted repository context

**Example:**
```python
# Claude-optimized XML
context = infiniloom.pack("/path/to/repo", format="xml", model="claude")

# GPT-4o Markdown with aggressive compression
context = infiniloom.pack("/path/to/repo", format="markdown", model="gpt4o", compression="aggressive")

# Token-efficient TOON format
context = infiniloom.pack("/path/to/repo", format="toon")
```

---

### `scan()`

Scan a repository and return statistics.

```python
def scan(
    path: str,
    include_hidden: bool = False,
    respect_gitignore: bool = True,
    exclude: Optional[List[str]] = None,
) -> ScanResult
```

**Returns:** `ScanResult` dictionary:
```python
{
    "name": str,           # Repository name
    "path": str,           # Repository path
    "total_files": int,    # Total files
    "total_lines": int,    # Total lines of code
    "total_tokens": {      # Token counts by model
        "o200k": int,      # GPT-4o, GPT-5 (exact)
        "cl100k": int,     # GPT-4 (exact)
        "claude": int,     # Claude (estimated)
        "gemini": int,     # Gemini (estimated)
        "llama": int,      # Llama (estimated)
        ...
    },
    "languages": [         # Language breakdown
        {"language": str, "files": int, "lines": int, "percentage": float}
    ],
    "branch": Optional[str],
    "commit": Optional[str],
}
```

**Example:**
```python
stats = infiniloom.scan("/path/to/repo")
print(f"Repository: {stats['name']}")
print(f"Total files: {stats['total_files']}")
print(f"Claude tokens: {stats['total_tokens']['claude']}")

for lang in stats['languages']:
    print(f"  {lang['language']}: {lang['files']} files ({lang['percentage']:.1f}%)")
```

---

### `embed()`

Generate embedding chunks for vector databases.

```python
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
) -> EmbedResult
```

**Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `path` | `str` | required | Path to repository |
| `max_tokens` | `int` | `1000` | Maximum tokens per chunk |
| `min_tokens` | `int` | `50` | Minimum tokens per chunk |
| `context_lines` | `int` | `5` | Context lines around symbols |
| `include_imports` | `bool` | `True` | Include import statements |
| `include_top_level` | `bool` | `True` | Include top-level code |
| `include_tests` | `bool` | `False` | Include test files |
| `security_scan` | `bool` | `True` | Enable secret scanning |
| `include_patterns` | `List[str]` | `None` | Glob patterns to include |
| `exclude_patterns` | `List[str]` | `None` | Glob patterns to exclude |
| `manifest_path` | `str` | `None` | Custom manifest path |
| `diff_only` | `bool` | `False` | Only return changed chunks |

**Returns:** `EmbedResult` dictionary:
```python
{
    "version": int,
    "settings": EmbedSettings,
    "chunks": [EmbedChunk, ...],
    "summary": {
        "total_chunks": int,
        "total_tokens": int,
        "added": Optional[int],
        "modified": Optional[int],
        "removed": Optional[int],
        "unchanged": Optional[int],
    },
    "diff": Optional[EmbedDiff],
}
```

**Chunk structure:**
```python
{
    "id": "ec_a1b2c3d4...",        # Content-addressable ID
    "full_hash": "a1b2c3d4...",    # Full BLAKE3 hash
    "content": "fn foo() {...}",   # Chunk content
    "tokens": 150,                  # Token count
    "kind": "function",             # function, class, method, etc.
    "source": {
        "file": "src/main.rs",
        "lines": [10, 25],
        "symbol": "foo",
        "fqn": "src::main::foo",    # Fully qualified name
        "language": "Rust",
        "parent": Optional[str],
        "visibility": "public",
        "is_test": False,
    },
    "context": {
        "docstring": Optional[str],
        "signature": Optional[str],
        "calls": ["bar", "baz"],        # Functions this calls
        "called_by": ["main"],          # Functions that call this
        "imports": ["std::io"],
        "tags": ["async", "public-api"], # Auto-generated semantic tags
    },
}
```

**Example:**
```python
# Generate chunks for RAG pipeline
result = infiniloom.embed("/path/to/repo", max_tokens=1500)

for chunk in result["chunks"]:
    # Embed and upsert to vector DB
    embedding = get_embedding(chunk["content"])
    vector_db.upsert(
        id=chunk["id"],
        vector=embedding,
        metadata={
            "file": chunk["source"]["file"],
            "symbol": chunk["source"]["symbol"],
            "kind": chunk["kind"],
            "tags": chunk["context"]["tags"],
        }
    )

# Incremental updates
result = infiniloom.embed("/path/to/repo", diff_only=True)
print(f"Added: {result['summary']['added']}, Modified: {result['summary']['modified']}")
```

---

### `count_tokens()`

Count tokens in text for a specific model.

```python
def count_tokens(text: str, model: str = "claude") -> int
```

**Example:**
```python
tokens = infiniloom.count_tokens("Hello, world!", model="claude")
print(f"Tokens: {tokens}")

# Exact counting for OpenAI models
gpt4o_tokens = infiniloom.count_tokens(code, model="gpt4o")
```

---

### `scan_security()`

Scan repository for security issues.

```python
def scan_security(path: str) -> List[SecurityFinding]
```

**Returns:** List of security findings:
```python
{
    "file": str,       # File path
    "line": int,       # Line number
    "severity": str,   # "Critical", "High", "Medium", "Low"
    "kind": str,       # Type of finding
    "pattern": str,    # Matched pattern
}
```

**Example:**
```python
findings = infiniloom.scan_security("/path/to/repo")
for finding in findings:
    if finding["severity"] == "Critical":
        print(f"CRITICAL: {finding['kind']} in {finding['file']}:{finding['line']}")
```

---

### `semantic_compress()`

Compress text while preserving meaning.

```python
def semantic_compress(
    text: str,
    similarity_threshold: float = 0.7,
    budget_ratio: float = 0.5,
) -> str
```

**Example:**
```python
long_text = "..." # Your long text
compressed = infiniloom.semantic_compress(long_text, budget_ratio=0.3)
print(f"Reduced from {len(long_text)} to {len(compressed)} chars")
```

---

## Index & Call Graph API

### `build_index()`

Build or update the symbol index for fast queries.

```python
def build_index(
    path: str,
    force: bool = False,
    include_tests: bool = False,
    max_file_size: Optional[int] = None,
    exclude: Optional[List[str]] = None,
    incremental: bool = False,
) -> IndexStatus
```

**Returns:** `IndexStatus` dictionary:
```python
{
    "exists": bool,
    "file_count": int,
    "symbol_count": int,
    "last_built": Optional[str],  # ISO 8601
    "version": Optional[str],
    "files_updated": Optional[int],
    "incremental": bool,
}
```

**Example:**
```python
# Build index
status = infiniloom.build_index("/path/to/repo")
print(f"Indexed {status['symbol_count']} symbols")

# Incremental update
status = infiniloom.build_index("/path/to/repo", incremental=True)
print(f"Updated {status['files_updated']} files")
```

---

### `find_symbol()`

Find a symbol by name.

```python
def find_symbol(path: str, name: str) -> List[SymbolInfo]
```

**Example:**
```python
symbols = infiniloom.find_symbol("/path/to/repo", "authenticate")
for sym in symbols:
    print(f"{sym['kind']}: {sym['name']} at {sym['file']}:{sym['line']}")
```

---

### `get_callers()`

Get all functions that call a symbol.

```python
def get_callers(path: str, symbol_name: str) -> List[SymbolInfo]
```

**Example:**
```python
callers = infiniloom.get_callers("/path/to/repo", "validate_input")
print(f"validate_input is called by {len(callers)} functions")
```

---

### `get_callees()`

Get all functions that a symbol calls.

```python
def get_callees(path: str, symbol_name: str) -> List[SymbolInfo]
```

---

### `get_call_graph()`

Get the complete call graph.

```python
def get_call_graph(
    path: str,
    max_nodes: Optional[int] = None,
    max_edges: Optional[int] = None,
) -> CallGraph
```

**Returns:** `CallGraph` dictionary:
```python
{
    "nodes": [SymbolInfo, ...],
    "edges": [
        {
            "caller_id": int,
            "callee_id": int,
            "caller": str,
            "callee": str,
            "file": str,
            "line": int,
        }
    ],
    "stats": {
        "total_symbols": int,
        "total_calls": int,
        "functions": int,
        "classes": int,
    }
}
```

---

### `get_transitive_callers()`

Get all functions that eventually call a symbol (up to max depth).

```python
def get_transitive_callers(
    path: str,
    symbol_name: str,
    max_depth: int = 3,
    max_results: int = 100,
) -> List[TransitiveCaller]
```

**Example:**
```python
callers = infiniloom.get_transitive_callers("/path/to/repo", "dangerous_function", max_depth=5)
for c in callers:
    print(f"Depth {c['depth']}: {' -> '.join(c['call_path'])}")
```

---

### `find_circular_dependencies()`

Detect circular import/dependency cycles in the codebase.

```python
def find_circular_dependencies(path: str) -> List[DependencyCycle]
```

**Returns:** List of `DependencyCycle` dictionaries:
```python
{
    "files": List[str],      # File paths forming the cycle
    "file_ids": List[int],   # Internal file IDs
    "length": int,           # Number of files in the cycle
}
```

**Example:**
```python
# Build index first
infiniloom.build_index("/path/to/repo")

# Find circular dependencies
cycles = infiniloom.find_circular_dependencies("/path/to/repo")
if cycles:
    print(f"Found {len(cycles)} circular dependency cycles:")
    for cycle in cycles:
        print(f"  Cycle of {cycle['length']} files: {' -> '.join(cycle['files'])}")
else:
    print("No circular dependencies found")
```

---

### `get_exported_symbols()`

Get all public/exported symbols in the repository or a specific file.

```python
def get_exported_symbols(
    path: str,
    file_path: Optional[str] = None,
) -> List[SymbolInfo]
```

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | `str` | Path to repository |
| `file_path` | `str` | Optional file path to filter symbols |

**Returns:** List of `SymbolInfo` dictionaries for public/exported symbols only.

**Example:**
```python
# Build index first
infiniloom.build_index("/path/to/repo")

# Get all exported symbols
exports = infiniloom.get_exported_symbols("/path/to/repo")
print(f"Found {len(exports)} exported symbols")

for sym in exports:
    print(f"  {sym['visibility']} {sym['kind']}: {sym['name']} at {sym['file']}:{sym['line']}")

# Get exports from a specific file
file_exports = infiniloom.get_exported_symbols("/path/to/repo", file_path="src/lib.rs")
print(f"Exports from src/lib.rs: {', '.join(s['name'] for s in file_exports)}")
```

---

## Analysis API

### `extract_documentation()`

Extract structured documentation from a docstring/comment.

```python
def extract_documentation(raw_doc: str, language: str) -> Documentation
```

**Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `raw_doc` | `str` | Raw documentation string (JSDoc, Python docstring, etc.) |
| `language` | `str` | Language of the code (`"javascript"`, `"python"`, `"rust"`, etc.) |

**Returns:** `Documentation` dictionary:
```python
{
    "summary": Optional[str],
    "description": Optional[str],
    "params": [
        {
            "name": str,
            "type_info": Optional[str],
            "description": Optional[str],
            "is_optional": bool,
            "default_value": Optional[str],
        }
    ],
    "returns": {
        "type_info": Optional[str],
        "description": Optional[str],
    },
    "throws": [{"exception_type": str, "description": Optional[str]}],
    "examples": [{"code": str, "title": Optional[str], "language": Optional[str]}],
    "is_deprecated": bool,
    "deprecation_message": Optional[str],
    "since": Optional[str],
    "see_also": List[str],
    "tags": Dict[str, List[str]],
    "raw": Optional[str],
}
```

**Example:**
```python
doc = """
@param name - The user's name
@param age - The user's age
@returns The greeting message
@throws ValueError If name is empty
"""
result = infiniloom.extract_documentation(doc, "javascript")
print(f"Params: {len(result['params'])}")
for param in result['params']:
    print(f"  {param['name']}: {param['description']}")
```

---

### `calculate_complexity()`

Calculate complexity metrics for source code.

```python
def calculate_complexity(source: str, language: str) -> ComplexityMetrics
```

**Returns:** `ComplexityMetrics` dictionary:
```python
{
    "cyclomatic": int,           # Cyclomatic complexity
    "cognitive": int,            # Cognitive complexity
    "halstead": {
        "distinct_operators": int,
        "distinct_operands": int,
        "total_operators": int,
        "total_operands": int,
        "vocabulary": int,
        "length": int,
        "calculated_length": float,
        "volume": float,
        "difficulty": float,
        "effort": float,
        "time": float,
        "bugs": float,
    },
    "loc": {
        "total": int,
        "source": int,
        "comments": int,
        "blank": int,
    },
    "maintainability_index": Optional[float],
    "max_nesting_depth": int,
    "parameter_count": int,
    "return_count": int,
}
```

**Example:**
```python
code = """
def complex_function(a, b, c):
    if a > 0:
        if b > 0:
            return a + b
        else:
            return a - b
    return c
"""
metrics = infiniloom.calculate_complexity(code, "python")
print(f"Cyclomatic: {metrics['cyclomatic']}")
print(f"Cognitive: {metrics['cognitive']}")
print(f"Maintainability: {metrics['maintainability_index']:.1f}")
```

---

### `check_complexity()`

Check code complexity against thresholds.

```python
def check_complexity(
    source: str,
    language: str,
    max_cyclomatic: int = 10,
    max_cognitive: int = 15,
    max_nesting: int = 4,
    max_params: int = 5,
    min_maintainability: float = 40.0,
) -> List[ComplexityIssue]
```

**Returns:** List of complexity issues:
```python
{
    "message": str,    # Description of the issue
    "severity": str,   # "warning" or "error"
}
```

**Example:**
```python
issues = infiniloom.check_complexity(code, "python", max_cyclomatic=5)
for issue in issues:
    print(f"[{issue['severity']}] {issue['message']}")
```

---

### `detect_dead_code()`

Detect dead code in a repository.

```python
def detect_dead_code(
    path: str,
    languages: Optional[List[str]] = None,
) -> DeadCodeInfo
```

**Returns:** `DeadCodeInfo` dictionary:
```python
{
    "unused_exports": [
        {
            "name": str,
            "kind": str,
            "file_path": str,
            "line": int,
            "confidence": float,
            "reason": str,
        }
    ],
    "unreachable_code": [
        {
            "file_path": str,
            "start_line": int,
            "end_line": int,
            "reason": str,
            "snippet": str,
        }
    ],
    "unused_private": [
        {"name": str, "kind": str, "file_path": str, "line": int}
    ],
    "unused_imports": [
        {"name": str, "import_path": str, "file_path": str, "line": int}
    ],
    "unused_variables": [
        {"name": str, "file_path": str, "line": int, "scope": str}
    ],
}
```

**Example:**
```python
result = infiniloom.detect_dead_code("/path/to/repo")
print(f"Unused exports: {len(result['unused_exports'])}")
print(f"Unreachable code: {len(result['unreachable_code'])}")
for export in result['unused_exports']:
    print(f"  {export['name']} in {export['file_path']}:{export['line']}")
```

---

### `detect_breaking_changes()`

Detect breaking changes between two versions.

```python
def detect_breaking_changes(
    path: str,
    old_ref: str,
    new_ref: str,
) -> BreakingChangeReport
```

**Returns:** `BreakingChangeReport` dictionary:
```python
{
    "changes": [
        {
            "symbol_name": str,
            "change_type": str,        # "removed", "signature_changed", "visibility_changed"
            "severity": str,           # "breaking", "warning", "info"
            "file_path": str,
            "old_line": Optional[int],
            "new_line": Optional[int],
            "old_value": Optional[str],
            "new_value": Optional[str],
            "description": str,
            "suggestion": Optional[str],
        }
    ],
    "summary": {
        "total_changes": int,
        "by_severity": {"breaking": int, "warning": int, "info": int},
        "by_type": {"removed": int, "signature_changed": int, ...},
    },
    "old_ref": str,
    "new_ref": str,
}
```

**Example:**
```python
report = infiniloom.detect_breaking_changes("/path/to/repo", "v1.0.0", "v2.0.0")
print(f"Breaking changes: {report['summary']['by_severity'].get('breaking', 0)}")
for change in report['changes']:
    if change['severity'] == 'breaking':
        print(f"  BREAKING: {change['symbol_name']} - {change['description']}")
```

---

### `get_type_hierarchy()`

Get the type hierarchy for a class/interface.

```python
def get_type_hierarchy(path: str, symbol_name: str) -> TypeHierarchy
```

**Returns:** `TypeHierarchy` dictionary:
```python
{
    "name": str,
    "kind": str,
    "file": str,
    "line": int,
    "ancestors": [
        {
            "name": str,
            "file": str,
            "line": int,
            "kind": str,
            "depth": int,
            "is_direct": bool,
        }
    ],
    "descendants": [...],
    "interfaces": List[str],
}
```

**Example:**
```python
hierarchy = infiniloom.get_type_hierarchy("/path/to/repo", "BaseController")
print(f"Ancestors: {[a['name'] for a in hierarchy['ancestors']]}")
print(f"Descendants: {[d['name'] for d in hierarchy['descendants']]}")
```

---

### `get_type_ancestors()`

Get all ancestors (parent classes/interfaces) of a type.

```python
def get_type_ancestors(path: str, symbol_name: str) -> List[AncestorInfo]
```

---

### `get_type_descendants()`

Get all descendants (child classes) of a type.

```python
def get_type_descendants(path: str, symbol_name: str) -> List[AncestorInfo]
```

---

### `get_implementors()`

Get all classes that implement an interface.

```python
def get_implementors(path: str, interface_name: str) -> List[SymbolInfo]
```

**Example:**
```python
implementors = infiniloom.get_implementors("/path/to/repo", "Serializable")
print(f"Classes implementing Serializable: {len(implementors)}")
for impl in implementors:
    print(f"  {impl['name']} at {impl['file']}:{impl['line']}")
```

---

## Diff Context API

### `get_diff_context()`

Get context-aware diff with surrounding symbols.

```python
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
) -> DiffContext
```

**Example:**
```python
# Get context for staged changes
context = infiniloom.get_diff_context("/path/to/repo", from_ref="", to_ref="HEAD", include_diff=True)
print(f"Changed files: {len(context['changed_files'])}")
print(f"Related symbols: {len(context['context_symbols'])}")
print(f"Related tests: {len(context['related_tests'])}")
```

---

### `analyze_impact()`

Analyze the impact of changes to files.

```python
def analyze_impact(
    path: str,
    files: List[str],
    depth: int = 2,
    include_tests: bool = False,
    model: Optional[str] = None,
) -> ImpactResult
```

**Example:**
```python
impact = infiniloom.analyze_impact("/path/to/repo", ["src/auth.py"])
print(f"Impact level: {impact['impact_level']}")
print(f"Dependent files: {impact['dependent_files']}")
print(f"Test files to run: {impact['test_files']}")
```

---

## Classes

### `Infiniloom`

Object-oriented interface for repository operations.

```python
class Infiniloom:
    def __init__(self, path: str) -> None
    def load(self, include_hidden: bool = False, respect_gitignore: bool = True) -> None
    def stats(self) -> ScanResult
    def pack(self, format: str = "xml", model: str = "claude", ...) -> str
    def map(self, map_budget: int = 2000, max_symbols: int = 50) -> RepoMap
    def scan_security(self) -> List[SecurityFinding]
    def files(self) -> List[FileInfo]
```

**Example:**
```python
from infiniloom import Infiniloom

loom = Infiniloom("/path/to/repo")
stats = loom.stats()
print(f"Repository: {stats['name']}, Files: {stats['total_files']}")

context = loom.pack(format="xml", model="claude")
findings = loom.scan_security()
```

---

### `GitRepo`

Git repository operations.

```python
class GitRepo:
    def __init__(self, path: str) -> None
    def current_branch(self) -> str
    def current_commit(self) -> str
    def status(self) -> List[ChangedFileInfo]
    def diff_files(self, from_ref: str, to_ref: str) -> List[ChangedFileInfo]
    def log(self, count: int = 10) -> List[Commit]
    def file_log(self, path: str, count: int = 10) -> List[Commit]
    def blame(self, path: str) -> List[BlameInfo]
    def ls_files(self) -> List[str]
    def diff_hunks(self, from_ref: str, to_ref: str, path: Optional[str] = None) -> List[DiffHunk]
```

**Example:**
```python
from infiniloom import GitRepo, is_git_repo

if is_git_repo("/path/to/repo"):
    repo = GitRepo("/path/to/repo")
    print(f"Branch: {repo.current_branch()}")
    print(f"Commit: {repo.current_commit()}")

    for commit in repo.log(count=5):
        print(f"{commit['short_hash']}: {commit['message']}")
```

---

## Async Functions

All core functions have async versions:

```python
import asyncio
import infiniloom

async def main():
    context = await infiniloom.pack_async("/path/to/repo")
    stats = await infiniloom.scan_async("/path/to/repo")
    result = await infiniloom.embed_async("/path/to/repo")

asyncio.run(main())
```

Available async functions:
- `pack_async()`
- `scan_async()`
- `count_tokens_async()`
- `scan_security_async()`
- `semantic_compress_async()`
- `build_index_async()`
- `chunk_async()`
- `analyze_impact_async()`
- `get_diff_context_async()`
- `find_symbol_async()`
- `get_callers_async()`
- `get_callees_async()`
- `get_references_async()`
- `get_call_graph_async()`

---

## Exceptions

### `InfiniloomError`

Base exception for all Infiniloom errors.

```python
from infiniloom import InfiniloomError

try:
    context = infiniloom.pack("/nonexistent/path")
except InfiniloomError as e:
    print(f"Error: {e}")
```

---

## Supported Models

| Model | Accuracy | Description |
|-------|----------|-------------|
| `claude` | ~95% | Anthropic Claude (default) |
| `gpt52`, `gpt51`, `gpt5` | Exact | OpenAI GPT-5 series |
| `o4-mini`, `o3`, `o1` | Exact | OpenAI reasoning models |
| `gpt4o`, `gpt4o-mini` | Exact | OpenAI GPT-4o |
| `gpt4`, `gpt35-turbo` | Exact | OpenAI legacy |
| `gemini` | ~95% | Google Gemini |
| `llama`, `codellama` | ~95% | Meta Llama |
| `mistral` | ~95% | Mistral AI |
| `deepseek` | ~95% | DeepSeek |
| `qwen` | ~95% | Alibaba Qwen |
| `cohere` | ~95% | Cohere |
| `grok` | ~95% | xAI Grok |

---

## Integration Examples

### Pinecone

```python
import json
from pinecone import Pinecone
from openai import OpenAI
import infiniloom

# Initialize
pc = Pinecone(api_key="...")
index = pc.Index("code-embeddings")
openai = OpenAI()

# Generate chunks
result = infiniloom.embed("./my-repo", max_tokens=1500)

# Embed and upsert
for chunk in result["chunks"]:
    response = openai.embeddings.create(
        model="text-embedding-3-small",
        input=chunk["content"]
    )
    index.upsert(vectors=[{
        "id": chunk["id"],
        "values": response.data[0].embedding,
        "metadata": {
            "file": chunk["source"]["file"],
            "symbol": chunk["source"]["symbol"],
            "language": chunk["source"]["language"],
            "kind": chunk["kind"],
            "tags": chunk["context"]["tags"],
        }
    }])
```

### LangChain

```python
from langchain.text_splitter import RecursiveCharacterTextSplitter
from langchain.vectorstores import Chroma
import infiniloom

# Generate chunks
result = infiniloom.embed("./my-repo", max_tokens=1000)

# Convert to LangChain documents
from langchain.schema import Document
docs = [
    Document(
        page_content=chunk["content"],
        metadata={
            "id": chunk["id"],
            "file": chunk["source"]["file"],
            "symbol": chunk["source"]["symbol"],
        }
    )
    for chunk in result["chunks"]
]

# Create vector store
vectorstore = Chroma.from_documents(docs, embedding_function)
```

---

## See Also

- [Node.js API Reference](nodejs.md)
- [CLI Reference](../commands/pack.md)
- [Recipes Cookbook](../RECIPES.md)
- [Reference](../REFERENCE.md)
