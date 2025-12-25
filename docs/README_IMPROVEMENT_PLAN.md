# README & Documentation Improvement Plan

## Executive Summary

Analysis of top GitHub repositories (ruff, ripgrep, deno, tauri, uv, tokio, eza) reveals clear patterns for professional, best-in-class READMEs. This plan transforms Infiniloom's README from 700+ lines to a focused ~300 lines while improving professionalism and discoverability.

---

## Part 1: README Badge Strategy

### Current State (3 badges)
```
License: MIT | Rust 1.91+ | Crates.io version
```

### Recommended State (7-8 badges)

**Row 1 - Core Status:**
```markdown
[![CI](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml/badge.svg)](https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/Topos-Labs/infiniloom/graph/badge.svg)](https://codecov.io/gh/Topos-Labs/infiniloom)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
```

**Row 2 - Distribution:**
```markdown
[![Crates.io](https://img.shields.io/crates/v/infiniloom.svg)](https://crates.io/crates/infiniloom)
[![Downloads](https://img.shields.io/crates/d/infiniloom.svg)](https://crates.io/crates/infiniloom)
[![npm](https://img.shields.io/npm/v/infiniloom.svg)](https://www.npmjs.com/package/infiniloom)
[![PyPI](https://img.shields.io/pypi/v/infiniloom.svg)](https://pypi.org/project/infiniloom/)
```

**Row 3 - Technical:**
```markdown
[![MSRV](https://img.shields.io/badge/MSRV-1.91+-orange.svg)](https://www.rust-lang.org/)
```

### Badge Rationale

| Badge | Purpose | Signal |
|-------|---------|--------|
| **CI Status** | Shows code quality | "Tests pass, maintained" |
| **Coverage** | Shows test rigor | "Well-tested codebase" |
| **Crates.io** | Version + validity | "Published, versioned" |
| **Downloads** | Social proof | "People use this" |
| **npm/PyPI** | Multi-platform | "Available everywhere" |
| **MSRV** | Stability | "Predictable requirements" |

---

## Part 2: README Structure Redesign

### Current Problems
1. **Too long**: 703 lines is overwhelming
2. **No performance evidence**: Claims "blazing fast" without proof
3. **No social proof**: No testimonials, users, or star history
4. **Dense middle section**: Walls of code examples
5. **Missing CI badge**: No visible build status

### Proposed Structure (~300 lines)

```
[HERO SECTION]
- Logo/Name (centered)
- One-line tagline
- Badges (2 rows max)
- Quick nav links

[QUICK VALUE - 30 seconds to understand]
- 3-4 bullet points (what it does)
- Single install command
- Single usage example

[PERFORMANCE SECTION - optional but impactful]
- Benchmark chart/table vs alternatives
- Concrete numbers (X files in Y ms)

[KEY FEATURES - scannable]
- 5-6 features as icons/bullets
- Link to full docs for details

[INSTALLATION - collapsible or minimal]
- Primary method prominent
- Others in expandable section

[QUICK EXAMPLES - 3 max]
- Pack (core use case)
- Scan (discovery)
- Diff (advanced)

[ECOSYSTEM]
- Supported languages table (compact)
- Model support table (compact)

[LINKS]
- Documentation | Contributing | Changelog

[FOOTER]
- License, Made by
```

---

## Part 3: Specific Additions

### 3.1 Test Coverage Badge

**Setup Required:**
1. Codecov is already configured in CI (`.github/workflows/ci.yml` lines 136-159)
2. Add badge to README after first successful coverage upload:

```markdown
[![codecov](https://codecov.io/gh/Topos-Labs/infiniloom/graph/badge.svg?token=YOUR_TOKEN)](https://codecov.io/gh/Topos-Labs/infiniloom)
```

**Current Test Count:** ~800+ tests across workspace
- Engine: 273 core tests + ~500 in submodules
- CLI: 92 tests
- Bindings: 100+ tests

