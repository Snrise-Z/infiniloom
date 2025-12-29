# Clippy Configuration and Best Practices

This document describes Infiniloom's clippy configuration and linting strategy.

## Overview

Infiniloom uses a comprehensive clippy configuration in the workspace `Cargo.toml` that enforces strict code quality standards while allowing pragmatic exceptions for real-world development.

## Lint Levels

### Deny (Fatal Errors)

These lint groups are set to **deny**, meaning violations will fail compilation:

- **`correctness`**: Catches code that is definitely wrong or buggy
- **`perf`**: Catches code with performance issues

### Warn (Should Fix)

These lint groups are set to **warn**, meaning violations should be addressed:

- **`suspicious`**: Catches code that is likely wrong or suspicious
- **`complexity`**: Catches unnecessarily complex code
- **`style`**: Catches code that doesn't follow Rust idioms

### Allow (Disabled)

- **`restriction`**: Too pedantic for most projects, disabled by default

## Specific Lint Configuration

### Debugging and Logging Hygiene

```toml
dbg_macro = "warn"          # Don't commit debug macros
todo = "warn"               # Replace with proper error handling
print_stdout = "warn"       # Use logging instead (allowed in CLI)
print_stderr = "warn"       # Use logging instead (allowed in CLI)
```

**Note**: `print_stdout` and `print_stderr` are allowed in the `cli` crate via `#![allow(clippy::print_stdout)]` since CLI tools legitimately need console output.

### Memory Safety

```toml
rc_buffer = "warn"          # Prefer Arc<[T]> over Rc<Vec<T>>
rc_mutex = "warn"           # Prefer Arc<Mutex<T>> over Rc<Mutex<T>>
clone_on_ref_ptr = "warn"   # Avoid cloning Rc/Arc pointers
```

### String Handling

```toml
str_to_string = "warn"      # Prefer to_owned() for clarity
```

### Code Quality and Best Practices (Added in Phase 4)

```toml
explicit_deref_methods = "warn"        # Use * instead of deref()
explicit_iter_loop = "warn"            # Use for loop instead of .iter().for_each()
filter_map_next = "warn"               # Use find_map() instead
flat_map_option = "warn"               # Use and_then() instead
inefficient_to_string = "warn"         # Avoid inefficient ToString impls
manual_ok_or = "warn"                  # Use ok_or() instead of match
map_flatten = "warn"                   # Use flat_map() instead
map_unwrap_or = "warn"                 # Use map_or() instead
needless_pass_by_value = "warn"        # Take references when possible
redundant_clone = "warn"               # Avoid unnecessary clones
redundant_pattern_matching = "warn"    # Simplify pattern matching
single_char_pattern = "warn"           # Use char instead of string for single chars
unnecessary_wraps = "warn"             # Don't wrap return values unnecessarily
unused_self = "warn"                   # Methods that don't use self should be functions
```

### Documentation Quality (Added in Phase 4)

```toml
missing_docs_in_private_items = "allow"  # Too noisy, but good for libraries
undocumented_unsafe_blocks = "warn"      # All unsafe code must be documented
```

### Error Handling (Added in Phase 4)

```toml
panic_in_result_fn = "warn"  # Result functions shouldn't panic
unwrap_in_result = "warn"    # Avoid unwrap() in Result-returning functions
```

### Pragmatic Allows

These lints are disabled because they're too noisy or impractical for real-world code:

```toml
too_long_first_doc_paragraph = "allow"   # First paragraph can be long
single_match = "allow"                   # Single match is fine
result_unit_err = "allow"                # Result<T, ()> is fine
len_without_is_empty = "allow"           # Not always needed
enum_variant_names = "allow"             # Enum variant prefixes are fine
new_ret_no_self = "allow"                # Builder pattern is fine
useless_asref = "allow"                  # Sometimes needed for lifetimes
assigning_clones = "allow"               # Common pattern
vec_init_then_push = "allow"             # Common pattern
literal_string_with_formatting_args = "allow"  # Can be more readable
unnecessary_map_or = "allow"             # Sometimes more readable
too_many_arguments = "allow"             # Addressed with builder pattern
type_complexity = "allow"                # Sometimes unavoidable
wrong_self_convention = "allow"          # Sometimes necessary
module_name_repetitions = "allow"        # Common in large projects
similar_names = "allow"                  # Too noisy
multiple_crate_versions = "allow"        # Common in large dependency trees
missing_errors_doc = "allow"             # Documentation in progress
missing_panics_doc = "allow"             # Documentation in progress
must_use_candidate = "allow"             # Too many false positives
doc_markdown = "allow"                   # Too pedantic
items_after_statements = "allow"         # Common pattern
redundant_closure_for_method_calls = "allow"  # Sometimes more readable
significant_drop_tightening = "allow"    # Too pedantic
```

## Rust Lints

In addition to clippy, the workspace configures Rust's built-in lints:

### Lifetime and Reference Clarity

```toml
elided_lifetimes_in_paths = "warn"
explicit_outlives_requirements = "warn"
unused_lifetimes = "warn"
```

### Safety

```toml
unsafe_op_in_unsafe_fn = "warn"  # Unsafe operations in unsafe fn need unsafe blocks
unsafe_code = "warn"              # Unsafe code requires justification
```

