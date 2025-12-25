# `infiniloom impact` Command

## Overview

The `impact` command analyzes the impact of changes to a specific file or symbol. It shows what depends on the target, what the target depends on, and optionally visualizes the call graph. Essential for understanding blast radius before making changes.

## Synopsis

```bash
infiniloom impact [PATH] [TARGET] [OPTIONS]
```

**Default PATH**: Current directory (`.`)

## Description

The `impact` command:

1. **Identifies Target**: Locates the specified file or symbol
2. **Analyzes Dependencies**: Finds all dependents (what uses this)
3. **Analyzes Requirements**: Finds all dependencies (what this uses)
4. **Computes Impact Score**: Estimates change propagation
5. **Optionally Shows Call Graph**: Visualizes relationships

## Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--symbol` | | Analyze a symbol instead of a file | `false` |
| `--call-graph` | | Show visual call graph | `false` |
| `--json` | | Output as JSON | `false` |
| `--model <MODEL>` | `-m` | Target model for token counting | `claude` |
| `--depth <LEVEL>` | `-d` | Analysis depth (1=direct 5 items, 2=10 items, 3=20 items) | `2` |
| `--include <PATTERN>` | `-i` | Include only files matching glob pattern (repeatable) | all |
| `--exclude <PATTERN>` | `-e` | Exclude files/directories matching pattern (repeatable) | none |
| `--include-tests` | | Include test files in analysis | `false` |
| `--verbose` | `-v` | Show detailed progress | `false` |

## Output

### Human-Readable (Default)

```
Impact Analysis: src/api/handler.rs
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Impact Level: HIGH
Reason: Public API with 15 direct dependents

Dependents (files that import this):
  • src/routes/user.rs
  • src/routes/admin.rs
  • src/middleware/auth.rs
  • src/services/user_service.rs
  ... and 11 more

Dependencies (files this imports):
  • src/models/user.rs
  • src/database/connection.rs
  • src/utils/validation.rs

Related Tests:
  • tests/api/handler_tests.rs
  • tests/integration/api_tests.rs

Symbols:
  handle_request (function)
    ├─ Called by: 8 functions
    └─ Calls: 5 functions

  ResponseBuilder (struct)
    ├─ Used by: 12 locations
    └─ Uses: 3 types
```

### JSON Output

```json
{
  "target": "src/api/handler.rs",
  "target_type": "file",
  "impact_level": "high",
  "impact_reason": "Public API with 15 direct dependents",
  "dependents": {
    "count": 15,
    "files": [
      "src/routes/user.rs",
      "src/routes/admin.rs"
    ]
  },
  "dependencies": {
    "count": 3,
    "files": [
      "src/models/user.rs",
      "src/database/connection.rs"
    ]
  },
  "related_tests": [
    "tests/api/handler_tests.rs"
  ],
  "symbols": [
    {
      "name": "handle_request",
      "kind": "function",
      "callers": 8,
      "callees": 5
    }
  ]
}
```

### Call Graph (`--call-graph`)

```
Call Graph for: handle_request

Callers (what calls this):
┌─────────────────────────────────────────────────────┐
│ user_routes::get_user                               │
│ user_routes::update_user                            │
│ admin_routes::list_users                            │
│ middleware::auth::authenticate                       │
└─────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│              handle_request (TARGET)                 │
└─────────────────────────────────────────────────────┘
                          │
                          ▼
Callees (what this calls):
┌─────────────────────────────────────────────────────┐
│ database::query                                      │
│ validator::validate_input                            │
│ serializer::to_json                                  │
│ logger::log_request                                  │
│ metrics::record                                      │
└─────────────────────────────────────────────────────┘
```

## Impact Level Calculation

| Level | Criteria |
|-------|----------|
| **CRITICAL** | Entry point (main), or 50+ dependents |
| **HIGH** | Public API, or 10-50 dependents |
| **MEDIUM** | Internal module, or 3-10 dependents |
| **LOW** | Leaf node, or <3 dependents |

