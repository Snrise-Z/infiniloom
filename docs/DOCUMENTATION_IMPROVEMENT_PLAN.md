# Documentation Improvement Plan

**Created:** 2025-01-24
**Status:** Proposed
**Version:** 1.0

---

## Executive Summary

This document outlines a comprehensive plan to improve Infiniloom's documentation by:
1. Verifying all documented features against actual implementation
2. Separating implemented features from roadmap/Phase 2 items
3. Eliminating duplication and inconsistencies
4. Creating a clear, maintainable documentation structure

---

## Current State Analysis

### Documentation Inventory

| File | Lines | Purpose | Issues |
|------|-------|---------|--------|
| `README.md` | ~275 | Main entry point | Good, recently improved |
| `CLAUDE.md` | ~500 | AI assistant guide | Overlaps with ARCHITECTURE.md |
| `CHANGELOG.md` | ~230 | Version history | Good |
| `CONTRIBUTING.md` | ~170 | Contributor guide | Basic but adequate |
| `docs/README.md` | ~60 | Doc index | Incomplete |
| `docs/CONFIGURATION.md` | ~430 | Config reference | Contains unverified options |
| `docs/IMPLEMENTATION_STATUS.md` | ~505 | Feature tracking | Outdated, needs verification |
| `docs/INFINILOOM_DESIGN.md` | ~1400 | Design document | Mixes implemented + Phase 2 |
| `docs/GIT_CONTEXT_DESIGN.md` | ~575 | Git context design | Mostly implemented |
| `docs/README_IMPROVEMENT_PLAN.md` | ~380 | Previous plan | Partially executed |
| `docs/commands/*.md` | Various | Command references | Need verification |
| `docs/guides/*.md` | Various | User guides | Mostly good |
| `engine/ARCHITECTURE.md` | ~330 | Engine internals | Duplicates CLAUDE.md |
| `bindings/python/README.md` | ~800 | Python API | Need verification |
| `bindings/node/README.md` | ~830 | Node.js API | Need verification |

### Critical Issues Identified

#### 1. Phase 2 Features Mixed with Implemented Features

**Problem:** `docs/INFINILOOM_DESIGN.md` contains both implemented and planned features without clear separation. Users/developers cannot easily distinguish what's available.

**Examples of potentially unimplemented features documented as if available:**
- Watch mode for index updates (only `pack --watch` is implemented)
- IDE integration / LSP
- GitHub integration (auto-comment PR context)
- Semantic search
- AI suggestions
- Cross-repository diffs
- Smart budget allocation
- Interactive diff mode
- Custom expansion rules
- Semantic diff detection

#### 2. Documentation Duplication

**Problem:** Architecture information is scattered across multiple files:

| Content | Locations |
|---------|-----------|
| Module structure | CLAUDE.md, engine/ARCHITECTURE.md, INFINILOOM_DESIGN.md |
| Thread-local parser pattern | CLAUDE.md, engine/ARCHITECTURE.md, docs/commands/pack.md |
| Data structures | CLAUDE.md, GIT_CONTEXT_DESIGN.md, INFINILOOM_DESIGN.md |
| Performance info | README.md, CLAUDE.md, engine/ARCHITECTURE.md |

#### 3. Inconsistencies Found

| Issue | Location | Details |
|-------|----------|---------|
| Version mismatch | CHANGELOG vs code | CHANGELOG shows v0.3.4, code is v0.4.3 |
| Feature flags | Cargo.toml vs docs | `async` feature documented but placeholder |
| Token models | Various docs | GPT-5.x documented but not released models |
| Config options | CONFIGURATION.md | Some options may not be implemented |

#### 4. Missing Verification

The following need code verification:
- All CLI flags in command docs
- All config file options
- All Python binding functions
- All Node.js binding functions
- Feature flag implementations

---

## Proposed Documentation Structure

### Tier 1: Core Documentation (Essential)

```
README.md                      # Project overview, quick start, badges
CLAUDE.md                      # AI assistant development guide
CHANGELOG.md                   # Release history
CONTRIBUTING.md                # How to contribute
LICENSE                        # MIT License
```

### Tier 2: User Documentation (docs/)

```
docs/
├── README.md                  # Documentation index/hub
├── getting-started/
│   ├── installation.md        # All installation methods
│   └── quick-start.md         # 5-minute tutorial
├── commands/
│   ├── pack.md                # Main command
│   ├── scan.md                # Repository scanning
│   ├── map.md                 # Symbol map generation
│   ├── diff.md                # Diff context
│   ├── index.md               # Index building
│   ├── impact.md              # Impact analysis
│   ├── chunk.md               # Repository chunking
│   ├── init.md                # Config initialization
│   └── info.md                # Version/config info
├── guides/
│   ├── llm-optimization.md    # Model-specific tips
│   ├── large-repos.md         # Scaling strategies
│   ├── ci-integration.md      # CI/CD workflows
│   └── security-scanning.md   # Secret detection guide
├── reference/
│   ├── configuration.md       # Full config reference
│   ├── output-formats.md      # XML/MD/JSON/YAML/TOON specs
│   ├── supported-languages.md # Tree-sitter language support
│   └── token-models.md        # Tokenizer model details
└── troubleshooting.md         # Common issues & solutions
```

