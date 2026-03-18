# Infiniloom Style Guide

## Project Overview

Infiniloom is a high-performance repository context generator for Large Language Models. It transforms codebases into optimized formats (XML, Markdown, YAML) using Tree-sitter AST parsing and PageRank-based symbol ranking. The project is built in pure Rust for maximum performance and portability.

## Architecture

This is a Cargo workspace with three main areas:

- **`cli/`** -- Clap-based CLI application. Contains command implementations (`pack`, `scan`, `map`, `chunk`, `diff`, `index`, `impact`, `embed`, `ingest`, `init`, `info`) and the parallel file scanner.
- **`engine/`** -- Core library crate (`infiniloom-engine`). Houses all domain logic: Tree-sitter parsing (23 languages), tokenization (27 models), PageRank ranking, output formatting, security scanning, embedding chunk generation, document ingestion, and symbol indexing.
- **`bindings/`** -- Native language bindings:
  - `bindings/common/` -- Shared utilities used by both binding crates.
  - `bindings/python/` -- PyO3 + Maturin bindings.
  - `bindings/node/` -- NAPI-RS bindings.

Additionally, `packages/infiniloom/` contains the npm CLI wrapper that downloads the prebuilt binary.

## Rust Conventions

### Formatting and Linting

All code must pass:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Clippy is configured strictly in `Cargo.toml`:
- `correctness` and `perf` lints are **deny** (build errors).
- `suspicious`, `complexity`, `style` lints are **warn**.
- Print macros (`print_stdout`, `print_stderr`) are warned except in the CLI crate.

### Error Handling

- Use `thiserror` for library error types in the engine crate.
- Use `anyhow` (or similar) in the CLI crate for ad-hoc errors.
- Never `unwrap()` or `expect()` on user-supplied data. These are acceptable only for programmer invariants that are guaranteed by construction.

### Dependencies

Keep the binary lean. Do not add dependencies unless clearly justified. Every new crate increases compile time and binary size. If a small utility function can be written in 20 lines, prefer that over pulling in a crate.

Feature gates exist for optional functionality:
- `document` -- Document ingestion (MD, HTML, CSV, DOCX).
- `document-xlsx` -- XLSX support via `calamine`.
- `watch` -- File watching via `notify`.
- `embeddings` -- Local embeddings (heuristic-based).

New optional functionality should follow this pattern and be feature-gated.

## Security Model

### Threat Boundaries

- **CLI arguments and file contents are untrusted input.** Validate, sanitize, and bound all user-provided paths, patterns, and data. Assume adversarial input for anything that comes from the filesystem or command line.
- **Environment variables are trusted.** They are set by the user or CI system and do not require the same level of validation.

### Secret Scanning

The engine includes a secret detection and redaction system (`engine/src/security.rs`). Any new output pathway must integrate with the security scanner. Never emit raw file content without offering a security scan option.

### Resource Limits

The embedding system enforces `ResourceLimits` to prevent denial-of-service from malicious repositories (max file size, max chunks, max files). Respect these limits in any new code that processes untrusted repositories.

## Testing

### Coverage Expectations

- All new and modified lines must have corresponding tests.
- Unit tests live in `#[cfg(test)] mod tests` blocks within the same file.
- Integration tests go in `tests/` directories.
- Property-based tests use `proptest` where appropriate (e.g., for parsers and serialization).

### Running Tests

```bash
cargo test --workspace                    # All tests
cargo test -p infiniloom-engine           # Engine crate only
cargo test -- --nocapture                 # With stdout/stderr output
cargo test test_name                      # Specific test
```

### Test Quality

- Tests should be deterministic. Avoid relying on timing, network, or filesystem ordering.
- Use `assert_eq!` with descriptive messages over bare `assert!`.
- For the embedding system, verify determinism: same input must produce identical output across runs and platforms.

## Performance

### Guidelines

- Performance matters. Infiniloom processes large codebases and users expect speed.
- Do not introduce O(n^2) or worse algorithms without justification and a size cap.
- Use Rayon parallel iterators for CPU-bound batch processing (file parsing, token counting).
- Use thread-local storage for Tree-sitter parsers to avoid mutex contention.
- Profile before optimizing. Use `cargo bench` to measure, not guess.
- Never merge a change that regresses benchmark performance without discussion.

### Key Patterns Already in Use

- Thread-local Tree-sitter parsers for lock-free parallel parsing.
- Memory-mapped I/O for large file scanning.
- Incremental caching with mtime/size fast path and content hash fallback.
- Lazy tiktoken initialization for token counting.

## Pull Request Guidelines

### Scope

- Each PR should address one concern (feature, bugfix, refactor).
- If you discover an improvement opportunity outside the PR's scope, note it as a "follow-up" comment -- do not block the current PR on it.
- Keep PRs reviewable: prefer smaller, focused changes over large sweeping ones.

### Checklist

Before submitting a PR:

1. `cargo fmt --all` passes.
2. `cargo clippy --workspace --all-targets --all-features` passes with no warnings.
3. `cargo test --workspace` passes.
4. New public APIs have doc comments.
5. No new `unsafe` blocks without a `// SAFETY:` comment explaining the invariant.
6. No new dependencies without justification in the PR description.
7. Commit messages are clear and descriptive.

### Review Severity

Code review comments are prioritized as:

- **CRITICAL** -- Security vulnerabilities, data loss, correctness bugs. Must fix before merge.
- **HIGH** -- Performance regressions, missing tests, API design issues. Should fix before merge.
- **MEDIUM** -- Code clarity, documentation gaps, minor design concerns. Fix or acknowledge.
- **LOW** -- Style preferences, naming nitpicks. Author's discretion.