```rust
fn calculate_impact_level(dependents: usize, is_public: bool) -> ImpactLevel {
    match (dependents, is_public) {
        (50.., _) => ImpactLevel::Critical,
        (_, true) if dependents >= 10 => ImpactLevel::Critical,
        (10..50, _) => ImpactLevel::High,
        (3..10, _) => ImpactLevel::Medium,
        _ => ImpactLevel::Low,
    }
}
```

## Examples

### File Impact

```bash
# Analyze impact of a file
infiniloom impact src/core/parser.rs

# Analyze with call graph
infiniloom impact src/core/parser.rs --call-graph
```

### Symbol Impact

```bash
# Analyze impact of a specific function
infiniloom impact --symbol "parse_expression"

# Analyze class/struct
infiniloom impact --symbol "UserService"
```

### JSON for Tooling

```bash
# Get impact data for CI/CD
infiniloom impact src/api.rs --json | jq '.impact_level'

# Check if change is high-impact
if [ "$(infiniloom impact src/api.rs --json | jq -r '.impact_level')" = "high" ]; then
    echo "Requires senior review"
fi
```

## Best Practices for LLM Context

### Pre-Change Analysis

Before making changes, understand the blast radius:

```bash
# Check impact before refactoring
infiniloom impact src/core/types.rs

# If high impact, request more careful review
```

### Change Planning

```bash
# Generate context for high-impact change
if infiniloom impact src/api.rs --json | jq -e '.impact_level == "high"' > /dev/null; then
    infiniloom diff --depth 3 --budget 150000 -o context.xml
else
    infiniloom diff --depth 2 --budget 50000 -o context.xml
fi
```

### CI/CD Integration

```yaml
# GitHub Actions example
- name: Check change impact
  run: |
    IMPACT=$(infiniloom impact ${{ github.event.pull_request.changed_files }} --json)
    if echo "$IMPACT" | jq -e '.impact_level == "critical"'; then
      echo "::warning::Critical impact change requires additional review"
    fi
```

## Performance Characteristics

### With Index

| Operation | Time |
|-----------|------|
| Load index | ~100ms |
| Analyze file | <50ms |
| Analyze symbol | <100ms |
| Build call graph | <200ms |

### Without Index

| Operation | Time |
|-----------|------|
| Full repo parse | 2-30s (depends on size) |
| Analyze file | ~1s |
| Analyze symbol | ~2s |

## Potential Improvements

### 1. Transitive Impact Scoring

```bash
# Future: score based on transitive impact
infiniloom impact src/core.rs --transitive
# Shows: "Transitively affects 234 files through 12 intermediate modules"
```

### 2. Change Probability

```bash
# Future: historical analysis of co-changes
infiniloom impact src/api.rs --co-change-analysis
# Shows files that typically change together
```

### 3. Test Coverage Impact

```bash
# Future: integrate with test coverage
infiniloom impact src/api.rs --coverage
# Shows: "67% of dependents have test coverage"
```

### 4. API Surface Impact

```bash
# Future: analyze public API changes
infiniloom impact src/lib.rs --api-surface
# Shows: "This change affects 3 public functions, 2 public types"
```

### 5. Breaking Change Detection

```bash
# Future: semantic versioning impact
infiniloom impact src/api.rs --semver
# Shows: "This is a BREAKING change (signature modification)"
```

### 6. Visualization Export

```bash
# Future: export graph for visualization tools
infiniloom impact src/core.rs --export-dot > graph.dot
infiniloom impact src/core.rs --export-mermaid > graph.mmd
```

### 7. Reverse Impact (What This Affects)

```bash
# Future: trace forward impact
infiniloom impact src/utils.rs --forward
# Shows everything that would be affected by changes
```

### 8. Impact Diff

```bash
# Future: compare impact between versions
infiniloom impact --compare HEAD~10..HEAD
# Shows how impact has changed over time
```

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | Target file/symbol not found |
| 1 | Repository not indexed (without --symbol) |

## Related Commands

- [`index`](index.md) - Build index for fast impact analysis
- [`diff`](diff.md) - Generate context for changes
- [`map`](map.md) - See overall symbol importance