### Tier 3: Developer Documentation

```
docs/
├── architecture/
│   ├── overview.md            # High-level architecture
│   ├── engine.md              # Core engine design
│   ├── parser.md              # Tree-sitter parsing
│   ├── tokenizer.md           # Token counting
│   ├── index.md               # Symbol index design
│   └── performance.md         # Performance patterns
└── IMPLEMENTATION_STATUS.md   # Current feature status
```

### Tier 4: API Documentation (bindings/)

```
bindings/
├── python/README.md           # Python API reference
└── node/README.md             # Node.js API reference
```

### Tier 5: Planning Documents (docs/planning/)

**NEW:** Create separate directory for unimplemented features

```
docs/planning/
├── ROADMAP.md                 # Future feature roadmap
├── phase2-features.md         # Detailed Phase 2 specifications
└── rfc/                       # Request for comments on new features
    └── 001-semantic-search.md
```

---

## Action Plan

### Phase 1: Verification & Audit (Priority: Critical)

#### Task 1.1: Verify CLI Implementation
**Goal:** Ensure all documented CLI flags exist and work

```bash
# For each command, compare docs vs --help output
infiniloom pack --help
infiniloom scan --help
infiniloom diff --help
infiniloom map --help
infiniloom index --help
infiniloom impact --help
infiniloom chunk --help
infiniloom init --help
infiniloom info --help
```

**Files to update:**
- `docs/commands/*.md`

#### Task 1.2: Verify Config Options
**Goal:** Ensure all documented config options are implemented

**Verification steps:**
1. Read `engine/src/config.rs` to get actual config struct
2. Compare with `docs/CONFIGURATION.md`
3. Remove/flag undocumented options

**Files to update:**
- `docs/CONFIGURATION.md`

#### Task 1.3: Verify Python Bindings
**Goal:** Ensure all documented Python functions exist

**Verification steps:**
1. Read `bindings/python/src/lib.rs`
2. Compare exported functions with `bindings/python/README.md`
3. Test each function

**Files to update:**
- `bindings/python/README.md`

#### Task 1.4: Verify Node.js Bindings
**Goal:** Ensure all documented Node.js functions exist

**Verification steps:**
1. Read `bindings/node/src/lib.rs`
2. Compare exported functions with `bindings/node/README.md`
3. Test each function

**Files to update:**
- `bindings/node/README.md`

#### Task 1.5: Verify Feature Flags
**Goal:** Ensure documented feature flags are implemented

**Verification steps:**
1. Read `engine/Cargo.toml` feature definitions
2. Check if features have actual code
3. Update docs to reflect reality

**Files to update:**
- `README.md` (features section)
- `CLAUDE.md` (feature flags section)

---

### Phase 2: Separation & Reorganization (Priority: High)

#### Task 2.1: Create Phase 2/Roadmap Document
**Goal:** Move all unimplemented features to planning docs

**New file:** `docs/planning/ROADMAP.md`

Content to move from `INFINILOOM_DESIGN.md`:
- Watch mode for index updates
- IDE integration / LSP
- GitHub integration
- Semantic search
- AI suggestions
- Cross-repository diffs
- Smart budget allocation
- Interactive diff mode
- Custom expansion rules
- Semantic diff detection
- Test coverage correlation

Content to move from `docs/commands/diff.md`:
- "Potential Improvements" section (lines 357-405)

#### Task 2.2: Clean INFINILOOM_DESIGN.md
**Goal:** Keep only implemented features, make it a reference doc

**Actions:**
1. Remove all "Future Extensions" sections
2. Remove all "Phase 2" content
3. Add cross-references to ROADMAP.md
4. Rename to `docs/architecture/design-decisions.md`

#### Task 2.3: Consolidate Architecture Docs
**Goal:** Single source of truth for architecture

**Actions:**
1. Merge `engine/ARCHITECTURE.md` into `docs/architecture/engine.md`
2. Keep CLAUDE.md focused on development workflow
3. Remove architecture duplication from CLAUDE.md
4. Update cross-references

#### Task 2.4: Update IMPLEMENTATION_STATUS.md
**Goal:** Accurate feature status tracking

**Actions:**
1. Verify each feature against code
2. Add completion percentages
3. Link to relevant code files
4. Add "Last Verified" dates

---

### Phase 3: Content Improvements (Priority: Medium)

