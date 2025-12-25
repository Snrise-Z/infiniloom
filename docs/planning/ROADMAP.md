# Infiniloom Roadmap

**Status:** Planning Document
**Last Updated:** 2025-01-24
**Current Version:** 0.4.3

This document outlines planned features for future versions of Infiniloom. These features are **not yet implemented** and are subject to change based on community feedback and priorities.

---

## Overview

Infiniloom has a solid foundation with:
- AST-based symbol extraction (21 languages)
- PageRank-based ranking
- Model-specific output formats
- Secret detection and redaction
- Git context engine (index, diff, impact)
- Python and Node.js bindings

The features below represent the next phase of development.

---

## Phase 2: Advanced Intelligence

### 2.1 Semantic Code Embeddings

**Priority:** High
**Estimated Effort:** Large

Neural embeddings for semantic search and similarity using CodeBERT/StarCoder.

**Planned Capabilities:**
- Vector-based code search ("find functions that handle authentication")
- Semantic similarity between code sections
- Embedding-enhanced context selection
- Support for local on-device embeddings (privacy)

**Potential API:**
```python
from infiniloom.embeddings import CodeEmbedder

embedder = CodeEmbedder(model="starcoderbase", on_device=True)
embedder.index("/path/to/repo")
results = embedder.search("function that handles user authentication", top_k=5)
```

**Implementation Notes:**
- Would integrate with candle-transformers (already an optional dependency)
- Vector store options: in-memory, SQLite, or external (Qdrant/ChromaDB)
- Current `semantic.rs` uses character-frequency heuristics as a placeholder

---

### 2.2 Trigram Search (Zoekt-Style)

**Priority:** Medium
**Estimated Effort:** Medium

Fast code search using trigram indexing for interactive use cases.

**Planned Capabilities:**
- Regex search across codebase
- Symbol and definition search
- BM25 ranking for results
- Sub-second search on large repos

**Potential CLI:**
```bash
infiniloom search "def authenticate" --language python --path "src/**"
```

---

### 2.3 SCIP-Based Navigation

**Priority:** Low
**Estimated Effort:** Large

Integration with Sourcegraph's SCIP for precise code intelligence.

**Planned Capabilities:**
- Precise go-to-definition
- Find all references
- Find implementations of interfaces/traits
- Hover documentation

---

## Phase 2: Platform Integration

### 2.4 IDE Extensions

**Priority:** High
**Estimated Effort:** Medium

Native IDE integrations for VS Code, JetBrains, and Neovim.

**VS Code Extension Features:**
- Command: "Pack Repository" → generates context to clipboard
- Command: "Pack Selected Files" → partial context
- Status bar token count
- Configuration UI

**JetBrains Plugin:**
- Similar functionality via IntelliJ Platform SDK

**Neovim:**
- Lua plugin using CLI backend

---

### 2.5 MCP Server Integration

**Priority:** High
**Estimated Effort:** Small

Model Context Protocol server for direct integration with Claude and other MCP-compatible clients.

**Planned Capabilities:**
- Expose `pack`, `scan`, `map`, `diff` as MCP tools
- Automatic context selection based on conversation
- Resource access for repository files

---

### 2.6 GitHub App / Action

**Priority:** Medium
**Estimated Effort:** Medium

GitHub integration for automated context generation.

**GitHub Action:**
```yaml
- uses: infiniloom/context-action@v1
  with:
    path: ./
    format: xml
    output: context.xml
```

**GitHub App:**
- Auto-comment PR context for AI review
- Bot command: `/infiniloom context` in PR comments

---

## Phase 2: Advanced Processing

### 2.7 Streaming with Backpressure

**Priority:** Medium
**Estimated Effort:** Medium

Handle massive repositories (100K+ files) with streaming output.

**Planned Capabilities:**
- Chunked output generation
- Memory-limited processing
- Progress callbacks
- Pause/resume support

**Potential API:**
```python
async for chunk in infiniloom.stream("/huge/repo"):
    await send_to_api(chunk)
```

---

### 2.8 Multi-Modal Context

**Priority:** Low
**Estimated Effort:** Medium

Include visual context for multi-modal LLMs.

**Planned Capabilities:**
- Include architecture diagrams (PNG/SVG from docs/)
- Auto-generate UML class diagrams from code
- Include UI screenshots for frontend code
- Base64 encoding or URL references

---

### 2.9 Watch Mode for Index

**Priority:** Medium
**Estimated Effort:** Small

Automatic index updates on file changes.

**Current State:**
- `infiniloom pack --watch` exists (regenerates output on changes)
- Index requires manual `infiniloom index` run

**Planned:**
- `infiniloom index --watch` for continuous index updates
- Integration with IDE extensions for real-time navigation

---

## Phase 2: Extensibility

### 2.10 Plugin SDK

**Priority:** Low
**Estimated Effort:** Large

Extensible plugin system for custom analyzers and transformers.

**Planned Plugin Types:**
- Language enhancers (additional analysis per language)
- Output exporters (Notion, Confluence, etc.)
- Analysis plugins (complexity, duplication)
- Integration plugins (Jira, Linear, Slack)

**Potential API:**
```python
from infiniloom.plugins import Plugin, hook

class MyPlugin(Plugin):
    @hook("file.read")
    def process_file(self, file):
        # Custom file processing
        return file
```

---

### 2.11 Configuration Language (Nickel)

**Priority:** Low
**Estimated Effort:** Medium

Advanced typed configuration using Nickel.

**Current State:**
- YAML/TOML/JSON config supported
- Basic validation

**Planned:**
- Type contracts for config validation
- Custom transformer definitions
- Composable configuration

---

## Not Planned

The following features have been evaluated but are **not currently planned**:

| Feature | Reason |
|---------|--------|
| GPU token counting | CPU is fast enough, adds complexity |
| Real-time collaboration | Out of scope for CLI tool |
| Cloud-hosted version | Focus on local-first privacy |
| Custom language support | 21 languages cover 99% of use cases |

---

## Contributing to Roadmap

If you'd like to contribute to any of these features or suggest new ones:

1. Open an issue on GitHub to discuss
2. Check existing issues tagged `roadmap` or `phase-2`
3. PRs welcome for any planned features

---

## Version History

| Version | Features |
|---------|----------|
| 0.1.0 | Initial release - pack, scan, map |
| 0.2.0 | Semantic compression, fuzz testing |
| 0.3.x | Git bindings, Call Graph API, npm/PyPI packages |
| 0.4.x | Architecture refactoring, documentation, UTF-8 safety |
| 0.5.0 (planned) | IDE extensions, MCP server |
| 0.6.0 (planned) | Embeddings, semantic search |
