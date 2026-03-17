<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/Topos-Labs/infiniloom/main/assets/logo-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/Topos-Labs/infiniloom/main/assets/logo-light.svg">
  <img alt="Infiniloom" src="https://raw.githubusercontent.com/Topos-Labs/infiniloom/main/assets/logo-light.svg" width="320">
</picture>

# Infiniloom

**Code intelligence engine for LLMs, AI agents, and RAG pipelines**

Understands your codebase. Ranks what matters. Generates context that makes AI actually useful.

[![CI](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml/badge.svg)](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/Topos-Labs/infiniloom/graph/badge.svg)](https://codecov.io/gh/Topos-Labs/infiniloom)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/infiniloom.svg)](https://crates.io/crates/infiniloom)
[![npm](https://img.shields.io/npm/v/infiniloom.svg)](https://www.npmjs.com/package/infiniloom)
[![PyPI](https://img.shields.io/pypi/v/infiniloom.svg)](https://pypi.org/project/infiniloom/)

[Install](#install) · [What It Does](#what-infiniloom-does) · [Use Cases](#use-cases) · [Commands](#commands) · [RAG](#for-rag--vector-databases) · [Docs](#documentation)

</div>

## Install

```bash
npm install -g infiniloom
```

Also available via `brew install infiniloom`, `cargo install infiniloom`, or `pip install infiniloom`.

## What Infiniloom Does

Infiniloom parses your codebase with Tree-sitter, builds a dependency graph, ranks symbols with PageRank, and generates structured context that fits your model's token budget. It's not a file concatenator — it understands code.

```bash
# Understand what's in a repo
infiniloom scan .
# → 847 files, 12 languages, 142K tokens (Claude), 6.2s

# Generate ranked, compressed context for Claude
infiniloom pack . -o context.xml
# → XML with symbol map, dependency graph, ranked files, git history

# Generate context for GPT with exact tiktoken counts
infiniloom pack . -f markdown -m gpt4o --max-tokens 80000 -o ctx.md

# See what depends on a file before you change it
infiniloom index . && infiniloom impact . src/auth.rs
# → 12 files depend on auth.rs, 3 test files, call graph with 47 edges

# Get diff context that includes callers and callees, not just changed lines
infiniloom diff --staged --include-diff --depth 2 -o review.xml

# Generate chunks for your vector database
infiniloom embed . -o chunks.jsonl
# → 1,203 content-addressable chunks with call graphs, tags, and signatures
```

### What Makes It Different

| Capability | What It Actually Does |
|---|---|
| **AST parsing** | Tree-sitter super-queries extract symbols, signatures, docstrings, and call relationships in a single pass across 23 languages |
| **PageRank ranking** | Builds a symbol graph from imports/calls/inheritance, runs PageRank (damping 0.85, parallel for >100 nodes), filters generic accessors |
| **Smart diff expansion** | Classifies changes (deletion 1.5x, signature 1.3x, docs 0.3x) and expands context proportionally — deletions get more callers included |
| **Content-addressable chunks** | BLAKE3 hashing with Unicode NFC normalization. Same code anywhere = same ID. Enables cross-repo deduplication and incremental RAG updates |
| **Exact token counting** | tiktoken for all OpenAI models (o200k, cl100k). Calibrated estimation (~95% prose, ~85% code) for Claude, Gemini, Llama, and 20+ others |
| **Security scanning** | 30+ regex patterns detect AWS keys, GitHub tokens, private keys, JWTs, database URLs, Slack webhooks. NFKC normalization catches homoglyph attacks |
| **Cache-optimized output** | XML output marks cacheable vs dynamic sections for Claude prompt caching |
| **Document distillation** | 5-stage pipeline (strip, dedup, compress, score, arrange) grounded in LLMLingua research showing 17-21% accuracy improvement |

---

## Use Cases

### With AI Coding Agents

Works with [Claude Code](docs/guides/claude-code-integration.md), [Codex](docs/guides/codex-integration.md), Gemini CLI, OpenCode, and any terminal-based AI tool:

```bash
# Feed structured context to Claude Code
infiniloom pack . -f xml --redact-secrets | claude "Review this codebase for security issues"

# PR review with dependency context for Codex
infiniloom diff main..feature --include-diff --depth 2 -f markdown | codex "Review these changes"

# Check blast radius before asking any agent to refactor
infiniloom impact . src/core/parser.rs --depth 3 --call-graph
```

### With AI SDKs

**Anthropic Claude SDK:**
```python
from anthropic import Anthropic
import subprocess

context = subprocess.run(["infiniloom", "pack", ".", "-f", "xml", "--redact-secrets",
                          "--max-tokens", "80000"], capture_output=True, text=True).stdout

client = Anthropic()
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=4096,
    messages=[{"role": "user", "content": f"{context}\n\nExplain the authentication flow."}]
)
```

**Vercel AI SDK / Mastra / any TypeScript agent framework:**
```typescript
import { pack } from 'infiniloom-node';
import { streamText } from 'ai';
import { anthropic } from '@ai-sdk/anthropic';

const context = pack('.', { format: 'xml', model: 'claude', tokenBudget: 60000 });
const result = streamText({
  model: anthropic('claude-sonnet-4-20250514'),
  system: `Codebase context:\n${context}`,
  messages,
});
```

**LangChain / LlamaIndex (RAG):**
```bash
infiniloom embed . -o chunks.jsonl --max-tokens 1000
# → Load chunks.jsonl into Pinecone/Weaviate/Qdrant/ChromaDB
# Each chunk has: id, content, symbol, signature, calls, called_by, tags
```

### As an MCP Server (Coming Soon)

Infiniloom will expose `pack`, `map`, `diff`, `impact`, and `embed` as [MCP](https://modelcontextprotocol.io/) tools — making them available to Claude Desktop, Claude Code, Codex, and any MCP-compatible client without custom integration.

### In CI/CD

```yaml
# .github/workflows/ai-review.yml
- run: infiniloom diff origin/main..HEAD --include-diff --redact-secrets -o context.xml
- run: infiniloom embed . --diff -o updated-chunks.jsonl  # Incremental RAG update
```

---

## Commands

| Command | What It Does |
|---------|--------------|
| **`pack`** | Generate AI-ready context (XML, Markdown, YAML, TOON, JSON) |
| **`scan`** | Repository statistics — files, tokens across 27 models, languages |
| **`map`** | PageRank-ranked symbol overview of the most important code |
| **`embed`** | Content-addressable chunks for vector databases and RAG |
| **`diff`** | Context for code changes — includes callers, callees, and related tests |
| **`index`** | Build symbol index and dependency graph (powers `diff` and `impact`) |
| **`impact`** | Analyze what depends on a file or symbol — blast radius with call graph |
| **`chunk`** | Split repo into token-budgeted chunks for multi-turn conversations |
| **`ingest`** | Convert documents (Markdown, HTML, CSV, DOCX, XLSX) with PII redaction |
| **`init`** | Create `.infiniloom.yaml` with language-specific templates |
| **`info`** | Show supported models, formats, and configuration |

**Output formats:** XML (Claude) · Markdown (GPT/Codex) · YAML (Gemini) · TOON (~40% smaller) · JSON (pipelines)

---

## For RAG & Vector Databases

`embed` generates deterministic, AST-aware chunks designed for retrieval:

```json
{
  "id": "ec_a1b2c3d4e5f6g7h8",
  "content": "async fn authenticate(token: &str) -> Result<User, AuthError> {...}",
  "tokens": 245,
  "kind": "function",
  "source": { "file": "src/auth.rs", "symbol": "authenticate", "fqn": "src::auth::authenticate" },
  "context": {
    "signature": "async fn authenticate(token: &str) -> Result<User, AuthError>",
    "calls": ["verify_jwt", "find_user_by_id"],
    "called_by": ["login_handler", "refresh_token"],
    "tags": ["async", "security", "public-api"],
    "cyclomatic_complexity": 4
  }
}
```

**Key properties:** Content-addressable IDs (BLAKE3) · AST-aware boundaries · Incremental manifest diffing · Call graph context · Hierarchical parent-child linking · Auto-generated semantic tags · Streaming mode for large repos · pgvector/Neptune export

Works with **Pinecone, Weaviate, Qdrant, ChromaDB, pgvector, Milvus** — or any system that accepts JSONL.

---

## Performance

| Metric | Value |
|--------|-------|
| 100 files | ~400ms |
| 5,000 files | ~8 seconds |
| Languages | 23 (Tree-sitter AST) |
| Tokenizers | 27 models (exact tiktoken for OpenAI, calibrated for rest) |
| Secret patterns | 30+ (AWS, GitHub, OpenAI, Stripe, SSH keys, JWTs, DB strings, ...) |
| Parallelism | Thread-local parsers, zero mutex contention (Rayon) |

---

## Documentation

| | |
|---|---|
| **Getting Started** | [Quick Start](docs/QUICK_START_GUIDE.md) · [Installation](docs/getting-started/installation.md) · [Configuration](docs/CONFIGURATION.md) |
| **Integration Guides** | [Claude Code](docs/guides/claude-code-integration.md) · [Codex / GPT](docs/guides/codex-integration.md) · [CI/CD](docs/guides/ci-integration.md) |
| **Reference** | [All Commands](docs/commands/) · [Full Reference](docs/REFERENCE.md) · [Recipes](docs/RECIPES.md) |
| **Deep Dives** | [Languages (23)](docs/LANGUAGES.md) · [Tokenizers (27)](docs/TOKENIZERS.md) · [Output Formats](docs/INFINILOOM_OUTPUT_FORMATS.md) |
| **Support** | [FAQ](docs/FAQ.md) · [Troubleshooting](docs/TROUBLESHOOTING.md) · [Large Repos](docs/guides/large-repos.md) |
| **API Bindings** | [Python](docs/api/python.md) · [Node.js](docs/api/nodejs.md) |

---

## Contributing

- **Found a bug?** [Open an issue](https://github.com/Topos-Labs/infiniloom/issues)
- **Have an idea?** Start a [discussion](https://github.com/Topos-Labs/infiniloom/discussions)
- **Want to contribute?** See [CONTRIBUTING.md](CONTRIBUTING.md)

```bash
cargo test --workspace && cargo clippy --workspace && cargo fmt --all
```

## License

MIT — see [LICENSE](LICENSE).

<div align="center">
<sub>Built with Rust by <a href="https://github.com/Topos-Labs">Topos Labs</a></sub>
</div>
