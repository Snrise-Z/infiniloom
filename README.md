<div align="center">

# 🧵 Infiniloom

**The fastest and most feature-rich way to pack repository context for LLMs**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.91+-orange.svg)](https://www.rust-lang.org/)
[![Crates.io](https://img.shields.io/crates/v/infiniloom.svg)](https://crates.io/crates/infiniloom)

[Installation](#installation) • [Quick Start](#quick-start) • [Features](#features) • [Unique Features](#unique-features) • [Documentation](#documentation)

</div>

---

## What is Infiniloom?

Infiniloom transforms your codebase into optimized context for Large Language Models. It extracts code, symbols, and structure from repositories and outputs them in formats specifically optimized for Claude, GPT-4, Gemini, and other LLMs.

```bash
# Pack your repo for Claude in under a second
infiniloom pack . --format xml --output context.xml
```

**Why Infiniloom?**

- **Blazing Fast**: High-performance pure Rust architecture
- **Smart**: AST-based symbol extraction with PageRank importance ranking
- **Optimized**: Model-specific output formats (XML for Claude, Markdown for GPT)
- **Secure**: Automatic detection and redaction of secrets and API keys
- **Flexible**: Python and Node.js bindings included

---

## Installation

### Quick Install (Recommended)

```bash
# npm (easiest, works on macOS/Linux/Windows)
npm install -g infiniloom

# Homebrew Cask (macOS - fast, pre-built binary)
brew tap Topos-Labs/infiniloom
brew install --cask infiniloom

# Cargo (if you have Rust installed)
cargo install infiniloom
```

### All Installation Options

| Method | Command | Notes |
|--------|---------|-------|
| **npm** | `npm install -g infiniloom` | Easiest, cross-platform |
| **Homebrew Cask** | `brew install --cask infiniloom` | Fast, pre-built binary (macOS) |
| **Homebrew Formula** | `brew install infiniloom` | Builds from source, needs Rust |
| **Cargo** | `cargo install infiniloom` | Rust users |
| **pip** | `pip install infiniloom` | Python library |
| **npm library** | `npm install infiniloom-node` | Node.js library |

### From Source

```bash
# Clone and build
git clone https://github.com/Topos-Labs/infiniloom.git
cd infiniloom
cargo build --release

# Binary at ./target/release/infiniloom
```

Requires Rust 1.91+: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

---

## Quick Start

### Basic Commands

```bash
# Pack repository into XML (optimized for Claude)
infiniloom pack /path/to/repo --format xml

# Scan repository and show statistics
infiniloom scan /path/to/repo

# Fast estimation for large repos via sampling
infiniloom scan /path/to/repo --sample 500
infiniloom scan /path/to/repo --sample-percent 1

# Generate repository map with key symbols
infiniloom map /path/to/repo --budget 2000

# Split large repo into chunks for multi-turn conversations
infiniloom chunk /path/to/repo --strategy semantic --max-tokens 8000

# Chunk with overlap for context continuity
infiniloom chunk /path/to/repo --overlap 500

# Priority ordering (core modules first)
infiniloom chunk /path/to/repo --priority-first

# Show repository information
infiniloom info /path/to/repo
```

### Output Formats

```bash
# XML format — optimized for Claude (with prompt caching hints)
infiniloom pack . --format xml --model claude

# Markdown format — optimized for GPT-4/GPT-4o
infiniloom pack . --format markdown --model gpt-4o

# TOON format — most token-efficient (~40% smaller than JSON)
infiniloom pack . --format toon

# JSON format — for programmatic use
infiniloom pack . --format json

# YAML format — optimized for Gemini
infiniloom pack . --format yaml --model gemini
```

### Working with Git

```bash
# Include recent commits in output
infiniloom pack . --include-logs --logs-count 10

# Include uncommitted changes with diff content (+/- lines)
infiniloom pack . --include-diffs

# Pack a remote GitHub repository
infiniloom pack github:facebook/react
infiniloom pack https://github.com/tokio-rs/tokio.git
```

### Diff Context for AI Code Reviews

```bash
# Build symbol index for fast diff context
infiniloom index /path/to/repo

# Keep index fresh with watch mode
infiniloom index /path/to/repo --watch

# Get context for unstaged changes
infiniloom diff .

# Get context for staged changes
infiniloom diff . --staged

# Get context for a specific commit or range
infiniloom diff . HEAD~1
infiniloom diff . main..feature

# Options
infiniloom diff . --depth 2             # Context depth (1-3, default: 2)
infiniloom diff . --budget 50000        # Token budget limit
infiniloom diff . --include-diff        # Include actual diff content (+/- lines)
infiniloom diff . --format json         # Output format (xml/json/markdown)
infiniloom diff . --include-history     # Include recent commit history per file
infiniloom diff . --history-count 5     # Number of commits to include (default: 3)

# Analyze impact of changes to a file or symbol
infiniloom impact . src/auth.rs
infiniloom impact . --symbol "authenticate"
```

Note: `diff` and `impact` take the repository path first; any commit/range or target comes after the path.

The `--include-diff` flag embeds the actual git diff (+/- lines) within the semantic context, giving AI reviewers both WHAT changed and its surrounding context in one view.

The `--include-history` flag adds recent commit history for each changed file, showing when and why code was previously modified.

### File Selection

```bash
# Include only specific file types
infiniloom pack . --include "*.rs" --include "*.py"

# Exclude directories
infiniloom pack . --exclude "tests/*" --exclude "docs/*"

# Set token budget
infiniloom pack . --max-tokens 50000

# Limit to top N most important files (applied after ranking)
infiniloom pack . --top-files 50

# Use compression
infiniloom pack . --compression aggressive
```

### Watch Mode

```bash
# Continuously regenerate output when files change
infiniloom pack . --output context.xml --watch

# Watch mode respects all pack options (compression, security, etc.)
infiniloom pack . --output context.xml --watch \
  --compression aggressive --security-check --truncate-base64
```

### Pack Extras (Format-Safe)

```bash
# Add header/instructions and include token tree + security scan
infiniloom pack . --header-text "My header" \
  --instruction-file instructions.md --token-tree --security-check
```

- XML: extras are inserted inside `<repository>` as `<extras>...</extras>`
- JSON/YAML: extras are added as top-level fields (output remains valid)
- Markdown/Plain/TOON: header is prepended; other extras are appended as sections

### Copy to Clipboard (macOS)

```bash
# Pack and copy directly to clipboard for pasting into Claude/ChatGPT
infiniloom pack . --format xml | pbcopy
```

---

## Features

### Model-Specific Optimization

| Model | Format | Optimizations |
|-------|--------|---------------|
| **Claude** | XML | Prompt caching hints, CDATA sections, structured tags |
| **GPT-4/4o** | Markdown | Tables, code fences, hierarchical headers |
| **Gemini** | YAML | Query at end, hierarchical structure |
| **Any** | TOON | ~40% smaller than JSON, tabular metadata, minimal syntax |
| **JSON** | JSON | Full metadata, programmatic access |

### AST-Based Symbol Extraction

Infiniloom uses [Tree-sitter](https://tree-sitter.github.io/) to parse source code and extract symbols from **21 languages**:

| Language | Symbols Extracted |
|----------|-------------------|
| Python | Functions, Classes, Methods, Decorators |
| JavaScript/TypeScript | Functions, Classes, Interfaces, Types |
| Rust | Functions, Structs, Enums, Traits, Impl blocks |
| Go | Functions, Methods, Structs, Interfaces |
| Java | Classes, Interfaces, Methods, Enums |
| C/C++ | Functions, Structs, Classes |
| C# | Classes, Methods, Interfaces, Properties |
| Ruby | Classes, Modules, Methods |
| PHP | Classes, Functions, Methods |
| Kotlin | Classes, Functions, Interfaces |
| Swift | Classes, Structs, Functions, Protocols |
| Scala | Classes, Objects, Traits, Functions |
| Haskell | Functions, Types, Data, Typeclasses |
| Elixir | Modules, Functions, Macros |
| Clojure | Functions, Macros, Vars |
| OCaml | Functions, Modules, Types |
| Lua | Functions, Tables |
| R | Functions, Classes |
| Bash | Functions |

### PageRank Symbol Ranking

Important symbols are ranked using PageRank algorithm based on:
- Reference count (how often a symbol is used)
- Import centrality (position in dependency graph)
- File importance (entry points, main modules)

### Security Scanning

Automatically detects and redacts:
- API keys and tokens
- Passwords and secrets
- Private keys (RSA, SSH)
- Database connection strings
- Cloud credentials (AWS, GCP, Azure)

**Note**: Comment lines (starting with `//`, `#`, `/*`, etc.) are automatically skipped to reduce false positives from example code and documentation.

### Compression Levels

| Level | Token Reduction | What's Removed |
|-------|-----------------|----------------|
| `none` | 0% | Nothing |
| `minimal` | 10-20% | Empty lines, trailing whitespace |
| `balanced` | 30-40% | Comments, redundant whitespace |
| `aggressive` | 50-60% | Docstrings, inline comments |
| `extreme` | 70-80% | Everything except signatures |

---

## Language Bindings

### Python

```bash
cd bindings/python
pip install maturin
maturin develop
```

```python
import infiniloom

# Pack a repository
context = infiniloom.pack("/path/to/repo", format="xml", model="claude")

# Get repository statistics
stats = infiniloom.scan("/path/to/repo")
print(f"Files: {stats['total_files']}, Tokens: {stats['total_tokens']}")

# Count tokens
tokens = infiniloom.count_tokens("def hello(): pass", model="claude")
```

### Node.js

```bash
cd bindings/node
npm install
npm run build
```

```javascript
const { pack, scan, Infiniloom } = require('@infiniloom/node');

// Pack a repository
const context = pack('./my-repo', { format: 'xml', model: 'claude' });

// Get statistics
const stats = scan('./my-repo');
console.log(`Files: ${stats.total_files}`);
```

---

## Performance

Infiniloom is designed for speed and efficiency, significantly outperforming existing solutions through its pure Rust architecture. Typical processing times for medium-sized repositories (100-500 files) are under 100ms.

---

## Unique Features

Infiniloom offers capabilities not found in other repository packing tools:

### Repository Map Generation

Generate a concise map of your codebase showing the most important symbols, ranked by importance:

```bash
infiniloom map /path/to/repo --budget 2000
```

The map uses PageRank algorithm to identify key entry points, heavily-used functions, and central abstractions — giving LLMs a bird's-eye view of your architecture.

Map output sections:
- Summary (repo stats, primary language, key modules)
- Key symbols (ranked list with file + line)
- Module graph (directory dependencies)
- File index (top files by importance)

### Intelligent Token Budgeting

Set a token budget and Infiniloom will intelligently select the most relevant files:

```bash
infiniloom pack . --budget 50000
```

Files are prioritized based on:
- Symbol importance (PageRank scores)
- File centrality in the dependency graph
- Recent modification time
- Configuration file detection (package.json, Cargo.toml, etc.)

### Multi-Model Token Counting

Accurate token counts for 20+ LLM models:

```bash
# OpenAI models (exact tokenization via tiktoken)
infiniloom scan . --model gpt52      # GPT-5.2 (o200k_base)
infiniloom scan . --model gpt51      # GPT-5.1 (o200k_base)
infiniloom scan . --model gpt5       # GPT-5 (o200k_base)
infiniloom scan . --model o3         # O3 reasoning model
infiniloom scan . --model o1         # O1 reasoning model
infiniloom scan . --model gpt4o      # GPT-4o (o200k_base)
infiniloom scan . --model gpt4       # GPT-4 (cl100k_base, legacy)

# Other vendors (calibrated estimation)
infiniloom scan . --model claude     # Anthropic Claude (default)
infiniloom scan . --model gemini     # Google Gemini
infiniloom scan . --model llama      # Meta Llama 3/4
infiniloom scan . --model mistral    # Mistral AI
infiniloom scan . --model deepseek   # DeepSeek V3/R1
infiniloom scan . --model qwen       # Alibaba Qwen
infiniloom scan . --model cohere     # Cohere Command R+
infiniloom scan . --model grok       # xAI Grok
```

### Secret Detection & Redaction

Automatically scans for and redacts sensitive information before output:

- AWS access keys and secrets
- GitHub tokens (classic and fine-grained PATs)
- Private keys (RSA, EC, DSA, OPENSSH)
- API keys and generic secrets
- Database connection strings (MongoDB, PostgreSQL, MySQL, Redis)
- JWT tokens
- Slack and Stripe tokens
- Custom patterns via configuration

```bash
infiniloom pack .                   # No scanning (fast, for trusted code)
infiniloom pack . --security-check  # Scan and report findings in metadata
infiniloom pack . --redact-secrets  # Scan and redact secrets with [REDACTED]
```

**Note**: With a config file (from `infiniloom init`), scanning is enabled by default.

**Security configuration options:**
- `fail_on_secrets`: Exit with error if secrets are detected (CI/CD integration)
- `allowlist`: Patterns to exclude from detection
- `custom_patterns`: Additional regex patterns for company-specific secrets

### Call Graph API

Query caller/callee relationships and navigate your codebase programmatically:

```bash
# Build symbol index (required once, auto-updates)
infiniloom index /path/to/repo

# Analyze impact of changes
infiniloom impact . src/auth.rs
infiniloom impact . --symbol "authenticate"
```

```python
import infiniloom

# Build index first
infiniloom.build_index("/path/to/repo")

# Find who calls a function
callers = infiniloom.get_callers("/path/to/repo", "authenticate")
for c in callers:
    print(f"{c['name']} at {c['file']}:{c['line']}")

# Find what a function calls
callees = infiniloom.get_callees("/path/to/repo", "main")

# Get all references (calls, imports, inheritance)
refs = infiniloom.get_references("/path/to/repo", "UserService")

# Get complete call graph
graph = infiniloom.get_call_graph("/path/to/repo")
print(f"{graph['stats']['total_symbols']} symbols, {graph['stats']['total_calls']} call edges")
```

```javascript
const { buildIndex, getCallers, getCallGraph } = require('infiniloom-node');

buildIndex('./my-repo');

// Find callers
const callers = getCallers('./my-repo', 'authenticate');
callers.forEach(c => console.log(`${c.name} at ${c.file}:${c.line}`));

// Get call graph
const graph = getCallGraph('./my-repo');
console.log(`${graph.stats.totalSymbols} symbols, ${graph.stats.totalCalls} calls`);
```

### Native Language Bindings

Use Infiniloom directly in your applications — no shell commands needed:

- **Python**: PyO3-based native extension
- **Node.js**: NAPI-RS bindings with TypeScript types

---

## Configuration

### `.infiniloomignore`

Create a `.infiniloomignore` file to exclude files (in addition to `.gitignore`):

```gitignore
# Build artifacts
target/
dist/
build/

# Dependencies
node_modules/
vendor/

# Large files
*.bin
*.dat
data/

# Generated
*.generated.*
```

### Environment Variables

Environment variables use double underscore (`__`) to separate nested config keys:

| Variable | Description | Default |
|----------|-------------|---------|
| `INFINILOOM_OUTPUT__MODEL` | Default tokenizer model | `claude` |
| `INFINILOOM_OUTPUT__FORMAT` | Default output format | `xml` |
| `INFINILOOM_OUTPUT__COMPRESSION` | Default compression | `balanced` |
| `INFINILOOM_OUTPUT__TOKEN_BUDGET` | Default token budget | `0` (no limit) |
| `INFINILOOM_SCAN__INCLUDE_HIDDEN` | Include hidden files | `false` |
| `INFINILOOM_SCAN__RESPECT_GITIGNORE` | Respect .gitignore | `true` |
| `INFINILOOM_SECURITY__SCAN_SECRETS` | Enable secret scanning | `false` |
| `INFINILOOM_SECURITY__REDACT_SECRETS` | Redact detected secrets | `true` |

**Note**: CLI default for `--max-tokens` is 0 (no limit). Config files can override this.

### Configuration File

Run `infiniloom init` to create a configuration file. Supports YAML, TOML, and JSON:

```bash
infiniloom init                    # Creates .infiniloom.yaml
infiniloom init --format toml      # Creates .infiniloom.toml
infiniloom init --format json      # Creates .infiniloom.json
infiniloom init --template rust    # Pre-configured for Rust projects
infiniloom init --template python  # Pre-configured for Python projects
infiniloom init --template typescript  # Pre-configured for TypeScript projects
```

Example `.infiniloom.yaml`:

```yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 100000
  line_numbers: true
  show_file_summary: true

scan:
  include:
    - "*.rs"
    - "*.py"
    - "*.ts"
  exclude:
    - "tests/*"
    - "docs/*"
    - "*.test.*"

security:
  scan_secrets: true           # Enable secret scanning
  fail_on_secrets: false       # Exit with error if secrets found
  redact_secrets: true         # Replace detected secrets with [REDACTED]
  allowlist:                   # Patterns to ignore (won't be flagged)
    - "EXAMPLE"
    - "test_key"
  custom_patterns:             # Additional regex patterns to detect
    - "MY_SECRET_[A-Z0-9]{32}"
    - "CUSTOM_TOKEN_\\w+"
```

Both flat and nested formats are supported for backward compatibility:

```yaml
# Flat format (also works)
include:
  - "*.rs"
exclude:
  - "tests/*"
include_tests: false
include_docs: false
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/INFINILOOM_DESIGN.md) | System design and architecture |
| [Output Formats](docs/INFINILOOM_OUTPUT_FORMATS.md) | Detailed format specifications |
| [Implementation Status](docs/IMPLEMENTATION_STATUS.md) | Feature implementation status |
| [Command Reference](docs/commands/README.md) | CLI command documentation |

---

## Development

### Project Structure

```
infiniloom/
├── cli/                 # Rust CLI application
├── engine/              # Core Rust engine
│   └── src/
│       ├── parser/      # Tree-sitter AST parsing (21 languages)
│       ├── tokenizer/   # Multi-model token counting (tiktoken)
│       ├── index/       # Symbol index for diff context
│       ├── chunking/    # Semantic code chunking
│       ├── repomap/     # PageRank symbol ranking
│       ├── output/      # Format generators
│       └── security.rs  # Secret detection
├── bindings/
│   ├── common/          # Shared bindings code
│   ├── python/          # PyO3 bindings
│   └── node/            # NAPI-RS bindings
└── docs/                # Documentation
```

### Building

```bash
# Build everything
cargo build --release

# Run tests
cargo test

# Run clippy
cargo clippy

# Run benchmarks
cargo bench
```

### Running Tests

```bash
# All tests
cargo test --all

# Specific crate
cargo test -p infiniloom-engine

# With output
cargo test -- --nocapture
```

---

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing`)
3. Make your changes
4. Run tests (`cargo test`) and lints (`cargo clippy`)
5. Commit with clear messages
6. Push and open a Pull Request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

## Acknowledgments

- [Tree-sitter](https://tree-sitter.github.io/) for fast, reliable parsing
- [tiktoken-rs](https://github.com/zurawiki/tiktoken-rs) for token counting
- [Aider](https://github.com/paul-gauthier/aider) for the repo-map concept

### Alternatives & Inspiration

These projects inspired Infiniloom and are great alternatives:

- [repomix](https://github.com/yamadashy/repomix) — Node.js-based repository packer
- [gitingest](https://github.com/cyclotruc/gitingest) — Python-based repository ingestion tool

---

<div align="center">

**[⬆ Back to Top](#-infiniloom)**

Made with 🧵 by [Topos Labs](https://github.com/Topos-Labs)

</div>
