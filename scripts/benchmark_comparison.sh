#!/usr/bin/env bash
#
# Benchmark comparison script for Infiniloom vs repomix vs gitingest
#
# This script compares the performance and output quality of Infiniloom
# against similar tools: repomix and gitingest.
#
# Prerequisites:
#   - cargo build --release (for infiniloom)
#   - npm install -g repomix
#   - pip install gitingest
#
# Usage:
#   ./scripts/benchmark_comparison.sh [test_repo_path]
#
# If no path is provided, a test repository will be created.

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RESULTS_DIR="$PROJECT_ROOT/benchmark_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo -e "${BLUE}=== Infiniloom Benchmark Comparison ===${NC}"
echo "Timestamp: $TIMESTAMP"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"

# Check if tools are available
check_tool() {
    local tool=$1
    local check_cmd=$2
    if eval "$check_cmd" &>/dev/null; then
        echo -e "  ${GREEN}✓${NC} $tool is available"
        return 0
    else
        echo -e "  ${RED}✗${NC} $tool is not available"
        return 1
    fi
}

echo "Checking available tools..."
INFINILOOM_AVAILABLE=false
REPOMIX_AVAILABLE=false
GITINGEST_AVAILABLE=false

if check_tool "Infiniloom" "$PROJECT_ROOT/target/release/infiniloom --version"; then
    INFINILOOM_AVAILABLE=true
fi

if check_tool "repomix" "command -v repomix"; then
    REPOMIX_AVAILABLE=true
fi

if check_tool "gitingest" "python3 -c 'import gitingest'"; then
    GITINGEST_AVAILABLE=true
fi

echo ""

# Create or use test repository
if [ -n "$1" ] && [ -d "$1" ]; then
    TEST_REPO="$1"
    echo "Using provided repository: $TEST_REPO"
else
    echo "Creating test repository..."
    TEST_REPO=$(mktemp -d)
    trap "rm -rf $TEST_REPO" EXIT

    # Create a realistic test repository structure
    mkdir -p "$TEST_REPO/src/components"
    mkdir -p "$TEST_REPO/src/utils"
    mkdir -p "$TEST_REPO/src/services"
    mkdir -p "$TEST_REPO/tests"
    mkdir -p "$TEST_REPO/docs"

    # Generate Rust files
    for i in $(seq 1 20); do
        cat > "$TEST_REPO/src/module_$i.rs" << 'EOF'
//! Module documentation

use std::collections::HashMap;

/// A sample struct
pub struct DataProcessor {
    name: String,
    items: Vec<i32>,
}

impl DataProcessor {
    /// Create a new processor
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            items: Vec::new(),
        }
    }

    /// Add an item
    pub fn add(&mut self, item: i32) {
        self.items.push(item);
    }

    /// Process all items
    pub fn process(&self) -> i32 {
        self.items.iter().sum()
    }
}

