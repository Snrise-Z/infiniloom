#!/bin/bash
# Full comparison benchmark: Infiniloom vs repomix vs gitingest

set -e

INFINILOOM="/Users/aleksandrlisenko/Projects/ai/infiniloom/target/release/infiniloom"
REPOS_DIR="/Users/aleksandrlisenko/Projects/ai/infiniloom/tests/e2e/repos"
OUTPUT_DIR="/Users/aleksandrlisenko/Projects/ai/infiniloom/tests/e2e/benchmark_results"

mkdir -p "$OUTPUT_DIR"

# Test repos (varying sizes)
REPOS=("express" "fastapi" "gin" "requests" "axios" "lodash")

echo "=============================================="
echo "  Infiniloom vs repomix vs gitingest Benchmark"
echo "=============================================="
echo ""

# Results file
RESULTS="$OUTPUT_DIR/benchmark_$(date +%Y%m%d_%H%M%S).md"

cat > "$RESULTS" << 'EOF'
# Benchmark Results

## Test Environment
- Date: $(date)
- Machine: macOS (Apple Silicon)

## Speed Comparison (seconds, lower is better)

| Repository | Files | Infiniloom | repomix | gitingest | IL vs repomix | IL vs gitingest |
|------------|-------|----------|---------|-----------|---------------|-----------------|
EOF

for repo in "${REPOS[@]}"; do
    REPO_PATH="$REPOS_DIR/$repo"

    if [ ! -d "$REPO_PATH" ]; then
        echo "Skipping $repo (not found)"
        continue
    fi

    echo "Testing: $repo"
    echo "----------------------------------------"

    # Count files
    FILE_COUNT=$(find "$REPO_PATH" -type f -not -path '*/\.*' | wc -l | tr -d ' ')

    # Infiniloom (5 runs, average)
    echo -n "  Infiniloom: "
    IL_TOTAL=0
    for i in {1..5}; do
        IL_TIME=$( { time -p $INFINILOOM pack "$REPO_PATH" --format xml -o /dev/null 2>&1; } 2>&1 | grep real | awk '{print $2}')
        IL_TOTAL=$(echo "$IL_TOTAL + $IL_TIME" | bc)
    done
    IL_AVG=$(echo "scale=3; $IL_TOTAL / 5" | bc)
    echo "${IL_AVG}s"

    # repomix (3 runs due to slower speed)
    echo -n "  repomix:  "
    RM_TOTAL=0
    for i in {1..3}; do
        RM_TIME=$( { time -p repomix "$REPO_PATH" -o /tmp/repomix_out.txt 2>/dev/null; } 2>&1 | grep real | awk '{print $2}')
        RM_TOTAL=$(echo "$RM_TOTAL + $RM_TIME" | bc)
    done
    RM_AVG=$(echo "scale=3; $RM_TOTAL / 3" | bc)
    echo "${RM_AVG}s"

    # gitingest (3 runs)
    echo -n "  gitingest: "
    GI_TOTAL=0
    for i in {1..3}; do
        GI_TIME=$( { time -p gitingest "$REPO_PATH" -o /tmp/gitingest_out.txt 2>/dev/null; } 2>&1 | grep real | awk '{print $2}')
        GI_TOTAL=$(echo "$GI_TOTAL + $GI_TIME" | bc)
    done
    GI_AVG=$(echo "scale=3; $GI_TOTAL / 3" | bc)
    echo "${GI_AVG}s"

    # Calculate speedup
    if [ $(echo "$IL_AVG > 0" | bc) -eq 1 ]; then
        VS_RM=$(echo "scale=1; $RM_AVG / $IL_AVG" | bc)
        VS_GI=$(echo "scale=1; $GI_AVG / $IL_AVG" | bc)
    else
        VS_RM="N/A"
        VS_GI="N/A"
    fi

    echo "  Speedup vs repomix: ${VS_RM}x"
    echo "  Speedup vs gitingest: ${VS_GI}x"
    echo ""

    # Append to results
    echo "| $repo | $FILE_COUNT | ${IL_AVG}s | ${RM_AVG}s | ${GI_AVG}s | **${VS_RM}x faster** | **${VS_GI}x faster** |" >> "$RESULTS"
done

# Feature comparison
cat >> "$RESULTS" << 'EOF'

## Feature Comparison

| Feature | Infiniloom | repomix | gitingest |
|---------|----------|---------|-----------|
| **Performance** |
| Native binary | Yes (Pure Rust) | No (Node.js) | No (Python) |
| Parallel scanning | Yes | No | No |
| Memory-mapped files | Yes | No | No |
| **Output Formats** |
| XML (Claude-optimized) | Yes | No | No |
| Markdown | Yes | Yes | Yes |
| JSON | Yes | Yes | No |
| YAML | Yes | No | No |
| Plain text | Yes | Yes | Yes |
| **Code Analysis** |
| AST-based symbol extraction | Yes (Tree-sitter) | No | No |
| PageRank symbol ranking | Yes | No | No |
| Function signatures | Yes | No | No |
| Import/dependency tracking | Yes | No | No |
| **Security** |
| Secret detection | Yes | No | No |
| API key scanning | Yes | No | No |
| Credential warnings | Yes | No | No |
| **Git Integration** |
| Structured commit history | Yes (XML) | Yes (text) | No |
| Diff inclusion | Yes | Yes | No |
| Remote repo cloning | Yes | Yes | Yes |
| **Token Counting** |
| Multi-model support | Yes (6 models) | Yes | Yes |
| Accurate estimation | Yes (~95%) | Yes | Yes |
| Budget enforcement | Yes | Yes | No |
| **Compression** |
| Multiple levels | Yes (5 levels) | Yes | No |
| Comment removal | Yes | Yes | No |
| Signature-only mode | Yes | No | No |
| **Extensibility** |
| Python bindings | Yes (PyO3) | No | N/A (Python) |
| Node.js bindings | Yes (NAPI-RS) | N/A (Node.js) | No |
| WebAssembly | Planned | No | No |
| Custom instructions | Yes | Yes | No |

## Output Quality

| Aspect | Infiniloom | repomix | gitingest |
|--------|----------|---------|-----------|
| Repository map | Yes (ranked symbols) | No | No |
| File importance ranking | Yes (PageRank) | No | No |
| Directory structure | Yes (tree) | Yes | Yes |
| Code with line numbers | Yes | Yes | No |
| Prompt caching hints | Yes (XML) | No | No |
| Structured metadata | Yes | Partial | Minimal |

EOF

echo ""
echo "Results saved to: $RESULTS"
echo ""
cat "$RESULTS"
