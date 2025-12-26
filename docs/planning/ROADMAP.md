# Infiniloom Roadmap

**Status:** Planning Document
**Last Updated:** 2025-12-25
**Current Version:** 0.4.8

This document outlines planned features for future versions of Infiniloom. Features are prioritized by ROI (user impact / implementation effort).

---

## Overview

Infiniloom has a solid foundation with:
- AST-based symbol extraction (22 languages)
- PageRank-based ranking
- Model-specific output formats
- Secret detection and redaction
- Git context engine (index, diff, impact)
- Python and Node.js bindings

---

## Priority 1: Highest ROI (Next Release)

### 1.1 MCP Server Integration

**Priority:** Critical
**Effort:** Small
**Target:** v0.5.0

Model Context Protocol server for direct integration with Claude Desktop, Claude Code, and other MCP-compatible clients.

**Capabilities:**
- Expose `pack`, `scan`, `map`, `diff`, `impact` as MCP tools
- Resource access for repository files
- Automatic context selection based on conversation

**Why Critical:**
- MCP is becoming the standard for AI-tool integration
- Small effort (core functionality exists, just needs MCP wrapper)
- Eliminates copy-paste friction entirely
- Works with Claude Desktop, Cursor, and other MCP clients

---

### 1.2 Streaming Output

**Priority:** High
**Effort:** Medium
**Target:** v0.5.0

Stream output for massive repositories and real-time API integrations.

**Capabilities:**
- Chunked output generation for 100K+ file repos
- Memory-limited processing
- Progress callbacks
- Integration with MCP streaming

**Why High Priority:**
- Required for MCP integration with large repos
- Enables real-time processing
- Current blocking I/O limits scalability

**API:**
```python
async for chunk in infiniloom.stream("/huge/repo"):
    await send_to_api(chunk)
```

---

### 1.3 GitHub Action

**Priority:** High
**Effort:** Small
**Target:** v0.5.0

Official GitHub Action for CI/CD integration.

**Usage:**
```yaml
- uses: infiniloom/pack-action@v1
  with:
    path: ./
    format: xml
    output: context.xml
```

**Why High Priority:**
- Very common use case (CI-generated context)
- Trivial to implement (wrapper around CLI)
- Increases adoption in team workflows

---

## Priority 2: High ROI (v0.6.0)

### 2.1 Smart Context Selection

**Priority:** High
**Effort:** Medium

Given a task description, automatically select the most relevant files.

**Capabilities:**
- Task-aware file selection using existing PageRank + index
- "What files do I need to understand X?" → returns optimized context
- Integration with MCP for conversational context building

**CLI:**
```bash
infiniloom context "implement user authentication" --budget 50000
```

**Why Valuable:**
- Solves the "what files should I include?" problem
- Leverages existing infrastructure (index, PageRank)
- High user value, medium implementation effort

---

### 2.2 Prompt Templates

**Priority:** High
**Effort:** Small

Pre-built prompt templates for common development tasks.

**Templates:**
- `code-review` - Review changes for bugs, style, security
- `explain` - Explain how code works
- `document` - Generate documentation
- `refactor` - Suggest refactoring improvements
- `test` - Generate test cases

**CLI:**
```bash
infiniloom pack . --template code-review --staged
infiniloom pack . --template explain --include "src/auth/*"
```

**Why Valuable:**
- Zero-effort implementation (just markdown templates)
- Immediately useful for common workflows
- Helps users get better results from LLMs

---

### 2.3 Watch Mode for Index

**Priority:** Medium
**Effort:** Small

Automatic index updates on file changes.

**Current State:**
- `infiniloom pack --watch` exists
- Index requires manual `infiniloom index` run

**Planned:**
- `infiniloom index --watch` for continuous updates
- File system events trigger incremental re-indexing

---

### 2.4 VS Code Extension

**Priority:** Medium
**Effort:** Medium

Native VS Code integration.

**Features:**
- Command: "Pack Repository" → clipboard
- Command: "Pack Selected Files"
- Status bar token count
- Right-click context menu

**Note:** MCP integration (1.1) may reduce the need for this, as Claude Code already integrates via MCP.

---

## Priority 3: Medium ROI (Future)

### 3.1 Direct LLM Integration

**Priority:** Medium
**Effort:** Medium

Built-in LLM querying with automatic context management.

**CLI:**
```bash
# Uses ANTHROPIC_API_KEY or OPENAI_API_KEY
infiniloom ask "explain how authentication works" src/auth/
infiniloom ask "find security vulnerabilities" --staged
```

**Considerations:**
- Requires API key management
- May overlap with MCP use case
- Could be valuable for scripting/CI

---

### 3.2 GitHub App

**Priority:** Medium
**Effort:** Medium

GitHub App for PR integration.

**Features:**
- Bot command: `/infiniloom context` in PR comments
- Auto-comment with context for AI review
- Integration with GitHub Actions

---

### 3.3 JetBrains Plugin

**Priority:** Low
**Effort:** Medium

Plugin for IntelliJ-based IDEs.

---

## Priority 4: Low ROI (Backlog)

### 4.1 Semantic Code Embeddings

**Priority:** Low
**Effort:** Large

Neural embeddings for semantic search.

**Rationale for Low Priority:**
- Large implementation effort (ML infrastructure)
- LLMs already understand code semantically
- Unclear user demand
- Can revisit if there's clear need

---

### 4.2 Trigram Search

**Priority:** Low
**Effort:** Medium

Fast code search using trigram indexing.

**Rationale for Low Priority:**
- Ripgrep/grep already excellent
- Not core to Infiniloom's mission
- Can use existing tools

---

### 4.3 Multi-Modal Context

**Priority:** Low
**Effort:** Medium

Include images/diagrams in context.

---

### 4.4 Plugin SDK

**Priority:** Low
**Effort:** Large

Extensible plugin system.

**Rationale for Low Priority:**
- Too early for extensibility
- Focus on core features first

---

## Not Planned

| Feature | Reason |
|---------|--------|
| SCIP Navigation | IDEs have this built-in |
| GPU token counting | CPU is fast enough |
| Cloud-hosted version | Focus on local-first privacy |
| Nickel config language | Over-engineering; YAML/TOML sufficient |
| Real-time collaboration | Out of scope for CLI tool |

---

## Release Plan

| Version | Target | Features |
|---------|--------|----------|
| **0.5.0** | Q1 2025 | MCP Server, Streaming, GitHub Action |
| **0.6.0** | Q2 2025 | Smart Context, Prompt Templates, Index Watch |
| **0.7.0** | Q3 2025 | VS Code Extension, Direct LLM Integration |

---

## Contributing

If you'd like to contribute to any of these features:

1. Open an issue on GitHub to discuss
2. Check existing issues tagged `roadmap` or `enhancement`
3. PRs welcome for any planned features

**Most Wanted Contributions:**
- MCP Server implementation
- GitHub Action wrapper
- Prompt templates