### 3.2 Performance Benchmarks Section

**Add concrete numbers:**

```markdown
## Performance

Infiniloom processes repositories 10-50x faster than alternatives:

| Repository | Files | Infiniloom | repomix | gitingest |
|------------|-------|------------|---------|-----------|
| tokio      | 847   | 89ms       | 2.1s    | 3.4s      |
| react      | 2,341 | 210ms      | 5.8s    | 8.2s      |
| linux      | 78K   | 4.2s       | timeout | timeout   |

*Benchmarks on M2 MacBook Pro. See [benchmarks](docs/BENCHMARKS.md) for methodology.*
```

**Action Required:** Run actual benchmarks to get real numbers

### 3.3 Star History (Optional)

Using star-history.com:

```markdown
[![Star History Chart](https://api.star-history.com/svg?repos=Topos-Labs/infiniloom&type=Date)](https://star-history.com/#Topos-Labs/infiniloom&Date)
```

**Recommendation:** Add only after reaching 500+ stars for visual impact

### 3.4 "Who Uses Infiniloom" Section

```markdown
## Used By

- Company/Project 1
- Company/Project 2
- [Your project here?](CONTRIBUTING.md)
```

**Action Required:** Reach out to early adopters for permission

### 3.5 Testimonials (High Impact)

From ruff's approach - add 1-2 quotes:

```markdown
> "Infiniloom is what I reach for every time I need to give Claude context about a codebase."
> — *Developer Name, Company*
```

---

## Part 4: Content to Move Out of README

### Move to `/docs/` or separate files:

| Current Section | Move To | Reason |
|-----------------|---------|--------|
| Full CLI reference | `docs/commands/README.md` | Too detailed |
| All output format examples | `docs/OUTPUT_FORMATS.md` | Already exists |
| Configuration file examples | `docs/CONFIGURATION.md` | New file |
| Language bindings details | `bindings/*/README.md` | Already exists |
| Full compression levels table | `docs/COMPRESSION.md` | New file |
| Environment variables table | `docs/CONFIGURATION.md` | Group with config |
| 21-language symbol table | Collapse in README | Use `<details>` |

### Use Collapsible Sections:

```markdown
<details>
<summary>Supported Languages (21)</summary>

| Language | Symbols Extracted |
|----------|-------------------|
| Python   | Functions, Classes, Methods |
...
</details>
```

---

## Part 5: Documentation Restructure

### Current State
```
docs/
├── commands/           # Good - keep
├── INFINILOOM_DESIGN.md
├── INFINILOOM_OUTPUT_FORMATS.md
├── IMPLEMENTATION_STATUS.md
├── GIT_CONTEXT_DESIGN.md
└── TEST_SPECIFICATION.md
```

### Proposed Structure
```
docs/
├── getting-started/
│   ├── installation.md      # All install methods in detail
│   ├── quick-start.md       # First 5 minutes
│   └── configuration.md     # Config files, env vars
├── commands/                # Keep as-is
├── guides/
│   ├── llm-optimization.md  # Model-specific tips
│   ├── large-repos.md       # Handling big codebases
│   ├── ci-integration.md    # Using in CI/CD
│   └── security.md          # Secret scanning deep dive
├── reference/
│   ├── output-formats.md    # Rename from INFINILOOM_OUTPUT_FORMATS.md
│   ├── compression.md       # Compression levels detail
│   └── api.md               # For bindings
├── development/
│   ├── architecture.md      # Rename from INFINILOOM_DESIGN.md
│   ├── contributing.md      # Link from CONTRIBUTING.md
│   └── changelog.md         # Link from CHANGELOG.md
└── README.md                # Docs index
```

### Key Changes:
1. **User-centric organization**: getting-started > guides > reference
2. **Clear navigation**: Numbered or categorized
3. **Consolidated**: Move internal docs to `development/`

---

## Part 6: Action Items (Priority Order)

