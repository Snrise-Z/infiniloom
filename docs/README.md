# Infiniloom Documentation

Welcome to the Infiniloom documentation. This guide will help you get started and make the most of Infiniloom's features.

## Start Here

**New to Infiniloom?** Pick the guide that matches your goal:

| I want to... | Start here |
|--------------|------------|
| **Try Infiniloom in 2 minutes** | [Quick Start](getting-started/quick-start.md) |
| **Use it with Claude Code** | [Claude Code Integration](guides/claude-code-integration.md) |
| **Use it with OpenAI Codex** | [Codex Integration](guides/codex-integration.md) |
| **Build a RAG pipeline** | [Recipes: RAG Pipeline](RECIPES.md) |
| **Set up CI/CD** | [CI/CD Integration](guides/ci-integration.md) |
| **Handle a large codebase** | [Large Repos Guide](guides/large-repos.md) |
| **Review a PR with AI** | [Claude Code: Code Review](guides/claude-code-integration.md#workflow-2-code-review-with-infiniloom-diff) |
| **Scan for secrets** | [Configuration: Security](CONFIGURATION.md) |

## Quick Links

- **[Reference](REFERENCE.md)** - All commands and options at a glance
- **[Recipes](RECIPES.md)** - Ready-to-use code patterns
- **[FAQ](FAQ.md)** - Frequently asked questions
- **[Troubleshooting](TROUBLESHOOTING.md)** - Common issues and solutions

## Getting Started

| Guide | Description |
|-------|-------------|
| [Installation](getting-started/installation.md) | All installation methods (npm, Homebrew, Cargo, pip) |
| [Quick Start](getting-started/quick-start.md) | Your first 5 minutes with Infiniloom |
| [Configuration](CONFIGURATION.md) | Config files, environment variables, and `infiniloom init` |

## Commands

| Command | Description |
|---------|-------------|
| [pack](commands/pack.md) | Transform repository into LLM context |
| [scan](commands/scan.md) | Analyze repository statistics |
| [map](commands/map.md) | Generate symbol map with PageRank ranking |
| [embed](commands/embed.md) | Generate chunks for vector databases (RAG) |
| [diff](commands/diff.md) | Get context for code changes (requires index) |
| [index](commands/index.md) | Build symbol index for fast diff and impact queries |
| [impact](commands/impact.md) | Analyze change impact and blast radius (requires index) |
| [chunk](commands/chunk.md) | Split repository into chunks for multi-turn conversations |
| [ingest](commands/ingest.md) | Convert documents (MD, HTML, CSV, DOCX, XLSX) to LLM format |
| [init](commands/init.md) | Create `.infiniloom.yaml` configuration file |
| [info](commands/info.md) | Show version, supported models, and config |

### Command Relationships

```
infiniloom init          Create .infiniloom.yaml config
        |
infiniloom scan          Understand your repo (files, tokens, languages)
        |
infiniloom pack          Generate full context for an LLM
infiniloom map           Generate ranked symbol overview
        |
infiniloom index         Build symbol index (one-time)
       / \
infiniloom diff          Context for code changes (uses index)
infiniloom impact        Blast radius analysis (uses index)
        |
infiniloom embed         Chunks for vector databases / RAG
infiniloom chunk         Chunks for multi-turn conversations
infiniloom ingest        Convert non-code documents
```

## Guides

| Guide | Description |
|-------|-------------|
| [Claude Code Integration](guides/claude-code-integration.md) | Workflows for using Infiniloom with Claude Code |
| [Codex Integration](guides/codex-integration.md) | Workflows for using Infiniloom with OpenAI Codex CLI |
| [LLM Optimization](guides/llm-optimization.md) | Model-specific tips and token budget management |
| [Large Repositories](guides/large-repos.md) | Strategies for handling big codebases |
| [CI/CD Integration](guides/ci-integration.md) | GitHub Actions, GitLab CI, CircleCI, Jenkins |

## Reference

| Document | Description |
|----------|-------------|
| [Reference](REFERENCE.md) | Complete command reference with all flags |
| [Recipes](RECIPES.md) | Ready-to-use code patterns for common tasks |
| [Languages](LANGUAGES.md) | All 23 supported programming languages |
| [Tokenizers](TOKENIZERS.md) | All 27 supported tokenizer models |
| [Output Formats](INFINILOOM_OUTPUT_FORMATS.md) | XML, Markdown, JSON, YAML, TOON specs |
| [Configuration](CONFIGURATION.md) | Config files, env vars, and templates |
| [Roadmap](planning/ROADMAP.md) | Future development plans |

### Choosing an Output Format

| Format | Best for | Token efficiency |
|--------|----------|-----------------|
| **XML** | Claude (all versions) | Good |
| **Markdown** | GPT-4o, GPT-5, Codex, O-series | Good |
| **YAML** | Gemini, general purpose | Good |
| **TOON** | Any model when tokens are tight | Best (~40% smaller than JSON) |
| **JSON** | Programmatic parsing, pipelines | Baseline |

## Support

| Document | Description |
|----------|-------------|
| [FAQ](FAQ.md) | Frequently asked questions |
| [Troubleshooting](TROUBLESHOOTING.md) | Common issues and solutions |

## Language Bindings

| Binding | API Reference | Install |
|---------|---------------|---------|
| Python | [API Reference](api/python.md) | `pip install infiniloom` |
| Node.js | [API Reference](api/nodejs.md) | `npm install infiniloom-node` |

## Contributing

| Document | Description |
|----------|-------------|
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Contribution guidelines |
| [Architecture](ARCHITECTURE.md) | System architecture overview |
| [Design](contributing/INFINILOOM_DESIGN.md) | System design and internals |
| [Git Context Design](contributing/GIT_CONTEXT_DESIGN.md) | Diff and index design |
| [Test Specifications](contributing/TEST_SPECIFICATION.md) | CLI test specifications |
| [Clippy Guide](contributing/CLIPPY_GUIDE.md) | Linting configuration |
| [Parser Documentation](../engine/PARSER_README.md) | Tree-sitter parser details |

## Changelog

See [CHANGELOG.md](../CHANGELOG.md) for version history.
