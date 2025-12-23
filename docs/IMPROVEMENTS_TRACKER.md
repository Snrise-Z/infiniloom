# Infiniloom Improvements Tracker

This document tracks the implementation status of high-ROI improvements identified during codebase analysis.

## Status Legend

- ⬜ **Not Started**
- 🔄 **In Progress**
- ✅ **Completed**
- 🧪 **Testing**
- ❌ **Blocked/Cancelled**

---

## Tier 1: Quick Wins (Low effort, High impact)

### 1. ✅ Chunk Overlap
**Command:** `chunk`
**Estimated Effort:** ~2 hours
**Files to Modify:**
- `engine/src/chunking/mod.rs`
- `cli/src/main.rs` (add `--overlap` flag)

**Description:**
Carry forward the last N tokens from each chunk to the next chunk to maintain context continuity in multi-turn LLM conversations.

**Implementation Notes:**
- Add `--overlap <TOKENS>` CLI flag (default: 0, suggested: 500-1000)
- Modify `ChunkStrategy` to include overlap parameter
- Prepend overlap content with marker: `<!-- [OVERLAP FROM PREVIOUS CHUNK] -->`

**Acceptance Criteria:**
- [x] `--overlap` flag added to CLI
- [x] Overlap content correctly copied between chunks
- [x] Overlap marked in output for LLM awareness (`<!-- [OVERLAP FROM PREVIOUS CHUNK] -->`)
- [x] Tests pass
- [x] Documentation updated

---

### 2. ✅ Summary Headers in Chunks
**Command:** `chunk`
**Estimated Effort:** ~4 hours
**Files to Modify:**
- `engine/src/chunking/mod.rs`
- `cli/src/main.rs` (generate_chunk_summary helper)

**Description:**
Auto-generate a summary header at the start of each chunk describing its contents (files, key symbols, module purpose).

**Implementation Notes:**
- Generate summary from: file paths, detected languages, symbol names
- Format: "Chunk N/M: [Module description] | Files: file1.rs, file2.rs | ~N tokens"
- Add `--no-chunk-summary` flag to disable (summaries enabled by default)

**Acceptance Criteria:**
- [x] Summary generation logic implemented (`generate_chunk_summary()` helper)
- [x] Summary included in XML/Markdown/Plain/TOON output formats (JSON/YAML use structured metadata)
- [x] `--no-chunk-summary` flag to disable
- [x] Tests pass
- [x] Documentation updated

---

### 3. ✅ Config Templates
**Command:** `init`
**Estimated Effort:** ~2 hours
**Files to Modify:**
- `engine/src/config.rs`
- `cli/src/main.rs` (add `--template` flag)

**Description:**
Pre-built configuration templates for common project types (Rust, Python, TypeScript, Go, Java).

**Implementation Notes:**
- Add `--template <TYPE>` flag: `rust`, `python`, `typescript`, `go`, `java`, `generic`
- Templates include appropriate include/exclude patterns, recommended settings
- Templates stored as const strings or embedded files

**Templates to Create:**
- [x] `rust` - Cargo.toml, *.rs, exclude target/
- [x] `python` - *.py, requirements.txt, exclude venv/, __pycache__/
- [x] `typescript` - *.ts, *.tsx, package.json, exclude node_modules/, dist/
- [x] `go` - *.go, go.mod, exclude vendor/
- [x] `java` - *.java, pom.xml/build.gradle, exclude target/, build/

**Acceptance Criteria:**
- [x] `--template` flag added to `init` command
- [x] All 5 templates implemented
- [x] Templates generate valid, well-commented configs
- [x] Tests pass
- [x] Documentation updated

---

### 4. ✅ Parallel Security Scan
**Command:** `pack`, `scan`
**Estimated Effort:** ~1 hour
**Files to Modify:**
- `cli/src/main.rs` (security scan sections)

**Description:**
Parallelize security scanning across files using Rayon for faster secret detection.

**Implementation Notes:**
- Already using Rayon for file processing
- Security scanner is stateless, safe to parallelize
- Use `par_iter_mut()` on files, scan each in parallel

**Acceptance Criteria:**
- [x] Security scanning parallelized in `pack` command
- [x] Security scanning parallelized in `scan` command
- [x] No race conditions or thread safety issues (SecurityScanner is stateless)
- [x] Performance improvement measured (expect 2-4x on multi-core)
- [x] Tests pass

---

## Tier 2: High Value (Medium effort, High impact)

### 5. ✅ Historical Context in Diff
**Command:** `diff`
**Estimated Effort:** ~8 hours
**Files to Modify:**
- `engine/src/git.rs`
- `engine/src/index/context.rs`
- `cli/src/main.rs`

**Description:**
For each changed function/file, include recent commit history showing when and why it was previously modified.

**Implementation Notes:**
- Add `--include-history` flag
- For each changed file/symbol, fetch last N commits that touched it
- Include commit hash, author, date, and message
- Use `git log --follow -p -- <file>` or similar

**Output Example:**
```xml
<change file="src/api.rs">
  <history>
    <commit hash="abc123" date="2024-01-10" author="dev">
      Fix authentication bug in API handler
    </commit>
    <commit hash="def456" date="2024-01-05" author="dev">
      Add rate limiting to API endpoints
    </commit>
  </history>
  <diff>...</diff>
</change>
```

**Acceptance Criteria:**
- [x] `--include-history` flag added
- [x] `--history-count <N>` flag for number of commits (default: 3)
- [x] History fetched per changed file
- [x] History included in XML/JSON/Markdown output (and YAML/TOON/Plain)
- [x] Tests pass
- [x] Documentation updated