#### Task 3.1: Standardize Command Documentation

Each command doc should follow this template:

```markdown
# `infiniloom <command>` Command

## Overview
One paragraph description

## Synopsis
```bash
infiniloom <command> [PATH] [OPTIONS]
```

## Options
| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| ... | ... | ... | ... |

## Examples
### Basic Usage
### Advanced Usage
### Integration Examples

## See Also
- Related commands
```

#### Task 3.2: Add Missing Documentation

**Missing docs to create:**
- `docs/reference/supported-languages.md` - Full language support matrix
- `docs/reference/token-models.md` - Detailed tokenizer info
- `docs/troubleshooting.md` - Common issues
- `docs/guides/security-scanning.md` - Security feature guide

#### Task 3.3: Improve Getting Started

**Actions:**
- Add installation verification steps
- Add troubleshooting for common install issues
- Create platform-specific notes (macOS, Linux, Windows)

---

### Phase 4: Maintenance Setup (Priority: Low)

#### Task 4.1: Add Documentation CI Checks

**Create:** `.github/workflows/docs.yml`

```yaml
name: Documentation Check
on: [pull_request]
jobs:
  docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Check CLI docs match implementation
        run: |
          # Script to verify --help matches docs
      - name: Check links
        uses: lycheeverse/lychee-action@v1
      - name: Spell check
        uses: streetsidesoftware/cspell-action@v2
```

#### Task 4.2: Create Documentation Templates

**Create:** `docs/templates/`
- `command.md` - Command documentation template
- `guide.md` - Guide documentation template
- `rfc.md` - RFC template for new features

#### Task 4.3: Add Last Updated Timestamps

Add to each doc file header:
```markdown
---
last_verified: 2024-01-24
version: 0.4.3
---
```

---

## Specific File Actions

### Files to DELETE or DEPRECATE

| File | Action | Reason |
|------|--------|--------|
| `docs/README_IMPROVEMENT_PLAN.md` | Delete | Superseded by this document |
| `docs/INFINILOOM_OUTPUT_FORMATS.md` | Merge | Into `docs/reference/output-formats.md` |

### Files to MOVE

| From | To | Reason |
|------|-----|--------|
| `engine/ARCHITECTURE.md` | `docs/architecture/engine.md` | Consolidation |
| `docs/INFINILOOM_DESIGN.md` | `docs/architecture/design-decisions.md` | Clarity |
| `docs/GIT_CONTEXT_DESIGN.md` | `docs/architecture/git-context.md` | Consistency |

### Files to SPLIT

| File | Split Into | Reason |
|------|------------|--------|
| `CLAUDE.md` | Keep development workflow only | Remove architecture (link to docs/architecture/) |
| `docs/CONFIGURATION.md` | Keep as-is but add verification status | Accuracy |

### Files to CREATE

| File | Purpose |
|------|---------|
| `docs/planning/ROADMAP.md` | Future features roadmap |
| `docs/planning/phase2-features.md` | Detailed Phase 2 specs |
| `docs/architecture/overview.md` | High-level architecture |
| `docs/reference/supported-languages.md` | Language support matrix |
| `docs/reference/token-models.md` | Tokenizer details |
| `docs/troubleshooting.md` | Common issues |
| `docs/guides/security-scanning.md` | Security guide |

---

## Feature Verification Results (Completed 2025-01-24)

### CLI Commands - VERIFIED

All 9 commands exist in implementation:

| Command | Documented | Verified | Status |
|---------|------------|----------|--------|
| `pack` | Yes | **YES** | Main command |
| `scan` | Yes | **YES** | Repository scanning |
| `map` | Yes | **YES** | Symbol map generation |
| `diff` | Yes | **YES** | Diff context |
| `index` | Yes | **YES** | Index building |
| `impact` | Yes | **YES** | Impact analysis |
| `chunk` | Yes | **YES** | Repository chunking |
| `init` | Yes | **YES** | Config initialization |
| `info` | Yes | **YES** | Version/config info |

### Feature Flags - VERIFIED

From `engine/Cargo.toml`:

| Feature | Status | Notes |
|---------|--------|-------|
| `async` | **PLACEHOLDER** | Comment says "not yet implemented" |
| `embeddings` | **HEURISTIC** | Uses char-frequency, not neural |
| `watch` | **IMPLEMENTED** | For `pack --watch` |
| `full` | Combines above | |

**Documentation Issues:**
- README mentions neural embeddings as if implemented
- `async` feature documented but not functional

### Python Bindings - VERIFIED (17 functions)