### P0 - Immediate (This PR)
- [ ] Add CI status badge
- [ ] Add Codecov badge (after token setup)
- [ ] Add downloads badge
- [ ] Add npm/PyPI badges
- [ ] Restructure README to ~300 lines

### P1 - This Week
- [ ] Create `docs/CONFIGURATION.md`
- [ ] Run benchmarks and add performance section
- [ ] Add collapsible sections for detailed tables
- [ ] Update all doc links

### P2 - This Month
- [ ] Collect testimonials from users
- [ ] Create "Who Uses" section
- [ ] Reorganize docs/ directory structure
- [ ] Add star history (when >500 stars)

### P3 - Ongoing
- [ ] Monitor and update benchmarks
- [ ] Collect user testimonials
- [ ] Keep docs in sync with features

---

## Part 7: README Draft Outline

```markdown
<div align="center">

# Infiniloom

**Transform codebases into optimized context for LLMs**

[![CI][ci-badge]][ci-url] [![Coverage][cov-badge]][cov-url] [![License][lic-badge]][lic-url]
[![Crates.io][crate-badge]][crate-url] [![npm][npm-badge]][npm-url] [![PyPI][pypi-badge]][pypi-url]

[Install](#installation) · [Quick Start](#quick-start) · [Docs](docs/) · [Contributing](CONTRIBUTING.md)

</div>

---

Infiniloom extracts code, symbols, and structure from repositories and outputs
them in formats optimized for Claude, GPT, Gemini, and other LLMs.

- **Fast**: Pure Rust, processes 1000+ files in <100ms
- **Smart**: AST-based symbol extraction (21 languages), PageRank ranking
- **Secure**: Automatic secret detection and redaction
- **Flexible**: XML, Markdown, JSON, YAML, TOON output formats

## Installation

```bash
npm install -g infiniloom    # Recommended
cargo install infiniloom     # Rust users
pip install infiniloom       # Python library
```

## Quick Start

```bash
# Pack repo for Claude
infiniloom pack . --format xml

# Scan repo statistics
infiniloom scan .

# Get context for a diff
infiniloom diff . --staged
```

## Performance

| Repository | Files | Time |
|------------|-------|------|
| tokio      | 847   | 89ms |
| react      | 2,341 | 210ms |

## Features

<details>
<summary><strong>21 Supported Languages</strong></summary>
...
</details>

<details>
<summary><strong>Model-Specific Optimization</strong></summary>
...
</details>

## Documentation

- [Command Reference](docs/commands/)
- [Configuration Guide](docs/CONFIGURATION.md)
- [Output Formats](docs/INFINILOOM_OUTPUT_FORMATS.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT - see [LICENSE](LICENSE).

---

<div align="center">
Made with care by <a href="https://github.com/Topos-Labs">Topos Labs</a>
</div>
```

---

## Appendix: Badge URLs Reference

```markdown
<!-- Badges -->
[ci-badge]: https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml/badge.svg
[ci-url]: https://github.com/Topos-Labs/infiniloom/actions/workflows/ci.yml
[cov-badge]: https://codecov.io/gh/Topos-Labs/infiniloom/graph/badge.svg
[cov-url]: https://codecov.io/gh/Topos-Labs/infiniloom
[lic-badge]: https://img.shields.io/badge/License-MIT-blue.svg
[lic-url]: LICENSE
[crate-badge]: https://img.shields.io/crates/v/infiniloom.svg
[crate-url]: https://crates.io/crates/infiniloom
[npm-badge]: https://img.shields.io/npm/v/infiniloom.svg
[npm-url]: https://www.npmjs.com/package/infiniloom
[pypi-badge]: https://img.shields.io/pypi/v/infiniloom.svg
[pypi-url]: https://pypi.org/project/infiniloom/
[msrv-badge]: https://img.shields.io/badge/MSRV-1.91+-orange.svg
```
