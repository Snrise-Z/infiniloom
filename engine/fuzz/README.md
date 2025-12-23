# Fuzzing Targets for Infiniloom Engine

This directory contains fuzz targets for testing the Infiniloom engine with random inputs.

## Prerequisites

Install cargo-fuzz (requires nightly Rust):

```bash
cargo install cargo-fuzz
```

## Available Fuzz Targets

| Target | Description |
|--------|-------------|
| `fuzz_parse_rust` | Fuzz the Rust language parser |
| `fuzz_parse_python` | Fuzz the Python language parser |
| `fuzz_parse_javascript` | Fuzz the JavaScript language parser |
| `fuzz_tokenize` | Fuzz the multi-model tokenizer |
| `fuzz_security_scan` | Fuzz the security scanner |

## Running Fuzz Tests

```bash
cd engine/fuzz

# Run a specific fuzz target
cargo +nightly fuzz run fuzz_parse_rust

# Run with a time limit (e.g., 60 seconds)
cargo +nightly fuzz run fuzz_parse_rust -- -max_total_time=60

# Run with multiple jobs
cargo +nightly fuzz run fuzz_parse_rust -- -jobs=4 -workers=4

# Run all targets sequentially
for target in fuzz_parse_rust fuzz_parse_python fuzz_parse_javascript fuzz_tokenize fuzz_security_scan; do
    cargo +nightly fuzz run $target -- -max_total_time=60
done
```

## Corpus

Fuzz corpus is stored in `corpus/<target_name>/`. You can seed it with interesting inputs:

```bash
mkdir -p corpus/fuzz_parse_rust
cp /path/to/interesting/rust/files/* corpus/fuzz_parse_rust/
```

## Crashes

If a crash is found, it will be stored in `artifacts/<target_name>/`. To reproduce:

```bash
cargo +nightly fuzz run fuzz_parse_rust artifacts/fuzz_parse_rust/crash-...
```

## Coverage

To generate coverage reports:

```bash
cargo +nightly fuzz coverage fuzz_parse_rust
```
