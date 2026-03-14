# Test Coverage Improvement Plan - Focused on Bug Detection

## Current State
- **Overall Coverage**: 78.56% line coverage
- **Total Tests**: 2,377 passing
- **Gap to 90%**: ~6,000 lines need coverage

## Critical Low-Coverage Areas (Business Logic)

### Priority 1: Type Analysis (High Bug Risk)
1. **type_signature.rs** (1.78% - 1,216 missed lines)
   - 21 language parsers with minimal testing
   - Edge cases: Unicode identifiers, deeply nested generics, variadic params
   - **Bug Risk**: HIGH - untested parsers = silent failures

2. **complexity.rs** (20.84% - 452 missed lines)
   - Cyclomatic complexity calculation
   - Edge cases: nested loops, boolean operators, exception handlers
   - **Bug Risk**: HIGH - incorrect metrics mislead developers

3. **documentation.rs** (39.20% - 532 missed lines)
   - JSDoc/docstring parsing
   - Edge cases: malformed docs, special chars, multilanguage styles
   - **Bug Risk**: MEDIUM - parsing errors cause info loss

### Priority 2: Embedding System
4. **identifiers.rs** (43.86% - 535 missed lines)
   - Identifier extraction for RAG
   - Edge cases: reserved words, Unicode, very long names
   - **Bug Risk**: MEDIUM - affects search quality

5. **embedding/error.rs** (66.55% - 92 missed lines)
   - Error handling and classification
   - Edge cases: error propagation, user vs internal errors
   - **Bug Risk**: LOW - mostly error display logic

### Priority 3: CLI Commands (Integration)
6. **cli/src/commands/embed.rs** (0.00% - 648 missed lines)
   - CLI entry point - needs integration tests
   - Edge cases: file I/O errors, invalid args, manifest corruption
   - **Bug Risk**: HIGH for users, but CLI tested differently

7. **cli/src/commands/diff/* (23-50% coverage)
   - Diff formatting and context expansion
   - Edge cases: binary files, large diffs, conflicting changes
   - **Bug Risk**: MEDIUM - visual output issues

## Testing Strategy

### Phase 1: API Contract Tests (Weeks 1-2)
Focus on public API surface and expected behavior:

```rust
// Example: Type signature contract tests
#[test]
fn empty_param_list_returns_empty_vec() {
    // Validates: empty input → empty output (not null, not error)
}

#[test]
fn invalid_utf8_does_not_panic() {
    // Bug prevention: DoS via malformed input
}

#[test]
fn deeply_nested_generics_bounded() {
    // Bug prevention: stack overflow protection
}
```

**Target**: type_signature.rs to 50%+ (600 lines)

### Phase 2: Boundary Condition Tests (Weeks 3-4)
Edge cases that commonly cause bugs:

```rust
#[test]
fn complexity_single_branch_is_two() {
    // Validates: if statement adds exactly +1
}

#[test]
fn complexity_empty_function_is_one() {
    // Validates: base complexity calculation
}

#[test]
fn boolean_and_operator_adds_one() {
    // Validates: short-circuit evaluation counted
}
```

**Target**: complexity.rs to 70%+ (400 lines), identifiers.rs to 70%+ (250 lines)

### Phase 3: Integration Tests (Weeks 5-6)
CLI commands and end-to-end flows:

```bash
# Integration test examples
$ infiniloom embed . --max-tokens 1000
# Expected: chunks generated, no crashes

$ infiniloom embed . --diff-only
# Expected: manifest diff computed correctly

$ infiniloom pack . | infiniloom pack --watch
# Expected: watch mode detects file changes
```

**Target**: CLI commands to 50%+ (300 lines)

## Metrics for Success

### Coverage Targets (Progressive)
- ✅ **Current**: 78.56%
- 🎯 **Phase 1**: 82% (+600 lines, 2 weeks)
- 🎯 **Phase 2**: 87% (+650 lines, 4 weeks)
- 🎯 **Phase 3**: 90%+ (+350 lines, 6 weeks)

### Quality Metrics (More Important than Coverage %)
1. **Bug Detection**: Each test must verify expected behavior, not just hit lines
2. **Edge Case Focus**: Prioritize boundary conditions over happy paths
3. **No Flaky Tests**: All tests must be deterministic
4. **Fast Execution**: Test suite stays under 60 seconds
5. **Clear Assertions**: Each test documents what bug it prevents

## Implementation Approach

### DO:
✅ Test one function at a time with clear expected behavior
✅ Focus on edge cases: empty input, null, max values, Unicode
✅ Use property-based testing for parsers (proptest)
✅ Add regression tests for every bug found
✅ Document what each test validates

### DON'T:
❌ Write tests just to hit coverage lines
❌ Test private implementation details
❌ Create brittle tests that break on refactoring
❌ Add tests without understanding the code
❌ Ignore flaky or slow tests

## Example: Good vs Bad Tests

### ❌ Bad Test (Line Coverage Only)
```rust
#[test]
fn test_extract_python() {
    let code = "def foo(): pass";
    let sig = extract(code, Language::Python);
    assert!(sig.parameters.is_empty()); // What does this prove?
}
```

### ✅ Good Test (Bug Prevention)
```rust
#[test]
fn python_kwargs_parameter_kind_is_var_keyword() {
    // Bug: **kwargs was classified as VarPositional instead of VarKeyword
    // Expected: ParameterKind::VarKeyword for **kwargs parameters
    let code = "def foo(**kwargs): pass";
    let sig = extract(code, Language::Python);
    
    let kwargs = sig.parameters.iter().find(|p| p.name == "kwargs")
        .expect("should have kwargs parameter");
    
    assert_eq!(kwargs.kind, ParameterKind::VarKeyword,
        "**kwargs must be VarKeyword, not VarPositional or Positional");
}
```

## Timeline and Resources

### Week 1-2: Foundation
- Set up proptest for parser testing
- Create test helpers for all 21 languages
- Add API contract tests for type signatures
- **Deliverable**: 82% coverage

### Week 3-4: Edge Cases
- Boundary condition tests for complexity
- Unicode and internationalization tests
- DoS protection tests (very long inputs)
- **Deliverable**: 87% coverage

### Week 5-6: Integration
- CLI integration test framework
- End-to-end workflow tests
- Performance regression tests
- **Deliverable**: 90%+ coverage

## Current Commit Summary
- Fixed 3 blocking sqlite-manifest issues
- Applied 24 clippy auto-fixes
- Added all-features CI job
- Current coverage: 78.56%
- **Next**: Incremental test addition per above plan
