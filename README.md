<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/Topos-Labs/infiniloom/main/assets/logo-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/Topos-Labs/infiniloom/main/assets/logo-light.svg">
  <img alt="Infiniloom" src="https://raw.githubusercontent.com/Topos-Labs/infiniloom/main/assets/logo-light.svg" width="320">
</picture>

# Infiniloom

**Give your AI the context it needs. Not the context it doesn't.**

Transform codebases into structured, ranked, security-scanned context for Claude, GPT, Gemini, and any LLM. Built in Rust for speed.

[![CI](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml/badge.svg)](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/Topos-Labs/infiniloom/graph/badge.svg)](https://codecov.io/gh/Topos-Labs/infiniloom)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/infiniloom.svg)](https://crates.io/crates/infiniloom)
[![npm](https://img.shields.io/npm/v/infiniloom.svg)](https://www.npmjs.com/package/infiniloom)
[![PyPI](https://img.shields.io/pypi/v/infiniloom.svg)](https://pypi.org/project/infiniloom/)

[Quick Start](#quick-start) &bull; [Why Infiniloom?](#why-infiniloom) &bull; [Commands](#commands) &bull; [Documentation](#documentation) &bull; [RAG & Vector DBs](#for-rag--vector-databases)

</div>

---

```bash
npm install -g infiniloom

# Generate AI-ready context in one command
infiniloom pack . -o context.xml          # XML for Claude
infiniloom pack . -f markdown -o ctx.md   # Markdown for GPT/Codex
infiniloom embed . -o chunks.jsonl        # Chunks for Pinecone/Weaviate/Qdrant
```

---

## Why Infiniloom?

When you ask an AI to help with code, quality depends on what context you provide. Pasting random files gives fragments without structure. Dumping entire repos overwhelms with noise. Token limits force you to leave out important code.

**Infiniloom fixes this.** It reads your codebase, understands what matters, and produces a structured context package designed specifically for AI consumption.

| | |
|---|---|
| **30-60% smaller** | Smart compression reduces context vs. raw files. TOON format saves another ~40%. |
| **~400ms** for 100 files | Pure Rust + Rayon parallelism. 5,000 files in ~8 seconds. |
| **23 languages** | Full AST parsing via Tree-sitter. Never splits mid-function. |
| **27 tokenizers** | Exact counts for GPT-5/4o (tiktoken). ~95% accurate for Claude/Gemini. |
| **15+ secret patterns** | Detects and redacts AWS keys, GitHub tokens, private keys before any code reaches AI. |
| **PageRank ranking** | Identifies core business logic. Deprioritizes utilities and boilerplate. |
| **Content-addressable** | BLAKE3 hashing. Same code anywhere = same chunk ID. Cross-repo deduplication. |
| **Incremental updates** | Manifest-based diffing. Only re-process what changed. |

---

## Quick Start

**Install:**

```bash
npm install -g infiniloom       # or: brew install infiniloom / cargo install infiniloom / pip install infiniloom
```

**Generate context:**

```bash
infiniloom pack . -o context.xml                    # Full repo context (XML for Claude)
infiniloom pack . -f markdown -m gpt4o -o ctx.md    # Markdown for GPT with exact token counts
infiniloom scan .                                   # See repo stats first
```

**Build a symbol index** (enables `diff` and `impact` commands):

```bash
infiniloom index .
infiniloom diff --staged --include-diff         # Context for your staged changes
infiniloom impact . src/auth.rs                 # What depends on this file?
```

**Configure your project:**

```bash
infiniloom init                     # Creates .infiniloom.yaml
infiniloom init --template rust     # Language-specific template
```

Config sets defaults for format, model, compression, patterns, and security. CLI flags override config. See the [Configuration Guide](docs/CONFIGURATION.md).

---

## Commands

| Command | What It Does |
|---------|--------------|
| **`pack`** | Generate AI-ready context from a repository |
| **`scan`** | Show repository statistics (files, tokens, languages) |
| **`map`** | Generate ranked symbol overview (PageRank) |
| **`embed`** | Generate chunks for vector databases / RAG |
| **`diff`** | Build context for code changes with callers/callees |
| **`index`** | Build symbol index (powers `diff` and `impact`) |
| **`impact`** | Analyze blast radius of a file or function |
| **`chunk`** | Split repo for multi-turn conversations |
| **`ingest`** | Convert documents (MD, HTML, CSV, DOCX, XLSX) to LLM format |
| **`init`** | Create `.infiniloom.yaml` configuration |
| **`info`** | Show version, models, and config |

### Choose Your Format

| Format | Best for | Flag |
|--------|----------|------|
| **XML** | Claude | `-f xml` |
| **Markdown** | GPT-4o, GPT-5, Codex, O-series | `-f markdown` |
| **YAML** | Gemini | `-f yaml` |
| **TOON** | Any model, tight budgets (~40% smaller) | `-f toon` |
| **JSON** | Programmatic parsing | `-f json` |

---

## For RAG & Vector Databases

The `embed` command generates deterministic, content-addressable code chunks for retrieval-augmented generation:

```bash
infiniloom embed . -o chunks.jsonl                 # Full generation
infiniloom embed . --diff -o updates.jsonl         # Only changed chunks
infiniloom embed . --streaming -o chunks.jsonl     # Memory-efficient for large repos
```

Each chunk includes rich metadata:

```json
{
  "id": "ec_a1b2c3d4e5f6g7h8",
  "content": "async fn authenticate(token: &str) -> Result<User, AuthError> {...}",
  "tokens": 245,
  "kind": "function",
  "source": { "file": "src/auth.rs", "symbol": "authenticate", "language": "Rust" },
  "context": {
    "calls": ["verify_jwt", "find_user_by_id"],
    "called_by": ["login_handler", "refresh_token"],
    "tags": ["async", "security", "public-api"]
  }
}
```

| Feature | What It Enables |
|---------|----------------|
| **Content-addressable IDs** | Cross-repo deduplication. Same code = same ID. |
| **AST-aware chunking** | Never splits mid-function. Preserves semantic boundaries. |
| **Incremental updates** | Only re-embed what changed. Manifest tracks state. |
| **Call graph context** | `calls` and `called_by` enable dependency-aware retrieval. |
| **Auto-generated tags** | `async`, `security`, `database`, `http` improve search relevance. |
| **Hierarchical chunks** | Parent-child linking. Class summaries link to member chunks. |

Works with **Pinecone, Weaviate, Qdrant, ChromaDB, pgvector, Milvus**, or any vector DB accepting JSONL/JSON.

See [embed command docs](docs/commands/embed.md) for complete details.

---

## Using with AI Coding Assistants

### Claude Code

```bash
infiniloom pack . -f xml --redact-secrets -o context.xml           # Full context
infiniloom index . && infiniloom diff main..feature --depth 2      # PR review context
infiniloom impact . src/auth.rs                                    # Blast radius
```

See the full [Claude Code Integration Guide](docs/guides/claude-code-integration.md) &mdash; 8 workflows covering CLAUDE.md enrichment, PR reviews, impact analysis, security, onboarding, and team config.

### OpenAI Codex / GPT

```bash
infiniloom pack . -f markdown -m gpt4o --max-tokens 80000 -o ctx.md    # GPT-4o
infiniloom pack . -f toon -m gpt4 --max-tokens 6000 -o ctx.toon       # Tight budget
```

See the full [Codex Integration Guide](docs/guides/codex-integration.md) &mdash; 8 workflows with model-specific token budgets and format recommendations.

### When to Use Infiniloom vs. Direct File Reading

| Scenario | Direct reading | With Infiniloom |
|----------|---------------|-----------------|
| Small repo (<50 files) | Sufficient | Not needed |
| Large repo (500+ files) | Exceeds context window | Ranks and fits what matters |
| PR review | Manual file selection | `diff` auto-includes callers/callees |
| Security-sensitive code | Risk of leaking secrets | Automatic redaction |
| Team consistency | Varies per person | Config standardizes context |
| CI/CD automation | N/A | Generates context as artifacts |
| RAG / vector DBs | N/A | Content-addressable chunks |

---

## How Infiniloom Compares

> *As of March 2026. Capabilities evolve &mdash; check official docs for latest.*

| Feature | Infiniloom | Repomix | Aider | Continue | Cursor |
|---------|:----------:|:-------:|:-----:|:--------:|:------:|
| **AST Parsing** | 23 languages | Limited | Limited | Limited | Yes |
| **PageRank Ranking** | Yes | &mdash; | &mdash; | &mdash; | &mdash; |
| **Content-Addressable Chunks** | BLAKE3 | &mdash; | &mdash; | &mdash; | &mdash; |
| **Incremental Diffing** | Manifest | &mdash; | Git-based | &mdash; | Yes |
| **Secret Detection** | 15+ patterns | Limited | &mdash; | &mdash; | &mdash; |
| **Multi-Model Tokens** | 27 models | &mdash; | Few | &mdash; | &mdash; |
| **Call Graphs** | Yes | &mdash; | &mdash; | &mdash; | &mdash; |
| **Vector DB Output** | Native JSONL | &mdash; | &mdash; | &mdash; | &mdash; |
| **CLI + Library** | Rust/Python/Node | CLI only | CLI | IDE | IDE |
| **Price** | Free/OSS | Free/OSS | Free/OSS | Free tier | $20/mo |

---

## Installation

| Method | Command |
|--------|---------|
| **npm** (recommended) | `npm install -g infiniloom` |
| **Homebrew** (macOS) | `brew tap Topos-Labs/infiniloom && brew install --cask infiniloom` |
| **Cargo** | `cargo install infiniloom` |
| **pip** (Python library) | `pip install infiniloom` |
| **From source** | `git clone https://github.com/Topos-Labs/infiniloom && cd infiniloom && cargo build --release` |

Shell completions: `infiniloom completions bash|zsh|fish|powershell|elvish`. See [docs](docs/getting-started/installation.md) for setup.

---

## Project Status

**Stable and actively maintained.**

- 23 languages with full Tree-sitter AST parsing (including Zig, Dart, HCL/Terraform)
- Production-ready `embed` for RAG at scale (streaming, SQLite manifests, pgvector schemas)
- Document ingestion (`ingest`) for Markdown, HTML, CSV, DOCX, XLSX
- All output formats: XML, Markdown, YAML, JSON, TOON
- Security scanning (15+ patterns) and PII redaction
- Python and Node.js bindings

**Coming next:** MCP server for Claude Desktop &bull; GitHub Action &bull; VS Code extension

---

## Documentation

| | |
|---|---|
| **Getting Started** | [Quick Start](docs/QUICK_START_GUIDE.md) &bull; [Installation](docs/getting-started/installation.md) &bull; [Configuration](docs/CONFIGURATION.md) |
| **Integration Guides** | [Claude Code](docs/guides/claude-code-integration.md) &bull; [Codex / GPT](docs/guides/codex-integration.md) &bull; [CI/CD](docs/guides/ci-integration.md) |
| **Reference** | [All Commands](docs/commands/) &bull; [Full Reference](docs/REFERENCE.md) &bull; [Recipes](docs/RECIPES.md) |
| **Deep Dives** | [Languages (23)](docs/LANGUAGES.md) &bull; [Tokenizers (27)](docs/TOKENIZERS.md) &bull; [Output Formats](docs/INFINILOOM_OUTPUT_FORMATS.md) |
| **Support** | [FAQ](docs/FAQ.md) &bull; [Troubleshooting](docs/TROUBLESHOOTING.md) &bull; [Large Repos Guide](docs/guides/large-repos.md) |
| **API Bindings** | [Python](docs/api/python.md) &bull; [Node.js](docs/api/nodejs.md) |

---

## Contributing

We welcome contributions of all kinds.

- **Found a bug?** [Open an issue](https://github.com/Topos-Labs/infiniloom/issues)
- **Have an idea?** Start a [discussion](https://github.com/Topos-Labs/infiniloom/discussions)
- **Want to contribute code?** See [CONTRIBUTING.md](CONTRIBUTING.md)

```bash
cargo test --workspace && cargo clippy --workspace && cargo fmt --all
```

---

## License

MIT &mdash; see [LICENSE](LICENSE).

<div align="center">
<br>
<sub>Built with Rust by <a href="https://github.com/Topos-Labs">Topos Labs</a></sub>
</div>
