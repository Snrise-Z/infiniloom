# Infiniloom Refactoring Progress

This document tracks the progress of codebase improvements identified during the comprehensive code analysis.

## Overview

**Analysis Date:** 2025-12-26
**Total Issues Identified:** 7
**Estimated Duplicate Code:** ~1,200 lines

**Progress Summary:**
- **Completed:** 6 issues (~610 lines saved)
- **Needs Design:** 1 issue (scanner unification requires architectural decisions)
- **Remaining Estimate:** ~300 lines (after design is complete)

---

## Progress Tracker

### Phase 1: Critical - Major Code Duplication

| # | Issue | Status | Files Changed | Lines Saved |
|---|-------|--------|---------------|-------------|
| 1 | Unify Scanner (CLI + Bindings) | [~] Design Needed | - | ~300* |
| 2 | Consolidate Language Detection | [x] Done | 3 | ~170 |

*Note: Full unification requires architectural decisions. See detailed analysis below.

### Phase 2: Medium - API Consistency

| # | Issue | Status | Files Changed | Lines Saved |
|---|-------|--------|---------------|-------------|
| 3 | Fix Chunking Method Duplication | [x] Done | 1 | ~25 |
| 4 | Extract Bindings Shared Utils | [x] Done | 4 | ~240 |

### Phase 3: Minor - Cleanup

| # | Issue | Status | Files Changed | Lines Saved |
|---|-------|--------|---------------|-------------|
| 5 | Remove Dead Code (`#[allow(dead_code)]`) | [x] Done | 2 | ~10 |
| 6 | Fix Unused Parameter in Node Bindings | [x] Done | 1 | ~5 |
| 7 | Add Tokenizer Cache Limits | [x] Done | 1 | N/A |

---

## Detailed Issue Descriptions

### Issue 1: Scanner Duplication (CLI vs Bindings)

**Problem:** `cli/src/scanner.rs` (~970 lines) and `bindings/common/src/scanner.rs` (~430 lines) implement similar functionality with different architectures.

**Architectural Differences:**
| Feature | CLI Scanner | Bindings Scanner |
|---------|-------------|------------------|
| Architecture | Pipelined with crossbeam channels | Simple rayon parallel |
| Tokenization | Quick estimation (~4 chars/token) | Accurate tiktoken counting |
| Large files | Memory-mapped I/O (mmap) | Regular read with batching |
| Caching | Incremental hash-based cache | None |
| Git info | Branch/commit detection | None |
| Dependencies | Extracts external deps | None |
| Dir structure | Tree generation | None |

**Duplicated Code (~300 lines):**
- `ScanConfig` struct - identical
- `FileInfo` struct - identical
- `is_binary_extension()` - CLI has more extensions
- `is_binary_content()` - nearly identical
- Walk builder setup - similar pattern
- Statistics aggregation - similar logic

**Recommended Approach (Future Work):**
1. Create `engine/src/scanner/mod.rs` with:
   - Shared types (`ScanConfig`, `FileInfo`)
   - Binary detection utilities
   - Common trait for scanner implementations

2. Create `engine/src/scanner/parallel.rs`:
   - Simple parallel scanner (current bindings approach)

3. Create `engine/src/scanner/pipelined.rs`:
   - Pipelined scanner with channels (current CLI approach)

4. Let CLI and bindings choose appropriate implementation

**Status:** Requires design - architectural differences make quick unification risky

**Files to modify (when ready):**
- `engine/src/lib.rs` - Add scanner module export
- `engine/src/scanner/mod.rs` - Shared types and traits
- `engine/src/scanner/parallel.rs` - Simple parallel scanner
- `engine/src/scanner/pipelined.rs` - Pipelined scanner
- `cli/src/scanner.rs` - Use engine scanner with pipelined backend
- `bindings/common/src/scanner.rs` - Use engine scanner with parallel backend

---

### Issue 2: Language Detection Duplication

**Problem:** CLI has 170-line `detect_language()` function that duplicates `Language::from_extension()` in engine.

**Solution:**
- Extend `Language` enum to handle all edge cases (special filenames)
- Remove duplicate function from CLI scanner

**Files to modify:**
- `engine/src/parser/language.rs` - Add filename detection
- `cli/src/scanner.rs` - Remove `detect_language()` function

---

### Issue 3: Chunking Method Duplication

**Problem:** `determine_focus()` and `determine_focus_refs()` are nearly identical, differing only in input type.

**Solution:** Use generic implementation with `AsRef<RepoFile>` or iterator-based approach.

**Files to modify:**
- `engine/src/chunking/mod.rs`

---

### Issue 4: Bindings Shared Utils

**Problem:** Python and Node bindings have duplicated utility functions.

**Solution:** Extract common functions to `bindings/common/src/diff_utils.rs`

**Duplicated functions extracted:**
- `reconstruct_diff_from_hunks()` - Reconstruct unified diff from hunks
- `find_call_site_in_body()` - Find function call site within body
- `find_call_in_line()` - Find function call in a single line
- `get_line_context()` - Get code context around a specific line
- `load_file_lines()` - Load file content with caching (FileCache type)

**Files modified:**
- `bindings/common/src/diff_utils.rs` - New shared diff utilities (created)
- `bindings/common/src/lib.rs` - Export diff_utils module
- `bindings/python/src/lib.rs` - Import from common, removed ~140 lines
- `bindings/node/src/lib.rs` - Import from common, removed ~100 lines

---

### Issue 5: Dead Code Removal

**Problem:** Several functions marked with `#[allow(dead_code)]`

**Files and items:**
- `engine/src/index/builder/graph.rs:15` - `options` field
- `engine/src/remote.rs:233` - `clone()` method
- `engine/src/remote.rs:370` - `sparse_clone()` method

**Solution:** Remove `#[allow(dead_code)]` and either use or remove the code.

---

### Issue 6: Unused Parameter

**Problem:** `_model` parameter unused in Node bindings.

**File:** `bindings/node/src/lib.rs` - `scan_repository_with_options()`

**Solution:** Either use the parameter or remove it.

---

### Issue 7: Tokenizer Cache Limits

**Problem:** Token cache can grow unbounded in long-running processes.

**File:** `engine/src/tokenizer/core.rs`

**Solution:** Add LRU eviction or size limits to `TOKEN_CACHE`.

---

## Completion Log

| Date | Issue # | Description | Commit |
|------|---------|-------------|--------|
| 2025-12-26 | 6 | Removed unused `_model` parameter from Node bindings `scan_repository_with_options()` | pending |
| 2025-12-26 | 3 | Unified `determine_focus()` and `determine_focus_refs()` into `determine_focus_impl()` | pending |
| 2025-12-26 | 5 | Removed unused `options` field from `GraphBuilder` (other dead code items are intentional public API) | pending |
| 2025-12-26 | 7 | Added 100K entry limit to TOKEN_CACHE with automatic cleanup | pending |
| 2025-12-26 | 2 | Created `detect_file_language()` in engine, removed 170-line duplicate from CLI | pending |
| 2025-12-26 | 4 | Created `bindings/common/src/diff_utils.rs` with shared diff utilities, updated Node and Python bindings | pending |

---

## Notes

- All changes should maintain backward compatibility
- Run `cargo test --workspace` after each change
- Run `cargo clippy --workspace` to verify no new warnings