| Function | Documented | In Code | Status |
|----------|------------|---------|--------|
| `pack()` | Yes | **YES** | |
| `scan()` | Yes | **YES** | |
| `count_tokens()` | Yes | **YES** | |
| `semantic_compress()` | Yes | **YES** | |
| `scan_security()` | Yes | **YES** | |
| `is_git_repo()` | Yes | **YES** | |
| `build_index()` | Yes | **YES** | |
| `index_status()` | **NO** | **YES** | UNDOCUMENTED |
| `find_symbol()` | Yes | **YES** | |
| `get_callers()` | Yes | **YES** | |
| `get_callees()` | Yes | **YES** | |
| `get_references()` | Yes | **YES** | |
| `get_call_graph()` | Yes | **YES** | |
| `chunk()` | **NO** | **YES** | UNDOCUMENTED |
| `get_impact()` | **NO** | **YES** | UNDOCUMENTED |
| `get_diff_context()` | **NO** | **YES** | UNDOCUMENTED |

**Documentation Issues:**
- 4 Python functions not documented in README
- Missing: `index_status()`, `chunk()`, `get_impact()`, `get_diff_context()`

### Node.js Bindings - VERIFIED (50+ exports)

All core functions verified present. Additional undocumented functions:

| Function | Documented | In Code | Status |
|----------|------------|---------|--------|
| `pack()` | Yes | **YES** | |
| `scan()` | Yes | **YES** | |
| `scanWithOptions()` | Yes | **YES** | |
| `countTokens()` | Yes | **YES** | |
| `semanticCompress()` | Yes | **YES** | |
| `scanSecurity()` | Yes | **YES** | |
| `isGitRepo()` | Yes | **YES** | |
| `buildIndex()` | Yes | **YES** | |
| `indexStatus()` | **NO** | **YES** | UNDOCUMENTED |
| `findSymbol()` | Yes | **YES** | |
| `getCallers()` | Yes | **YES** | |
| `getCallees()` | Yes | **YES** | |
| `getReferences()` | Yes | **YES** | |
| `getCallGraph()` | Yes | **YES** | |
| `chunk()` | **NO** | **YES** | UNDOCUMENTED |
| `analyzeImpact()` | **NO** | **YES** | UNDOCUMENTED |
| `getDiffContext()` | **NO** | **YES** | UNDOCUMENTED |
| All `*Async` versions | Partial | **YES** | Need full docs |

**Documentation Issues:**
- Several Node.js functions not documented
- Async versions partially documented

### Version Discrepancy - ISSUE FOUND

| Location | Version |
|----------|---------|
| `engine/Cargo.toml` | **0.4.3** |
| `CHANGELOG.md` | **0.3.4** (latest entry) |

**Action Required:** CHANGELOG.md missing entries for v0.4.0, v0.4.1, v0.4.2, v0.4.3

---

## Unimplemented Features to Move to Roadmap

Based on analysis, the following appear to be unimplemented or partially implemented:

### Definitely Unimplemented

1. **IDE Integration / LSP** - No LSP code exists
2. **GitHub Integration** - No GitHub API integration
3. **Semantic Search** - No embedding/vector search
4. **AI Suggestions** - No AI integration
5. **Cross-Repository Diffs** - Single repo only
6. **Smart Budget Allocation** - Basic budget, not AI-guided
7. **Interactive Diff Mode** - No interactive mode
8. **Custom Expansion Rules** - No language-specific expansion config
9. **Semantic Diff Detection** - No semantic analysis of changes
10. **Test Coverage Correlation** - No coverage integration

### Needs Verification

1. **Watch mode for index** - `pack --watch` exists, index watch unclear
2. **`async` feature flag** - Documented but may be placeholder
3. **`embeddings` feature flag** - Documented as "heuristic-based"
4. **Some config options** - Need to verify all are wired up

---

## Timeline Estimate

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 1: Verification | 2-3 days | Code access |
| Phase 2: Separation | 1-2 days | Phase 1 complete |
| Phase 3: Content | 2-3 days | Phase 2 complete |
| Phase 4: Maintenance | 1 day | Phase 3 complete |

**Total:** 6-9 days of focused work

---

## Success Criteria

1. **Accuracy:** All documented features verified against implementation
2. **Clarity:** Clear separation between implemented and planned features
3. **Single Source:** No duplicate information across docs
4. **Maintainability:** Templates and CI for ongoing documentation health
5. **Usability:** User can find any information within 2 clicks from README

---

## Appendix: Quick Wins

These can be done immediately with minimal risk:

1. **Delete** `docs/README_IMPROVEMENT_PLAN.md` (superseded)
2. **Create** `docs/planning/ROADMAP.md` with Phase 2 features
3. **Add** "Unimplemented" warnings to speculative sections
4. **Update** IMPLEMENTATION_STATUS.md with current dates
5. **Fix** version numbers in CHANGELOG.md if mismatched

---

## Next Steps

1. Review and approve this plan
2. Begin Phase 1 verification tasks
3. Create GitHub issues for each major task
4. Assign ownership and deadlines