---

### 6. ✅ Watch Mode for Index
**Command:** `index`
**Estimated Effort:** ~4 hours
**Files to Modify:**
- `cli/src/main.rs`

**Description:**
Watch for file changes and automatically update the symbol index, keeping it fresh for fast `diff` and `impact` queries.

**Implementation Notes:**
- Reuse `notify` crate (PollWatcher) already used in `pack --watch`
- Debounce events (500ms threshold)
- Full rebuild on changes (incremental would add complexity)
- Show status with emoji indicators (👀, 🔄, ✓, ✗)

**Acceptance Criteria:**
- [x] `--watch` flag added to `index` command
- [x] File watcher correctly detects changes (via PollWatcher)
- [x] Debounce to avoid excessive rebuilds (500ms)
- [x] Index saved after each rebuild
- [x] Graceful shutdown on Ctrl+C (via channel disconnect)
- [x] Tests pass
- [x] Documentation updated

---

### 7. ✅ Sampling Mode for Scan
**Command:** `scan`
**Estimated Effort:** ~4 hours
**Files to Modify:**
- `cli/src/main.rs`
- `cli/Cargo.toml` (added `rand` dependency)

**Description:**
For very large repositories (100K+ files), estimate statistics from a random sample for fast approximate results.

**Implementation Notes:**
- Add `--sample <N>` flag (e.g., `--sample 100` = sample 100 files)
- Add `--sample-percent <P>` flag (e.g., `--sample-percent 1` = 1% of files)
- Use random shuffling with truncation for sampling
- Extrapolate totals: `estimated_total = sample_total * (total_files / sample_size)`
- Mark output as "ESTIMATED" when sampling (both JSON and human-readable)

**Acceptance Criteria:**
- [x] `--sample` and `--sample-percent` flags added
- [x] Random sampling implemented correctly (using `rand` crate)
- [x] Extrapolation formula applied to tokens, bytes, and security issues
- [x] Output clearly marked as estimated (`[ESTIMATED]` label, `~` prefix for values)
- [x] Sampling metadata in JSON output (`is_estimated`, `sample_size`, `extrapolation_factor`)
- [x] Tests pass
- [x] Documentation updated

---

### 8. ✅ Priority-Based Chunking
**Command:** `chunk`
**Estimated Effort:** ~6 hours
**Files to Modify:**
- `cli/src/main.rs`

**Description:**
Order chunks so that core/important modules appear first, utilities and tests last. LLM gets critical context in early chunks.

**Implementation Notes:**
- Add `--priority-first` flag
- Priority scoring based on path patterns:
  - Entry points (main.rs, index.ts, __main__.py) → 100 (highest)
  - Config files (Cargo.toml, package.json) → 90
  - Core modules (lib/, core/, lib.rs, mod.rs) → 80
  - API/handlers/routes/controllers → 75
  - Source code (src/) → 60
  - Default files → 50
  - Utilities (utils/, helpers/) → 30
  - Tests → 20
  - Examples/docs → 10 (lowest)
- Sort chunks by average file priority descending

**Acceptance Criteria:**
- [x] `--priority-first` flag added
- [x] Priority scoring algorithm implemented (`file_priority_score()`)
- [x] Chunks ordered by priority (highest first)
- [x] Core modules appear in chunk 1
- [x] Tests/utilities appear in last chunks
- [x] Tests pass
- [x] Documentation updated

---

## Implementation Order

Recommended order based on dependencies and quick wins first:

1. **Parallel Security Scan** (1h) - Quickest, no dependencies
2. **Config Templates** (2h) - Standalone, improves UX immediately
3. **Chunk Overlap** (2h) - Core chunking improvement
4. **Summary Headers** (4h) - Builds on chunking
5. **Sampling Mode** (4h) - Standalone scan improvement
6. **Watch Mode for Index** (4h) - Reuses existing watch code
7. **Priority-Based Chunking** (6h) - Needs chunking improvements done
8. **Historical Context in Diff** (8h) - Most complex, save for last

**Total Estimated Time:** ~31 hours

---

## Progress Log

| Date | Item | Status | Notes |
|------|------|--------|-------|
| 2024-XX-XX | Document created | ✅ | Initial tracking setup |
| 2024-12-22 | Parallel Security Scan | ✅ | Already implemented with `par_iter()` and `par_iter_mut()` |
| 2024-12-22 | Config Templates | ✅ | Added `--template` flag with rust/python/typescript/go/java templates |
| 2024-12-22 | Chunk Overlap | ✅ | Added `--overlap` flag, content extraction with markers |
| 2024-12-22 | Summary Headers | ✅ | Added `generate_chunk_summary()`, `--no-chunk-summary` flag |
| 2024-12-22 | Sampling Mode | ✅ | Added `--sample`, `--sample-percent` with extrapolation and [ESTIMATED] labels |
| 2024-12-22 | Watch Mode for Index | ✅ | Added `--watch` flag with PollWatcher, 500ms debounce |
| 2024-12-22 | Priority-Based Chunking | ✅ | Added `--priority-first` flag with file priority scoring |
| 2024-12-22 | Historical Context in Diff | ✅ | Added `--include-history`, `--history-count` flags with history in all output formats |

---

## Notes

- All improvements should include tests
- All improvements should update relevant documentation in `docs/commands/`
- Performance improvements should be benchmarked before/after
- Breaking changes to CLI should be avoided (new flags only)
