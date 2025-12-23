# Infiniloom Makefile
# Unified build, lint, test, and coverage commands

.PHONY: all build build-release clean test test-all test-python test-node lint fmt check coverage doc ci install-tools help

# Default target
all: fmt lint test build

# ============================================================================
# Build Commands
# ============================================================================

build:
	cargo build --workspace

build-release:
	cargo build --workspace --release

clean:
	cargo clean

# ============================================================================
# Testing
# ============================================================================

test:
	cargo test --workspace --all-features

test-release:
	cargo test --workspace --all-features --release

test-all: test test-python test-node

test-python:
	cd bindings/python && pip install maturin pytest
	cd bindings/python && maturin develop --release
	cd bindings/python && pytest -v

test-node:
	cd bindings/node && npm install
	cd bindings/node && npm run build
	cd bindings/node && npm test

# ============================================================================
# Linting & Formatting
# ============================================================================

# Format code
fmt:
	cargo fmt --all

# Check formatting without changing files
fmt-check:
	cargo fmt --all -- --check

# Run clippy with strictest settings
lint:
	cargo clippy --workspace --all-targets --all-features -- \
		-D warnings \
		-D clippy::all \
		-D clippy::pedantic \
		-D clippy::nursery \
		-W clippy::cargo

# Full check (compile without codegen)
check:
	cargo check --workspace --all-targets --all-features

# ============================================================================
# Code Coverage
# ============================================================================

# Install coverage tools if not present
install-coverage-tools:
	@command -v cargo-llvm-cov >/dev/null 2>&1 || cargo install cargo-llvm-cov
	@command -v grcov >/dev/null 2>&1 || cargo install grcov

# Generate coverage report (HTML)
coverage: install-coverage-tools
	cargo llvm-cov --workspace --all-features --html --output-dir target/coverage
	@echo "Coverage report generated at target/coverage/html/index.html"

# Generate coverage report (LCOV format for CI)
coverage-lcov: install-coverage-tools
	cargo llvm-cov --workspace --all-features --lcov --output-path target/coverage/lcov.info
	@echo "LCOV report generated at target/coverage/lcov.info"

# Generate coverage report (JSON for badges)
coverage-json: install-coverage-tools
	cargo llvm-cov --workspace --all-features --json --output-path target/coverage/coverage.json
	@echo "JSON report generated at target/coverage/coverage.json"

# Show coverage summary in terminal
coverage-summary: install-coverage-tools
	cargo llvm-cov --workspace --all-features

# Coverage with branch coverage
coverage-branches: install-coverage-tools
	CARGO_LLVM_COV_SHOW_BRANCHES=1 cargo llvm-cov --workspace --all-features --html --output-dir target/coverage-branches
	@echo "Branch coverage report generated at target/coverage-branches/html/index.html"

# Clean coverage data
coverage-clean:
	cargo llvm-cov clean --workspace

# ============================================================================
# Documentation
# ============================================================================

doc:
	cargo doc --workspace --all-features --no-deps
	@echo "Documentation generated at target/doc/"

doc-open: doc
	open target/doc/infiniloom_engine/index.html 2>/dev/null || xdg-open target/doc/infiniloom_engine/index.html

# ============================================================================
# CI Pipeline
# ============================================================================

ci: fmt-check lint test coverage-lcov
	@echo "CI pipeline completed successfully"

# Pre-commit checks (fast)
pre-commit: fmt-check check lint
	@echo "Pre-commit checks passed"

# ============================================================================
# Security Audit
# ============================================================================

audit:
	@command -v cargo-audit >/dev/null 2>&1 || cargo install cargo-audit
	cargo audit

# ============================================================================
# Benchmarks
# ============================================================================

bench:
	cargo bench --workspace

# ============================================================================
# Tool Installation
# ============================================================================

install-tools:
	@echo "Installing development tools..."
	rustup component add rustfmt clippy llvm-tools-preview
	cargo install cargo-llvm-cov cargo-audit cargo-deny cargo-outdated cargo-machete
	@echo "All tools installed successfully"

# Check for outdated dependencies
outdated:
	@command -v cargo-outdated >/dev/null 2>&1 || cargo install cargo-outdated
	cargo outdated --workspace

# Find unused dependencies
unused-deps:
	@command -v cargo-machete >/dev/null 2>&1 || cargo install cargo-machete
	cargo machete

# ============================================================================
# Release
# ============================================================================

release-check:
	cargo publish --dry-run -p infiniloom-engine
	cargo publish --dry-run -p infiniloom

# ============================================================================
# Help
# ============================================================================

help:
	@echo "Infiniloom Development Commands"
	@echo "==============================="
	@echo ""
	@echo "Build:"
	@echo "  make build          - Build debug version"
	@echo "  make build-release  - Build release version"
	@echo "  make clean          - Clean all build artifacts"
	@echo ""
	@echo "Testing:"
	@echo "  make test           - Run all tests"
	@echo "  make test-release   - Run tests in release mode"
	@echo "  make test-all       - Run Rust + Python + Node tests"
	@echo ""
	@echo "Code Quality:"
	@echo "  make fmt            - Format all code"
	@echo "  make fmt-check      - Check formatting"
	@echo "  make lint           - Run clippy lints"
	@echo "  make check          - Quick compile check"
	@echo ""
	@echo "Coverage:"
	@echo "  make coverage       - Generate HTML coverage report"
	@echo "  make coverage-lcov  - Generate LCOV report (for CI)"
	@echo "  make coverage-json  - Generate JSON report"
	@echo "  make coverage-summary - Show coverage in terminal"
	@echo ""
	@echo "CI/CD:"
	@echo "  make ci             - Run full CI pipeline"
	@echo "  make pre-commit     - Run pre-commit checks"
	@echo "  make audit          - Security audit"
	@echo ""
	@echo "Documentation:"
	@echo "  make doc            - Generate documentation"
	@echo "  make doc-open       - Generate and open docs"
	@echo ""
	@echo "Utilities:"
	@echo "  make install-tools  - Install all dev tools"
	@echo "  make outdated       - Check for outdated deps"
	@echo "  make unused-deps    - Find unused dependencies"
	@echo "  make help           - Show this help"
