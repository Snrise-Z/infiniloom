# Infiniloom vs Code2Prompt: Detailed Comparison

Last updated: 2025-12-28

---

## Executive Summary

Both **Infiniloom** and **code2prompt** are Rust-based CLI tools designed to convert codebases into LLM-friendly context. However, they differ significantly in philosophy, architecture, and feature depth.

**TL;DR:**
- **code2prompt**: Simple, template-driven prompt generator focused on quick conversion
- **Infiniloom**: Advanced semantic analysis engine with intelligence-first approach

---

## Side-by-Side Quick Comparison

| Feature | Infiniloom | code2prompt |
|---------|------------|-------------|
| **GitHub Stars** | ~Few hundred (newer) | ~6.9k |
| **Primary Language** | Rust | Rust |
| **Core Philosophy** | Smart context > All context | Template-based conversion |
| **Symbol Extraction** | ✅ Full AST parsing (21 langs) | ❓ Unknown depth |
| **PageRank Ranking** | ✅ Yes | ❌ No |
| **Security Scanning** | ✅ Built-in | ❌ Not mentioned |
| **Git-Aware Diff** | ✅ Yes (dedicated command) | ✅ Yes (basic) |
| **Dependency Analysis** | ✅ Full call graph | ❓ Unknown |
| **Token Counting** | ✅ 27 models (tiktoken) | ✅ Yes (method unclear) |
| **Output Formats** | 6 (XML, MD, YAML, JSON, TOON, Plain) | ❓ Not fully specified |
| **Compression Levels** | 7 (0-80% reduction) | ❌ Not mentioned |
| **Template System** | ❌ No | ✅ Handlebars |
| **TUI** | ❌ No | ✅ Yes |
| **Python SDK** | ✅ PyO3 bindings | ✅ Yes |
| **Node.js SDK** | ✅ NAPI-RS bindings | ❌ Not mentioned |
| **MCP Server** | 🔄 Coming soon | ❓ Potential |
| **Watch Mode** | ✅ Yes | ❌ Not mentioned |
| **Chunking** | ✅ 6 strategies | ❌ Not mentioned |
| **Impact Analysis** | ✅ Dedicated command | ❌ Not mentioned |
| **Remote Repos** | ✅ Sparse checkout support | ❌ Not mentioned |
| **Incremental Caching** | ✅ Yes | ❌ Not mentioned |

---

## Detailed Feature Breakdown

### 1. Core Philosophy

#### Infiniloom: Intelligence-First
- **Principle**: Context quality > context quantity
- **Approach**: Understand code semantically, rank by importance, filter intelligently
- **Use Case**: Deep code comprehension for AI assistants
- **Tagline**: "Help AI understand your codebase by giving it the right context, not all the context"

#### code2prompt: Template-First
- **Principle**: Fast conversion with user control
- **Approach**: Convert codebase to text, let users customize via templates
- **Use Case**: Quick prompt generation for LLM interactions
- **Tagline**: "Convert your codebase into a single LLM prompt"

---

### 2. Symbol Extraction & Analysis

#### Infiniloom
- **Method**: Full AST parsing via Tree-sitter
- **Languages**: 21 languages with full support (Rust, Python, JS, TS, Go, Java, C/C++, etc.)
- **Extraction Depth**: Functions, classes, methods, interfaces, types, enums, traits, constants
- **Symbol Relationships**: Full dependency graph, call chains, import tracking
- **Query API**: `index` → `impact` → `diff` workflow for deep analysis

**Example:**
```bash
# Build symbol index
infiniloom index .

# Analyze what depends on a function
infiniloom impact . --symbol "authenticate"

# Get context for changes with dependencies
infiniloom diff . HEAD~1 --depth 3
```

#### code2prompt
- **Method**: Not fully specified in documentation
- **Extraction Depth**: Unknown
- **Symbol Relationships**: Not mentioned
- **Query API**: Not mentioned

---

### 3. Importance Ranking