/// A helper function
pub fn helper_function(x: i32, y: i32) -> i32 {
    x * y + (x - y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor() {
        let mut p = DataProcessor::new("test");
        p.add(1);
        p.add(2);
        assert_eq!(p.process(), 3);
    }
}
EOF
    done

    # Generate Python files
    for i in $(seq 1 15); do
        cat > "$TEST_REPO/src/services/service_$i.py" << 'EOF'
"""Service module for data processing."""

from typing import List, Dict, Optional
from dataclasses import dataclass


@dataclass
class ServiceConfig:
    """Configuration for the service."""
    name: str
    timeout: int = 30
    retries: int = 3


class DataService:
    """Main service class for data operations."""

    def __init__(self, config: ServiceConfig):
        """Initialize the service with config."""
        self.config = config
        self._cache: Dict[str, any] = {}

    def process(self, data: List[int]) -> int:
        """Process a list of integers."""
        return sum(data)

    def fetch(self, key: str) -> Optional[any]:
        """Fetch from cache."""
        return self._cache.get(key)

    def store(self, key: str, value: any) -> None:
        """Store in cache."""
        self._cache[key] = value


def create_service(name: str) -> DataService:
    """Factory function to create a service."""
    config = ServiceConfig(name=name)
    return DataService(config)


if __name__ == "__main__":
    service = create_service("main")
    result = service.process([1, 2, 3, 4, 5])
    print(f"Result: {result}")
EOF
    done

    # Generate TypeScript files
    for i in $(seq 1 15); do
        cat > "$TEST_REPO/src/components/Component$i.tsx" << 'EOF'
/**
 * React component module
 */

import React, { useState, useEffect } from 'react';

interface Props {
  title: string;
  items: string[];
  onSelect?: (item: string) => void;
}

interface State {
  selected: string | null;
  loading: boolean;
}

/**
 * A sample component
 */
export const DataComponent: React.FC<Props> = ({ title, items, onSelect }) => {
  const [state, setState] = useState<State>({
    selected: null,
    loading: false,
  });

  useEffect(() => {
    // Effect for loading
    setState(prev => ({ ...prev, loading: true }));
    setTimeout(() => {
      setState(prev => ({ ...prev, loading: false }));
    }, 100);
  }, [items]);

  const handleSelect = (item: string) => {
    setState(prev => ({ ...prev, selected: item }));
    onSelect?.(item);
  };

  if (state.loading) {
    return <div>Loading...</div>;
  }

  return (
    <div className="component">
      <h2>{title}</h2>
      <ul>
        {items.map((item, index) => (
          <li
            key={index}
            onClick={() => handleSelect(item)}
            className={state.selected === item ? 'selected' : ''}
          >
            {item}
          </li>
        ))}
      </ul>
    </div>
  );
};

export default DataComponent;
EOF
    done

    # Create package files
    cat > "$TEST_REPO/Cargo.toml" << 'EOF'
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = { version = "1.0", features = ["full"] }
EOF

    cat > "$TEST_REPO/package.json" << 'EOF'
{
  "name": "test-project",
  "version": "1.0.0",
  "scripts": {
    "build": "tsc",
    "test": "jest"
  },
  "dependencies": {
    "react": "^18.0.0"
  }
}
EOF

    cat > "$TEST_REPO/.gitignore" << 'EOF'
target/
node_modules/
__pycache__/
*.pyc
dist/
.cache/
EOF

    cat > "$TEST_REPO/README.md" << 'EOF'
# Test Project

This is a test project for benchmarking repository context tools.

## Features

- Multi-language support
- Component library
- Service layer

## Installation

```bash
npm install
cargo build
```
EOF

    echo "  Created test repository at: $TEST_REPO"
    echo "  - 20 Rust files"
    echo "  - 15 Python files"
    echo "  - 15 TypeScript files"
fi

# Count files and lines
FILE_COUNT=$(find "$TEST_REPO" -type f \( -name "*.rs" -o -name "*.py" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" \) | wc -l | tr -d ' ')
LINE_COUNT=$(find "$TEST_REPO" -type f \( -name "*.rs" -o -name "*.py" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" \) -exec cat {} \; | wc -l | tr -d ' ')

echo ""
echo -e "${YELLOW}Test Repository Stats:${NC}"
echo "  Files: $FILE_COUNT"
echo "  Lines: $LINE_COUNT"
echo ""

# Function to measure execution time
measure_time() {
    local name=$1
    shift
    local start=$(python3 -c 'import time; print(time.time())')
    "$@" > /dev/null 2>&1
    local end=$(python3 -c 'import time; print(time.time())')
    local elapsed=$(python3 -c "print(f'{$end - $start:.3f}')")
    echo "$elapsed"
}

# Results file
RESULTS_FILE="$RESULTS_DIR/benchmark_$TIMESTAMP.md"

cat > "$RESULTS_FILE" << EOF
# Infiniloom Benchmark Results

**Date:** $(date)
**Test Repository:** $TEST_REPO
**Files:** $FILE_COUNT
**Lines:** $LINE_COUNT

## Performance Comparison

| Tool | Execution Time (s) | Output Size (bytes) | Tokens (est.) |
|------|-------------------|--------------------| --------------|
EOF

# Benchmark Infiniloom
if [ "$INFINILOOM_AVAILABLE" = true ]; then
    echo -e "${BLUE}Benchmarking Infiniloom...${NC}"

    INFINILOOM_OUTPUT="$RESULTS_DIR/infiniloom_output_$TIMESTAMP.xml"

    # Warm up
    "$PROJECT_ROOT/target/release/infiniloom" scan "$TEST_REPO" > /dev/null 2>&1 || true

    # Measure time (5 runs, take average)
    TOTAL_TIME=0
    for i in $(seq 1 5); do
        TIME=$(measure_time "infiniloom" "$PROJECT_ROOT/target/release/infiniloom" pack "$TEST_REPO" --format xml -o "$INFINILOOM_OUTPUT")
        TOTAL_TIME=$(python3 -c "print($TOTAL_TIME + $TIME)")
    done
    INFINILOOM_TIME=$(python3 -c "print(f'{$TOTAL_TIME / 5:.3f}')")

    # Measure output size
    "$PROJECT_ROOT/target/release/infiniloom" pack "$TEST_REPO" --format xml -o "$INFINILOOM_OUTPUT" 2>/dev/null
    INFINILOOM_SIZE=$(wc -c < "$INFINILOOM_OUTPUT" | tr -d ' ')

    # Estimate tokens
    INFINILOOM_TOKENS=$(python3 -c "print(int($INFINILOOM_SIZE / 3.5))")

    echo "| Infiniloom | $INFINILOOM_TIME | $INFINILOOM_SIZE | ~$INFINILOOM_TOKENS |" >> "$RESULTS_FILE"
    echo -e "  ${GREEN}✓${NC} Time: ${INFINILOOM_TIME}s, Size: ${INFINILOOM_SIZE} bytes"
fi

# Benchmark repomix
if [ "$REPOMIX_AVAILABLE" = true ]; then
    echo -e "${BLUE}Benchmarking repomix...${NC}"

    REPOMIX_OUTPUT="$RESULTS_DIR/repomix_output_$TIMESTAMP.txt"

    # Warm up
    repomix "$TEST_REPO" -o "$REPOMIX_OUTPUT" > /dev/null 2>&1 || true

    # Measure time
    TOTAL_TIME=0
    for i in $(seq 1 5); do
        TIME=$(measure_time "repomix" repomix "$TEST_REPO" -o "$REPOMIX_OUTPUT")
        TOTAL_TIME=$(python3 -c "print($TOTAL_TIME + $TIME)")
    done
    REPOMIX_TIME=$(python3 -c "print(f'{$TOTAL_TIME / 5:.3f}')")

    # Measure output size
    repomix "$TEST_REPO" -o "$REPOMIX_OUTPUT" 2>/dev/null
    REPOMIX_SIZE=$(wc -c < "$REPOMIX_OUTPUT" 2>/dev/null | tr -d ' ' || echo "N/A")

    # Estimate tokens
    if [ "$REPOMIX_SIZE" != "N/A" ]; then
        REPOMIX_TOKENS=$(python3 -c "print(int($REPOMIX_SIZE / 3.5))")
    else
        REPOMIX_TOKENS="N/A"
    fi

    echo "| repomix | $REPOMIX_TIME | $REPOMIX_SIZE | ~$REPOMIX_TOKENS |" >> "$RESULTS_FILE"
    echo -e "  ${GREEN}✓${NC} Time: ${REPOMIX_TIME}s, Size: ${REPOMIX_SIZE} bytes"
fi

# Benchmark gitingest
if [ "$GITINGEST_AVAILABLE" = true ]; then
    echo -e "${BLUE}Benchmarking gitingest...${NC}"

    GITINGEST_OUTPUT="$RESULTS_DIR/gitingest_output_$TIMESTAMP.txt"

    # Measure time using Python
    GITINGEST_TIME=$(python3 << EOF
import time
import sys
sys.path.insert(0, '$TEST_REPO')

try:
    from gitingest import ingest

    # Warm up
    try:
        ingest('$TEST_REPO')
    except:
        pass

    # Measure
    times = []
    for _ in range(5):
        start = time.time()
        try:
            result = ingest('$TEST_REPO')
            with open('$GITINGEST_OUTPUT', 'w') as f:
                f.write(str(result) if result else '')
        except Exception as e:
            pass
        times.append(time.time() - start)

    print(f'{sum(times) / len(times):.3f}')
except Exception as e:
    print('N/A')
EOF
)

    # Measure output size
    if [ -f "$GITINGEST_OUTPUT" ]; then
        GITINGEST_SIZE=$(wc -c < "$GITINGEST_OUTPUT" | tr -d ' ')
        GITINGEST_TOKENS=$(python3 -c "print(int($GITINGEST_SIZE / 3.5))")
    else
        GITINGEST_SIZE="N/A"
        GITINGEST_TOKENS="N/A"
    fi

    echo "| gitingest | $GITINGEST_TIME | $GITINGEST_SIZE | ~$GITINGEST_TOKENS |" >> "$RESULTS_FILE"
    echo -e "  ${GREEN}✓${NC} Time: ${GITINGEST_TIME}s, Size: ${GITINGEST_SIZE} bytes"
fi

# Add feature comparison
cat >> "$RESULTS_FILE" << 'EOF'

## Feature Comparison

| Feature | Infiniloom | repomix | gitingest |
|---------|------------|---------|-----------|
| Multi-format output (XML/MD/JSON) | ✓ | ✓ | ✗ |
| Repository map/summary | ✓ | ✓ | ✓ |
| Symbol extraction (AST) | ✓ | ✗ | ✗ |
| Security scanning | ✓ | ✗ | ✗ |
| Token counting | ✓ | ✓ | ✓ |
| Multi-model support | ✓ | ✗ | ✗ |
| Compression levels | ✓ | ✗ | ✗ |
| .gitignore respect | ✓ | ✓ | ✓ |
| Python bindings | ✓ | ✗ | N/A |
| Node.js bindings | ✓ | N/A | ✗ |
| PageRank importance | ✓ | ✗ | ✗ |

## Output Format Comparison

### Infiniloom (XML)
- Structured XML with proper escaping
- Includes repository metadata
- Optional repository map with key symbols
- Compression levels for different use cases

### repomix
- Markdown-style output
- Simple concatenation of files
- Basic file tree structure

### gitingest
- Plain text output
- Focused on GitHub repositories
- Basic file listing

## Notes

- Times are averages of 5 runs
- Token estimates use ~3.5 chars per token (Claude approximation)
- All tools run with default settings
- Test repository contains mixed Rust/Python/TypeScript code
EOF

echo ""
echo -e "${GREEN}Benchmark complete!${NC}"
echo "Results saved to: $RESULTS_FILE"

# Print summary
echo ""
echo -e "${YELLOW}=== Summary ===${NC}"
if [ "$INFINILOOM_AVAILABLE" = true ]; then
    echo -e "  Infiniloom:  ${INFINILOOM_TIME}s, ${INFINILOOM_SIZE} bytes"
fi
if [ "$REPOMIX_AVAILABLE" = true ]; then
    echo -e "  repomix:   ${REPOMIX_TIME}s, ${REPOMIX_SIZE} bytes"
fi
if [ "$GITINGEST_AVAILABLE" = true ]; then
    echo -e "  gitingest: ${GITINGEST_TIME}s, ${GITINGEST_SIZE} bytes"
fi

# Quick analysis
echo ""
if [ "$INFINILOOM_AVAILABLE" = true ] && [ "$REPOMIX_AVAILABLE" = true ]; then
    SPEEDUP=$(python3 -c "
infiniloom = $INFINILOOM_TIME
repomix = float('$REPOMIX_TIME') if '$REPOMIX_TIME' != 'N/A' else 0
if repomix > 0:
    if infiniloom < repomix:
        print(f'Infiniloom is {repomix/infiniloom:.1f}x faster than repomix')
    else:
        print(f'repomix is {infiniloom/repomix:.1f}x faster than Infiniloom')
")
    echo -e "${BLUE}$SPEEDUP${NC}"
fi
