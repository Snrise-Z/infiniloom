# `infiniloom diff` Command

## Overview

The `diff` command generates LLM-optimized context for code changes. It identifies changed files, expands context to include related code (callers, callees, tests), and outputs in a format suitable for AI code review or debugging assistance.

## Synopsis

```bash
infiniloom diff [PATH] [REFERENCE] [OPTIONS]
```

**Default PATH**: Current directory (`.`)
**Default REFERENCE**: Unstaged changes

## Description

The `diff` command:

1. **Identifies Changes**: Parses git diff to find modified files/symbols
2. **Classifies Changes**: Categorizes as signature change, body change, import change, etc.
3. **Expands Context**: Includes related code based on depth level
4. **Applies Budget**: Prioritizes most relevant context within token budget
5. **Formats Output**: Generates LLM-optimized context with change markers

## Context Depth Levels

| Level | Description | Includes |
|-------|-------------|----------|
| **L1** | Containing context | Changed files only |
| **L2** | Direct dependencies (default) | L1 + immediate callers/callees |
| **L3** | Transitive dependencies | L2 + callers of callers, full import chains |

### Depth Visualization

```
L3 ─────────────────────────────────────────────────────
│  Callers of callers, transitive imports
│
L2 ─────────────────────────────────────────────────────
│  Direct callers, direct callees, test files
│
L1 ─────────────────────────────────────────────────────
│  Changed files, containing functions
│
┌─────────────────────────────────────────────────────┐
│                    CHANGED CODE                      │
└─────────────────────────────────────────────────────┘
```

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--staged` | | Use staged changes instead of unstaged | `false` |
| `--depth <LEVEL>` | `-d` | Context depth (1, 2, or 3) | `2` |
| `--budget <TOKENS>` | `-b` | Token budget for context | `50000` |
| `--format <FORMAT>` | `-f` | Output format (xml, json, markdown, yaml, toon, plain) | `xml` |
| `--output <PATH>` | `-o` | Output file (default: stdout) | stdout |
| `--model <MODEL>` | `-m` | Target model for token counting | `claude` |
| `--include-diff` | | Include actual diff content (+/- lines) | `false` |
| `--include-history` | | Include recent commit history for changed files | `false` |
| `--history-count <N>` | | Number of recent commits to include per file | `3` |
| `--include <PATTERN>` | `-i` | Include only files matching glob pattern (repeatable) | all |
| `--exclude <PATTERN>` | `-e` | Exclude files/directories matching pattern (repeatable) | none |
| `--include-tests` | | Include test files in context (normally excluded) | `false` |
| `--verbose` | `-v` | Show detailed progress | `false` |

## Reference Formats

| Format | Description | Example |
|--------|-------------|---------|
| (none) | Unstaged changes | `infiniloom diff` |
| `--staged` | Staged changes | `infiniloom diff --staged` |
| `HEAD~N` | Last N commits | `infiniloom diff HEAD~1` |
| `COMMIT` | Specific commit | `infiniloom diff abc1234` |
| `BRANCH1..BRANCH2` | Branch comparison | `infiniloom diff main..feature` |
| `BRANCH1...BRANCH2` | Common ancestor diff | `infiniloom diff main...feature` |

## Output Structure

### XML Format (Default)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<diff_context repository="myproject" depth="L2" budget="50000">
  <impact_summary>
    <severity>high</severity>
    <description>Signature change in public API function</description>
    <affected_files>12</affected_files>
    <affected_symbols>34</affected_symbols>
  </impact_summary>

  <changes>
    <change file="src/api.rs" type="signature_change">
      <symbol name="process_request" kind="function"/>
      <diff><![CDATA[
- pub fn process_request(input: &str) -> Result<String>
+ pub fn process_request(input: &str, options: Options) -> Result<Response>
      ]]></diff>
    </change>
  </changes>

  <context>
    <file path="src/api.rs" role="changed">
      <content><![CDATA[...]]></content>
    </file>
    <file path="src/handlers.rs" role="caller">
      <symbol name="handle_api_call"/>
      <content><![CDATA[...]]></content>
    </file>
    <file path="tests/api_tests.rs" role="test">
      <content><![CDATA[...]]></content>
    </file>
  </context>
</diff_context>
```

### JSON Format

```json
{
  "repository": "myproject",
  "depth": "L2",
  "budget": 50000,
  "impact_summary": {
    "severity": "high",
    "description": "Signature change in public API function",
    "affected_files": 12,
    "affected_symbols": 34
  },
  "changes": [
    {
      "file": "src/api.rs",
      "type": "signature_change",
      "symbol": { "name": "process_request", "kind": "function" },
      "diff": "- pub fn process_request...\n+ pub fn process_request..."
    }
  ],
  "context": {
    "changed": [...],
    "callers": [...],
    "tests": [...]
  }
}
```

## Change Classification

| Type | Description | Impact Level | Context Expansion |
|------|-------------|--------------|-------------------|
| `added` | New file/symbol | Medium | Include file |
| `deleted` | Removed file/symbol | High | Include former callers |
| `renamed` | File/symbol renamed | Medium | Include all references |
| `signature_change` | Function signature modified | High | Include all callers |
| `body_change` | Implementation modified | Low | Include direct callers |
| `import_change` | Import statements modified | Medium | Include dependents |
| `type_definition` | Type/struct modified | High | Include all users |
| `doc_only` | Only documentation changed | Low | Minimal context |

## Technical Implementation

### Lazy Context Building

When no pre-built index exists, the command uses lazy context building:

```rust
// On-the-fly parsing without persistent index
let lazy_builder = LazyContextBuilder::new(&repo_path);
let context = lazy_builder.expand_diff(&diff_changes, depth)?;
```

