<div align="center">

# Infiniloom

**AST-aware code context engine for RAG, vector databases, and AI code assistants**

A high-performance Rust library and CLI for generating intelligent code context for LLMs. Uses Tree-sitter AST parsing (22 languages), PageRank symbol ranking, and BLAKE3 content-addressable hashing. Integrates with Pinecone, Weaviate, Qdrant, ChromaDB, and any vector database. Optimized for Claude, GPT-5, GPT-4o, Gemini, Llama, and 27+ LLM tokenizers.

[![CI](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml/badge.svg)](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/Topos-Labs/infiniloom/graph/badge.svg)](https://codecov.io/gh/Topos-Labs/infiniloom)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/infiniloom.svg)](https://crates.io/crates/infiniloom)
[![npm](https://img.shields.io/npm/v/infiniloom.svg)](https://www.npmjs.com/package/infiniloom)
[![PyPI](https://img.shields.io/pypi/v/infiniloom.svg)](https://pypi.org/project/infiniloom/)

</div>

---

## Try It Now

```bash
# Install
npm install -g infiniloom

# Generate AI-ready context in one command
infiniloom pack . --output context.xml
# → Paste into Claude, GPT, or any LLM

# Generate chunks for your vector database
infiniloom embed . -o chunks.jsonl
# → Import into Pinecone, Weaviate, Qdrant, etc.
```

---

## The Problem

When you ask an AI to help with code, quality depends almost entirely on what context you provide. Most approaches fail:

- **Pasting random files** gives the AI fragments without structure or relationships
- **Dumping entire repositories** overwhelms the AI with noise and irrelevant code
- **Token limits** force you to leave out important context, leading to incorrect suggestions
- **AI doesn't know what it doesn't know** — it can't ask for the files it needs
- **Copy-paste workflows** are slow, error-prone, and don't scale
- **Every question requires re-gathering context** from scratch

The result: AI gives generic answers, hallucinates function signatures, or misses critical dependencies — not because the AI is bad, but because the context is bad.

---

## What Infiniloom Does

Infiniloom reads your codebase and produces a structured summary designed specifically for AI consumption.

Think of it like this: instead of handing someone a filing cabinet and saying "figure it out," you give them a well-organized briefing document that highlights what matters.

Here's how it works:

1. **Analyzes structure** — Infiniloom understands how your code is organized: which files exist, how they relate, what languages are used.

2. **Extracts meaning** — It identifies the important pieces: functions, classes, interfaces, types. Not just text, but semantic units that AI can reason about.

3. **Ranks importance** — Using techniques similar to how search engines rank web pages, it determines which code is central to your project and which is peripheral.

4. **Filters noise** — Generated files, build artifacts, vendored dependencies, and other distractions are automatically excluded.

5. **Formats for AI** — The output is structured in ways that different AI models understand best — XML for Claude, Markdown for GPT-4o/GPT-5, YAML for Gemini.

The result is a context package that helps AI give you accurate, relevant answers about your actual code.

---

## What You Can Do With It

### For Developers

- **AI-assisted code review** — Give your AI the context to understand what a pull request actually changes
- **Ask architectural questions** — "How does authentication flow through this system?"
- **Generate documentation** — Let AI explain your code with full visibility into dependencies
- **Onboard faster** — Understand a new codebase in hours instead of weeks
- **Debug complex issues** — Provide AI with the relevant code paths, not just the error message

### For AI / RAG / Agents

- **Build better context** — Prepare high-quality input for LLM applications
- **Reduce token usage** — Send what matters, not everything
- **Improve answer accuracy** — Relevant context produces relevant answers
- **Enable code-aware agents** — Give autonomous systems the context they need to act correctly

---

## For RAG & Vector Databases

Infiniloom's `embed` command generates deterministic, content-addressable code chunks designed specifically for retrieval-augmented generation:

```bash
# Generate chunks for your vector database
infiniloom embed ./my-repo -o chunks.jsonl

# Only get changed chunks (incremental updates)
infiniloom embed ./my-repo --diff -o updates.jsonl
```

### Key Features for RAG

| Feature | Benefit |
|---------|---------|
| **Content-Addressable IDs** | Same code anywhere produces same ID (`ec_a1b2c3d4...`) — enables cross-repo deduplication |
| **AST-Aware Chunking** | Never splits mid-function or mid-class — preserves semantic boundaries |
| **Incremental Updates** | Manifest-based diffing detects added/modified/removed chunks — only re-embed what changed |
| **Hierarchical Chunks** | Parent-child relationships preserved — container summaries link to member chunks |
| **Auto-Generated Tags** | Semantic tags (`async`, `security`, `database`, `http`) improve retrieval relevance |
| **Call Graph Context** | `calls` and `called_by` fields enable dependency-aware retrieval |

### Vector Database Integration

Works with any vector database that accepts JSON/JSONL:

```bash
# Pinecone / Weaviate / Qdrant
infiniloom embed . --max-tokens 1500 -o chunks.jsonl
# Import chunks.jsonl using your vector DB's bulk import

# ChromaDB / pgvector / Milvus
infiniloom embed . --format json -o chunks.json
# Parse JSON array and insert with your preferred client
```

### Chunk Output Format

Each chunk includes rich metadata for filtering and retrieval:

```json
{
  "id": "ec_a1b2c3d4e5f6g7h8",
  "content": "async fn authenticate(token: &str) -> Result<User, AuthError> {...}",
  "tokens": 245,
  "kind": "function",
  "source": {
    "file": "src/auth.rs",
    "symbol": "authenticate",
    "fqn": "src::auth::authenticate",
    "language": "Rust"
  },
  "context": {
    "docstring": "Validates JWT token and returns authenticated user",
    "calls": ["verify_jwt", "find_user_by_id"],
    "called_by": ["login_handler", "refresh_token"],
    "tags": ["async", "security", "public-api"]
  }
}
```

See the [embed command documentation](docs/commands/embed.md) for complete details.

---

## Quick Start

**Install:**

```bash
npm install -g infiniloom
```

**Generate context for your repository:**

```bash
infiniloom pack . --output context.xml
```

This produces an XML file containing your codebase's structure, key symbols, and content — ready to paste into Claude, GPT, or any other AI assistant.

---

## Core Capabilities

| Capability | Why It Matters |
|------------|----------------|
| **Repository analysis** | Understands project structure, languages, and file relationships automatically |
| **Symbol extraction** | Identifies functions, classes, and types — the units AI reasons about |
| **Importance ranking** | Highlights central code, deprioritizes utilities and boilerplate |
| **Noise reduction** | Excludes generated files, dependencies, and artifacts by default |
| **Security filtering** | Detects and redacts API keys, tokens, and credentials before they reach AI |
| **Multiple output formats** | XML, Markdown, YAML, JSON — optimized for different AI models |
| **Token-aware packaging** | Respects context limits so you can fit within model constraints |
| **Git integration** | Understands diffs, branches, and commit history for change-aware context |
| **22 language support** | Full parsing for Python, JavaScript, TypeScript, Rust, Go, Java, C/C++, and more |

---

## CLI Overview

| Command | What It Does |
|---------|--------------|
| `pack` | Analyze a repository and generate AI-ready context |
| `scan` | Show repository statistics: files, tokens, languages |
| `map` | Generate a ranked overview of key symbols |
| `embed` | Generate chunks for vector databases / RAG systems |
| `diff` | Build context focused on recent changes |
| `index` | Create a symbol index for fast queries |
| `impact` | Analyze what depends on a file or function |
| `chunk` | Split large repositories for multi-turn conversations |
| `init` | Create a configuration file |

See the [Command Reference](docs/commands/) for detailed documentation.

---

## Why Infiniloom?

| Feature | Benefit |
|---------|---------|
| 🎯 **Smart Ranking** | PageRank algorithm identifies important symbols — prioritizes core business logic over utilities |
| 🔗 **Content-Addressable** | BLAKE3 hashing produces stable chunk IDs — same code anywhere = same ID for deduplication |
| 🌳 **AST-Aware** | Tree-sitter parsing (22 languages) preserves semantic boundaries — never splits mid-function |
| 🔒 **Security-First** | Automatic secret detection with regex + NFKC normalization prevents API key leaks |
| 📊 **27+ Tokenizers** | Exact counts for GPT-5/4o via tiktoken, calibrated estimation for Claude/Gemini/Llama |
| 🚀 **Blazing Fast** | Pure Rust + Rayon parallelism — handles 100K+ file repos in seconds |
| 🔄 **Incremental** | Manifest-based diffing tracks added/modified/removed chunks — only re-embed what changed |
| 📈 **Call Graphs** | `calls` and `called_by` fields enable dependency-aware retrieval |

---

## How Infiniloom Compares

### Feature Comparison Matrix

| Feature | Infiniloom | Repomix | Aider | Continue | Cursor |
|---------|:----------:|:-------:|:-----:|:--------:|:------:|
| **AST Parsing (Tree-sitter)** | ✅ 22 languages | ❌ | ❌ | ❌ | ✅ |
| **PageRank Symbol Ranking** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Content-Addressable Chunks** | ✅ BLAKE3 | ❌ | ❌ | ❌ | ❌ |
| **Incremental Updates (Diffing)** | ✅ Manifest-based | ❌ | ✅ Git-based | ❌ | ✅ |
| **Secret Detection/Redaction** | ✅ 15+ patterns | ❌ | ❌ | ❌ | ❌ |
| **Multi-Model Token Counting** | ✅ 27 models | ❌ | ✅ Few models | ❌ | ❌ |
| **Call Graph Extraction** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Vector DB Integration** | ✅ Native JSONL | ❌ | ❌ | ❌ | ❌ |
| **Hierarchical Chunking** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **CLI Tool** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Library (Rust/Python/Node)** | ✅ | ✅ | ❌ | ❌ | ❌ |
| **IDE Integration** | 🔜 Coming | ❌ | ❌ | ✅ Native | ✅ Native |
| **Price** | Free/OSS | Free/OSS | Free/OSS | Free tier | $20/mo |

### When to Use What

| Tool | Best For | Not Ideal For |
|------|----------|---------------|
| **Infiniloom** | RAG pipelines, vector DBs, security-conscious teams, large codebases, CI/CD automation | Real-time IDE completions |
| **Repomix** | Quick one-off context dumps, small projects | Large repos, incremental updates, security |
| **Aider** | Interactive pair programming, git-based workflows | Headless automation, RAG systems |
| **Continue.dev** | IDE code completion, inline suggestions | Batch processing, RAG pipelines |
| **Cursor** | Full AI-powered development environment | Headless/CLI workflows, custom pipelines |

---

## How This Is Different

**Compared to "just paste the code":**

Infiniloom understands code structure. It knows the difference between a core business function and a utility helper. It understands imports, dependencies, and relationships. Pasting files gives AI text; Infiniloom gives AI understanding.

**Compared to generic RAG tools:**

Most RAG systems treat code as documents. They chunk by character count, embed text, and retrieve by similarity. This misses the structure that makes code meaningful. Infiniloom preserves semantic boundaries — functions stay whole, relationships stay intact.

**Compared to embedding-based approaches:**

Embeddings are useful for "find code similar to X." They're less useful for "understand how X works." Infiniloom focuses on comprehension: what exists, how it connects, what matters. This is about building complete context, not searching fragments.

**Our philosophy:**

Context quality beats context quantity. A smaller, well-structured context produces better AI responses than a larger, noisier one. Infiniloom prioritizes signal over volume.

---

## Who This Is For

**Good fit:**

- Developers using AI assistants for code review, debugging, or documentation
- Teams building AI-powered developer tools or code analysis products
- Engineers working with large or unfamiliar codebases
- Anyone who needs AI to understand real production code, not toy examples

**Probably not needed:**

- Single-file scripts or small utilities (just paste them directly)
- Projects where you already have perfect context (rare, but possible)
- Use cases where code search is more important than code comprehension

---

## Project Status

Infiniloom is **stable and actively maintained**.

**What's solid today:**
- Core packing workflow across 22 languages
- **NEW in v0.6.0**: `embed` command for vector database chunking
- All output formats (XML, Markdown, YAML, JSON)
- Security scanning and secret redaction
- Git-aware diff context
- Python and Node.js bindings

**Coming next:**
- MCP server integration for Claude Desktop and other MCP clients
- Streaming output for very large repositories
- GitHub Action for CI/CD workflows
- VS Code extension

---

## Installation Options

| Method | Command |
|--------|---------|
| **npm** (recommended) | `npm install -g infiniloom` |
| **Homebrew** (macOS) | `brew tap Topos-Labs/infiniloom && brew install --cask infiniloom` |
| **Cargo** (Rust users) | `cargo install infiniloom` |
| **pip** (Python library) | `pip install infiniloom` |
| **From source** | `git clone https://github.com/Topos-Labs/infiniloom && cd infiniloom && cargo build --release` |

---

## Shell Completions

Infiniloom supports tab completion for bash, zsh, fish, PowerShell, and Elvish.

### Bash

```bash
infiniloom completions bash > /tmp/infiniloom.bash
sudo mv /tmp/infiniloom.bash /etc/bash_completion.d/
```

### Zsh

```bash
infiniloom completions zsh > ~/.zfunc/_infiniloom
# Add to ~/.zshrc:
fpath=(~/.zfunc $fpath)
autoload -U compinit && compinit
```

### Fish

```bash
infiniloom completions fish > ~/.config/fish/completions/infiniloom.fish
```

### PowerShell

```powershell
infiniloom completions powershell | Out-String | Invoke-Expression
# Or add to your profile:
infiniloom completions powershell >> $PROFILE
```

### Elvish

```bash
infiniloom completions elvish > ~/.config/elvish/completions/infiniloom.elv
```

---

## Contributing

We welcome contributions of all kinds: bug reports, feature requests, documentation improvements, and code.

- **Found a bug?** [Open an issue](https://github.com/Topos-Labs/infiniloom/issues)
- **Have an idea?** Start a [discussion](https://github.com/Topos-Labs/infiniloom/discussions)
- **Want to contribute code?** See [CONTRIBUTING.md](CONTRIBUTING.md)

```bash
cargo test --workspace    # Run tests
cargo clippy --workspace  # Lint
cargo fmt --all           # Format
```

---

## Documentation

- [Cheat Sheet](docs/CHEATSHEET.md) — Quick reference
- [Command Reference](docs/commands/) — Detailed CLI documentation
- [Configuration Guide](docs/CONFIGURATION.md) — Config files and options
- [FAQ](docs/FAQ.md) — Common questions answered

---

## License

MIT — see [LICENSE](LICENSE).

---

<div align="center">

Made by [Topos Labs](https://github.com/Topos-Labs)

</div>
