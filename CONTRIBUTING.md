# Contributing to Infiniloom

Thank you for your interest in contributing to Infiniloom! This document provides guidelines and information for contributors.

## Getting Started

### Prerequisites

- **Rust**: 1.91 or later
- **Git**: For version control

### Setting Up the Development Environment

```bash
# Clone the repository
git clone https://github.com/Topos-Labs/infiniloom.git
cd infiniloom

# Build the project
cargo build

# Run tests
cargo test

# Run with release optimizations
cargo build --release
```

## Development Workflow

### Branch Naming

- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation changes
- `refactor/` - Code refactoring
- `test/` - Test additions or modifications

### Commit Messages

Write clear, concise commit messages:

```
feat: add support for Ruby AST parsing
fix: correct token count for multi-byte characters
docs: update installation instructions
refactor: simplify output format selection
test: add integration tests for XML output
```

### Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- Follow existing code patterns and conventions
- Add documentation for public APIs

## Making Changes

### 1. Create a Branch

```bash
git checkout -b feature/your-feature-name
```

### 2. Make Your Changes

- Write clean, readable code
- Add tests for new functionality
- Update documentation as needed

### 3. Test Your Changes

```bash
# Run all tests
cargo test --all

# Run clippy
cargo clippy --all-targets --all-features

# Format code
cargo fmt
```

### 4. Submit a Pull Request

- Push your branch to your fork
- Create a pull request against `main`
- Provide a clear description of changes
- Reference any related issues

## Project Structure

```
infiniloom/
├── cli/          # Command-line interface
├── engine/       # Core processing engine
├── bindings/
│   ├── python/   # Python bindings (PyO3)
│   └── node/     # Node.js bindings (NAPI-RS)
└── docs/         # Documentation
```

## Testing

### Unit Tests

Located alongside source files in `src/` directories.

```bash
cargo test -p infiniloom-engine
cargo test -p infiniloom
```

### Integration Tests

Located in `tests/` directories.

```bash
cargo test --test '*'
```

### Benchmarks

```bash
cargo bench
```

## Areas for Contribution

### Good First Issues

Look for issues labeled `good-first-issue` for beginner-friendly tasks.

### Feature Ideas

- Additional language support for AST parsing
- New output formats
- Performance optimizations
- Documentation improvements
- Bug fixes

## Reporting Issues

When reporting issues, please include:

- Infiniloom version (`infiniloom --version`)
- Operating system and version
- Steps to reproduce
- Expected vs actual behavior
- Relevant error messages or logs

## Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help others learn and grow

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Questions?

- Open a GitHub issue for questions
- Check existing issues and discussions

Thank you for contributing to Infiniloom!
