<div align="center">

# Infiniloom

**Transform codebases into optimized context for LLMs**

[![CI](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml/badge.svg)](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/Topos-Labs/infiniloom/graph/badge.svg)](https://codecov.io/gh/Topos-Labs/infiniloom)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[![Crates.io](https://img.shields.io/crates/v/infiniloom.svg)](https://crates.io/crates/infiniloom)
[![npm](https://img.shields.io/npm/v/infiniloom.svg)](https://www.npmjs.com/package/infiniloom)
[![PyPI](https://img.shields.io/pypi/v/infiniloom.svg)](https://pypi.org/project/infiniloom/)
[![MSRV](https://img.shields.io/badge/MSRV-1.91+-orange.svg)](https://www.rust-lang.org/)

[Installation](#installation) · [Quick Start](#quick-start) · [Features](#features) · [Documentation](docs/)

</div>

---

Infiniloom extracts code, symbols, and structure from repositories and outputs them in formats optimized for Claude, GPT, Gemini, and other LLMs.

- **Fast** — Pure Rust, processes 1000+ files in <500ms
- **Smart** — AST-based symbol extraction (21 languages), PageRank ranking
- **Secure** — Automatic secret detection and redaction
- **Flexible** — XML, Markdown, JSON, YAML output formats

## Installation

```bash
npm install -g infiniloom      # Cross-platform (recommended)
cargo install infiniloom       # Rust users
brew tap Topos-Labs/infiniloom && brew install --cask infiniloom  # macOS
```

<details>
<summary>More installation options</summary>

| Method | Command | Notes |
|--------|---------|-------|
| **npm** | `npm install -g infiniloom` | Easiest, cross-platform |
| **Homebrew Cask** | `brew install --cask infiniloom` | macOS, pre-built binary |
| **Homebrew Formula** | `brew install infiniloom` | macOS, builds from source |
| **Cargo** | `cargo install infiniloom` | Rust users |
| **pip** | `pip install infiniloom` | Python library |
| **npm library** | `npm install infiniloom-node` | Node.js library |

**From source:**
```bash
git clone https://github.com/Topos-Labs/infiniloom.git
cd infiniloom && cargo build --release
```

</details>

## Quick Start

```bash
# Pack repository for Claude (XML format)
infiniloom pack . --format xml --output context.xml

# Scan repository statistics
infiniloom scan .

# Generate symbol map with PageRank ranking
infiniloom map . --budget 2000

# Get context for staged changes (AI code review)
infiniloom diff . --staged --include-diff
```

### Output Formats

```bash
infiniloom pack . --format xml       # Claude (prompt caching hints)
infiniloom pack . --format markdown  # GPT-4/GPT-4o
infiniloom pack . --format yaml      # Gemini
infiniloom pack . --format json      # Programmatic use
infiniloom pack . --format toon      # Token-efficient (~40% smaller)
```

## Performance

| Repository | Files | Tokens | Time |
|------------|-------|--------|------|
| infiniloom | 174 | 667K | 420ms |
| medium project | 1,200 | 1.8M | 1.8s |

*Benchmarks on M2 MacBook Pro, including full AST parsing and symbol extraction.*

## Features

### Model-Specific Optimization

| Model | Format | Optimizations |
|-------|--------|---------------|
| **Claude** | XML | Prompt caching hints, CDATA sections |
| **GPT-4/4o** | Markdown | Tables, code fences, hierarchical headers |
| **Gemini** | YAML | Query at end, structured hierarchy |
| **Any** | TOON | ~40% smaller than JSON |

### Smart Filtering

```bash
infiniloom pack . --include "*.rs" --exclude "tests/*"
infiniloom pack . --max-tokens 50000      # Token budget
infiniloom pack . --top-files 50          # Limit to N most important files
infiniloom pack . --compression aggressive # Remove comments, docstrings
```

### Security Scanning

Automatically detects and redacts API keys, tokens, private keys, and credentials:

```bash
infiniloom pack . --security-check   # Scan and report
infiniloom pack . --redact-secrets   # Redact with [REDACTED]
```

### Git Integration

```bash
infiniloom pack . --include-logs --logs-count 10  # Recent commits
infiniloom pack . --include-diffs                  # Uncommitted changes
infiniloom pack github:facebook/react              # Remote repos
```

### Diff Context for AI Code Reviews

```bash
infiniloom index .                    # Build symbol index
infiniloom diff . --staged            # Context for staged changes
infiniloom diff . HEAD~1              # Context for last commit
infiniloom diff . main..feature       # Context for branch diff
infiniloom impact . src/auth.rs       # What depends on this file?
```

<details>
<summary><strong>Supported Languages (21)</strong></summary>

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

</details>

<details>
<summary><strong>Supported LLM Models (20+)</strong></summary>

**Exact tokenization (tiktoken):**
- GPT-5.2, GPT-5.1, GPT-5, O4-mini, O3, O1 (o200k_base)
- GPT-4o, GPT-4o-mini (o200k_base)
- GPT-4, GPT-3.5-turbo (cl100k_base)

**Calibrated estimation:**
- Claude (Anthropic)
- Gemini (Google)
- Llama 3/4, CodeLlama (Meta)
- Mistral, Mixtral (Mistral AI)
- DeepSeek V3/R1
- Qwen (Alibaba)
- Cohere Command R+
- Grok (xAI)

</details>

<details>
<summary><strong>Compression Levels</strong></summary>

| Level | Reduction | What's Removed |
|-------|-----------|----------------|
| `none` | 0% | Nothing |
| `minimal` | 10-20% | Empty lines, trailing whitespace |
| `balanced` | 30-40% | Comments, redundant whitespace |
| `aggressive` | 50-60% | Docstrings, inline comments |
| `extreme` | 70-80% | Everything except signatures |

</details>

## Language Bindings

### Python

```python
import infiniloom

context = infiniloom.pack("/path/to/repo", format="xml", model="claude")
stats = infiniloom.scan("/path/to/repo")
```

### Node.js

```javascript
const { pack, scan } = require('infiniloom-node');

const context = pack('./repo', { format: 'xml', model: 'claude' });
const stats = scan('./repo');
```

## Configuration

Create `.infiniloom.yaml` with `infiniloom init`:

```yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 100000

scan:
  include: ["*.rs", "*.py", "*.ts"]
  exclude: ["tests/*", "docs/*"]

security:
  scan_secrets: true
  redact_secrets: true
```

See [Configuration Guide](docs/CONFIGURATION.md) for all options.

## Documentation

| Document | Description |
|----------|-------------|
| [Command Reference](docs/commands/) | All CLI commands |
| [Configuration](docs/CONFIGURATION.md) | Config files and environment variables |
| [Output Formats](docs/INFINILOOM_OUTPUT_FORMATS.md) | Format specifications |
| [Architecture](docs/INFINILOOM_DESIGN.md) | System design |

## Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md).

```bash
cargo test --workspace    # Run tests (800+)
cargo clippy --workspace  # Lint
cargo fmt --all           # Format
```

## License

MIT — see [LICENSE](LICENSE).

## Acknowledgments

- [Tree-sitter](https://tree-sitter.github.io/) — Fast, reliable parsing
- [tiktoken-rs](https://github.com/zurawiki/tiktoken-rs) — Accurate token counting
- [Aider](https://github.com/paul-gauthier/aider) — Repo-map concept inspiration

---

<div align="center">

Made with care by [Topos Labs](https://github.com/Topos-Labs)

</div>
