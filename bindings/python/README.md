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
- `format` (str): Output format - "xml", "markdown", "json", or "yaml"
- `model` (str): Target model for token counting. Supports:
  - OpenAI: "gpt-5.2", "gpt-5.1", "gpt-5", "o4-mini", "o3", "o1", "gpt-4o", "gpt-4"
  - Anthropic: "claude" (default)
  - Google: "gemini"
  - Meta: "llama"
  - Others: "deepseek", "mistral", "qwen", "cohere", "grok"
- `compression` (str): Compression level - "none", "minimal", "balanced", "aggressive", "extreme"
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
- `model` (str): Target model - "claude", "gpt", "gpt-4o", "gemini", or "llama"

**Returns:** int - Number of tokens

#### `semantic_compress(text, similarity_threshold=0.7, budget_ratio=0.5)`

Compress text using semantic compression while preserving important content.

**Parameters:**
- `text` (str): Text to compress
- `similarity_threshold` (float): Threshold for grouping similar chunks (0.0-1.0, default: 0.7)
- `budget_ratio` (float): Target size as ratio of original (0.0-1.0, default: 0.5)

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
- `severity`: Severity level
- `category`: Issue category
- `message`: Description
- `code`: Code snippet (optional)

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
- [Documentation](https://infiniloom.dev/docs)
- [PyPI](https://pypi.org/project/infiniloom)