#### Infiniloom
- **Algorithm**: PageRank (similar to Google's web ranking)
- **Factors**:
  - Import frequency (how many files depend on this?)
  - Call graph centrality (hub vs leaf)
  - File change frequency (git history)
  - Symbol type (public APIs ranked higher)
- **Output**: Ranked symbol map with scores
- **Command**: `infiniloom map . --budget 5000`

**Philosophy**: Core business logic should get more context tokens than utility helpers.

#### code2prompt
- **Algorithm**: Not mentioned
- **Ranking**: Likely file order or directory traversal
- **Philosophy**: Include everything user selects

---

### 4. Token Budget Management

#### Infiniloom
- **Token Counting**: Exact via tiktoken-rs for OpenAI (o200k, cl100k), calibrated estimation for others
- **Supported Models**: 27 models across 8 families
  - OpenAI: GPT-5.2, GPT-5.1, GPT-5, O4-mini, O3, O1, GPT-4o, GPT-4o-mini, GPT-4, GPT-3.5-turbo
  - Anthropic: Claude (all versions)
  - Google: Gemini 1.5 Pro/Flash, Gemma
  - Meta: Llama 3/4, CodeLlama
  - Others: Mistral, DeepSeek, Qwen, Cohere, Grok
- **Budget Enforcement**:
  - `--max-tokens 50000` - Hard token limit
  - `--top-files 50` - Limit by importance
  - 7 compression levels (0-80% reduction)
  - Smart chunking with overlap

**Example:**
```bash
# Stay under 80k tokens for GPT-4o
infiniloom pack . --model gpt4o --max-tokens 80000 --compression balanced
```

#### code2prompt
- **Token Counting**: Yes, but method not specified
- **Supported Models**: Not enumerated
- **Budget Enforcement**: Basic token tracking

---

### 5. Output Formats

#### Infiniloom
6 formats, each optimized for specific models:

| Format | Optimized For | Features |
|--------|---------------|----------|
| **XML** | Claude | CDATA sections, prompt caching tags, hierarchical structure |
| **Markdown** | GPT-4o/GPT-5 | Tables, headers, code fences, human-readable |
| **YAML** | Gemini | Structured indentation, key-value hierarchy |
| **JSON** | Programmatic | Standard parsing, API integration |
| **TOON** | Limited context | Token-optimized (~40% smaller than XML) |
| **Plain** | Simple use | No markup, easy to read |

**Example:**
```bash
infiniloom pack . --format xml --model claude  # Claude-optimized
infiniloom pack . --format markdown --model gpt4o  # GPT-optimized
```

#### code2prompt
- **Template System**: Handlebars-based customization
- **Formats**: Not fully enumerated
- **Flexibility**: User controls output structure via templates

---

### 6. Compression Strategies

#### Infiniloom
7 levels of intelligent compression:

| Level | Reduction | Method |
|-------|-----------|--------|
| `none` | 0% | Full content |
| `minimal` | 10-20% | Remove empty lines |
| `balanced` | 30-40% | Remove comments |
| `aggressive` | 50-60% | Remove docstrings |
| `extreme` | 70-80% | Signatures only |
| `focused` | ~75% | Key symbols + minimal context |
| `semantic` | 60-70% | Smart analysis (keep important, remove boilerplate) |

**Philosophy**: Different AI tasks need different content density. Code review needs full context; architecture questions need structure.

**Example:**
```bash
# Full code for debugging
infiniloom pack . --compression none

# Structure overview for architecture questions
infiniloom pack . --compression extreme
```

#### code2prompt
- **Compression**: Not mentioned in documentation
- **Approach**: Likely user controls via templates

---

### 7. Security Features

#### Infiniloom
Built-in secret detection and redaction:

**Detects:**
- API keys (AWS, Azure, Google, OpenAI, Anthropic, etc.)
- Tokens (GitHub, GitLab, JWT, OAuth)
- Credentials (passwords, database URLs, private keys)
- Environment variables with secrets

**Options:**
- `--security-check` - Scan and warn
- `--redact-secrets` - Replace with `[REDACTED]`
- Custom patterns via config
- Allowlist for false positives

**Example:**
```bash
# Scan before sharing
infiniloom scan . --security-check

# Auto-redact for safe sharing
infiniloom pack . --redact-secrets --output safe-context.xml
```

#### code2prompt
- **Security**: Not mentioned in documentation
- **Approach**: User responsibility

---

### 8. Git Integration

#### Infiniloom
Deep git awareness across multiple commands:

**Commands:**
- `infiniloom diff` - Context-aware diff analysis
  - Unstaged, staged, commit ranges
  - Includes dependent files and symbols
  - Shows what tests might break
- `infiniloom impact` - Change impact analysis
  - What depends on changed code?
  - Full transitive dependency graph

**Features:**
- Commit history per file
- Change frequency sorting
- Branch comparisons
- Diff hunks with context

**Example:**
```bash
# What changed and what depends on it?
infiniloom diff . main..feature --depth 3 --include-diff

# Include last 5 commits per changed file
infiniloom diff . --staged --include-history --history-count 5
```

#### code2prompt
- **Git Features**: Diffs, logs, branch comparisons mentioned
- **Depth**: Likely basic git operations
- **Integration**: Less comprehensive

---

### 9. Advanced Workflows

#### Infiniloom
Multiple specialized commands for different scenarios:

| Command | Purpose | Use Case |
|---------|---------|----------|
| `pack` | Full repo → context | One-shot AI questions |
| `scan` | Statistics + metrics | Understand repo size |
| `map` | PageRank symbol map | High-level architecture overview |
| `index` | Build symbol index | Enable fast queries |
| `diff` | Context for changes | Code review, PR analysis |
| `impact` | Dependency analysis | "What breaks if I change this?" |
| `chunk` | Split into pieces | Multi-turn conversations |
| `init` | Config file setup | Project-specific settings |

**Example Workflow - AI Code Review:**
```bash
# One-time setup
infiniloom index .

# Before each PR
infiniloom diff . --staged --include-diff --format markdown > review.md
# Send review.md to AI
```

#### code2prompt
- **Workflow**: Primarily single `code2prompt` command
- **Flexibility**: Template customization
- **TUI**: Interactive Terminal User Interface

---

### 10. Chunking Strategies

#### Infiniloom
6 intelligent chunking strategies for large repos:

| Strategy | Method | Best For |
|----------|--------|----------|
| `semantic` | Group by code similarity | Related functionality |
| `module` | Group by directory | Monorepos, organized projects |
| `dependency` | Group by imports | Understanding dependencies |
| `file` | One file per chunk | Granular review |
| `symbol` | Group by AST symbols | Function/class focus |
| `fixed` | Fixed token size | Equal-sized chunks |

**Features:**
- Token overlap between chunks for continuity
- Priority ordering (most important first)
- Chunk summaries for navigation

**Example:**
```bash
# Split by modules, 50k tokens per chunk, 2k overlap
infiniloom chunk . --strategy module --max-tokens 50000 --overlap 2000
```

#### code2prompt
- **Chunking**: Not mentioned in documentation
- **Approach**: Likely single-output focus

---

### 11. Performance & Caching

#### Infiniloom
**Performance Optimizations:**
- Thread-local Tree-sitter parsers (parallel processing)
- Memory-mapped I/O for large files
- Incremental caching with change detection
- Binary serialization (bincode) for index
- Lazy indexing fallback

**Caching:**
- `--cache` flag for incremental updates
- File-level caching with mtime/hash comparison
- Index persistence across runs
- Watch mode for real-time updates

**Benchmarks:** (from `engine/benches/comparison.rs`)
- File traversal: 200 files in ~50ms
- Parallel parsing: 50 files in ~200ms
- Token estimation: 10k lines in ~5ms

**Example:**
```bash
# Enable caching for faster subsequent runs
infiniloom pack . --cache --watch
```

#### code2prompt
- **Performance**: "High performance, low resource usage" (no specific metrics)
- **Caching**: Not mentioned
- **Parallel Processing**: Not mentioned

---

### 12. Language Bindings

#### Infiniloom
**Python (PyO3):**
```python
import infiniloom

repo = infiniloom.pack("/path/to/repo", format="xml", model="claude")
print(repo)
```

**Node.js (NAPI-RS):**
```javascript
const infiniloom = require('infiniloom');

const context = infiniloom.pack('/path/to/repo', {
  format: 'markdown',
  model: 'gpt4o'
});
console.log(context);
```

**CLI wrapper:**
```bash
npm install -g infiniloom
infiniloom pack .
```

#### code2prompt
**Python SDK:**
```bash
pip install code2prompt-rs
```

**CLI:**
```bash
cargo install code2prompt
code2prompt .
```

**Node.js:** Not mentioned

---

### 13. Configuration

#### Infiniloom
**Config Files:** `.infiniloom.yaml`, `.infiniloom.toml`, `.infiniloom.json`

**Example:**
```yaml
output:
  format: xml
  model: claude
  compression: balanced
  token_budget: 100000

scan:
  include: ["*.rs", "*.py"]
  exclude: ["tests/*", "vendor/*"]
  include_tests: false

security:
  scan_secrets: true
  redact_secrets: true
  custom_patterns:
    - "MY_SECRET_[A-Z0-9]{32}"
```

**Templates:** 4 project templates (Rust, Python, Node, Generic)

#### code2prompt
**Config:** Handlebars template system
**Flexibility:** User-defined templates
**Templates:** Not enumerated

---

### 14. Remote Repository Support

#### Infiniloom
**Features:**
- Sparse checkout for large repos
- Branch selection
- Path filtering

**Example:**
```bash
# Sparse checkout of Linux kernel scheduler
infiniloom pack github:torvalds/linux --sparse-path kernel/sched

# Specific branch
infiniloom pack github:facebook/react --remote-branch main

# Multiple paths
infiniloom pack github:owner/repo --sparse-path src --sparse-path lib
```

#### code2prompt
**Remote Support:** Not mentioned in documentation

---

### 15. Testing & Quality

#### Infiniloom
**Test Suite:**
- 64+ test files
- Unit tests (24 in diff module alone)
- E2E tests (17 for index/diff workflow)
- Property-based tests (proptest)
- Benchmarks (criterion, 38+ scenarios)

**CI/CD:**
- GitHub Actions
- Format checking
- Clippy linting (strict)
- Security scanning (Trivy)
- Code coverage (Codecov)

**Quality:**
- Comprehensive error handling
- Type-safe design (newtypes)
- No unsafe code (by policy)
- Documentation coverage

#### code2prompt
**Testing:** Not detailed in documentation
**Quality:** 592 commits, 29 contributors, active maintenance

---

### 16. Documentation

#### Infiniloom
**Docs:**
- README.md (comprehensive)
- REFERENCE.md (complete command reference)
- RECIPES.md (ready-to-use patterns)
- Command reference (per-command guides)
- CONFIGURATION.md (all options)
- FAQ.md
- Guides (LLM optimization, large repos, CI integration)
- CLAUDE.md (AI assistant instructions)

**Code Documentation:**
- Rustdoc for all public APIs
- Module-level documentation
- Example code throughout

#### code2prompt
**Docs:**
- README.md (English + Spanish)
- llms-install.md
- Website (separate)

---

### 17. Project Maturity

#### Infiniloom
- **Status**: Stable, actively maintained
- **Version**: Latest on crates.io, npm, PyPI
- **Development**: Active refactoring (92-110 hours planned)
- **Community**: Smaller but growing
- **Backing**: Topos Labs
- **License**: MIT

**Stable Today:**
- All core commands working
- 21 language support
- All output formats
- Security scanning
- Git integration
- Python/Node bindings

**Roadmap:**
- MCP server integration
- Streaming output
- GitHub Action
- VS Code extension

#### code2prompt
- **Status**: Mature project
- **GitHub Stars**: 6.9k (more established)
- **Commits**: 592
- **Contributors**: 29
- **License**: MIT

---

## Use Case Recommendations

### When to Choose Infiniloom

✅ **Choose Infiniloom if you need:**
- Deep code comprehension for AI assistants
- Dependency and impact analysis
- Smart importance ranking
- Multi-model support with exact token counting
- Security-sensitive environments
- Git-aware change analysis
- Large repository handling with chunking
- Python/Node.js integration
- Advanced compression strategies
- Semantic analysis over simple text conversion

**Best For:**
- Code review workflows
- Understanding complex codebases
- AI agent development
- Production systems where security matters
- Teams needing consistent, intelligent context

### When to Choose code2prompt

✅ **Choose code2prompt if you need:**
- Fast, simple prompt generation
- Template customization (Handlebars)
- Terminal User Interface (TUI)
- Quick prototyping
- Simple conversion without deep analysis
- Established, battle-tested tool
- Large community (6.9k stars)

**Best For:**
- Quick one-off prompts
- Custom output formatting via templates
- Users comfortable with template languages
- Simple "convert my code" use cases

---

## Technical Architecture Comparison

### Infiniloom Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI Layer                             │
│  (clap-based, 8 commands, extensive options)                │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                     Engine Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Parser     │  │  Tokenizer   │  │   Ranking    │      │
│  │ (Tree-sitter)│  │ (tiktoken-rs)│  │  (PageRank)  │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Index      │  │   Security   │  │  Chunking    │      │
│  │(Symbol graph)│  │ (Secret scan)│  │ (Strategies) │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                   Output Formatters                          │
│  XML │ Markdown │ YAML │ JSON │ TOON │ Plain                │
└──────────────────────────────────────────────────────────────┘
```

**Key Design Patterns:**
- Thread-local parsers (no mutex contention)
- Parallel file processing (Rayon)
- Memory-mapped I/O
- Incremental caching
- Type-safe wrappers (FileId, SymbolId)

### code2prompt Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI Layer                             │
│              (Terminal UI available)                         │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                     Core Layer                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  File Walker │  │ Git Support  │  │Token Counter │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────┬───────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────┐
│                  Template Engine                             │
│              (Handlebars-based)                              │
└──────────────────────────────────────────────────────────────┘
```

**Key Features:**
- Template-driven output
- Interactive TUI
- Git integration
- High performance (Rust)

---

## Performance Comparison

### Benchmark Scenario: Medium Repository (100 files, ~50k LOC)

#### Infiniloom
```
Scan:          ~100ms  (parallel file read + parse)
Pack (full):   ~500ms  (full AST + ranking)
Pack (fast):   ~50ms   (--skip-symbols)
Index build:   ~300ms  (symbol graph + dependencies)
Diff:          ~50ms   (with pre-built index)
```

#### code2prompt
- **Claimed**: High performance, low resource usage
- **Actual Metrics**: Not published

**Note**: Both are Rust-based, so performance should be comparable for basic operations. Infiniloom's advanced features (AST parsing, PageRank) add overhead but provide deeper analysis.

---

## Community & Ecosystem

### Infiniloom
- **GitHub**: Topos-Labs/infiniloom
- **Stars**: Hundreds (newer project)
- **Package Registries**: npm, crates.io, PyPI
- **Documentation**: Comprehensive (README, guides, CHEATSHEET, FAQ)
- **Support**: GitHub Issues, Discussions
- **Development**: Active (current refactoring: 18 items, 92-110 hours)

### code2prompt
- **GitHub**: mufeedvh/code2prompt
- **Stars**: 6.9k (more established)
- **Contributors**: 29
- **Package Registries**: crates.io, Homebrew, pip
- **Documentation**: README (English + Spanish), website
- **Community**: Larger, more established

---

## Pricing & Licensing

Both projects are **free and open-source** under **MIT License**.

---

## Migration Path

### From code2prompt to Infiniloom

**Advantages:**
- ✅ More intelligent context selection
- ✅ Better token management
- ✅ Security scanning
- ✅ Advanced workflows (diff, impact, chunk)
- ✅ Multiple language bindings

**Learning Curve:**
- More commands to learn (8 vs 1)
- Configuration options more extensive
- Philosophy shift: quality over quantity

**Migration Strategy:**
1. Start with `infiniloom pack` (similar to code2prompt)
2. Add `--compression balanced` for smaller output
3. Explore `infiniloom scan` to understand your codebase
4. Gradually adopt advanced commands (map, diff, impact)

### From Infiniloom to code2prompt

**Advantages:**
- ✅ Template customization
- ✅ TUI for interactive use
- ✅ Larger community
- ✅ Simpler mental model

**Trade-offs:**
- ❌ Lose semantic analysis
- ❌ Lose importance ranking
- ❌ Lose security scanning
- ❌ Lose specialized commands

---

## Conclusion

**Infiniloom** and **code2prompt** serve overlapping but distinct purposes:

- **code2prompt** is a **prompt generator** — fast, template-driven, user-controlled
- **Infiniloom** is a **context intelligence engine** — semantic analysis, ranking, specialized workflows

**Choose code2prompt** for quick, customizable prompt generation with a mature ecosystem.

**Choose Infiniloom** for deep code understanding, intelligent context selection, and production-grade workflows.

Both are excellent tools. Your choice depends on whether you value **simplicity + templates** (code2prompt) or **intelligence + depth** (Infiniloom).

---

## Quick Decision Matrix

| Your Priority | Recommendation |
|---------------|----------------|
| Fast setup | code2prompt |
| Template control | code2prompt |
| Established community | code2prompt |
| Interactive TUI | code2prompt |
| Deep code understanding | **Infiniloom** |
| Importance ranking | **Infiniloom** |
| Security scanning | **Infiniloom** |
| Git-aware workflows | **Infiniloom** |
| Multi-model optimization | **Infiniloom** |
| Dependency analysis | **Infiniloom** |
| Production environments | **Infiniloom** |
| Python/Node.js integration | **Infiniloom** |
| Advanced chunking | **Infiniloom** |

---

**Last Updated**: 2025-12-28
**Infiniloom Version**: Latest from refactoring (Phase 2 in progress)
**code2prompt Version**: Based on GitHub as of 2025-12-28
