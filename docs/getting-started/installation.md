# Installation

Infiniloom can be installed via npm, Homebrew, Cargo, pip, or from source.

## Recommended Methods

### npm (Cross-platform)

The easiest way to install Infiniloom on any platform:

```bash
npm install -g infiniloom
```

This downloads a pre-built binary for your platform.

### Homebrew (macOS)

**Cask (recommended, pre-built binary):**

```bash
brew tap Topos-Labs/infiniloom
brew install --cask infiniloom
```

**Formula (builds from source):**

```bash
brew tap Topos-Labs/infiniloom
brew install infiniloom
```

### Cargo (Rust users)

If you have Rust installed:

```bash
cargo install infiniloom
```

Requires Rust 1.91+.

## Language Libraries

### Python

```bash
pip install infiniloom
```

```python
import infiniloom

context = infiniloom.pack("/path/to/repo", format="xml")
stats = infiniloom.scan("/path/to/repo")
```

### Node.js

```bash
npm install infiniloom-node
```

```javascript
const { pack, scan } = require('infiniloom-node');

const context = pack('./repo', { format: 'xml' });
const stats = scan('./repo');
```

## From Source

Clone and build:

```bash
git clone https://github.com/Topos-Labs/infiniloom.git
cd infiniloom
cargo build --release
```

The binary will be at `./target/release/infiniloom`.

### Prerequisites

- **Rust 1.91+**: Install via [rustup](https://rustup.rs/)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

### Build Options

```bash
# Debug build (faster compile, slower runtime)
cargo build

# Release build (slower compile, optimized)
cargo build --release

# With all features
cargo build --release --all-features

# Run tests
cargo test --workspace
```

## Verify Installation

```bash
infiniloom --version
infiniloom info
```

Expected output:

```
infiniloom 0.4.8
```

## Updating

### npm

```bash
npm update -g infiniloom
```

### Homebrew

```bash
brew upgrade infiniloom
```

### Cargo

```bash
cargo install infiniloom --force
```

### pip

```bash
pip install --upgrade infiniloom
```

## Uninstalling

### npm

```bash
npm uninstall -g infiniloom
```

### Homebrew

```bash
brew uninstall infiniloom
brew untap Topos-Labs/infiniloom
```

### Cargo

```bash
cargo uninstall infiniloom
```

### pip

```bash
pip uninstall infiniloom
```

## Troubleshooting

### "command not found"

Ensure the installation directory is in your PATH:

- **npm**: Usually `~/.npm-global/bin` or `/usr/local/bin`
- **Cargo**: `~/.cargo/bin`
- **Homebrew**: `/opt/homebrew/bin` (Apple Silicon) or `/usr/local/bin` (Intel)

### Build Errors

If building from source fails:

1. Ensure Rust 1.91+ is installed: `rustc --version`
2. Update Rust: `rustup update stable`
3. Clear cargo cache: `cargo clean`

### macOS Gatekeeper

If macOS blocks the binary:

```bash
xattr -d com.apple.quarantine $(which infiniloom)
```

Or allow in System Preferences → Security & Privacy.

## Next Steps

- [Quick Start](quick-start.md) — Get productive in 5 minutes
- [Cheat Sheet](../CHEATSHEET.md) — All commands at a glance
- [Configuration](../CONFIGURATION.md) — Set up your config file
- [Troubleshooting](../TROUBLESHOOTING.md) — Installation issues
