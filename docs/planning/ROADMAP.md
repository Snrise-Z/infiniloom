# Infiniloom Roadmap

**Last Updated:** 2026-03-15
**Current Version:** 0.7.0

Features are prioritized by ROI (user impact / implementation effort). Checkboxes indicate progress.

---

## Next — High Priority

### MCP Server Integration
> Native integration with Claude Desktop, Claude Code, Cursor, and other MCP clients.

- [ ] Expose `pack` as MCP tool
- [ ] Expose `scan` as MCP tool
- [ ] Expose `map` as MCP tool
- [ ] Expose `diff` as MCP tool
- [ ] Expose `impact` as MCP tool
- [ ] Resource provider for repository files
- [ ] Automatic context selection based on conversation

**Why Critical:** MCP is the standard for AI-tool integration. Small effort since core functionality exists.

---

### GitHub Action
> Official GitHub Action for CI/CD integration.

- [ ] Create `infiniloom/pack-action` repository
- [ ] Publish to GitHub Marketplace
- [ ] Support all pack options (format, model, compression)
- [ ] Output artifact upload
- [ ] Documentation and examples

**Why High:** Very common CI use case. Trivial to implement (wrapper around CLI).

**Usage:**
```yaml
- uses: infiniloom/pack-action@v1
  with:
    path: ./
    format: xml
    output: context.xml
```

---

## Planned — Medium Priority

### Smart Context Selection
> Given a task description, automatically select the most relevant files.

- [ ] Task-aware file selection using PageRank + index
- [ ] CLI: `infiniloom context "implement auth" --budget 50000`
- [ ] Integration with MCP for conversational context building
- [ ] Relevance scoring output

**Why Valuable:** Solves the "what files should I include?" problem. Leverages existing infrastructure.

---

### VS Code Extension
> Native VS Code integration.

- [ ] Command: "Infiniloom: Pack Repository"
- [ ] Command: "Infiniloom: Pack Selected Files"
- [ ] Status bar token count
- [ ] Right-click context menu
- [ ] Configuration UI
- [ ] Publish to VS Code Marketplace

**Note:** MCP integration may reduce the need for this, as Claude Code integrates via MCP.

---

### Prompt Templates
> Pre-built templates for common development tasks.

- [ ] `code-review` — Review changes for bugs, style, security
- [ ] `explain` — Explain how code works
- [ ] `document` — Generate documentation
- [ ] `refactor` — Suggest refactoring improvements
- [ ] `test` — Generate test cases
- [ ] CLI: `infiniloom pack . --template code-review`
- [ ] Custom template support from config file

**Why Valuable:** Zero-effort implementation (markdown templates). Helps users get better LLM results.

---

### Watch Mode for Index
> Automatic index updates on file changes.

- [ ] `infiniloom index --watch` command
- [ ] File system event detection
- [ ] Incremental re-indexing on change
- [ ] Integration with IDE extensions

**Current:** `infiniloom pack --watch` exists. Index requires manual rebuild.

---

## Backlog — Low ROI

### Direct LLM Integration
> Built-in LLM querying with automatic context management.

- [ ] `infiniloom ask "question" [path]` command
- [ ] Support ANTHROPIC_API_KEY and OPENAI_API_KEY
- [ ] Automatic context sizing based on model
- [ ] Streaming responses
- [ ] Cost estimation

**CLI:**
```bash
infiniloom ask "explain authentication flow" src/auth/
infiniloom ask "find security issues" --staged
```

---

### GitHub App
> GitHub App for PR integration.

- [ ] Bot command: `/infiniloom context`
- [ ] Auto-comment PR context for AI review
- [ ] Webhook integration
- [ ] GitHub App listing

---

### JetBrains Plugin
> Plugin for IntelliJ-based IDEs.

- [ ] Basic pack/scan functionality
- [ ] Tool window UI
- [ ] Publish to JetBrains Marketplace

---

### Semantic Code Embeddings
> Neural embeddings for semantic search.

- [ ] CodeBERT/StarCoder integration via candle
- [ ] Vector store (in-memory, SQLite)
- [ ] Semantic search CLI

**Rationale for Low Priority:** Large effort, unclear value when LLMs understand code semantically.

---

### Trigram Search
> Fast code search using trigram indexing.

- [ ] Trigram index builder
- [ ] Regex search
- [ ] BM25 ranking

**Rationale for Low Priority:** Ripgrep/grep already excellent. Not core to mission.

---

### Multi-Modal Context
> Include images/diagrams in context.

- [ ] Include PNG/SVG from docs/
- [ ] Auto-generate UML diagrams
- [ ] Base64 encoding option

---

### Plugin SDK
> Extensible plugin system.

- [ ] Plugin API design
- [ ] Language enhancer plugins
- [ ] Output exporter plugins

**Rationale for Low Priority:** Too early for extensibility. Focus on core features first.

---

## Completed

### v0.7.0
- [x] Streaming JSONL output for embed command (`--streaming`)
- [x] SQLite manifest storage (`--sqlite-manifest`)
- [x] Parent/children chunk linking with hierarchy
- [x] Type signature extraction across 21 languages
- [x] Cross-repository identity (FQN metadata)
- [x] BM25-friendly identifier extraction
- [x] Cyclomatic complexity scoring per chunk
- [x] Heuristic NL summaries per chunk
- [x] Git metadata enrichment per chunk
- [x] Signature-only chunks for tiered retrieval
- [x] Import-aware call graph
- [x] Neptune graph export (`--graph-export`)
- [x] pgvector schema generation (`--generate-schema pgvector`)
- [x] Git-diff incremental updates (`--since`)
- [x] Zig language support (full Tree-sitter)
- [x] Dart language support (full Tree-sitter, including inheritance)
- [x] HCL extended queries (locals, dynamic blocks, modules)
- [x] Extended secret detection (GCP, Azure, HuggingFace)
- [x] International PII detection patterns
- [x] Atomic manifest writes (PID-unique temp files)
- [x] Poison-recovery on Mutex/RwLock operations
- [x] Document ingestion (Markdown, HTML, CSV, DOCX, XLSX)

### v0.6.x
- [x] Embed command for vector database chunking
- [x] Ingest command for document processing
- [x] PDF ingestion support
- [x] ZIP bomb protection in DOCX parser
- [x] HTML parser DoS limits

### v0.4.x
- [x] AST-based symbol extraction (21 languages)
- [x] PageRank-based symbol ranking
- [x] Model-specific output formats (XML, Markdown, YAML, JSON, TOON)
- [x] Secret detection and redaction
- [x] Git context engine (index, diff, impact)
- [x] Python bindings (PyPI: `infiniloom`)
- [x] Node.js bindings (npm: `infiniloom-node`)
- [x] Call graph query API
- [x] Incremental caching
- [x] Watch mode for pack
- [x] Remote repository support (github:owner/repo)
- [x] Chunking strategies (semantic, module, dependency)

### v0.3.x
- [x] Initial Git integration
- [x] npm and PyPI packages

### v0.2.x
- [x] Semantic compression (heuristic)
- [x] Fuzz testing

### v0.1.x
- [x] pack, scan, map commands
- [x] Tree-sitter parsing

---

## Not Planned

| Feature | Reason |
|---------|--------|
| SCIP Navigation | IDEs have this built-in |
| GPU token counting | CPU is fast enough |
| Cloud-hosted version | Focus on local-first privacy |
| Nickel config | Over-engineering; YAML/TOML sufficient |
| Custom language grammars | 21 languages cover 99% of use cases |

---

## Contributing

**Most Wanted:**
1. MCP Server implementation
2. GitHub Action wrapper
3. Smart Context Selection

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.
