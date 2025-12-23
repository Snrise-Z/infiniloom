#!/bin/bash
# E2E Test Runner for Infiniloom vs Repomix comparison

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "========================================"
echo "Infiniloom E2E Tests"
echo "========================================"
echo ""
echo "Project root: $PROJECT_ROOT"
echo "Test directory: $SCRIPT_DIR"
echo ""

# Check prerequisites
echo "Checking prerequisites..."

# Check for Python
if ! command -v python3 &> /dev/null; then
    echo "ERROR: Python 3 is required but not found"
    exit 1
fi
echo "  Python: $(python3 --version)"

# Check for pytest
if ! python3 -c "import pytest" &> /dev/null; then
    echo "  Installing pytest..."
    pip3 install pytest --quiet
fi
echo "  pytest: installed"

# Check for Infiniloom (Rust CLI)
INFINILOOM_BIN="$PROJECT_ROOT/target/release/infiniloom"
if [ -f "$INFINILOOM_BIN" ]; then
    echo "  Infiniloom: $INFINILOOM_BIN"
else
    echo "  Infiniloom: NOT FOUND - building..."
    cd "$PROJECT_ROOT"
    cargo build --release
    if [ -f "$INFINILOOM_BIN" ]; then
        echo "  Infiniloom: built successfully"
    else
        echo "ERROR: Failed to build Infiniloom"
        exit 1
    fi
fi

# Check for Repomix
if command -v repomix &> /dev/null; then
    echo "  Repomix: $(repomix --version)"
else
    echo "  Repomix: NOT FOUND - installing..."
    npm install -g repomix
    echo "  Repomix: $(repomix --version)"
fi

# Check for git
if ! command -v git &> /dev/null; then
    echo "ERROR: git is required but not found"
    exit 1
fi
echo "  git: $(git --version | head -1)"

echo ""
echo "========================================"
echo "Running Tests"
echo "========================================"
echo ""

cd "$SCRIPT_DIR"

# Parse arguments
TEST_MODE="${1:-quick}"
shift 2>/dev/null || true

case "$TEST_MODE" in
    quick)
        echo "Running quick smoke tests..."
        python3 -m pytest test_comparison.py -v -k "TestQuickSmoke" "$@"
        ;;
    full)
        echo "Running full E2E comparison tests..."
        python3 runner.py "$@"
        ;;
    pytest)
        echo "Running pytest suite..."
        python3 -m pytest test_comparison.py -v "$@"
        ;;
    repos)
        echo "Running specific repo tests..."
        python3 runner.py "$@"
        ;;
    *)
        echo "Usage: $0 [quick|full|pytest|repos] [repo_names...]"
        echo ""
        echo "Modes:"
        echo "  quick   - Run quick smoke tests (default)"
        echo "  full    - Run full E2E comparison on all repos"
        echo "  pytest  - Run pytest test suite"
        echo "  repos   - Run tests on specific repos (pass names as arguments)"
        echo ""
        echo "Examples:"
        echo "  $0 quick                    # Quick smoke tests"
        echo "  $0 full                     # Full comparison on all repos"
        echo "  $0 repos httpie ripgrep     # Test specific repos"
        echo "  $0 pytest -k 'performance'  # Run pytest with filter"
        exit 1
        ;;
esac

echo ""
echo "========================================"
echo "Tests Complete"
echo "========================================"