### With Pre-built Index

```rust
// Fast lookup using pre-built index
let storage = IndexStorage::new(&repo_path);
let (index, graph) = storage.load_all()?;
let expander = ContextExpander::new(&index, &graph);
let context = expander.expand_diff(&diff_changes, depth)?;
```

### Budget Prioritization

When budget is limited, context is prioritized:

1. **Changed code** (always included)
2. **Signature context** (callers of changed signatures)
3. **Test files** (related tests)
4. **Direct callers** (L2)
5. **Transitive callers** (L3, budget permitting)

```rust
fn apply_budget(context: &mut ExpandedContext, budget: u32) {
    let mut used = 0;

    // Changed files: mandatory
    for file in &context.changed {
        used += file.tokens;
    }

    // Prioritize by impact
    context.callers.sort_by_key(|c| -c.impact_score);

    for caller in context.callers.drain(..) {
        if used + caller.tokens <= budget {
            context.included_callers.push(caller);
            used += caller.tokens;
        }
    }
}
```

## Examples

### Basic Usage

```bash
# Context for unstaged changes
infiniloom diff

# Context for staged changes
infiniloom diff --staged

# Context for last commit
infiniloom diff HEAD~1
```

### Branch Comparison

```bash
# Compare feature branch to main
infiniloom diff main..feature

# Compare to common ancestor
infiniloom diff main...feature
```

### Code Review Context

```bash
# Generate context for PR review
infiniloom diff main..feature --depth 2 --budget 100000 -o review-context.xml

# Include actual diff content
infiniloom diff HEAD~1 --include-diff
```

### JSON for Tooling

```bash
# Machine-readable output
infiniloom diff --format json | jq '.impact_summary'
```

### Historical Context

Include recent commit history for each changed file to provide context about when and why code was previously modified:

```bash
# Include 3 recent commits per file (default)
infiniloom diff HEAD~1 --include-history

# Include 5 recent commits
infiniloom diff main..feature --include-history --history-count 5

# Combine with other options
infiniloom diff --include-history --include-diff --format xml
```

**XML Output with History:**

```xml
<change file="src/api.rs" type="signature_change">
  <history>
    <commit hash="abc1234" date="2024-01-10" author="dev@example.com">
      Fix authentication bug in API handler
    </commit>
    <commit hash="def5678" date="2024-01-05" author="dev@example.com">
      Add rate limiting to API endpoints
    </commit>
    <commit hash="ghi9012" date="2023-12-20" author="dev@example.com">
      Initial API implementation
    </commit>
  </history>
  <diff>...</diff>
</change>
```

**JSON Output with History:**

```json
{
  "changes": [
    {
      "file": "src/api.rs",
      "type": "signature_change",
      "history": [
        {
          "hash": "abc1234",
          "date": "2024-01-10T15:30:00Z",
          "author": "dev@example.com",
          "message": "Fix authentication bug in API handler"
        }
      ]
    }
  ]
}
```

## Best Practices for LLM Context

### Optimal Depth Selection

| Scenario | Recommended Depth | Rationale |
|----------|------------------|-----------|
| Bug fix | L2 | Include callers to verify fix doesn't break them |
| Refactoring | L3 | Full impact analysis |
| New feature | L1 | Focus on new code |
| API change | L2-L3 | All consumers need review |

### Budget Guidelines

| Model | Context Window | Recommended Budget |
|-------|---------------|-------------------|
| Claude 3.5 | 200K | 100,000-150,000 |
| GPT-5/GPT-4o | 128K | 80,000-100,000 |
| GPT-4 Turbo | 128K | 80,000-100,000 |
| Gemini 3.1 Pro | 1M+ | 200,000-500,000 |

### Effective Prompts

```markdown
Given the following diff context, please:
1. Review the changes for potential bugs
2. Identify any callers that might be affected
3. Suggest any missing test cases

[Diff context from infiniloom diff]
```

## Performance Characteristics

### With Index

| Operation | Time |
|-----------|------|
| Load index | ~100ms |
| Expand L1 | <10ms |
| Expand L2 | <50ms |
| Expand L3 | <200ms |

### Without Index (Lazy)

| Operation | Time |
|-----------|------|
| Parse changed files | ~50ms per file |
| Build dependency graph | ~500ms |
| Expand L2 | ~1-2s |
| Expand L3 | ~3-5s |

## Potential Improvements

### 1. Smart Budget Allocation

```bash
# Future: AI-guided relevance scoring
infiniloom diff --smart-budget
# Allocate budget based on semantic relevance, not just call graph
```

### 2. Interactive Mode

```bash
# Future: iteratively request more context
infiniloom diff --interactive
# LLM can request: "show me the implementation of X"
```

### 3. Custom Expansion Rules

```bash
# Future: language-specific expansion
infiniloom diff --expand-traits  # Rust: include trait implementations
infiniloom diff --expand-interfaces  # Java: include interface implementers
```

### 4. Semantic Diff

```bash
# Future: semantic change detection
infiniloom diff --semantic
# Detect: "renamed variable" vs "changed logic"
```

### 5. Test Correlation

```bash
# Future: correlate with test coverage
infiniloom diff --test-coverage
# Show which tests cover the changed code
```

### 6. Cross-Repository Diffs

```bash
# Future: compare across repos
infiniloom diff repo1/main..repo2/feature
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Git error or invalid reference |
| 1 | No changes found (with --staged but nothing staged) |

## Related Commands

- [`index`](index.md) - Build index for fast diff context
- [`impact`](impact.md) - Analyze impact of specific files/symbols
- [`pack`](pack.md) - Generate full repository context