### Visibility and Exports

```toml
unreachable_pub = "warn"          # Don't export items that can't be reached
unused_extern_crates = "warn"     # Clean up unused dependencies
```

### Future Compatibility

```toml
future_incompatible = { level = "deny", priority = -1 }
rust_2018_idioms = { level = "warn", priority = -1 }
rust_2021_compatibility = { level = "warn", priority = -1 }
```

### Code Correctness

```toml
unused = { level = "warn", priority = -1 }
nonstandard_style = { level = "warn", priority = -1 }
```

### Explicit Patterns

```toml
let_underscore_drop = "warn"      # Make drop explicit
meta_variable_misuse = "warn"     # Catch macro errors
trivial_casts = "warn"            # Remove unnecessary casts
trivial_numeric_casts = "warn"    # Remove unnecessary numeric casts
unused_qualifications = "warn"    # Remove unnecessary path qualifications
```

## Running Clippy

### Local Development

```bash
# Check all crates with strict lints
cargo clippy --workspace --all-targets --all-features

# Fix automatically fixable issues
cargo clippy --workspace --all-targets --all-features --fix

# Check specific crate
cargo clippy -p infiniloom-engine
```

### CI Integration

Clippy runs automatically in GitHub Actions CI on every push and pull request:

```yaml
- name: Clippy
  run: cargo clippy --workspace
```

The CI uses the same lint configuration as local development, ensuring consistency.

## Common Patterns and Fixes

### Pattern 1: Unnecessary Clone

**Before** (flagged by `redundant_clone`):
```rust
let s = String::from("hello");
let t = s.clone();
do_something(s);  // s is not used again
```

**After**:
```rust
let s = String::from("hello");
do_something(s);
```

### Pattern 2: Inefficient String Conversion

**Before** (flagged by `str_to_string` and `inefficient_to_string`):
```rust
let s: String = "hello".to_string();
```

**After**:
```rust
let s: String = "hello".to_owned();
// or
let s = String::from("hello");
```

### Pattern 3: Unnecessary Dereference

**Before** (flagged by `explicit_deref_methods`):
```rust
let val = my_ref.deref();
```

**After**:
```rust
let val = *my_ref;
```

### Pattern 4: Single Character Pattern

**Before** (flagged by `single_char_pattern`):
```rust
s.split(",").collect()
```

**After**:
```rust
s.split(',').collect()  // char is more efficient than &str
```

### Pattern 5: Redundant Pattern Matching

**Before** (flagged by `redundant_pattern_matching`):
```rust
if let Some(_) = opt { true } else { false }
```

**After**:
```rust
opt.is_some()
```

### Pattern 6: Unwrap in Result Functions

**Before** (flagged by `unwrap_in_result`):
```rust
pub fn process() -> Result<String, Error> {
    let file = File::open("foo.txt").unwrap();  // Panics on error
    // ...
}
```

**After**:
```rust
pub fn process() -> Result<String, Error> {
    let file = File::open("foo.txt")?;  // Propagates error
    // ...
}
```

### Pattern 7: Panic in Result Functions

**Before** (flagged by `panic_in_result_fn`):
```rust
pub fn parse(s: &str) -> Result<u32, ParseError> {
    if s.is_empty() {
        panic!("empty string");  // Don't panic in Result functions
    }
    // ...
}
```

**After**:
```rust
pub fn parse(s: &str) -> Result<u32, ParseError> {
    if s.is_empty() {
        return Err(ParseError::EmptyString);
    }
    // ...
}
```

## Project-Specific Guidelines

### CLI Crate

The `cli` crate has `#![allow(clippy::print_stdout, clippy::print_stderr)]` at the top because CLI tools legitimately need to print to stdout/stderr.

### Unsafe Code

All unsafe code must:
1. Have `#[allow(unsafe_code)]` attribute with justification comment
2. Document safety invariants (flagged by `undocumented_unsafe_blocks`)
3. Use `unsafe` blocks inside `unsafe fn` (flagged by `unsafe_op_in_unsafe_fn`)

Example from `engine/src/mmap_scanner.rs`:
```rust
#[allow(unsafe_code)]
pub fn scan_with_mmap(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    // SAFETY: The file is opened read-only and we handle mmap errors
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    // ...
}
```

### Test Code

Test code (`#[cfg(test)]` modules and `tests/` directory) can use:
- `unwrap()` and `expect()` - tests should panic on failure
- `println!()` - useful for debugging test output
- Other pragmatic patterns that would be discouraged in production code

## Versioning and Updates

This configuration is based on:
- **Rust version**: 1.91 (workspace minimum)
- **Clippy version**: Stable channel (auto-updated with Rust)
- **Last review**: 2025-12-28 (Phase 4 Item 17)

The configuration should be reviewed periodically when:
- Upgrading to new Rust editions (2024, etc.)
- New useful lints are added to clippy
- Project patterns change

## References

- [Clippy Lint List](https://rust-lang.github.io/rust-clippy/master/index.html)
- [Rust Lints](https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html)
- [Rust 2021 Edition Guide](https://doc.rust-lang.org/edition-guide/rust-2021/index.html)
