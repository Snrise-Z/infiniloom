# Fair Comparison Plan: Infiniloom vs Repomix vs Gitingest

## Problem Statement

The initial benchmark showed Infiniloom producing 10x smaller output than competitors. However, this comparison was not apples-to-apples because:

- **Infiniloom defaults**: Excludes tests, docs, examples by default
- **Repomix defaults**: Includes everything (respects .gitignore only)
- **Gitingest defaults**: Includes everything (respects .gitignore only)

## Lodash Repository Analysis

| Tool | Files Included | Estimated Tokens | Notes |
|------|----------------|------------------|-------|
| Infiniloom (default) | 33 | ~83K | Excludes test/, doc/, .github/ |
| Repomix | 63 | ~513K | Includes all, test/test.js = 40% |
| Gitingest | 61 | ~512K | Includes all |

**Key insight**: `test/test.js` alone is 204K tokens (40% of Repomix output)

## Fair Comparison Strategy

### Comparison A: All Files Included (Match Competitors)

Run Infiniloom with all exclusions disabled to match what competitors include:

```bash
# Infiniloom with same content as competitors
infiniloom pack . --include-tests --include-docs --no-default-ignores --format xml
```

**What this tests**: Raw processing speed and output format overhead on identical content

### Comparison B: Smart Filtering Applied (Match Infiniloom)

Run competitors with exclusion patterns to match Infiniloom's defaults:

```bash
# Repomix with exclusions
repomix . -o output.xml --ignore "test/**,doc/**,docs/**,.github/**,examples/**,*.test.*,*.spec.*"

# Gitingest with exclusions
gitingest . --exclude-pattern "test/**" --exclude-pattern "doc/**" --exclude-pattern ".github/**"
```

**What this tests**: Whether competitors can achieve similar efficiency with configuration

### Comparison C: Token-Budgeted Output

All tools with a 100K token budget (if supported):

```bash
# Infiniloom with budget
infiniloom pack . --max-tokens 100000 --include-tests --include-docs

# Repomix (check if budget option exists)
repomix . --max-tokens 100000

# Gitingest (check if budget option exists)
gitingest . --max-tokens 100000
```

**What this tests**: How each tool handles content prioritization

## Metrics to Compare

### 1. Output Content Equivalence
- Count files included in each output
- Verify same files are present
- Compare actual text content (minus formatting)

### 2. Performance Metrics
- Execution time
- Memory usage (peak RSS)
- Output file size

### 3. Output Quality Metrics
- Token count (using same tokenizer for all)
- Metadata overhead (structure vs content)
- Line number preservation
- Code formatting preservation

### 4. Feature Parity Check
| Feature | Infiniloom | Repomix | Gitingest |
|---------|------------|---------|-----------|
| Exclude patterns | `--exclude` | `--ignore` | `--exclude-pattern` |
| Include patterns | `--include` | `--include` | `--include-pattern` |
| Output format | `--format xml/json/md` | `-s xml/markdown` | `--output-style` |
| Token budget | `--max-tokens` | ? | ? |
| Test exclusion | `--include-tests` toggle | manual ignore | manual ignore |

## Implementation Plan

### Phase 1: Feature Discovery
1. Run `repomix --help` and document all options
2. Run `gitingest --help` and document all options
3. Create equivalence mapping between tools

### Phase 2: Equivalent Configuration
1. Create config files for each tool that produce equivalent output
2. Verify file counts match across all tools
3. Verify content is equivalent (diff actual code sections)

### Phase 3: Fair Benchmarks
1. **Benchmark A**: All tools with all files included
2. **Benchmark B**: All tools with Infiniloom-style exclusions
3. **Benchmark C**: All tools with token budget (if supported)

### Phase 4: Analysis
1. Calculate true overhead (output size / source size)
2. Measure token efficiency per tool
3. Document feature gaps

## Expected Outcomes

After fair comparison, we expect to measure:

1. **Processing Speed**: Time to process identical content
2. **Format Overhead**: XML/MD structure bytes vs raw content bytes
3. **Compression Effectiveness**: With/without Infiniloom compression options
4. **Feature Value**: What extra value do Infiniloom's unique features add?

## Files to Create

1. `fair_benchmark.py` - New benchmark script with equivalent configurations
2. `tool_configs/` - Config files for each tool
3. `FAIR_BENCHMARK_RESULTS.md` - Results with methodology documented
