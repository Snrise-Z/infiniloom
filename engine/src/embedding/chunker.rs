//! Core chunking logic for embedding generation
//!
//! This module generates deterministic, location-aware code chunks from a
//! repository. It uses thread-local parsers for parallel processing and
//! enforces resource limits for DoS protection.
//!
//! # Thread Safety
//!
//! The chunker uses thread-local parsers to avoid mutex contention during
//! parallel file processing. Each Rayon worker thread gets its own parser
//! instance.
//!
//! # Determinism Guarantees
//!
//! 1. Files are processed in sorted lexicographic order
//! 2. Symbols within files are sorted by (line, name)
//! 3. Output chunks are sorted by (file, line, id)
//! 4. All hash computations use integer-only math (no floats)

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

use rayon::prelude::*;

use crate::parser::{extraction::collect_calls_recursive, parse_file_symbols, Language};
use crate::security::SecurityScanner;
use crate::tokenizer::{TokenModel, Tokenizer};
use crate::types::Symbol;

use super::error::EmbedError;
use super::git_enrichment::GitMetadataCollector;
use super::hasher::hash_content;
use super::hierarchy::{HierarchyBuilder, HierarchyConfig};
use super::identifiers::extract_identifiers;
use super::limits::ResourceLimits;
use super::progress::ProgressReporter;
use super::type_extraction;
use super::types::{
    default_repr, ChunkContext, ChunkKind, ChunkPart, ChunkSource, EmbedChunk, EmbedSettings,
    RepoIdentifier, Visibility,
};

/// Statistics returned from streaming chunk generation
#[derive(Debug, Clone, Default)]
pub struct StreamingStats {
    /// Total files discovered in the repository
    pub total_files: usize,
    /// Total files successfully processed
    pub files_processed: usize,
    /// Total files skipped due to non-critical errors
    pub files_skipped: usize,
    /// Total chunks written to output
    pub total_chunks: usize,
    /// Number of batches processed
    pub batches_processed: usize,
    /// Number of chunks with a parent name that could not be linked to a parent container
    pub orphaned_chunks: u32,
}

/// A token-bounded content segment created by split chunking.
struct BudgetedSegment {
    /// Segment content.
    content: String,
    /// 1-indexed inclusive start line.
    start_line: u32,
    /// 1-indexed inclusive end line.
    end_line: u32,
    /// Token count for the configured token model.
    tokens: u32,
    /// Actual overlap lines included from the previous segment.
    overlap_lines: u32,
    /// Byte range within a single source line for overlong-line token slices.
    line_byte_range: Option<(u32, u32)>,
}

struct LineSlice {
    content: String,
    start_byte: usize,
    end_byte: usize,
}

type LineByteRanges = BTreeMap<u32, Vec<(u32, u32)>>;

/// Core chunker for generating embedding chunks
pub struct EmbedChunker {
    settings: EmbedSettings,
    limits: ResourceLimits,
    tokenizer: Tokenizer,
    security_scanner: Option<SecurityScanner>,
    /// Repository identifier for multi-tenant RAG
    repo_id: RepoIdentifier,
}

impl EmbedChunker {
    /// Create a new chunker with the given settings and limits
    pub fn new(settings: EmbedSettings, limits: ResourceLimits) -> Self {
        // Initialize security scanner if secret scanning is enabled
        let security_scanner = if settings.scan_secrets {
            Some(SecurityScanner::new())
        } else {
            None
        };

        Self {
            settings,
            limits,
            tokenizer: Tokenizer::new(),
            security_scanner,
            repo_id: RepoIdentifier::default(),
        }
    }

    /// Create a new chunker with default limits
    pub fn with_defaults(settings: EmbedSettings) -> Self {
        Self::new(settings, ResourceLimits::default())
    }

    /// Set the repository identifier for multi-tenant RAG
    ///
    /// This identifier is attached to all generated chunks, enabling:
    /// - Multi-repository search with proper attribution
    /// - Access control filtering by repository
    /// - Cross-repository dependency tracking
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let chunker = EmbedChunker::with_defaults(settings)
    ///     .with_repo_id(RepoIdentifier::new("github.com/myorg", "auth-service"));
    /// ```
    pub fn with_repo_id(mut self, repo_id: RepoIdentifier) -> Self {
        self.repo_id = repo_id;
        self
    }

    /// Set the repository identifier (mutable borrow version)
    pub fn set_repo_id(&mut self, repo_id: RepoIdentifier) {
        self.repo_id = repo_id;
    }

    /// Get the current repository identifier
    pub fn repo_id(&self) -> &RepoIdentifier {
        &self.repo_id
    }

    /// Generate chunks only for specific files in a repository
    ///
    /// This is used for git-diff-driven incremental updates where only changed
    /// files need to be re-chunked. The `only_files` set contains relative paths
    /// (from repo root) of files to process.
    ///
    /// # Guarantees
    ///
    /// Same as `chunk_repository`: deterministic, thread-safe, resource-limited.
    pub fn chunk_repository_filtered(
        &mut self,
        repo_path: &Path,
        only_files: &std::collections::HashSet<PathBuf>,
        progress: &dyn ProgressReporter,
    ) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Validate repo path
        let repo_root = self.validate_repo_path(repo_path)?;

        // Build repo identity from settings and git info if not already set
        self.populate_repo_identity(&repo_root);

        // Discover all files, then filter to only the specified ones
        progress.set_phase("Scanning repository (filtered)...");
        let mut files = self.discover_files(&repo_root)?;

        // Filter to only files in the changed set (match by relative path)
        files.retain(|f| {
            if let Ok(rel) = f.strip_prefix(&repo_root) {
                only_files.contains(rel)
            } else {
                false
            }
        });

        files.sort(); // Critical for determinism

        // Delegate to the shared chunking pipeline
        self.chunk_files_impl(files, &repo_root, progress)
    }

    /// Populate repo identity from settings and git info.
    ///
    /// Uses `repo_namespace` and `repo_name` from settings if provided,
    /// falling back to the directory name for `name`. Queries git for
    /// branch and commit if the path is inside a git repository.
    fn populate_repo_identity(&mut self, repo_path: &Path) {
        // Only populate if the repo_id hasn't been explicitly set via with_repo_id
        if !self.repo_id.name.is_empty() {
            return;
        }

        let namespace = self.settings.repo_namespace.clone();
        let name = self
            .settings
            .repo_name
            .clone()
            .or_else(|| {
                repo_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| "unknown".to_owned());

        // Try to get git branch and commit (best-effort, ignore errors)
        let (branch, commit) = match crate::git::GitRepo::open(repo_path) {
            Ok(git) => {
                let branch = git.current_branch().ok();
                let commit = git.current_commit().ok();
                (branch, commit)
            },
            Err(_) => (None, None),
        };

        self.repo_id = RepoIdentifier { namespace, name, version: None, branch, commit };
    }

    /// Generate all chunks for a repository
    ///
    /// # Guarantees
    ///
    /// 1. Deterministic output (same input = same output)
    /// 2. Thread-safe parallel processing
    /// 3. Resource limits enforced
    /// 4. Errors collected, not swallowed
    pub fn chunk_repository(
        &mut self,
        repo_path: &Path,
        progress: &dyn ProgressReporter,
    ) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Validate repo path
        let repo_root = self.validate_repo_path(repo_path)?;

        // Build repo identity from settings and git info if not already set
        self.populate_repo_identity(&repo_root);

        // Phase 1: Discover files (deterministic order)
        progress.set_phase("Scanning repository...");
        let mut files = self.discover_files(&repo_root)?;
        files.sort(); // Critical for determinism

        self.chunk_files_impl(files, &repo_root, progress)
    }

    /// Shared implementation for chunking a list of files
    fn chunk_files_impl(
        &self,
        files: Vec<PathBuf>,
        repo_root: &Path,
        progress: &dyn ProgressReporter,
    ) -> Result<Vec<EmbedChunk>, EmbedError> {
        progress.set_total(files.len());

        if files.is_empty() {
            return Err(EmbedError::NoChunksGenerated {
                include_patterns: "default".to_owned(),
                exclude_patterns: "default".to_owned(),
            });
        }

        // Check file limit
        if !self.limits.check_file_count(files.len()) {
            return Err(EmbedError::TooManyFiles {
                count: files.len(),
                max: self.limits.max_files,
            });
        }

        // Phase 2: Process files in parallel
        progress.set_phase("Parsing and chunking...");
        let chunk_count = Mutex::new(0usize);
        let processed = AtomicUsize::new(0);

        // Collect results AND errors (don't swallow errors)
        let results: Vec<Result<Vec<EmbedChunk>, (PathBuf, EmbedError)>> = files
            .par_iter()
            .map(|file| {
                let result = self.chunk_file(file, repo_root);

                // Update progress
                let done = processed.fetch_add(1, Ordering::Relaxed) + 1;
                progress.set_progress(done);

                match result {
                    Ok(chunks) => {
                        // Atomically check and update chunk count to prevent race conditions
                        // Use Mutex to ensure thread-safe limit enforcement
                        let chunks_to_add = chunks.len();
                        let mut count = chunk_count.lock().unwrap_or_else(|e| e.into_inner());
                        let new_count = *count + chunks_to_add;

                        // Check chunk limit BEFORE incrementing
                        if !self.limits.check_chunk_count(new_count) {
                            return Err((
                                file.clone(),
                                EmbedError::TooManyChunks {
                                    count: new_count,
                                    max: self.limits.max_total_chunks,
                                },
                            ));
                        }

                        *count = new_count;
                        drop(count); // Release lock before returning

                        Ok(chunks)
                    },
                    Err(e) => Err((file.clone(), e)),
                }
            })
            .collect();

        // Separate successes and failures
        let mut all_chunks = Vec::new();
        let mut errors = Vec::new();

        for result in results {
            match result {
                Ok(chunks) => all_chunks.extend(chunks),
                Err((path, err)) => errors.push((path, err)),
            }
        }

        // Report errors (fail on critical, warn on non-critical)
        if !errors.is_empty() {
            let critical: Vec<_> = errors
                .iter()
                .filter(|(_, e)| e.is_critical())
                .cloned()
                .collect();

            if !critical.is_empty() {
                return Err(EmbedError::from_file_errors(critical));
            }

            // Non-critical errors: log warning, continue
            for (path, err) in &errors {
                if err.is_skippable() {
                    progress.warn(&format!("Skipped {}: {}", path.display(), err));
                }
            }
        }

        // Check if any chunks were generated
        if all_chunks.is_empty() {
            return Err(EmbedError::NoChunksGenerated {
                include_patterns: "default".to_owned(),
                exclude_patterns: "default".to_owned(),
            });
        }

        self.finalize_chunks(&mut all_chunks, repo_root, progress);

        progress.set_phase("Complete");
        Ok(all_chunks)
    }

    /// Run the global post-processing phases shared by all chunking modes.
    ///
    /// Keeping this as a single path ensures streaming and non-streaming modes
    /// produce equivalent dependency, hierarchy, signature, ordering, and git
    /// metadata fields.
    fn finalize_chunks(
        &self,
        all_chunks: &mut Vec<EmbedChunk>,
        repo_root: &Path,
        progress: &dyn ProgressReporter,
    ) {
        // Phase 3: Build reverse call graph (called_by + dependents_count)
        progress.set_phase("Sanitizing chunk metadata...");
        Self::sanitize_chunk_metadata(all_chunks);

        progress.set_phase("Building call graph...");
        self.populate_called_by(all_chunks);

        // Phase 3b: Merge exact duplicate source spans after relation building so
        // canonical chunks keep alias calls/called_by/import metadata.
        progress.set_phase("Canonicalizing duplicate chunks...");
        let canonicalized = self.canonicalize_duplicate_chunks(all_chunks);
        if canonicalized > 0 {
            progress.warn(&format!(
                "Canonicalized {canonicalized} duplicate AST chunks into alias metadata"
            ));

            // Canonicalization can replace the chunk that represents a source
            // span, for example when a Python method also matched the generic
            // function query. Rebuild the graph from the canonical chunk set so
            // called_by/dependents_count never retain alias FQNs or parent-level
            // metadata on non-entry split fragments.
            progress.set_phase("Sanitizing canonical chunk metadata...");
            Self::sanitize_chunk_metadata(all_chunks);
            progress.set_phase("Rebuilding canonical call graph...");
            self.populate_called_by(all_chunks);
        }

        // Phase 3c: Remove fragments that became pure whitespace after
        // container child-body masking. These chunks are not useful retrieval
        // units and must not become hierarchy targets.
        progress.set_phase("Pruning empty chunks...");
        let pruned = self.prune_empty_chunks(all_chunks);
        if pruned > 0 {
            progress.warn(&format!("Pruned {pruned} empty chunks after fragment generation"));
            Self::sanitize_chunk_metadata(all_chunks);
            self.populate_called_by(all_chunks);
        }

        // Phase 3d: Link parent/children chunk IDs
        progress.set_phase("Linking parent/children chunks...");
        self.link_parent_children(all_chunks, progress);
        self.repair_live_chunk_references(all_chunks);

        // Phase 4: Build hierarchy summaries (if enabled)
        if self.settings.enable_hierarchy {
            progress.set_phase("Building hierarchy summaries...");
            let hierarchy_config = HierarchyConfig {
                min_children_for_summary: self.settings.hierarchy_min_children,
                ..Default::default()
            };
            let builder = HierarchyBuilder::with_config(hierarchy_config);

            // Enrich existing chunks with hierarchy metadata tags
            builder.enrich_chunks(all_chunks);

            // Generate summary chunks for containers (classes, structs, etc.)
            let mut summaries = builder.build_hierarchy(all_chunks);

            // Count tokens for summary chunks
            let token_model = self.parse_token_model(&self.settings.token_model);
            for summary in &mut summaries {
                summary.tokens = self.tokenizer.count(&summary.content, token_model);
            }

            all_chunks.extend(summaries);
        }

        // Phase 5: Generate signature-only chunks (if enabled)
        if self.settings.include_signatures {
            progress.set_phase("Generating signature chunks...");
            let signature_chunks = self.generate_signature_chunks(all_chunks);
            all_chunks.extend(signature_chunks);
        }

        progress.set_phase("Repairing chunk references...");
        self.repair_live_chunk_references(all_chunks);

        // Phase 6: Sort for deterministic output
        // Note: par_sort_by is unstable, but our comparison uses multiple tiebreakers
        // to guarantee no two elements ever compare equal, making stability irrelevant.
        // Order: file -> start line -> end line -> symbol name -> chunk ID
        progress.set_phase("Sorting chunks...");
        all_chunks.par_sort_by(|a, b| {
            a.source
                .file
                .cmp(&b.source.file)
                .then_with(|| a.source.lines.0.cmp(&b.source.lines.0))
                .then_with(|| a.source.lines.1.cmp(&b.source.lines.1))
                .then_with(|| a.source.symbol.cmp(&b.source.symbol))
                .then_with(|| a.id.cmp(&b.id)) // Stable chunk ID as final tiebreaker
        });

        // Phase 7: Enrich with git metadata (if enabled)
        if self.settings.git_metadata {
            progress.set_phase("Collecting git metadata...");
            self.enrich_with_git_metadata(all_chunks, repo_root);
        }

        progress.set_phase("Sanitizing final chunk metadata...");
        Self::sanitize_chunk_metadata(all_chunks);
    }

    fn prune_empty_chunks(&self, chunks: &mut Vec<EmbedChunk>) -> usize {
        let before = chunks.len();
        chunks.retain(|chunk| !chunk.content.trim().is_empty());
        before - chunks.len()
    }

    fn repair_live_chunk_references(&self, chunks: &mut [EmbedChunk]) {
        let live_ids: BTreeSet<String> = chunks.iter().map(|chunk| chunk.id.clone()).collect();

        for chunk in chunks.iter_mut() {
            chunk.children_ids.retain(|id| live_ids.contains(id));
            chunk.children_ids.sort();
            chunk.children_ids.dedup();

            if chunk
                .source
                .parent_chunk_id
                .as_ref()
                .is_some_and(|id| !live_ids.contains(id))
            {
                chunk.source.parent_chunk_id = None;
            }

            if chunk
                .code_chunk_id
                .as_ref()
                .is_some_and(|id| !live_ids.contains(id))
            {
                chunk.code_chunk_id = None;
            }
        }

        Self::repair_split_part_parent_ids(chunks, &live_ids);
    }

    fn repair_split_part_parent_ids(chunks: &mut [EmbedChunk], live_ids: &BTreeSet<String>) {
        type SplitGroupKey = (String, String, String, String, u32);

        #[derive(Clone)]
        struct SplitPartRef {
            part_no: u32,
            start_line: u32,
            end_line: u32,
            chunk_id: String,
        }

        fn key_for(chunk: &EmbedChunk, part: &ChunkPart) -> SplitGroupKey {
            (
                chunk.source.repo.qualified_name(),
                chunk.source.file.clone(),
                chunk.source.symbol.clone(),
                part.parent_signature.clone(),
                part.of,
            )
        }

        fn split_sequences(mut parts: Vec<SplitPartRef>) -> Vec<Vec<SplitPartRef>> {
            parts.sort_by(|left, right| {
                left.start_line
                    .cmp(&right.start_line)
                    .then_with(|| left.end_line.cmp(&right.end_line))
                    .then_with(|| left.part_no.cmp(&right.part_no))
                    .then_with(|| left.chunk_id.cmp(&right.chunk_id))
            });

            let mut sequences: Vec<Vec<SplitPartRef>> = Vec::new();
            for item in parts {
                let starts_new_sequence = sequences
                    .last()
                    .and_then(|sequence| sequence.last())
                    .is_none_or(|previous| item.part_no <= previous.part_no);

                if starts_new_sequence {
                    sequences.push(vec![item]);
                } else if let Some(sequence) = sequences.last_mut() {
                    sequence.push(item);
                }
            }
            sequences
        }

        let mut grouped_parts: BTreeMap<SplitGroupKey, Vec<SplitPartRef>> = BTreeMap::new();
        for chunk in chunks.iter() {
            if let Some(part) = chunk.part.as_ref() {
                grouped_parts
                    .entry(key_for(chunk, part))
                    .or_default()
                    .push(SplitPartRef {
                        part_no: part.part,
                        start_line: chunk.source.lines.0,
                        end_line: chunk.source.lines.1,
                        chunk_id: chunk.id.clone(),
                    });
            }
        }

        let mut repaired_parent_by_child: BTreeMap<String, String> = BTreeMap::new();
        for parts in grouped_parts.into_values() {
            for mut sequence in split_sequences(parts) {
                sequence.sort_by(|left, right| {
                    left.part_no
                        .cmp(&right.part_no)
                        .then_with(|| left.start_line.cmp(&right.start_line))
                        .then_with(|| left.end_line.cmp(&right.end_line))
                        .then_with(|| left.chunk_id.cmp(&right.chunk_id))
                });
                let Some(entry_id) = sequence
                    .iter()
                    .find(|part| part.part_no == 1 && live_ids.contains(&part.chunk_id))
                    .or_else(|| {
                        sequence
                            .iter()
                            .find(|part| live_ids.contains(&part.chunk_id))
                    })
                    .map(|part| part.chunk_id.clone())
                else {
                    continue;
                };
                for part in sequence {
                    repaired_parent_by_child.insert(part.chunk_id, entry_id.clone());
                }
            }
        }

        for chunk in chunks.iter_mut() {
            if let Some(part) = chunk.part.as_mut() {
                if let Some(entry_id) = repaired_parent_by_child.get(&chunk.id) {
                    debug_assert!(live_ids.contains(entry_id));
                    part.parent_id = entry_id.clone();
                } else {
                    part.parent_id.clear();
                }
            }
        }
    }

    fn sanitize_chunk_metadata(chunks: &mut [EmbedChunk]) {
        for chunk in chunks {
            let lang = Self::language_for_source(&chunk.source);

            chunk.context.keywords = extract_fragment_keywords_for_language(&chunk.content, lang);
            chunk.context.identifiers = extract_local_identifier_terms(&chunk.content, lang);

            Self::sanitize_source_identity(chunk.kind, &mut chunk.source);
            Self::sanitize_context_docstring(&chunk.content, &mut chunk.context);
            Self::sanitize_context_signature(&chunk.source, &mut chunk.context);
            Self::sanitize_part_metadata(chunk);
            Self::sanitize_context_relations(&mut chunk.context);
            chunk.context.summary =
                generate_fragment_summary(chunk.kind, &chunk.source, &chunk.context);
        }
    }

    fn language_for_source(source: &ChunkSource) -> Option<Language> {
        Path::new(&source.file)
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(Language::from_extension)
            .or_else(|| match source.language.to_ascii_lowercase().as_str() {
                "python" => Some(Language::Python),
                "rust" => Some(Language::Rust),
                "javascript" | "jsx" => Some(Language::JavaScript),
                "typescript" | "tsx" => Some(Language::TypeScript),
                "go" => Some(Language::Go),
                "java" => Some(Language::Java),
                "c" => Some(Language::C),
                "cpp" | "c++" => Some(Language::Cpp),
                "csharp" | "c#" => Some(Language::CSharp),
                "ruby" => Some(Language::Ruby),
                "php" => Some(Language::Php),
                "kotlin" => Some(Language::Kotlin),
                "swift" => Some(Language::Swift),
                "scala" => Some(Language::Scala),
                _ => None,
            })
    }

    fn sanitize_source_identity(kind: ChunkKind, source: &mut ChunkSource) {
        let module_path = derive_module_path(&source.file, &source.language);
        source.module_path = (!module_path.is_empty()).then_some(module_path);

        if source.symbol == "<top_level>" || !Self::kind_supports_fqn(kind) {
            source.fqn = None;
            return;
        }
        if source
            .fqn
            .as_deref()
            .is_some_and(|fqn| !Self::fqn_matches_symbol(fqn, &source.symbol))
        {
            source.fqn = None;
        }
    }

    fn kind_supports_fqn(kind: ChunkKind) -> bool {
        !matches!(kind, ChunkKind::Imports | ChunkKind::TopLevel)
    }

    fn sanitize_context_signature(source: &ChunkSource, context: &mut ChunkContext) {
        let Some(signature) = context.signature.as_deref() else {
            context.type_signature = None;
            context.parameter_types.clear();
            context.return_type = None;
            context.error_types.clear();
            return;
        };
        if let Some(signature_name) = Self::python_signature_name(signature) {
            if signature_name != source.symbol {
                context.signature = None;
                context.type_signature = None;
                context.parameter_types.clear();
                context.return_type = None;
                context.error_types.clear();
            }
        }
    }

    fn sanitize_context_docstring(content: &str, context: &mut ChunkContext) {
        if context
            .docstring
            .as_deref()
            .is_some_and(|docstring| !contains_normalized(content, docstring))
        {
            context.docstring = None;
        }
    }

    fn sanitize_part_metadata(chunk: &mut EmbedChunk) {
        let Some(part) = chunk.part.as_mut() else {
            return;
        };

        if part.of <= 1 || part.part == 0 || part.part > part.of {
            chunk.part = None;
            return;
        }

        if part.parent_signature.trim().is_empty() {
            part.parent_signature = chunk
                .context
                .signature
                .clone()
                .or_else(|| chunk.source.fqn.clone())
                .unwrap_or_else(|| {
                    format!("{}:{}:{}", chunk.kind.name(), chunk.source.file, chunk.source.symbol)
                });
        }

        if part.parent_id.is_empty() {
            part.parent_id = chunk.id.clone();
        }
    }

    fn sanitize_context_relations(context: &mut ChunkContext) {
        context.called_by.sort();
        context.called_by.dedup();
        context.dependents_count = if context.called_by.is_empty() {
            None
        } else {
            Some(context.called_by.len() as u32)
        };
    }

    fn fqn_matches_symbol(fqn: &str, symbol: &str) -> bool {
        fqn.rsplit("::").next() == Some(symbol)
    }

    fn python_signature_name(signature: &str) -> Option<&str> {
        let trimmed = signature.trim_start();
        let rest = trimmed
            .strip_prefix("async def ")
            .or_else(|| trimmed.strip_prefix("def "))
            .or_else(|| trimmed.strip_prefix("class "))?;
        let end = rest
            .char_indices()
            .find_map(|(index, ch)| {
                (!matches!(ch, '_' | 'a'..='z' | 'A'..='Z' | '0'..='9')).then_some(index)
            })
            .unwrap_or(rest.len());
        (end > 0).then_some(&rest[..end])
    }

    /// Canonicalize chunks that describe the exact same source span and content.
    ///
    /// Some Tree-sitter queries intentionally overlap, for example a Python class
    /// method may also match the generic function rule. Those duplicates should be
    /// represented by one canonical chunk.
    fn canonicalize_duplicate_chunks(&self, chunks: &mut Vec<EmbedChunk>) -> usize {
        use std::collections::BTreeMap;

        let mut groups: BTreeMap<(String, String, u32, u32, String, String), Vec<EmbedChunk>> =
            BTreeMap::new();

        for chunk in std::mem::take(chunks) {
            let content_hash = if chunk.full_hash.is_empty() {
                hash_content(&chunk.content).full_hash
            } else {
                chunk.full_hash.clone()
            };
            let key = (
                chunk.source.repo.qualified_name(),
                chunk.source.file.clone(),
                chunk.source.lines.0,
                chunk.source.lines.1,
                chunk.repr.clone(),
                content_hash,
            );
            groups.entry(key).or_default().push(chunk);
        }

        let mut alias_count = 0usize;
        for (_, mut group) in groups {
            if group.len() == 1 {
                chunks.push(group.pop().expect("single chunk group should be non-empty"));
                continue;
            }

            group.sort_by(|a, b| {
                Self::duplicate_canonical_rank(a.kind)
                    .cmp(&Self::duplicate_canonical_rank(b.kind))
                    .then_with(|| b.source.parent.is_some().cmp(&a.source.parent.is_some()))
                    .then_with(|| b.source.fqn.is_some().cmp(&a.source.fqn.is_some()))
                    .then_with(|| a.id.cmp(&b.id))
            });

            let mut canonical = group.remove(0);
            for alias in group {
                alias_count += 1;
                Self::merge_alias_chunk(&mut canonical, alias);
            }
            chunks.push(canonical);
        }

        alias_count
    }

    fn duplicate_canonical_rank(kind: ChunkKind) -> u8 {
        match kind {
            ChunkKind::Method => 0,
            ChunkKind::Function => 1,
            ChunkKind::FunctionPart => 2,
            ChunkKind::ClassPart => 3,
            ChunkKind::Class => 4,
            ChunkKind::Struct => 5,
            ChunkKind::Interface => 6,
            ChunkKind::Trait => 7,
            ChunkKind::Enum => 8,
            ChunkKind::Module => 9,
            ChunkKind::Constant => 10,
            ChunkKind::Variable => 11,
            ChunkKind::TopLevel => 12,
            ChunkKind::Imports => 13,
        }
    }

    fn merge_alias_chunk(canonical: &mut EmbedChunk, alias: EmbedChunk) {
        Self::merge_sorted_unique(&mut canonical.children_ids, alias.children_ids);
        Self::merge_sorted_unique(&mut canonical.context.calls, alias.context.calls);
        Self::merge_sorted_unique(&mut canonical.context.called_by, alias.context.called_by);
        Self::merge_sorted_unique(&mut canonical.context.imports, alias.context.imports);
        Self::merge_sorted_unique(&mut canonical.context.tags, alias.context.tags);
        Self::merge_sorted_unique(&mut canonical.context.keywords, alias.context.keywords);
        Self::merge_sorted_unique(&mut canonical.context.comments, alias.context.comments);
        Self::merge_sorted_unique(
            &mut canonical.context.qualified_calls,
            alias.context.qualified_calls,
        );
        Self::merge_sorted_unique(
            &mut canonical.context.parameter_types,
            alias.context.parameter_types,
        );
        Self::merge_sorted_unique(&mut canonical.context.error_types, alias.context.error_types);

        if canonical.full_hash.is_empty() {
            canonical.full_hash = alias.full_hash;
        }
        let same_source_symbol = alias.source.symbol == canonical.source.symbol;
        if same_source_symbol {
            let alias_parent = alias.source.parent.clone();
            let alias_fqn = alias
                .source
                .fqn
                .clone()
                .filter(|fqn| Self::fqn_matches_symbol(fqn, &canonical.source.symbol));
            if canonical.source.parent.is_none() {
                canonical.source.parent = alias_parent.clone();
                if alias_fqn.is_some() {
                    canonical.source.fqn = alias_fqn.clone();
                }
            } else if canonical.source.fqn.is_none() {
                canonical.source.fqn = alias_fqn.clone();
            } else if let (Some(parent), Some(alias_fqn)) =
                (alias_parent.as_deref(), alias_fqn.as_ref())
            {
                let canonical_has_parent = canonical
                    .source
                    .fqn
                    .as_deref()
                    .is_some_and(|fqn| fqn.split("::").any(|segment| segment == parent));
                let alias_has_parent = alias_fqn.split("::").any(|segment| segment == parent);
                if !canonical_has_parent && alias_has_parent {
                    canonical.source.fqn = Some(alias_fqn.clone());
                }
            }
        }
        if canonical.source.module_path.is_none() {
            canonical.source.module_path = alias.source.module_path;
        }
        if canonical.source.parent_chunk_id.is_none() {
            canonical.source.parent_chunk_id = alias.source.parent_chunk_id;
        }
        if canonical.code_chunk_id.is_none() {
            canonical.code_chunk_id = alias.code_chunk_id;
        }

        if same_source_symbol && canonical.context.docstring.is_none() {
            canonical.context.docstring = alias.context.docstring;
        }
        if same_source_symbol && canonical.context.signature.is_none() {
            canonical.context.signature = alias.context.signature;
        }
        if canonical.context.context_prefix.is_none() {
            canonical.context.context_prefix = alias.context.context_prefix;
        }
        if canonical.context.summary.is_none() {
            canonical.context.summary = alias.context.summary;
        }
        if canonical.context.identifiers.is_none() {
            canonical.context.identifiers = alias.context.identifiers;
        }
        if same_source_symbol && canonical.context.type_signature.is_none() {
            canonical.context.type_signature = alias.context.type_signature;
        }
        if same_source_symbol && canonical.context.return_type.is_none() {
            canonical.context.return_type = alias.context.return_type;
        }
        if canonical.context.git.is_none() {
            canonical.context.git = alias.context.git;
        }
        if canonical.context.complexity_score.is_none() {
            canonical.context.complexity_score = alias.context.complexity_score;
        }

        canonical.context.lines_of_code = canonical
            .context
            .lines_of_code
            .max(alias.context.lines_of_code);
        canonical.context.max_nesting_depth = canonical
            .context
            .max_nesting_depth
            .max(alias.context.max_nesting_depth);

        if canonical.context.called_by.is_empty() {
            canonical.context.dependents_count = canonical
                .context
                .dependents_count
                .or(alias.context.dependents_count);
        } else {
            canonical.context.dependents_count = Some(canonical.context.called_by.len() as u32);
        }
    }

    fn merge_sorted_unique(target: &mut Vec<String>, source: Vec<String>) {
        if source.is_empty() {
            return;
        }
        target.extend(source);
        target.sort();
        target.dedup();
    }

    /// Enrich chunks with git metadata (change frequency, authors, last modified).
    ///
    /// Uses a per-file cache so each file is only queried once via git commands.
    fn enrich_with_git_metadata(&self, chunks: &mut [EmbedChunk], repo_root: &Path) {
        let mut collector = match GitMetadataCollector::new(repo_root) {
            Some(c) => c,
            None => return, // Not a git repo, skip silently
        };

        for chunk in chunks.iter_mut() {
            let metadata = collector.get_metadata(&chunk.source.file);
            chunk.context.git = Some(metadata);
        }
    }
    /// Generate chunks in streaming mode and write finalized chunks as JSONL.
    ///
    /// Files are parsed in batches to bound parsing-phase intermediates. The full
    /// chunk set is then globally finalized before writing so dependency metadata
    /// matches non-streaming output.
    ///
    /// # Determinism
    ///
    /// Files are parsed in bounded batches, then global post-processing is applied
    /// before writing. This preserves complete cross-batch dependency metadata and
    /// deterministic global ordering.
    ///
    /// # Writer protocol
    ///
    /// Each chunk is serialized as a single JSON line (JSONL) via `serde_json`. The
    /// caller is responsible for writing any header/footer lines around the chunks.
    pub fn chunk_repository_streaming<W: Write>(
        &mut self,
        repo_path: &Path,
        writer: &mut W,
        progress: &dyn ProgressReporter,
    ) -> Result<StreamingStats, EmbedError> {
        let (chunks, stats) = self.chunk_repository_streaming_chunks(repo_path, progress)?;

        for chunk in &chunks {
            let chunk_json = serde_json::json!({
                "type": "chunk",
                "data": chunk,
            });
            let line = serde_json::to_string(&chunk_json).map_err(|e| EmbedError::IoError {
                path: repo_path.to_path_buf(),
                source: std::io::Error::other(e),
            })?;
            writeln!(writer, "{}", line)
                .map_err(|e| EmbedError::IoError { path: repo_path.to_path_buf(), source: e })?;
        }
        writer
            .flush()
            .map_err(|e| EmbedError::IoError { path: repo_path.to_path_buf(), source: e })?;

        Ok(stats)
    }

    /// Parse files in bounded batches and return globally finalized chunks.
    ///
    /// This keeps the parsing phase bounded by `batch_size`, but still retains
    /// the complete chunk set for dependency resolution. Complete `called_by`,
    /// graph export, hierarchy, signatures, git metadata, and deterministic
    /// ordering all require a global post-processing pass.
    pub fn chunk_repository_streaming_chunks(
        &mut self,
        repo_path: &Path,
        progress: &dyn ProgressReporter,
    ) -> Result<(Vec<EmbedChunk>, StreamingStats), EmbedError> {
        // Validate repo path
        let repo_root = self.validate_repo_path(repo_path)?;

        // Build repo identity from settings and git info if not already set.
        self.populate_repo_identity(&repo_root);

        // Phase 1: Discover files (deterministic order)
        progress.set_phase("Scanning repository...");
        let mut files = self.discover_files(&repo_root)?;
        files.sort(); // Critical for determinism
        progress.set_total(files.len());

        if files.is_empty() {
            return Err(EmbedError::NoChunksGenerated {
                include_patterns: "default".to_owned(),
                exclude_patterns: "default".to_owned(),
            });
        }

        // Check file limit
        if !self.limits.check_file_count(files.len()) {
            return Err(EmbedError::TooManyFiles {
                count: files.len(),
                max: self.limits.max_files,
            });
        }

        let batch_size = if self.settings.batch_size == 0 {
            500
        } else {
            self.settings.batch_size
        };

        let mut stats = StreamingStats { total_files: files.len(), ..Default::default() };

        // Phase 2: Process files in batches
        progress.set_phase("Parsing and chunking (streaming)...");
        let total_chunk_count = Mutex::new(0usize);
        let mut all_chunks = Vec::new();

        for batch_files in files.chunks(batch_size) {
            let processed_in_batch = AtomicUsize::new(0);

            // Process this batch in parallel (same logic as chunk_repository)
            let results: Vec<Result<Vec<EmbedChunk>, (PathBuf, EmbedError)>> = batch_files
                .par_iter()
                .map(|file| {
                    let result = self.chunk_file(file, &repo_root);

                    let done = processed_in_batch.fetch_add(1, Ordering::Relaxed) + 1;
                    let global_done = stats.files_processed + done;
                    progress.set_progress(global_done);

                    match result {
                        Ok(chunks) => {
                            // Atomically check and update chunk count to prevent race conditions
                            // Use Mutex to ensure thread-safe limit enforcement
                            let chunks_to_add = chunks.len();
                            let mut count =
                                total_chunk_count.lock().unwrap_or_else(|e| e.into_inner());
                            let new_count = *count + chunks_to_add;

                            // Check chunk limit BEFORE incrementing
                            if !self.limits.check_chunk_count(new_count) {
                                return Err((
                                    file.clone(),
                                    EmbedError::TooManyChunks {
                                        count: new_count,
                                        max: self.limits.max_total_chunks,
                                    },
                                ));
                            }

                            *count = new_count;
                            drop(count); // Release lock before returning

                            Ok(chunks)
                        },
                        Err(e) => Err((file.clone(), e)),
                    }
                })
                .collect();

            // Separate successes and failures for this batch
            let mut batch_chunks = Vec::new();

            for result in results {
                match result {
                    Ok(chunks) => {
                        stats.files_processed += 1;
                        batch_chunks.extend(chunks);
                    },
                    Err((_path, err)) => {
                        if err.is_critical() {
                            return Err(err);
                        }
                        if err.is_skippable() {
                            stats.files_skipped += 1;
                            progress.warn(&format!("Skipped: {}", err));
                        }
                    },
                }
            }
            all_chunks.extend(batch_chunks);
            stats.batches_processed += 1;
        }

        if all_chunks.is_empty() {
            return Err(EmbedError::NoChunksGenerated {
                include_patterns: "default".to_owned(),
                exclude_patterns: "default".to_owned(),
            });
        }

        self.finalize_chunks(&mut all_chunks, &repo_root, progress);
        stats.total_chunks = all_chunks.len();
        progress.set_phase("Complete");
        Ok((all_chunks, stats))
    }
    /// Populate the called_by field for all chunks by building a reverse call graph.
    ///
    /// This method first runs import-aware resolution to populate `qualified_calls`,
    /// then builds the reverse map for `called_by` from resolved internal targets only.
    fn populate_called_by(&self, chunks: &mut [EmbedChunk]) {
        use super::import_resolver::ImportResolver;

        // Phase A: Resolve calls via imports (populates qualified_calls)
        let resolver = ImportResolver::from_chunks(chunks);
        resolver.resolve_all_calls(chunks);

        // Phase B: Build reverse call map from resolved targets only.
        let qualified_reverse = resolver.build_qualified_reverse_map(chunks);
        let logical_targets = Self::logical_target_indices_by_fqn(chunks);

        // Phase C: Populate called_by using resolved target FQNs.
        for (index, chunk) in chunks.iter_mut().enumerate() {
            let Some(fqn) = chunk.source.fqn.as_deref() else {
                chunk.context.called_by.clear();
                chunk.context.dependents_count = None;
                continue;
            };

            let mut called_by_set: BTreeSet<String> = BTreeSet::new();
            let is_logical_target = logical_targets
                .get(fqn)
                .is_some_and(|indices| indices.contains(&index));

            if is_logical_target && !Self::is_non_entry_part(chunk) {
                if let Some(callers) = qualified_reverse.get(fqn) {
                    called_by_set.extend(
                        callers
                            .iter()
                            .filter(|caller| caller.as_str() != fqn)
                            .cloned(),
                    );
                }
            }

            chunk.context.called_by = called_by_set.into_iter().collect();

            // Set dependents_count from called_by length, clearing stale parent-level
            // counts when split fragments do not receive incoming calls.
            let count = chunk.context.called_by.len() as u32;
            chunk.context.dependents_count = (count > 0).then_some(count);
        }
    }

    fn logical_target_indices_by_fqn(chunks: &[EmbedChunk]) -> BTreeMap<String, BTreeSet<usize>> {
        let mut fqn_to_indices: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, chunk) in chunks.iter().enumerate() {
            if let Some(fqn) = chunk.source.fqn.as_deref() {
                fqn_to_indices
                    .entry(fqn.to_owned())
                    .or_default()
                    .push(index);
            }
        }

        fqn_to_indices
            .into_iter()
            .map(|(fqn, mut indices)| {
                indices.sort_by(|left, right| {
                    Self::logical_target_chunk_cmp(&chunks[*left], &chunks[*right])
                });
                let selected = if indices.iter().any(|index| chunks[*index].kind.is_part()) {
                    indices
                        .into_iter()
                        .filter(|index| {
                            chunks[*index].kind.is_part()
                                && chunks[*index]
                                    .part
                                    .as_ref()
                                    .is_none_or(|part| part.part == 1)
                        })
                        .collect()
                } else {
                    indices.into_iter().next().into_iter().collect()
                };
                (fqn, selected)
            })
            .collect()
    }

    fn logical_target_chunk_cmp(left: &EmbedChunk, right: &EmbedChunk) -> std::cmp::Ordering {
        left.source
            .file
            .cmp(&right.source.file)
            .then_with(|| left.source.lines.0.cmp(&right.source.lines.0))
            .then_with(|| left.source.lines.1.cmp(&right.source.lines.1))
            .then_with(|| left.source.symbol.cmp(&right.source.symbol))
            .then_with(|| left.id.cmp(&right.id))
    }

    fn is_non_entry_part(chunk: &EmbedChunk) -> bool {
        chunk.kind.is_part() && chunk.part.as_ref().is_some_and(|part| part.part > 1)
    }

    /// Link parent and children chunks by setting parent_chunk_id and children_ids
    ///
    /// For each chunk with `source.parent` set, find the corresponding container chunk
    /// (Class/Struct/Enum/Trait/Interface) and set bidirectional links:
    /// - child's `source.parent_chunk_id` = parent's chunk ID
    /// - parent's `children_ids` includes child's chunk ID
    ///
    /// Emits an aggregate warning via the progress reporter if any chunks reference
    /// a parent container that was not found (orphaned chunks).
    fn link_parent_children(&self, chunks: &mut [EmbedChunk], progress: &dyn ProgressReporter) {
        use std::collections::{BTreeMap, BTreeSet};

        // Build map: (file, symbol_name) -> chunk indexes for container types.
        // Split container parts are logical views of the same parent symbol, but
        // child links should stay fragment-local. Attach children only to the
        // parent part(s) whose line range overlaps the child.
        for chunk in chunks.iter_mut() {
            chunk.children_ids.clear();
            chunk.source.parent_chunk_id = None;
        }

        let mut container_map: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        for (i, chunk) in chunks.iter().enumerate() {
            if matches!(
                chunk.kind,
                ChunkKind::Class
                    | ChunkKind::Struct
                    | ChunkKind::Enum
                    | ChunkKind::Trait
                    | ChunkKind::Interface
                    | ChunkKind::ClassPart
            ) && !chunk.content.trim().is_empty()
            {
                container_map
                    .entry((chunk.source.file.clone(), chunk.source.symbol.clone()))
                    .or_default()
                    .push(i);
            }
        }

        // First pass: set parent_chunk_id on children, collect children per parent
        let mut parent_children: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        let mut orphaned_count: u32 = 0;
        let mut orphaned_files: BTreeSet<String> = BTreeSet::new();

        for i in 0..chunks.len() {
            if let Some(ref parent_name) = chunks[i].source.parent {
                let key = (chunks[i].source.file.clone(), parent_name.clone());
                if let Some(parent_indexes) = container_map.get(&key) {
                    let selected_parent_indexes = Self::select_fragment_level_parent_indexes(
                        &chunks[i],
                        parent_indexes,
                        chunks,
                    );
                    if selected_parent_indexes.is_empty() {
                        orphaned_count += 1;
                        orphaned_files.insert(chunks[i].source.file.clone());
                        continue;
                    }
                    let parent_id = chunks[selected_parent_indexes[0]].id.clone();
                    chunks[i].source.parent_chunk_id = Some(parent_id);

                    for parent_idx in selected_parent_indexes {
                        parent_children
                            .entry(parent_idx)
                            .or_default()
                            .push(chunks[i].id.clone());
                    }
                } else {
                    orphaned_count += 1;
                    orphaned_files.insert(chunks[i].source.file.clone());
                }
            }
        }

        // Emit aggregate warning for orphaned chunks
        if orphaned_count > 0 {
            progress.warn(&format!(
                "{} chunks have missing parent containers across {} files",
                orphaned_count,
                orphaned_files.len()
            ));
        }

        // Second pass: set children_ids on parents (sorted for determinism)
        for (parent_idx, mut child_ids) in parent_children {
            child_ids.sort();
            child_ids.dedup();
            chunks[parent_idx].children_ids = child_ids;
        }
    }

    fn select_fragment_level_parent_indexes(
        child: &EmbedChunk,
        parent_indexes: &[usize],
        chunks: &[EmbedChunk],
    ) -> Vec<usize> {
        let mut exact_containing: Vec<usize> = parent_indexes
            .iter()
            .copied()
            .filter(|&idx| {
                let parent = &chunks[idx];
                !parent.content.trim().is_empty()
                    && Self::line_range_contains(parent.source.lines, child.source.lines)
            })
            .collect();

        if !exact_containing.is_empty() {
            exact_containing
                .sort_by(|left, right| Self::parent_container_cmp(&chunks[*left], &chunks[*right]));
            return exact_containing.first().copied().into_iter().collect();
        }

        let mut overlapping_parts: Vec<usize> = parent_indexes
            .iter()
            .copied()
            .filter(|&idx| {
                let parent = &chunks[idx];
                !parent.content.trim().is_empty()
                    && parent.kind.is_part()
                    && Self::line_ranges_overlap(parent.source.lines, child.source.lines)
            })
            .collect();

        if !overlapping_parts.is_empty() {
            overlapping_parts
                .sort_by(|left, right| Self::parent_container_cmp(&chunks[*left], &chunks[*right]));
            return overlapping_parts.first().copied().into_iter().collect();
        }

        let mut fallback: Vec<usize> = parent_indexes
            .iter()
            .copied()
            .filter(|&idx| !chunks[idx].content.trim().is_empty())
            .collect();
        fallback.sort_by(|left, right| Self::parent_container_cmp(&chunks[*left], &chunks[*right]));
        fallback.first().copied().into_iter().collect()
    }

    fn line_range_contains(parent: (u32, u32), child: (u32, u32)) -> bool {
        parent.0 <= child.0 && child.1 <= parent.1
    }

    fn line_ranges_overlap(a: (u32, u32), b: (u32, u32)) -> bool {
        a.0 <= b.1 && b.0 <= a.1
    }

    fn parent_container_cmp(left: &EmbedChunk, right: &EmbedChunk) -> std::cmp::Ordering {
        let left_span = left.source.lines.1.saturating_sub(left.source.lines.0);
        let right_span = right.source.lines.1.saturating_sub(right.source.lines.0);
        left_span
            .cmp(&right_span)
            .then_with(|| left.source.lines.0.cmp(&right.source.lines.0))
            .then_with(|| left.source.lines.1.cmp(&right.source.lines.1))
            .then_with(|| left.id.cmp(&right.id))
    }

    /// Generate signature-only chunks for code chunks that have signatures
    ///
    /// For each code chunk with a `signature` in its context, creates a compact
    /// signature-only chunk with:
    /// - `repr` = "signature"
    /// - `code_chunk_id` = the original code chunk's ID
    /// - Content = just the signature string
    /// - Minimal context (signature, docstring only)
    ///
    /// This enables tiered retrieval: search signatures broadly (cheap), then
    /// fetch full code for top matches (expensive).
    fn generate_signature_chunks(&self, chunks: &[EmbedChunk]) -> Vec<EmbedChunk> {
        let token_model = self.parse_token_model(&self.settings.token_model);

        chunks
            .iter()
            .filter(|chunk| {
                // Only generate signature chunks for code chunks that have signatures
                chunk.repr == "code"
                    && chunk.code_chunk_id.is_none()
                    && chunk.part.is_none() // Skip split parts (parent already has signature)
                    && chunk.context.signature.is_some()
                    && !matches!(chunk.kind, ChunkKind::Imports | ChunkKind::TopLevel)
            })
            .filter_map(|chunk| {
                let signature = chunk.context.signature.as_ref()?;
                let hash = hash_content(signature);
                let tokens = self.tokenizer.count(signature, token_model);
                let repr = "signature".to_owned();
                let source = chunk.source.clone();
                let context = ChunkContext {
                    signature: chunk.context.signature.clone(),
                    docstring: chunk.context.docstring.clone(),
                    context_prefix: chunk.context.context_prefix.clone(),
                    ..Default::default()
                };
                let location_key = EmbedChunk::build_location_key(
                    &source,
                    chunk.kind,
                    &repr,
                    context.signature.as_deref(),
                    None,
                );
                let id = EmbedChunk::build_chunk_id(&location_key, &hash.full_hash);

                Some(EmbedChunk {
                    id,
                    full_hash: hash.full_hash,
                    content: signature.clone(),
                    tokens,
                    kind: chunk.kind,
                    source,
                    context,
                    children_ids: Vec::new(),
                    repr,
                    code_chunk_id: Some(chunk.id.clone()),
                    part: None,
                })
            })
            .collect()
    }

    /// Chunk a single file using thread-local resources
    fn chunk_file(&self, path: &Path, repo_root: &Path) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Validate file size
        let metadata = std::fs::metadata(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        if !self.limits.check_file_size(metadata.len()) {
            return Err(EmbedError::FileTooLarge {
                path: path.to_path_buf(),
                size: metadata.len(),
                max: self.limits.max_file_size,
            });
        }

        // Read file
        let mut content = std::fs::read_to_string(path)
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Check for excessively long lines (e.g., minified files)
        // This prevents memory issues from processing single-line 10MB files
        if let Some(max_line_len) = content.lines().map(|l| l.len()).max() {
            if !self.limits.check_line_length(max_line_len) {
                return Err(EmbedError::LineTooLong {
                    path: path.to_path_buf(),
                    length: max_line_len,
                    max: self.limits.max_line_length,
                });
            }
        }

        // Get relative path (safe, validated)
        let relative_path = self.safe_relative_path(path, repo_root)?;

        let mut redacted_line_ranges = LineByteRanges::new();

        // Security scanning (if enabled)
        if let Some(ref scanner) = self.security_scanner {
            let findings = scanner.scan(&content, &relative_path);
            if !findings.is_empty() {
                // Check if we should fail on secrets
                if self.settings.fail_on_secrets {
                    let files = findings
                        .iter()
                        .map(|f| format!("  {}:{} - {}", f.file, f.line, f.kind.name()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    return Err(EmbedError::SecretsDetected { count: findings.len(), files });
                }

                // Redact secrets if configured
                if self.settings.redact_secrets {
                    let redacted = scanner.redact_content(&content, &relative_path);
                    if redacted != content {
                        redacted_line_ranges = Self::changed_line_byte_ranges(&content, &redacted);
                        content = redacted;
                    }
                }
            }
        }
        let language = self.detect_language(path);
        let lang_enum = self.detect_language_enum(path);

        // Use thread-local parser (from parser module)
        let mut symbols = parse_file_symbols(&content, path);

        // Sort symbols deterministically (stable sort preserves parser order for equal elements)
        symbols.sort_by(|a, b| {
            a.start_line
                .cmp(&b.start_line)
                .then_with(|| a.end_line.cmp(&b.end_line))
                .then_with(|| a.name.cmp(&b.name))
        });

        let lines: Vec<&str> = content.lines().collect();
        let mut chunks = Vec::with_capacity(symbols.len() + 2);
        let token_model = self.parse_token_model(&self.settings.token_model);

        for symbol in &symbols {
            // Skip imports if configured
            if !self.settings.include_imports
                && matches!(symbol.kind, crate::types::SymbolKind::Import)
            {
                continue;
            }

            // Extract content with context
            let (chunk_content, start_line, end_line) =
                self.extract_symbol_content(&lines, symbol, self.settings.context_lines);

            // Count tokens
            let tokens = self.tokenizer.count(&chunk_content, token_model);

            // Handle large symbols (with depth-limited splitting)
            if self.settings.max_tokens > 0 && tokens > self.settings.max_tokens {
                let split_chunks = self.split_large_symbol(
                    &chunk_content,
                    symbol,
                    &relative_path,
                    &language,
                    path,
                    start_line,
                    0, // Initial depth
                    &symbols,
                    lang_enum,
                    token_model,
                    &redacted_line_ranges,
                )?;
                chunks.extend(split_chunks);
            } else {
                // Generate hash (single pass)
                let hash = hash_content(&chunk_content);

                // Extract context (with complexity metrics)
                let mut context =
                    self.extract_context(symbol, &chunk_content, &relative_path, path);

                // Compute fully qualified name for symbol disambiguation
                let fqn = self.compute_fqn(&relative_path, symbol);

                let chunk_kind: ChunkKind = symbol.kind.into();
                let source = ChunkSource {
                    repo: self.repo_id.clone(),
                    file: relative_path.clone(),
                    lines: (start_line, end_line),
                    symbol: symbol.name.clone(),
                    fqn: Some(fqn),
                    language: language.clone(),
                    parent: symbol.parent.clone(),
                    visibility: symbol.visibility.into(),
                    is_test: self.is_test_code(path, symbol),
                    module_path: Some(derive_module_path(&relative_path, &language)),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: Self::content_transform_for_range(
                        start_line,
                        end_line,
                        None,
                        &redacted_line_ranges,
                        &BTreeSet::new(),
                    ),
                };

                // Generate natural language summary
                context.summary = generate_summary(chunk_kind, &source, &context);
                let repr = default_repr();
                let location_key = EmbedChunk::build_location_key(
                    &source,
                    chunk_kind,
                    &repr,
                    context.signature.as_deref(),
                    None,
                );
                let id = EmbedChunk::build_chunk_id(&location_key, &hash.full_hash);

                chunks.push(EmbedChunk {
                    id,
                    full_hash: hash.full_hash,
                    content: chunk_content,
                    tokens,
                    kind: chunk_kind,
                    source,
                    context,
                    children_ids: Vec::new(),
                    repr,
                    code_chunk_id: None,
                    part: None,
                });
            }
        }

        // Handle top-level code if configured
        if self.settings.include_top_level && !symbols.is_empty() {
            let top_level_chunks = self.extract_top_level(
                &lines,
                &symbols,
                &relative_path,
                &language,
                lang_enum,
                token_model,
                &redacted_line_ranges,
            );
            chunks.extend(top_level_chunks);
        }

        Ok(chunks)
    }

    /// Extract symbol content with context lines
    fn extract_symbol_content(
        &self,
        lines: &[&str],
        symbol: &Symbol,
        context_lines: u32,
    ) -> (String, u32, u32) {
        // Convert to 0-indexed, clamped to bounds
        let start_line = symbol.start_line.saturating_sub(1) as usize;
        let end_line = (symbol.end_line as usize).min(lines.len());

        // Add context lines (clamped)
        let context_start = start_line.saturating_sub(context_lines as usize);
        let context_end = (end_line + context_lines as usize).min(lines.len());

        // Extract content
        let content = lines[context_start..context_end].join("\n");

        // Return 1-indexed line numbers
        (content, (context_start + 1) as u32, context_end as u32)
    }

    /// Split contiguous lines into token-bounded segments.
    ///
    /// The normal path uses line boundaries and optional line overlap. If a
    /// single source line exceeds the budget, it falls back to UTF-8-safe
    /// token-budget slicing for that line only.
    fn split_lines_to_budgeted_segments(
        &self,
        lines: &[&str],
        base_line: u32,
        token_model: TokenModel,
        max_tokens: u32,
        overlap_lines: usize,
    ) -> Vec<BudgetedSegment> {
        let mut segments = Vec::new();
        let mut current_start = 0usize;
        let mut emitted_any = false;

        while current_start < lines.len() {
            let previous_start = current_start;
            let mut content_start = if emitted_any && overlap_lines > 0 {
                current_start.saturating_sub(overlap_lines)
            } else {
                current_start
            };
            let mut content_end =
                self.find_max_fitting_line_end(lines, content_start, token_model, max_tokens);

            // If the requested overlap consumes the whole budget, the fitted
            // segment may not reach any new line. Drop overlap for this segment
            // so the non-overlapped cursor always makes forward progress.
            if emitted_any && content_end <= previous_start {
                content_start = previous_start;
                content_end =
                    self.find_max_fitting_line_end(lines, content_start, token_model, max_tokens);
            }

            if content_end == content_start + 1 {
                let single_line = lines[content_start];
                let line_tokens = self.tokenizer.count(single_line, token_model);
                if line_tokens > max_tokens {
                    for piece in
                        self.split_overlong_line_to_budget(single_line, token_model, max_tokens)
                    {
                        let start_byte = (piece.start_byte).min(u32::MAX as usize) as u32;
                        let end_byte = (piece.end_byte).min(u32::MAX as usize) as u32;
                        let tokens = self.tokenizer.count(&piece.content, token_model);
                        if tokens == 0 {
                            continue;
                        }
                        let line = base_line + content_start as u32;
                        segments.push(BudgetedSegment {
                            content: piece.content,
                            start_line: line,
                            end_line: line,
                            tokens,
                            overlap_lines: 0,
                            line_byte_range: Some((start_byte, end_byte)),
                        });
                        emitted_any = true;
                    }
                    current_start = content_start + 1;
                    debug_assert!(current_start > previous_start);
                    continue;
                }
            }

            let content = lines[content_start..content_end].join("\n");
            let tokens = self.tokenizer.count(&content, token_model);
            if tokens > 0 {
                let overlap = if emitted_any {
                    current_start.saturating_sub(content_start) as u32
                } else {
                    0
                };
                segments.push(BudgetedSegment {
                    content,
                    start_line: base_line + content_start as u32,
                    end_line: base_line + content_end as u32 - 1,
                    tokens,
                    overlap_lines: overlap,
                    line_byte_range: None,
                });
                emitted_any = true;
            }

            current_start = content_end;
            debug_assert!(current_start > previous_start);
        }

        segments
    }

    /// Find the largest line end index whose joined content fits `max_tokens`.
    fn find_max_fitting_line_end(
        &self,
        lines: &[&str],
        start: usize,
        token_model: TokenModel,
        max_tokens: u32,
    ) -> usize {
        let mut low = start + 1;
        let mut high = lines.len();

        while low < high {
            let mid = (low + high).div_ceil(2);
            let candidate = lines[start..mid].join("\n");
            if self.tokenizer.count(&candidate, token_model) <= max_tokens {
                low = mid;
            } else {
                high = mid - 1;
            }
        }

        low
    }

    /// Split an over-budget single line with token-budget truncation.
    fn split_overlong_line_to_budget(
        &self,
        line: &str,
        token_model: TokenModel,
        max_tokens: u32,
    ) -> Vec<LineSlice> {
        let mut pieces = Vec::new();
        let mut remaining = line;
        let mut offset = 0usize;

        while !remaining.is_empty() {
            let prefix = self
                .tokenizer
                .truncate_to_budget(remaining, token_model, max_tokens);
            let cut = if prefix.is_empty() {
                remaining
                    .char_indices()
                    .nth(1)
                    .map(|(idx, _)| idx)
                    .unwrap_or(remaining.len())
            } else {
                prefix.len()
            };

            if cut == 0 {
                break;
            }

            pieces.push(LineSlice {
                content: remaining[..cut].to_owned(),
                start_byte: offset,
                end_byte: offset + cut,
            });
            if cut >= remaining.len() {
                break;
            }
            offset += cut;
            remaining = &remaining[cut..];
        }

        pieces
    }

    /// Split a large symbol into multiple chunks at line boundaries
    ///
    /// This implements overlap between consecutive chunks for context continuity.
    /// Each chunk (except the first) includes `overlap_tokens` worth of lines from
    /// the end of the previous chunk. This helps RAG systems understand context
    /// when retrieving individual chunks.
    fn split_large_symbol(
        &self,
        content: &str,
        symbol: &Symbol,
        file: &str,
        language: &str,
        source_path: &Path,
        base_line: u32,
        depth: u32,
        all_symbols: &[Symbol],
        lang_enum: Option<Language>,
        token_model: TokenModel,
        redacted_line_ranges: &LineByteRanges,
    ) -> Result<Vec<EmbedChunk>, EmbedError> {
        // Depth limit to prevent stack overflow
        if !self.limits.check_recursion_depth(depth) {
            return Err(EmbedError::RecursionLimitExceeded {
                depth,
                max: self.limits.max_recursion_depth,
                context: format!("splitting symbol {}", symbol.name),
            });
        }

        let original_kind: ChunkKind = symbol.kind.into();
        let mut content_lines: Vec<String> = content.lines().map(ToOwned::to_owned).collect();
        let masks_container_child_bodies = self.should_mask_container_child_bodies(original_kind);
        let masked_line_numbers = if masks_container_child_bodies {
            self.mask_container_child_bodies(&mut content_lines, symbol, all_symbols, base_line)
        } else {
            BTreeSet::new()
        };
        let line_refs: Vec<&str> = content_lines.iter().map(String::as_str).collect();
        let total_lines = line_refs.len();
        let max_tokens = self.settings.max_tokens;

        if total_lines == 0 || max_tokens == 0 {
            return Ok(Vec::new());
        }

        let split_content = content_lines.join("\n");
        let total_tokens = self.tokenizer.count(&split_content, token_model) as usize;
        if total_tokens == 0 {
            return Ok(Vec::new());
        }

        // Estimate overlap lines from the configured token overlap.
        let overlap_tokens = self.settings.overlap_tokens as usize;
        let overlap_lines = if overlap_tokens > 0 {
            ((total_lines * overlap_tokens) / total_tokens).max(1)
        } else {
            0
        };

        let part_kind = match original_kind {
            ChunkKind::Class
            | ChunkKind::Struct
            | ChunkKind::Enum
            | ChunkKind::Interface
            | ChunkKind::Trait
            | ChunkKind::Module => ChunkKind::ClassPart,
            ChunkKind::Imports => ChunkKind::Imports,
            _ => ChunkKind::FunctionPart,
        };
        let original_fqn = self.compute_fqn(file, symbol);
        let segments: Vec<BudgetedSegment> = self
            .split_lines_to_budgeted_segments(
                &line_refs,
                base_line,
                token_model,
                max_tokens,
                overlap_lines,
            )
            .into_iter()
            .filter_map(|segment| self.trim_blank_segment_edges(segment, token_model))
            .collect();
        if segments.is_empty() {
            return Ok(Vec::new());
        }

        let total_parts = segments.len() as u32;
        let mut chunks = Vec::with_capacity(segments.len());

        for (index, segment) in segments.into_iter().enumerate() {
            let hash = hash_content(&segment.content);
            let part_source = ChunkSource {
                repo: self.repo_id.clone(),
                file: file.to_owned(),
                lines: (segment.start_line, segment.end_line),
                symbol: symbol.name.clone(),
                fqn: Some(original_fqn.clone()),
                language: language.to_owned(),
                parent: symbol.parent.clone(),
                visibility: symbol.visibility.into(),
                is_test: self.is_test_code(source_path, symbol),
                module_path: Some(derive_module_path(file, language)),
                parent_chunk_id: None,
                line_byte_range: segment.line_byte_range,
                content_transform: Self::content_transform_for_range(
                    segment.start_line,
                    segment.end_line,
                    segment.line_byte_range,
                    redacted_line_ranges,
                    &masked_line_numbers,
                ),
            };
            let mut part_context = self.extract_fragment_context(
                symbol,
                &segment.content,
                file,
                source_path,
                lang_enum,
                index > 0,
            );
            if lang_enum == Some(Language::Python) {
                part_context.calls = self.extract_python_calls_in_line_range(
                    &split_content,
                    base_line,
                    segment.start_line,
                    segment.end_line,
                );
            }
            let repr = default_repr();
            part_context.summary = generate_summary(part_kind, &part_source, &part_context);
            let part = ChunkPart {
                part: (index as u32) + 1,
                of: total_parts,
                parent_id: String::new(),
                parent_signature: symbol.signature.clone().unwrap_or_default(),
                overlap_lines: segment.overlap_lines,
            };
            let location_key = EmbedChunk::build_location_key(
                &part_source,
                part_kind,
                &repr,
                part_context.signature.as_deref(),
                Some(&part),
            );
            let id = EmbedChunk::build_chunk_id(&location_key, &hash.full_hash);

            chunks.push(EmbedChunk {
                id,
                full_hash: hash.full_hash,
                content: segment.content,
                tokens: segment.tokens,
                kind: part_kind,
                source: part_source,
                context: part_context,
                children_ids: Vec::new(),
                repr,
                code_chunk_id: None,
                part: Some(part),
            });
        }

        Self::assign_entry_part_parent_id(&mut chunks);

        Ok(chunks)
    }

    fn assign_entry_part_parent_id(chunks: &mut [EmbedChunk]) {
        let Some(entry_id) = chunks.first().map(|chunk| chunk.id.clone()) else {
            return;
        };
        for chunk in chunks {
            if let Some(part) = chunk.part.as_mut() {
                part.parent_id = entry_id.clone();
            }
        }
    }

    fn trim_blank_segment_edges(
        &self,
        segment: BudgetedSegment,
        token_model: TokenModel,
    ) -> Option<BudgetedSegment> {
        let lines: Vec<&str> = segment.content.split('\n').collect();
        if lines.is_empty() {
            return None;
        }

        let leading_blank = lines
            .iter()
            .take_while(|line| line.trim().is_empty())
            .count();
        let trailing_blank = lines
            .iter()
            .rev()
            .take_while(|line| line.trim().is_empty())
            .count();

        if leading_blank + trailing_blank >= lines.len() {
            return None;
        }

        let end = lines.len() - trailing_blank;
        let content = lines[leading_blank..end].join("\n");
        let tokens = self.tokenizer.count(&content, token_model);
        if tokens == 0 {
            return None;
        }

        Some(BudgetedSegment {
            content,
            start_line: segment.start_line + leading_blank as u32,
            end_line: segment.end_line.saturating_sub(trailing_blank as u32),
            tokens,
            overlap_lines: segment.overlap_lines.saturating_sub(leading_blank as u32),
            line_byte_range: segment.line_byte_range,
        })
    }

    /// Extract metadata that is true for this exact chunk fragment.
    ///
    /// Split chunks keep logical parent identity in `source` and `part`, but
    /// retrieval context must not inherit facts from the full parent symbol. In
    /// particular, calls/type/docstring/signature fields are populated only when
    /// they can be observed in the fragment content itself.
    fn extract_fragment_context(
        &self,
        symbol: &Symbol,
        content: &str,
        file_path: &str,
        _source_path: &Path,
        lang: Option<Language>,
        allow_leading_orphan_string_closer: bool,
    ) -> ChunkContext {
        let signature = symbol
            .signature
            .as_ref()
            .filter(|signature| contains_normalized(content, signature))
            .cloned();
        let docstring = symbol
            .docstring
            .as_ref()
            .filter(|docstring| contains_normalized(content, docstring))
            .cloned();
        let type_info = if signature.is_some() {
            lang.and_then(|language| type_extraction::extract_types(content, language))
        } else {
            None
        };
        let tags = if signature.is_some() || contains_normalized(content, &symbol.name) {
            generate_tags_for_symbol(&symbol.name, signature.as_deref())
        } else {
            Vec::new()
        };
        let local_call_content =
            if lang == Some(Language::Python) && allow_leading_orphan_string_closer {
                strip_leading_python_orphan_string_closer(content)
            } else {
                content
            };

        ChunkContext {
            docstring,
            comments: Vec::new(),
            signature,
            calls: self.extract_local_calls(local_call_content, lang),
            called_by: Vec::new(),
            imports: Self::fragment_imports_for_symbol(symbol, content),
            tags,
            keywords: extract_keywords(content),
            context_prefix: Some(generate_context_prefix(
                file_path,
                symbol.parent.as_deref(),
                &symbol.kind,
            )),
            summary: None,
            qualified_calls: Vec::new(),
            identifiers: extract_identifiers(content, lang),
            type_signature: type_info
                .as_ref()
                .and_then(|info| info.type_signature.clone()),
            parameter_types: type_info
                .as_ref()
                .map(|info| info.parameter_types.clone())
                .unwrap_or_default(),
            return_type: type_info.as_ref().and_then(|info| info.return_type.clone()),
            error_types: type_info.map(|info| info.error_types).unwrap_or_default(),
            lines_of_code: self.count_lines_of_code(content),
            max_nesting_depth: self.calculate_nesting_depth(content),
            git: None,
            complexity_score: lang.and_then(|l| super::complexity::compute_complexity(content, l)),
            dependents_count: None,
        }
    }

    fn extract_local_calls(&self, content: &str, lang: Option<Language>) -> Vec<String> {
        let Some(language) = lang else {
            return Vec::new();
        };
        let Some(ts_language) = language.tree_sitter_language() else {
            return Vec::new();
        };

        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&ts_language).is_err() {
            return Vec::new();
        }
        let parse_content;
        let masked_content;
        let parse_source = if language == Language::Python {
            parse_content = dedent_python_fragment(content);
            masked_content = mask_python_strings_and_comments(&parse_content);
            masked_content.as_str()
        } else {
            content
        };

        let Some(tree) = parser.parse(parse_source, None) else {
            return Vec::new();
        };

        let mut calls = std::collections::HashSet::new();
        if language == Language::Python {
            Self::collect_python_calls_without_strings(tree.root_node(), parse_source, &mut calls);
        } else {
            collect_calls_recursive(tree.root_node(), parse_source, language, &mut calls);
        }
        let mut calls: Vec<String> = calls.into_iter().collect();
        calls.sort();
        calls
    }

    fn extract_python_calls_in_line_range(
        &self,
        content: &str,
        base_line: u32,
        start_line: u32,
        end_line: u32,
    ) -> Vec<String> {
        if start_line > end_line {
            return Vec::new();
        }

        let Some(ts_language) = Language::Python.tree_sitter_language() else {
            return Vec::new();
        };

        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&ts_language).is_err() {
            return Vec::new();
        }

        let parse_content = dedent_python_fragment(content);
        let Some(tree) = parser.parse(&parse_content, None) else {
            return Vec::new();
        };

        let mut calls = std::collections::HashSet::new();
        Self::collect_python_calls_without_strings_in_line_range(
            tree.root_node(),
            &parse_content,
            base_line,
            start_line,
            end_line,
            &mut calls,
        );
        let mut calls: Vec<String> = calls.into_iter().collect();
        calls.sort();
        calls
    }

    fn collect_python_calls_without_strings(
        root: tree_sitter::Node<'_>,
        source: &str,
        calls: &mut std::collections::HashSet<String>,
    ) {
        Self::collect_python_calls_without_strings_in_line_range(
            root,
            source,
            1,
            1,
            u32::MAX,
            calls,
        );
    }

    fn collect_python_calls_without_strings_in_line_range(
        root: tree_sitter::Node<'_>,
        source: &str,
        base_line: u32,
        start_line: u32,
        end_line: u32,
        calls: &mut std::collections::HashSet<String>,
    ) {
        let ignored_spans = node_spans_by_kind(root, &["string", "comment"]);
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            match node.kind() {
                "string" | "comment" | "ERROR" => continue,
                "call" => {
                    if let Some(function) = node.child_by_field_name("function") {
                        let call_line = base_line + node.start_position().row as u32;
                        if function.kind() == "identifier"
                            && start_line <= call_line
                            && call_line <= end_line
                            && !byte_in_spans(function.start_byte(), &ignored_spans)
                            && !byte_in_spans(node.start_byte(), &ignored_spans)
                        {
                            if let Ok(name) = function.utf8_text(source.as_bytes()) {
                                if !crate::parser::extraction::is_builtin(name, Language::Python) {
                                    calls.insert(name.to_owned());
                                }
                            }
                        }
                    }
                },
                _ => {},
            }

            for index in (0..node.child_count()).rev() {
                if let Some(child) = node.child(index as u32) {
                    stack.push(child);
                }
            }
        }
    }

    /// Extract top-level code (code outside symbols)
    fn extract_top_level(
        &self,
        lines: &[&str],
        symbols: &[Symbol],
        file: &str,
        language: &str,
        lang_enum: Option<Language>,
        token_model: TokenModel,
        redacted_line_ranges: &LineByteRanges,
    ) -> Vec<EmbedChunk> {
        if lines.is_empty() || symbols.is_empty() {
            return Vec::new();
        }

        // Find contiguous gaps between merged symbol ranges.
        let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            let start = symbol.start_line.saturating_sub(1) as usize;
            let end = (symbol.end_line as usize).min(lines.len());
            if start < end {
                ranges.push((start, end));
            }
        }
        ranges.sort_unstable_by_key(|range| range.0);

        let mut spans: Vec<(usize, usize)> = Vec::new();
        let mut cursor = 0usize;
        for (start, end) in ranges {
            if cursor < start {
                spans.push((cursor, start));
            }
            cursor = cursor.max(end);
        }
        if cursor < lines.len() {
            spans.push((cursor, lines.len()));
        }

        let mut chunks = Vec::new();
        for (span_start, span_end) in spans {
            chunks.extend(self.extract_top_level_span(
                lines,
                span_start,
                span_end,
                file,
                language,
                lang_enum,
                token_model,
                redacted_line_ranges,
            ));
        }

        chunks
    }

    fn extract_top_level_span(
        &self,
        lines: &[&str],
        span_start: usize,
        span_end: usize,
        file: &str,
        language: &str,
        lang_enum: Option<Language>,
        token_model: TokenModel,
        redacted_line_ranges: &LineByteRanges,
    ) -> Vec<EmbedChunk> {
        if span_start >= span_end || span_end > lines.len() {
            return Vec::new();
        }

        let span_lines = &lines[span_start..span_end];
        let leading_blank = span_lines
            .iter()
            .take_while(|line| line.trim().is_empty())
            .count();
        let trailing_blank = span_lines
            .iter()
            .rev()
            .take_while(|line| line.trim().is_empty())
            .count();
        if leading_blank + trailing_blank >= span_lines.len() {
            return Vec::new();
        }

        let content_start = span_start + leading_blank;
        let content_end = span_end - trailing_blank;
        let content = lines[content_start..content_end].join("\n");
        if content.is_empty() {
            return Vec::new();
        }

        let tokens = self.tokenizer.count(&content, token_model);

        if tokens < self.settings.min_tokens {
            return Vec::new();
        }

        let top_source = ChunkSource {
            repo: self.repo_id.clone(),
            file: file.to_owned(),
            lines: ((content_start + 1) as u32, content_end as u32),
            symbol: "<top_level>".to_owned(),
            fqn: None,
            language: language.to_owned(),
            parent: None,
            visibility: Visibility::Public,
            is_test: self.is_test_path(Path::new(file)),
            module_path: Some(derive_module_path(file, language)),
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: Self::content_transform_for_range(
                (content_start + 1) as u32,
                content_end as u32,
                None,
                redacted_line_ranges,
                &BTreeSet::new(),
            ),
        };
        let context_prefix =
            Some(generate_context_prefix(file, None, &crate::types::SymbolKind::Module));
        let repr = default_repr();

        if tokens <= self.settings.max_tokens {
            let hash = hash_content(&content);
            let keywords = extract_keywords(&content);
            let top_identifiers = extract_identifiers(&content, lang_enum);
            let mut top_context = ChunkContext {
                keywords,
                identifiers: top_identifiers,
                context_prefix,
                ..Default::default()
            };
            top_context.summary = generate_summary(ChunkKind::TopLevel, &top_source, &top_context);
            let location_key = EmbedChunk::build_location_key(
                &top_source,
                ChunkKind::TopLevel,
                &repr,
                top_context.signature.as_deref(),
                None,
            );
            let id = EmbedChunk::build_chunk_id(&location_key, &hash.full_hash);

            return vec![EmbedChunk {
                id,
                full_hash: hash.full_hash,
                content,
                tokens,
                kind: ChunkKind::TopLevel,
                source: top_source,
                context: top_context,
                children_ids: Vec::new(),
                repr,
                code_chunk_id: None,
                part: None,
            }];
        }

        let split_lines: Vec<&str> = content.lines().collect();
        let base_line = (content_start + 1) as u32;
        let segments: Vec<BudgetedSegment> = self
            .split_lines_to_budgeted_segments(
                &split_lines,
                base_line,
                token_model,
                self.settings.max_tokens,
                0,
            )
            .into_iter()
            .filter_map(|segment| self.trim_blank_segment_edges(segment, token_model))
            .collect();
        if segments.is_empty() {
            return Vec::new();
        }

        let total_parts = segments.len() as u32;
        let mut chunks = Vec::with_capacity(segments.len());

        for (index, segment) in segments.into_iter().enumerate() {
            let hash = hash_content(&segment.content);
            let keywords = extract_keywords(&segment.content);
            let top_identifiers = extract_identifiers(&segment.content, lang_enum);
            let part = ChunkPart {
                part: (index as u32) + 1,
                of: total_parts,
                parent_id: String::new(),
                parent_signature: String::new(),
                overlap_lines: segment.overlap_lines,
            };
            let mut top_context = ChunkContext {
                keywords,
                identifiers: top_identifiers,
                context_prefix: context_prefix.clone(),
                ..Default::default()
            };
            top_context.summary = generate_summary(ChunkKind::TopLevel, &top_source, &top_context);
            let part_source = ChunkSource {
                lines: (segment.start_line, segment.end_line),
                line_byte_range: segment.line_byte_range,
                content_transform: Self::content_transform_for_range(
                    segment.start_line,
                    segment.end_line,
                    segment.line_byte_range,
                    redacted_line_ranges,
                    &BTreeSet::new(),
                ),
                ..top_source.clone()
            };
            let location_key = EmbedChunk::build_location_key(
                &part_source,
                ChunkKind::TopLevel,
                &repr,
                top_context.signature.as_deref(),
                Some(&part),
            );
            let id = EmbedChunk::build_chunk_id(&location_key, &hash.full_hash);

            chunks.push(EmbedChunk {
                id,
                full_hash: hash.full_hash,
                content: segment.content,
                tokens: segment.tokens,
                kind: ChunkKind::TopLevel,
                source: part_source,
                context: top_context,
                children_ids: Vec::new(),
                repr: repr.clone(),
                code_chunk_id: None,
                part: Some(part),
            });
        }

        Self::assign_entry_part_parent_id(&mut chunks);

        chunks
    }

    fn should_mask_container_child_bodies(&self, kind: ChunkKind) -> bool {
        matches!(
            kind,
            ChunkKind::Class
                | ChunkKind::Struct
                | ChunkKind::Enum
                | ChunkKind::Interface
                | ChunkKind::Trait
                | ChunkKind::Module
        )
    }

    fn mask_container_child_bodies(
        &self,
        lines: &mut [String],
        symbol: &Symbol,
        all_symbols: &[Symbol],
        base_line: u32,
    ) -> BTreeSet<u32> {
        let mut changed_lines = BTreeSet::new();
        if lines.is_empty() {
            return changed_lines;
        }

        let slice_start = base_line;
        let slice_end = base_line + lines.len() as u32 - 1;
        let symbol_name = symbol.name.as_str();

        for child in all_symbols.iter() {
            if child.parent.as_deref() != Some(symbol_name) {
                continue;
            }
            if child.end_line < slice_start || child.start_line > slice_end {
                continue;
            }

            let child_start = child.start_line.max(slice_start);
            let child_end = child.end_line.min(slice_end);
            let start_idx = (child_start - slice_start) as usize;
            let end_idx = (child_end - slice_start) as usize;

            for (offset, line) in lines[start_idx..=end_idx].iter_mut().enumerate() {
                if !line.is_empty() {
                    line.clear();
                    changed_lines.insert(child_start + offset as u32);
                }
            }
        }
        changed_lines
    }

    /// Extract semantic context for retrieval
    fn extract_context(
        &self,
        symbol: &Symbol,
        content: &str,
        file_path: &str,
        source_path: &Path,
    ) -> ChunkContext {
        // Detect language for type extraction and complexity scoring
        let lang = source_path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(Language::from_extension);

        // Extract type information via Tree-sitter if this is a function/method
        let (type_signature, parameter_types, return_type, error_types) = if matches!(
            symbol.kind,
            crate::types::SymbolKind::Function | crate::types::SymbolKind::Method
        ) {
            if let Some(lang) = lang {
                if let Some(type_info) = type_extraction::extract_types(content, lang) {
                    (
                        type_info.type_signature,
                        type_info.parameter_types,
                        type_info.return_type,
                        type_info.error_types,
                    )
                } else {
                    (None, Vec::new(), None, Vec::new())
                }
            } else {
                (None, Vec::new(), None, Vec::new())
            }
        } else {
            (None, Vec::new(), None, Vec::new())
        };

        ChunkContext {
            docstring: symbol.docstring.clone(),
            comments: Vec::new(), // TODO: Extract inline comments
            signature: symbol.signature.clone(),
            calls: symbol.calls.clone(),
            called_by: Vec::new(), // Populated in populate_called_by pass
            imports: Self::imports_for_symbol(symbol),
            tags: self.generate_tags(symbol),
            keywords: extract_keywords(content),
            context_prefix: Some(generate_context_prefix(
                file_path,
                symbol.parent.as_deref(),
                &symbol.kind,
            )),
            summary: None,               // Populated after source is built
            qualified_calls: Vec::new(), // Populated by ImportResolver
            identifiers: extract_identifiers(content, lang),
            type_signature,
            parameter_types,
            return_type,
            error_types,
            lines_of_code: self.count_lines_of_code(content),
            max_nesting_depth: self.calculate_nesting_depth(content),
            git: None, // Populated later by enrich_with_git_metadata if enabled
            complexity_score: lang.and_then(|l| super::complexity::compute_complexity(content, l)),
            dependents_count: None,
        }
    }

    /// Count lines of code (excluding blank lines and simple comments)
    fn count_lines_of_code(&self, content: &str) -> u32 {
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                // Skip blank lines and pure comment lines
                !trimmed.is_empty()
                    && !trimmed.starts_with("//")
                    && !trimmed.starts_with('#')
                    && !trimmed.starts_with("/*")
                    && !trimmed.starts_with('*')
            })
            .count() as u32
    }

    fn imports_for_symbol(symbol: &Symbol) -> Vec<String> {
        if matches!(symbol.kind, crate::types::SymbolKind::Import) {
            vec![symbol.name.clone()]
        } else {
            Vec::new()
        }
    }

    fn fragment_imports_for_symbol(symbol: &Symbol, content: &str) -> Vec<String> {
        if !matches!(symbol.kind, crate::types::SymbolKind::Import) {
            return Vec::new();
        }

        let fragment = content.trim();
        if fragment.is_empty() {
            Vec::new()
        } else {
            vec![fragment.to_owned()]
        }
    }

    /// Calculate maximum nesting depth based on brace/indent patterns
    ///
    /// For brace-based languages (Rust, JS, Go, etc.): counts {}, (), [] nesting
    /// For indentation-based languages (Python, Haskell): counts indent levels
    fn calculate_nesting_depth(&self, content: &str) -> u32 {
        // First try brace-based nesting
        let brace_depth = self.calculate_brace_depth(content);

        // If no braces found (or very few), calculate indentation-based depth
        // This handles Python, Haskell, and other whitespace-sensitive languages
        if brace_depth <= 1 {
            let indent_depth = self.calculate_indent_depth(content);
            // Use the larger of the two (some Python code also uses brackets)
            brace_depth.max(indent_depth)
        } else {
            brace_depth
        }
    }

    /// Calculate nesting depth based on brace pairs
    fn calculate_brace_depth(&self, content: &str) -> u32 {
        let mut max_depth = 0u32;
        let mut current_depth = 0i32;

        for ch in content.chars() {
            match ch {
                '{' | '(' | '[' => {
                    current_depth += 1;
                    max_depth = max_depth.max(current_depth as u32);
                },
                '}' | ')' | ']' => {
                    current_depth = (current_depth - 1).max(0);
                },
                _ => {},
            }
        }

        max_depth
    }

    /// Calculate nesting depth based on indentation levels
    /// Used for Python, Haskell, and other whitespace-sensitive languages
    fn calculate_indent_depth(&self, content: &str) -> u32 {
        let mut max_depth = 0u32;
        let mut base_indent: Option<usize> = None;

        for line in content.lines() {
            // Skip empty lines and comment-only lines
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("--") {
                continue;
            }

            // Count leading whitespace (spaces or tabs)
            let leading_spaces = line.len() - line.trim_start().len();

            // Set base indent from first non-empty line
            if base_indent.is_none() {
                base_indent = Some(leading_spaces);
            }

            // Calculate relative depth (assuming 4-space or 1-tab = 1 level)
            let base = base_indent.unwrap_or(0);
            if leading_spaces >= base {
                let relative_indent = leading_spaces - base;
                // Normalize: assume 4 spaces or 1 tab per level
                let depth = (relative_indent / 4).max(relative_indent / 2) as u32;
                max_depth = max_depth.max(depth + 1); // +1 because base level is 1
            }
        }

        max_depth
    }

    /// Auto-generate semantic tags for improved RAG retrieval
    ///
    /// Tags are generated based on symbol names, signatures, and common patterns.
    /// These help with semantic search and filtering in vector databases.
    fn generate_tags(&self, symbol: &Symbol) -> Vec<String> {
        generate_tags_for_symbol(&symbol.name, symbol.signature.as_deref())
    }

    /// Compute fully qualified name for a symbol
    ///
    /// Format: `file_path::parent::symbol_name`
    /// - file_path: Relative path with extension stripped and slashes replaced with ::
    /// - parent: Parent symbol name if any (e.g., class for a method)
    /// - symbol_name: The symbol's own name
    fn compute_fqn(&self, file: &str, symbol: &Symbol) -> String {
        // Convert file path to module-like format: src/auth/login.rs -> src::auth::login
        let module_path = file
            .strip_suffix(".rs")
            .or_else(|| file.strip_suffix(".py"))
            .or_else(|| file.strip_suffix(".ts"))
            .or_else(|| file.strip_suffix(".tsx"))
            .or_else(|| file.strip_suffix(".js"))
            .or_else(|| file.strip_suffix(".jsx"))
            .or_else(|| file.strip_suffix(".go"))
            .or_else(|| file.strip_suffix(".java"))
            .or_else(|| file.strip_suffix(".c"))
            .or_else(|| file.strip_suffix(".cpp"))
            .or_else(|| file.strip_suffix(".h"))
            .or_else(|| file.strip_suffix(".hpp"))
            .or_else(|| file.strip_suffix(".rb"))
            .or_else(|| file.strip_suffix(".php"))
            .or_else(|| file.strip_suffix(".cs"))
            .or_else(|| file.strip_suffix(".swift"))
            .or_else(|| file.strip_suffix(".kt"))
            .or_else(|| file.strip_suffix(".scala"))
            .unwrap_or(file)
            .replace(['\\', '/'], "::"); // Normalize path separators

        // Build the symbol portion
        let symbol_part = if let Some(ref parent) = symbol.parent {
            format!("{}::{}::{}", module_path, parent, symbol.name)
        } else {
            format!("{}::{}", module_path, symbol.name)
        };

        // Prepend repo identity: "{namespace}/{name}::{symbol_part}" or "{name}::{symbol_part}"
        let repo_prefix = self.repo_id.qualified_name();
        if repo_prefix.is_empty() {
            symbol_part
        } else {
            format!("{}::{}", repo_prefix, symbol_part)
        }
    }

    /// Detect if code is test code
    fn is_test_code(&self, path: &Path, symbol: &Symbol) -> bool {
        if self.is_test_path(path) {
            return true;
        }

        // Symbol-based detection
        let name = symbol.name.to_lowercase();
        if name.starts_with("test_") || name.ends_with("_test") || name.contains("_test_") {
            return true;
        }

        false
    }

    fn is_test_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        path_str.contains("test") || path_str.contains("spec") || path_str.contains("__tests__")
    }

    fn combine_content_transforms(left: Option<&str>, right: Option<&str>) -> Option<String> {
        let mut transforms = Vec::new();
        for value in [left, right].into_iter().flatten() {
            for transform in value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                if !transforms.contains(&transform) {
                    transforms.push(transform);
                }
            }
        }
        if transforms.is_empty() {
            None
        } else {
            Some(transforms.join(","))
        }
    }

    fn content_transform_for_range(
        start_line: u32,
        end_line: u32,
        line_byte_range: Option<(u32, u32)>,
        redacted_line_ranges: &LineByteRanges,
        masked_line_numbers: &BTreeSet<u32>,
    ) -> Option<String> {
        let redacted = Self::redacted_range_intersects(
            start_line,
            end_line,
            line_byte_range,
            redacted_line_ranges,
            masked_line_numbers,
        )
        .then_some("redacted_secrets");
        let masked = Self::line_range_intersects(start_line, end_line, masked_line_numbers)
            .then_some("masked_container_child_bodies");
        Self::combine_content_transforms(redacted, masked)
    }

    fn redacted_range_intersects(
        start_line: u32,
        end_line: u32,
        line_byte_range: Option<(u32, u32)>,
        redacted_line_ranges: &LineByteRanges,
        masked_line_numbers: &BTreeSet<u32>,
    ) -> bool {
        if start_line > end_line || redacted_line_ranges.is_empty() {
            return false;
        }

        if let Some((slice_start, slice_end)) = line_byte_range {
            if start_line != end_line
                || slice_start >= slice_end
                || masked_line_numbers.contains(&start_line)
            {
                return false;
            }
            return redacted_line_ranges.get(&start_line).is_some_and(|ranges| {
                ranges
                    .iter()
                    .any(|range| Self::ranges_overlap(*range, (slice_start, slice_end)))
            });
        }

        redacted_line_ranges
            .range(start_line..=end_line)
            .any(|(line, _)| !masked_line_numbers.contains(line))
    }

    fn line_range_intersects(start_line: u32, end_line: u32, line_numbers: &BTreeSet<u32>) -> bool {
        if start_line > end_line || line_numbers.is_empty() {
            return false;
        }
        line_numbers.range(start_line..=end_line).next().is_some()
    }

    fn changed_line_byte_ranges(before: &str, after: &str) -> LineByteRanges {
        let before_lines: Vec<&str> = before.lines().collect();
        let after_lines: Vec<&str> = after.lines().collect();
        let max_len = before_lines.len().max(after_lines.len());
        let mut ranges = LineByteRanges::new();

        for index in 0..max_len {
            if before_lines.get(index) == after_lines.get(index) {
                continue;
            }
            let Some(line_no) = u32::try_from(index + 1).ok() else {
                continue;
            };
            let before_line = before_lines.get(index).copied().unwrap_or_default();
            let after_line = after_lines.get(index).copied().unwrap_or_default();
            if let Some(range) = Self::changed_byte_range_in_after_line(before_line, after_line) {
                ranges.entry(line_no).or_default().push(range);
            }
        }

        ranges
    }

    fn changed_byte_range_in_after_line(before: &str, after: &str) -> Option<(u32, u32)> {
        if before == after {
            return None;
        }

        let before_chars: Vec<char> = before.chars().collect();
        let after_chars: Vec<char> = after.chars().collect();
        let mut prefix_chars = 0usize;
        while prefix_chars < before_chars.len()
            && prefix_chars < after_chars.len()
            && before_chars[prefix_chars] == after_chars[prefix_chars]
        {
            prefix_chars += 1;
        }

        let mut suffix_chars = 0usize;
        while suffix_chars < before_chars.len().saturating_sub(prefix_chars)
            && suffix_chars < after_chars.len().saturating_sub(prefix_chars)
            && before_chars[before_chars.len() - 1 - suffix_chars]
                == after_chars[after_chars.len() - 1 - suffix_chars]
        {
            suffix_chars += 1;
        }

        let start = Self::byte_offset_for_char_index(after, prefix_chars);
        let end_char_index = after_chars.len().saturating_sub(suffix_chars);
        let end = Self::byte_offset_for_char_index(after, end_char_index);
        if start < end {
            Some((start.min(u32::MAX as usize) as u32, end.min(u32::MAX as usize) as u32))
        } else if !after.is_empty() {
            let bounded = start.min(after.len());
            Some((bounded.min(u32::MAX as usize) as u32, bounded.min(u32::MAX as usize) as u32))
        } else {
            None
        }
    }

    fn byte_offset_for_char_index(value: &str, char_index: usize) -> usize {
        value
            .char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or(value.len())
    }

    fn ranges_overlap(left: (u32, u32), right: (u32, u32)) -> bool {
        left.0 < right.1 && right.0 < left.1
    }

    /// Validate repository path
    fn validate_repo_path(&self, path: &Path) -> Result<PathBuf, EmbedError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Ensure it's a directory
        if !canonical.is_dir() {
            return Err(EmbedError::NotADirectory { path: path.to_path_buf() });
        }

        Ok(canonical)
    }

    /// Get safe relative path, validate no traversal
    fn safe_relative_path(&self, path: &Path, repo_root: &Path) -> Result<String, EmbedError> {
        let canonical = path
            .canonicalize()
            .map_err(|e| EmbedError::IoError { path: path.to_path_buf(), source: e })?;

        // Ensure path is within repo root
        if !canonical.starts_with(repo_root) {
            return Err(EmbedError::PathTraversal {
                path: canonical,
                repo_root: repo_root.to_path_buf(),
            });
        }

        // Return relative path with forward slashes (cross-platform)
        Ok(canonical
            .strip_prefix(repo_root)
            .unwrap_or(&canonical)
            .to_string_lossy()
            .replace('\\', "/"))
    }

    /// Discover all files in repository
    fn discover_files(&self, repo_root: &Path) -> Result<Vec<PathBuf>, EmbedError> {
        use glob::Pattern;
        use ignore::WalkBuilder;

        let mut files = Vec::new();

        // Compile and validate include patterns (fail fast on invalid patterns)
        let mut include_patterns = Vec::new();
        for pattern_str in &self.settings.include_patterns {
            match Pattern::new(pattern_str) {
                Ok(pattern) => include_patterns.push(pattern),
                Err(e) => {
                    return Err(EmbedError::InvalidPattern {
                        pattern: pattern_str.clone(),
                        reason: e.to_string(),
                    });
                },
            }
        }

        // Compile and validate exclude patterns (fail fast on invalid patterns)
        let mut exclude_patterns = Vec::new();
        for pattern_str in &self.settings.exclude_patterns {
            match Pattern::new(pattern_str) {
                Ok(pattern) => exclude_patterns.push(pattern),
                Err(e) => {
                    return Err(EmbedError::InvalidPattern {
                        pattern: pattern_str.clone(),
                        reason: e.to_string(),
                    });
                },
            }
        }

        let walker = WalkBuilder::new(repo_root)
            .hidden(false) // Include hidden files
            .git_ignore(true) // Respect .gitignore
            .git_global(true)
            .git_exclude(true)
            .follow_links(false) // Security: Don't follow symlinks to prevent escaping repo
            .build();

        for entry in walker {
            let entry = entry.map_err(|e| EmbedError::IoError {
                path: repo_root.to_path_buf(),
                source: std::io::Error::other(e.to_string()),
            })?;

            let path = entry.path();

            // Only process files
            if !path.is_file() {
                continue;
            }

            // Get relative path for pattern matching
            let relative_path = path
                .strip_prefix(repo_root)
                .unwrap_or(path)
                .to_string_lossy();

            // Check include patterns (if any, file must match at least one)
            if !include_patterns.is_empty()
                && !include_patterns.iter().any(|p| p.matches(&relative_path))
            {
                continue;
            }

            // Check exclude patterns (if any match, skip file)
            if exclude_patterns.iter().any(|p| p.matches(&relative_path)) {
                continue;
            }

            // Skip test files unless include_tests is true
            if !self.settings.include_tests && self.is_test_file(path) {
                continue;
            }

            // Only process supported languages (by extension or filename)
            let has_language = Language::from_path(path).is_some();
            if !has_language {
                continue;
            }

            files.push(path.to_path_buf());
        }

        Ok(files)
    }

    /// Check if a file is a test file based on path patterns
    fn is_test_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        // Common test directory patterns (handle both Unix and Windows separators)
        if path_str.contains("/tests/")
            || path_str.contains("\\tests\\")
            || path_str.contains("/test/")
            || path_str.contains("\\test\\")
            || path_str.contains("/__tests__/")
            || path_str.contains("\\__tests__\\")
            || path_str.contains("/spec/")
            || path_str.contains("\\spec\\")
        {
            return true;
        }

        // Common test file patterns
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        filename.starts_with("test_")
            || filename.ends_with("_test.rs")
            || filename.ends_with("_test.py")
            || filename.ends_with("_test.go")
            || filename.ends_with(".test.ts")
            || filename.ends_with(".test.js")
            || filename.ends_with(".test.tsx")
            || filename.ends_with(".test.jsx")
            || filename.ends_with(".spec.ts")
            || filename.ends_with(".spec.js")
            || filename.ends_with("_spec.rb")
    }

    /// Detect language from file path (by extension or filename)
    fn detect_language(&self, path: &Path) -> String {
        Self::detect_language_enum_static(path)
            .map_or_else(|| "unknown".to_owned(), |l| l.display_name().to_owned())
    }

    /// Detect the Language enum for a file path (returns None for unsupported)
    fn detect_language_enum(&self, path: &Path) -> Option<Language> {
        Self::detect_language_enum_static(path)
    }

    /// Static helper for language detection by extension or filename
    fn detect_language_enum_static(path: &Path) -> Option<Language> {
        Language::from_path(path)
    }

    /// Parse token model string
    fn parse_token_model(&self, model: &str) -> TokenModel {
        TokenModel::from_model_name(model).unwrap_or(TokenModel::Claude)
    }
}

/// Extract top keywords from chunk content for BM25/sparse retrieval.
///
/// Splits content on non-alphanumeric boundaries, splits identifiers by
/// camelCase/snake_case, filters stopwords and short tokens, then returns
/// the top 10 by frequency.
pub(crate) fn extract_keywords(content: &str) -> Vec<String> {
    extract_keywords_for_language(content, None)
}

fn extract_keywords_for_language(content: &str, language: Option<Language>) -> Vec<String> {
    use std::collections::BTreeMap;

    const STOPWORDS: &[&str] = &[
        "the", "a", "an", "and", "or", "not", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could", "should", "may",
        "might", "shall", "can", "need", "must", "let", "var", "const", "mut", "pub", "fn", "def",
        "class", "struct", "enum", "impl", "trait", "use", "import", "from", "return", "if",
        "else", "for", "while", "loop", "match", "true", "false", "none", "null", "self", "this",
        "super", "new", "type", "static", "async", "await", "try", "catch", "throw", "throws",
        "void", "int", "str", "string", "bool", "float", "double", "char", "byte",
    ];

    let mut freq: BTreeMap<String, usize> = BTreeMap::new();

    for token in local_identifier_tokens(content, language) {
        let sub_tokens = split_identifier(&token);
        for sub in &sub_tokens {
            let lower = sub.to_lowercase();
            if lower.len() >= 3 && !STOPWORDS.contains(&lower.as_str()) {
                *freq.entry(lower).or_insert(0) += 1;
            }
        }
    }

    let mut entries: Vec<(String, usize)> = freq.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    entries.into_iter().take(10).map(|(word, _)| word).collect()
}

fn extract_fragment_keywords_for_language(
    content: &str,
    language: Option<Language>,
) -> Vec<String> {
    use std::collections::BTreeMap;

    let mut freq: BTreeMap<String, usize> = BTreeMap::new();
    let stopwords = fragment_metadata_stopwords(language);
    for token in local_identifier_tokens(content, language) {
        let lower = token.to_lowercase();
        if is_fragment_metadata_token(&lower, &stopwords, 3) {
            *freq.entry(lower).or_insert(0) += 1;
        }
    }

    let mut entries: Vec<(String, usize)> = freq.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    entries.into_iter().take(10).map(|(word, _)| word).collect()
}

fn extract_local_identifier_terms(content: &str, language: Option<Language>) -> Option<String> {
    let mut terms = BTreeSet::new();
    let stopwords = fragment_metadata_stopwords(language);
    for token in local_identifier_tokens(content, language) {
        let lower = token.to_lowercase();
        if is_fragment_metadata_token(&lower, &stopwords, 2) {
            terms.insert(lower);
        }
    }

    (!terms.is_empty()).then(|| terms.into_iter().collect::<Vec<_>>().join(" "))
}

fn local_identifier_tokens(content: &str, language: Option<Language>) -> BTreeSet<String> {
    if language == Some(Language::Python) {
        if let Some(tokens) = python_ast_identifier_tokens(content) {
            return tokens;
        }
    }

    let without_prose = strip_comment_and_string_spans(content, language);
    without_prose
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|token| token.len() >= 2 && token.chars().any(|ch| ch.is_ascii_alphabetic()))
        .filter(|token| {
            token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
        .map(str::to_owned)
        .collect()
}

fn python_ast_identifier_tokens(content: &str) -> Option<BTreeSet<String>> {
    let ts_language = Language::Python.tree_sitter_language()?;
    let parse_content = dedent_python_fragment(content);
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_language).ok()?;
    let tree = parser.parse(&parse_content, None)?;
    if tree.root_node().has_error() {
        return Some(BTreeSet::new());
    }

    let ignored_spans = node_spans_by_kind(tree.root_node(), &["string", "comment"]);
    let mut tokens = BTreeSet::new();
    collect_clean_python_identifier_tokens(
        tree.root_node(),
        parse_content.as_bytes(),
        &ignored_spans,
        &mut tokens,
    );
    Some(tokens)
}

fn collect_clean_python_identifier_tokens(
    root: tree_sitter::Node<'_>,
    source: &[u8],
    ignored_spans: &[(usize, usize)],
    tokens: &mut BTreeSet<String>,
) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error()
            || node.is_missing()
            || matches!(node.kind(), "ERROR" | "MISSING" | "string" | "comment")
        {
            continue;
        }

        if node.kind() == "identifier" && !byte_in_spans(node.start_byte(), ignored_spans) {
            if let Ok(text) = node.utf8_text(source) {
                if text.len() >= 2
                    && text.chars().any(|ch| ch.is_ascii_alphabetic())
                    && text
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                {
                    tokens.insert(text.to_owned());
                }
            }
        }

        for index in (0..node.child_count()).rev() {
            if let Some(child) = node.child(index as u32) {
                stack.push(child);
            }
        }
    }
}

fn is_fragment_metadata_token(
    token: &str,
    stopwords: &BTreeSet<&'static str>,
    min_len: usize,
) -> bool {
    token.len() >= min_len
        && token.chars().any(|ch| ch.is_ascii_alphabetic())
        && !stopwords.contains(&token)
        && !token.chars().all(|ch| ch.is_ascii_digit())
}

fn fragment_metadata_stopwords(language: Option<Language>) -> BTreeSet<&'static str> {
    let mut stopwords: BTreeSet<&'static str> = [
        "the", "a", "an", "and", "or", "not", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could", "should", "may",
        "might", "shall", "can", "need", "must", "let", "var", "const", "mut", "pub", "fn", "def",
        "class", "struct", "enum", "impl", "trait", "use", "import", "from", "return", "if",
        "else", "for", "while", "loop", "match", "true", "false", "none", "null", "self", "this",
        "super", "new", "type", "static", "async", "await", "try", "catch", "throw", "throws",
        "void", "int", "str", "string", "bool", "float", "double", "char", "byte", "list", "dict",
        "set", "tuple", "len", "print", "repr", "format",
    ]
    .into_iter()
    .collect();

    if language == Some(Language::Python) {
        stopwords.extend([
            "as", "assert", "break", "continue", "del", "elif", "except", "finally", "global",
            "in", "is", "lambda", "nonlocal", "pass", "raise", "with", "yield", "true", "false",
            "none",
        ]);
    }

    stopwords
}

fn strip_comment_and_string_spans(content: &str, language: Option<Language>) -> String {
    let Some(language) = language else {
        return content.to_owned();
    };
    if language == Language::Python {
        return mask_python_strings_and_comments(content);
    }
    let Some(ts_language) = language.tree_sitter_language() else {
        return content.to_owned();
    };

    let parse_content = if language == Language::Python {
        dedent_python_fragment(content)
    } else {
        content.to_owned()
    };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&ts_language).is_err() {
        return content.to_owned();
    }
    let Some(tree) = parser.parse(&parse_content, None) else {
        return content.to_owned();
    };
    let spans = node_spans_by_kind(
        tree.root_node(),
        &[
            "string",
            "string_literal",
            "interpreted_string_literal",
            "raw_string_literal",
            "template_string",
            "comment",
        ],
    );
    if spans.is_empty() {
        return parse_content;
    }

    let bytes = parse_content.as_bytes();
    let mut stripped = String::with_capacity(parse_content.len());
    let mut cursor = 0usize;
    for (start, end) in spans {
        if cursor < start {
            stripped.push_str(std::str::from_utf8(&bytes[cursor..start]).unwrap_or_default());
        }
        stripped.push(' ');
        cursor = cursor.max(end);
    }
    if cursor < bytes.len() {
        stripped.push_str(std::str::from_utf8(&bytes[cursor..]).unwrap_or_default());
    }
    stripped
}

/// Generate a context prefix describing where the chunk fits in the codebase.
///
/// Examples:
/// - "From src/auth.rs, function"
/// - "From src/models/user.rs, in UserService, method"
pub(crate) fn generate_context_prefix(
    file_path: &str,
    parent: Option<&str>,
    kind: &crate::types::SymbolKind,
) -> String {
    let kind_name = match kind {
        crate::types::SymbolKind::Function => "function",
        crate::types::SymbolKind::Method => "method",
        crate::types::SymbolKind::Class => "class",
        crate::types::SymbolKind::Struct => "struct",
        crate::types::SymbolKind::Enum => "enum",
        crate::types::SymbolKind::Interface => "interface",
        crate::types::SymbolKind::Trait => "trait",
        crate::types::SymbolKind::Import => "import",
        crate::types::SymbolKind::Constant => "constant",
        crate::types::SymbolKind::Variable => "variable",
        crate::types::SymbolKind::TypeAlias => "type",
        crate::types::SymbolKind::Export => "export",
        crate::types::SymbolKind::Module => "module",
        crate::types::SymbolKind::Macro => "macro",
    };

    match parent {
        Some(p) => format!("From {file_path}, in {p}, {kind_name}"),
        None => format!("From {file_path}, {kind_name}"),
    }
}

fn contains_normalized(haystack: &str, needle: &str) -> bool {
    let normalized_haystack = normalize_for_contains(haystack);
    let normalized_needle = normalize_for_contains(needle);
    !normalized_needle.is_empty() && normalized_haystack.contains(&normalized_needle)
}

fn normalize_for_contains(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Generate semantic tags from symbol name and signature.
///
/// Standalone function so it can be used from both `EmbedChunker` and `ChunkStream`.
pub(crate) fn generate_tags_for_symbol(name: &str, sig: Option<&str>) -> Vec<String> {
    let mut tags = Vec::new();
    let signature = sig.unwrap_or("");
    let name_lower = name.to_lowercase();

    if signature.contains("async") || signature.contains("await") || signature.contains("suspend") {
        tags.push("async".to_owned());
    }
    if name_lower.contains("thread")
        || name_lower.contains("mutex")
        || name_lower.contains("lock")
        || name_lower.contains("spawn")
        || name_lower.contains("parallel")
        || name_lower.contains("goroutine")
        || name_lower.contains("channel")
        || signature.contains("Mutex")
        || signature.contains("RwLock")
        || signature.contains("Arc")
        || signature.contains("chan ")
        || signature.contains("<-chan")
        || signature.contains("chan<-")
        || signature.contains("sync.")
        || signature.contains("WaitGroup")
    {
        tags.push("concurrency".to_owned());
    }
    if name_lower.contains("password")
        || name_lower.contains("token")
        || name_lower.contains("secret")
        || name_lower.contains("auth")
        || name_lower.contains("crypt")
        || name_lower.contains("hash")
        || name_lower.contains("permission")
        || signature.contains("password")
        || signature.contains("token")
        || signature.contains("secret")
    {
        tags.push("security".to_owned());
    }
    if signature.contains("Error")
        || signature.contains("Result")
        || name_lower.contains("error")
        || name_lower.contains("exception")
        || name_lower.contains("panic")
        || name_lower.contains("unwrap")
    {
        tags.push("error-handling".to_owned());
    }
    if name_lower.contains("query")
        || name_lower.contains("sql")
        || name_lower.contains("database")
        || name_lower.contains("db_")
        || name_lower.starts_with("db")
        || name_lower.contains("repository")
        || name_lower.contains("transaction")
    {
        tags.push("database".to_owned());
    }
    if name_lower.contains("http")
        || name_lower.contains("request")
        || name_lower.contains("response")
        || name_lower.contains("endpoint")
        || name_lower.contains("route")
        || name_lower.contains("handler")
        || name_lower.contains("middleware")
    {
        tags.push("http".to_owned());
    }
    if name_lower.contains("command")
        || name_lower.contains("cli")
        || name_lower.contains("arg")
        || name_lower.contains("flag")
        || name_lower.contains("option")
        || name_lower.contains("subcommand")
    {
        tags.push("cli".to_owned());
    }
    if name_lower.contains("config")
        || name_lower.contains("setting")
        || name_lower.contains("preference")
        || name_lower.contains("option")
        || name_lower.contains("env")
    {
        tags.push("config".to_owned());
    }
    if name_lower.contains("log")
        || name_lower.contains("trace")
        || name_lower.contains("debug")
        || name_lower.contains("warn")
        || name_lower.contains("info")
        || name_lower.contains("metric")
    {
        tags.push("logging".to_owned());
    }
    if name_lower.contains("cache")
        || name_lower.contains("memoize")
        || name_lower.contains("invalidate")
    {
        tags.push("cache".to_owned());
    }
    if name_lower.contains("valid")
        || name_lower.contains("check")
        || name_lower.contains("verify")
        || name_lower.contains("assert")
        || name_lower.contains("sanitize")
    {
        tags.push("validation".to_owned());
    }
    if name_lower.contains("serial")
        || name_lower.contains("deserial")
        || name_lower.contains("json")
        || name_lower.contains("xml")
        || name_lower.contains("yaml")
        || name_lower.contains("toml")
        || name_lower.contains("encode")
        || name_lower.contains("decode")
        || name_lower.contains("parse")
        || name_lower.contains("format")
    {
        tags.push("serialization".to_owned());
    }
    if name_lower.contains("file")
        || name_lower.contains("read")
        || name_lower.contains("write")
        || name_lower.contains("path")
        || name_lower.contains("dir")
        || name_lower.contains("fs")
        || name_lower.contains("io")
    {
        tags.push("io".to_owned());
    }
    if name_lower.contains("socket")
        || name_lower.contains("connect")
        || name_lower.contains("network")
        || name_lower.contains("tcp")
        || name_lower.contains("udp")
        || name_lower.contains("client")
        || name_lower.contains("server")
    {
        tags.push("network".to_owned());
    }
    if name_lower == "new"
        || name_lower == "init"
        || name_lower == "setup"
        || name_lower == "create"
        || name_lower.starts_with("new_")
        || name_lower.starts_with("init_")
        || name_lower.starts_with("create_")
        || name_lower.ends_with("_new")
    {
        tags.push("init".to_owned());
    }
    if name_lower.contains("cleanup")
        || name_lower.contains("teardown")
        || name_lower.contains("close")
        || name_lower.contains("dispose")
        || name_lower.contains("shutdown")
        || name_lower == "drop"
    {
        tags.push("cleanup".to_owned());
    }
    if name.starts_with("test_")
        || name.ends_with("_test")
        || name.contains("Test")
        || name_lower.contains("mock")
        || name_lower.contains("stub")
        || name_lower.contains("fixture")
    {
        tags.push("test".to_owned());
    }
    if signature.contains("deprecated") || signature.contains("Deprecated") {
        tags.push("deprecated".to_owned());
    }
    if signature.starts_with("pub fn")
        || signature.starts_with("pub async fn")
        || signature.starts_with("export")
    {
        tags.push("public-api".to_owned());
    }
    if name_lower.contains("model")
        || name_lower.contains("train")
        || name_lower.contains("predict")
        || name_lower.contains("inference")
        || name_lower.contains("neural")
        || name_lower.contains("embedding")
        || name_lower.contains("classifier")
        || name_lower.contains("regressor")
        || name_lower.contains("optimizer")
        || name_lower.contains("loss")
        || name_lower.contains("gradient")
        || name_lower.contains("backprop")
        || name_lower.contains("forward")
        || name_lower.contains("layer")
        || name_lower.contains("activation")
        || name_lower.contains("weight")
        || name_lower.contains("bias")
        || name_lower.contains("epoch")
        || name_lower.contains("batch")
        || signature.contains("torch")
        || signature.contains("tensorflow")
        || signature.contains("keras")
        || signature.contains("sklearn")
        || signature.contains("nn.")
        || signature.contains("nn::")
    {
        tags.push("ml".to_owned());
    }
    if name_lower.contains("dataframe")
        || name_lower.contains("dataset")
        || name_lower.contains("tensor")
        || name_lower.contains("numpy")
        || name_lower.contains("pandas")
        || name_lower.contains("array")
        || name_lower.contains("matrix")
        || name_lower.contains("vector")
        || name_lower.contains("feature")
        || name_lower.contains("preprocess")
        || name_lower.contains("normalize")
        || name_lower.contains("transform")
        || name_lower.contains("pipeline")
        || name_lower.contains("etl")
        || name_lower.contains("aggregate")
        || name_lower.contains("groupby")
        || name_lower.contains("pivot")
        || signature.contains("pd.")
        || signature.contains("np.")
        || signature.contains("DataFrame")
        || signature.contains("ndarray")
    {
        tags.push("data-science".to_owned());
    }

    tags
}

/// Generate a natural language summary for a chunk.
///
/// Priority:
/// 1. First line of docstring (if available and under ~400 chars)
/// 2. Heuristic template based on kind, visibility, symbol name, file path, and signature
/// 3. `None` for import chunks
///
/// The summary is designed for semantic search — it includes key information
/// about what the symbol is and where it lives.
pub(crate) fn generate_summary(
    kind: ChunkKind,
    source: &ChunkSource,
    context: &ChunkContext,
) -> Option<String> {
    // Imports: no summary
    if kind == ChunkKind::Imports {
        return None;
    }

    // Priority 1: Use first line of docstring if available
    if let Some(ref docstring) = context.docstring {
        let cleaned = strip_doc_markers(docstring);
        if !cleaned.is_empty() && cleaned.len() <= 400 {
            return Some(cleaned);
        }
        // If docstring is too long, extract just the first sentence/line
        if !cleaned.is_empty() {
            let first_line = extract_first_sentence(&cleaned);
            if !first_line.is_empty() {
                return Some(first_line);
            }
        }
    }

    // Priority 2: Heuristic template
    let file_module = file_path_to_module(&source.file);

    match kind {
        ChunkKind::TopLevel => {
            return Some(format!("Top-level code in {}", source.file));
        },
        ChunkKind::Imports => return None,
        _ => {},
    }

    let visibility_prefix = format_visibility(source.visibility);
    let kind_label = kind.name();
    let symbol = &source.symbol;

    match kind {
        ChunkKind::Function | ChunkKind::Method | ChunkKind::FunctionPart => {
            let sig_part = context
                .signature
                .as_deref()
                .map(|s| format!(" -- {}", truncate_signature(s, 200)))
                .unwrap_or_default();
            Some(format!(
                "{}{} '{}' in {}{}",
                visibility_prefix, kind_label, symbol, file_module, sig_part
            ))
        },
        ChunkKind::Class | ChunkKind::Struct | ChunkKind::ClassPart => {
            Some(format!("{}{} '{}' in {}", visibility_prefix, kind_label, symbol, file_module))
        },
        ChunkKind::Enum => {
            Some(format!("{}enum '{}' in {}", visibility_prefix, symbol, file_module))
        },
        ChunkKind::Interface | ChunkKind::Trait => {
            Some(format!("{}{} '{}' in {}", visibility_prefix, kind_label, symbol, file_module))
        },
        ChunkKind::Constant | ChunkKind::Variable => {
            Some(format!("{}{} '{}' in {}", visibility_prefix, kind_label, symbol, file_module))
        },
        ChunkKind::Module => {
            Some(format!("{}module '{}' in {}", visibility_prefix, symbol, file_module))
        },
        _ => None,
    }
}

fn generate_fragment_summary(
    kind: ChunkKind,
    source: &ChunkSource,
    context: &ChunkContext,
) -> Option<String> {
    if matches!(kind, ChunkKind::Imports | ChunkKind::TopLevel) {
        return None;
    }

    if let Some(ref docstring) = context.docstring {
        let cleaned = strip_doc_markers(docstring);
        if !cleaned.is_empty() && cleaned.len() <= 400 {
            return Some(cleaned);
        }
        if !cleaned.is_empty() {
            let first_line = extract_first_sentence(&cleaned);
            if !first_line.is_empty() {
                return Some(first_line);
            }
        }
    }

    let Some(signature) = context.signature.as_deref() else {
        return None;
    };

    let visibility_prefix = format_visibility(source.visibility);
    let sig_part = truncate_signature(signature, 200);
    match kind {
        ChunkKind::Function | ChunkKind::Method | ChunkKind::FunctionPart => Some(format!(
            "{}{} '{}' -- {}",
            visibility_prefix,
            kind.name(),
            source.symbol,
            sig_part
        )),
        ChunkKind::Class | ChunkKind::Struct | ChunkKind::ClassPart => Some(format!(
            "{}{} '{}' -- {}",
            visibility_prefix,
            kind.name(),
            source.symbol,
            sig_part
        )),
        _ => None,
    }
}

fn dedent_python_fragment(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let min_indent = lines
        .iter()
        .filter_map(|line| {
            if line.trim().is_empty() {
                None
            } else {
                Some(
                    line.chars()
                        .take_while(|ch| *ch == ' ' || *ch == '\t')
                        .count(),
                )
            }
        })
        .min()
        .unwrap_or(0);

    if min_indent == 0 {
        return content.to_owned();
    }

    lines
        .iter()
        .map(|line| strip_indent_chars(line, min_indent))
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_leading_python_orphan_string_closer(content: &str) -> &str {
    let trimmed = content.trim_start_matches(|ch: char| ch.is_whitespace() && ch != '\n');
    for quote in ["\"\"\"", "'''"] {
        if let Some(rest) = trimmed.strip_prefix(quote) {
            let rest = rest.trim_start_matches(|ch: char| ch == ' ' || ch == '\t');
            if let Some(after_newline) = rest.strip_prefix('\n') {
                return after_newline;
            }
            if rest.is_empty() {
                return "";
            }
        }
    }
    content
}

fn mask_python_strings_and_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut output = String::with_capacity(source.len());
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] == '#' {
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }

        if let Some(start) = python_string_start(&chars, index) {
            mask_python_string(&chars, &mut output, &mut index, start);
            continue;
        }

        output.push(chars[index]);
        index += 1;
    }

    output
}

struct PythonStringStart {
    quote_index: usize,
    quote: char,
    triple: bool,
}

fn python_string_start(chars: &[char], index: usize) -> Option<PythonStringStart> {
    if matches!(chars.get(index), Some('\'' | '"')) {
        let quote = chars[index];
        return Some(PythonStringStart {
            quote_index: index,
            quote,
            triple: python_has_triple_quote(chars, index, quote),
        });
    }

    for prefix_len in (1..=2).rev() {
        let quote_index = index + prefix_len;
        let Some(quote @ ('\'' | '"')) = chars.get(quote_index).copied() else {
            continue;
        };
        let prefix: String = chars[index..quote_index]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        if matches!(
            prefix.as_str(),
            "r" | "u" | "b" | "f" | "br" | "rb" | "fr" | "rf" | "ur" | "ru"
        ) {
            return Some(PythonStringStart {
                quote_index,
                quote,
                triple: python_has_triple_quote(chars, quote_index, quote),
            });
        }
    }

    None
}

fn python_has_triple_quote(chars: &[char], quote_index: usize, quote: char) -> bool {
    quote_index + 2 < chars.len()
        && chars[quote_index + 1] == quote
        && chars[quote_index + 2] == quote
}

fn mask_python_string(
    chars: &[char],
    output: &mut String,
    index: &mut usize,
    start: PythonStringStart,
) {
    output.push_str("None");

    *index = if start.triple {
        start.quote_index + 3
    } else {
        start.quote_index + 1
    };

    while *index < chars.len() {
        if start.triple {
            if python_has_triple_quote(chars, *index, start.quote) {
                *index += 3;
                return;
            }
            push_masked_python_literal_char(output, chars[*index]);
            *index += 1;
            continue;
        }

        if chars[*index] == '\\' {
            *index += 1;
            if *index < chars.len() {
                *index += 1;
            }
            continue;
        }
        if chars[*index] == start.quote {
            *index += 1;
            return;
        }
        if chars[*index] == '\n' {
            output.push('\n');
            *index += 1;
            return;
        }

        *index += 1;
    }
}

fn push_masked_python_literal_char(output: &mut String, ch: char) {
    if ch == '\n' || ch == '\r' {
        output.push(ch);
    }
}

fn strip_indent_chars(line: &str, count: usize) -> &str {
    if line.trim().is_empty() {
        return "";
    }

    let mut removed = 0usize;
    for (byte_index, ch) in line.char_indices() {
        if removed >= count || (ch != ' ' && ch != '\t') {
            return &line[byte_index..];
        }
        removed += 1;
    }
    ""
}

fn node_spans_by_kind(root: tree_sitter::Node<'_>, kinds: &[&str]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if kinds.contains(&node.kind()) {
            spans.push((node.start_byte(), node.end_byte()));
            continue;
        }

        for index in (0..node.child_count()).rev() {
            if let Some(child) = node.child(index as u32) {
                stack.push(child);
            }
        }
    }
    spans.sort_unstable();
    spans
}

fn byte_in_spans(byte: usize, spans: &[(usize, usize)]) -> bool {
    spans
        .iter()
        .any(|(start, end)| *start <= byte && byte < *end)
}

/// Strip common doc-comment markers from a docstring.
///
/// Handles: `///`, `//!`, `/**`, `*/`, `*`, `#`, `"""`, `'''`, leading whitespace.
fn strip_doc_markers(docstring: &str) -> String {
    let first_line = docstring.lines().next().unwrap_or("");
    let trimmed = first_line.trim();

    // Strip leading markers
    let stripped = trimmed
        .strip_prefix("///")
        .or_else(|| trimmed.strip_prefix("//!"))
        .or_else(|| trimmed.strip_prefix("/**"))
        .or_else(|| trimmed.strip_prefix("/*"))
        .or_else(|| trimmed.strip_prefix("*/"))
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix('*'))
        .or_else(|| trimmed.strip_prefix("\"\"\""))
        .or_else(|| trimmed.strip_prefix("'''"))
        .or_else(|| trimmed.strip_prefix("# "))
        .or_else(|| trimmed.strip_prefix('#'))
        .unwrap_or(trimmed);

    // Strip trailing markers
    let stripped = stripped.trim();
    let stripped = stripped
        .strip_suffix("\"\"\"")
        .or_else(|| stripped.strip_suffix("'''"))
        .or_else(|| stripped.strip_suffix("*/"))
        .unwrap_or(stripped);

    stripped.trim().to_owned()
}

/// Extract the first sentence from text.
///
/// A sentence ends at the first `.`, `!`, or `?` followed by whitespace or end-of-string,
/// or at the first newline.
fn extract_first_sentence(text: &str) -> String {
    // Take first line
    let first_line = text.lines().next().unwrap_or(text);

    // Find sentence boundary
    let mut end = first_line.len();
    for (i, ch) in first_line.char_indices() {
        if matches!(ch, '.' | '!' | '?') {
            // Check if next char is whitespace or end of string
            let next_idx = i + ch.len_utf8();
            if next_idx >= first_line.len()
                || first_line[next_idx..].starts_with(char::is_whitespace)
            {
                end = next_idx;
                break;
            }
        }
    }

    let result = first_line[..end].trim();
    if result.len() > 400 {
        truncate_with_ellipsis(result, 400)
    } else {
        result.to_owned()
    }
}

/// Truncate a UTF-8 string to a byte budget without splitting a code point.
fn truncate_with_ellipsis(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_owned();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }

    let mut end = max_len - 3;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

/// Convert a file path to a module-like notation.
///
/// Strips common prefixes (`src/`, `lib/`, `main/`), drops file extension,
/// and replaces `/` with `::`.
///
/// Example: `src/auth/jwt.rs` -> `auth::jwt`
fn file_path_to_module(file_path: &str) -> String {
    let path = file_path.replace('\\', "/");

    // Strip common prefixes
    let stripped = path
        .strip_prefix("src/")
        .or_else(|| path.strip_prefix("lib/"))
        .or_else(|| path.strip_prefix("main/"))
        .unwrap_or(&path);

    // Drop file extension
    let without_ext = stripped.rsplit_once('.').map_or(stripped, |(base, _)| base);

    // Replace / with ::
    without_ext.replace('/', "::")
}

/// Format visibility for summary output.
///
/// Returns "Public ", "Private ", etc. for known visibilities,
/// or "" for the default (Public, which is omitted for brevity).
fn format_visibility(vis: Visibility) -> &'static str {
    match vis {
        Visibility::Public => "Public ",
        Visibility::Private => "Private ",
        Visibility::Protected => "Protected ",
        Visibility::Internal => "Internal ",
    }
}

/// Truncate a signature to at most `max_len` characters.
///
/// If truncated, appends "..." to indicate continuation.
/// Also collapses to a single line.
fn truncate_signature(sig: &str, max_len: usize) -> String {
    // Collapse to single line
    let oneliner: String = sig
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if oneliner.len() <= max_len {
        oneliner
    } else {
        truncate_with_ellipsis(&oneliner, max_len)
    }
}

/// Split an identifier into sub-tokens by camelCase, PascalCase, and snake_case boundaries.
///
/// Examples:
/// - "getUserName" -> ["get", "User", "Name"]
/// - "get_user_name" -> ["get", "user", "name"]
/// - "HTTPClient" -> ["HTTP", "Client"]
pub(crate) fn split_identifier(ident: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in ident.chars() {
        if ch == '_' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else if ch.is_uppercase() && !current.is_empty() {
            let last_was_upper = current.chars().last().is_some_and(|c| c.is_uppercase());
            if !last_was_upper {
                // camelCase boundary: "getUser" -> ["get", "U..."]
                tokens.push(std::mem::take(&mut current));
            }
            current.push(ch);
        } else {
            // Transition from uppercase run to lowercase: "HTTPClient" -> ["HTTP", "Client"]
            if ch.is_lowercase() && current.len() > 1 && current.chars().all(|c| c.is_uppercase()) {
                let last = current.pop().unwrap();
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                current.push(last);
            }
            current.push(ch);
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Derive a module path from a file path and language.
///
/// Converts file paths to language-idiomatic module paths:
/// - **Rust**: `src/auth/jwt.rs` -> `auth::jwt`, `src/lib.rs` -> root, `src/auth/mod.rs` -> `auth`
/// - **Python**: `src/auth/jwt.py` -> `auth.jwt`, `__init__.py` -> parent module
/// - **TypeScript/JavaScript**: `src/auth/jwt.ts` -> `auth/jwt`, `index.ts` -> parent
/// - **Java**: `src/main/java/com/foo/Bar.java` -> `com.foo`
/// - **Go**: `internal/auth/jwt.go` -> `auth/jwt`
/// - **Default**: strip `src/`/`lib/`, replace `/` with `::`, drop extension
pub(crate) fn derive_module_path(file_path: &str, language: &str) -> String {
    let path = file_path.replace('\\', "/");
    let lang_lower = language.to_lowercase();

    match lang_lower.as_str() {
        "rust" => derive_module_path_rust(&path),
        "python" => derive_module_path_python(&path),
        "typescript" | "tsx" | "javascript" | "jsx" => derive_module_path_js(&path),
        "java" => derive_module_path_java(&path),
        "go" => derive_module_path_go(&path),
        _ => derive_module_path_default(&path),
    }
}

fn derive_module_path_rust(path: &str) -> String {
    let mut p = path.to_owned();

    // Strip src/ prefix
    if let Some(rest) = p.strip_prefix("src/") {
        p = rest.to_owned();
    }

    // Handle special files
    if p == "lib.rs" || p == "main.rs" {
        return String::new(); // root module
    }

    // Strip mod.rs -> use parent directory
    if let Some(rest) = p.strip_suffix("/mod.rs") {
        p = rest.to_owned();
    } else if let Some(rest) = p.strip_suffix(".rs") {
        p = rest.to_owned();
    }

    p.replace('/', "::")
}

fn derive_module_path_python(path: &str) -> String {
    let mut p = path.trim_start_matches('/').to_owned();

    // Strip common prefixes
    for prefix in &["src/", "lib/"] {
        if let Some(rest) = p.strip_prefix(prefix) {
            p = rest.to_owned();
            break;
        }
    }

    let mut parts: Vec<&str> = p.split('/').filter(|part| !part.is_empty()).collect();
    if let Some(index) = parts
        .iter()
        .rposition(|part| matches!(*part, "site-packages" | "dist-packages"))
    {
        parts = parts.into_iter().skip(index + 1).collect();
    } else if let Some(index) = import_root_marker_index(&parts) {
        parts = parts.into_iter().skip(index + 1).collect();
    }

    let Some(last) = parts.last_mut() else {
        return String::new();
    };
    if let Some(rest) = last.strip_suffix(".py") {
        *last = rest;
    }
    if *last == "__init__" {
        parts.pop();
    }
    let importable_start = parts
        .iter()
        .position(|part| is_python_module_segment(part))
        .unwrap_or(parts.len());
    if importable_start >= parts.len() {
        return String::new();
    }
    let module_parts: Vec<&str> = parts
        .into_iter()
        .skip(importable_start)
        .filter(|part| is_python_module_segment(part))
        .collect();
    if module_parts.is_empty() {
        return String::new();
    }
    module_parts.join(".")
}

fn import_root_marker_index(parts: &[&str]) -> Option<usize> {
    parts.iter().enumerate().find_map(|(index, part)| {
        matches!(*part, "src" | "lib")
            .then_some(index)
            .filter(|index| {
                parts
                    .iter()
                    .skip(index + 1)
                    .any(|part| is_python_module_segment(part))
            })
    })
}

fn is_python_module_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && !segment.starts_with("__pycache__")
}

fn derive_module_path_js(path: &str) -> String {
    let mut p = path.to_owned();

    // Strip common prefixes
    for prefix in &["src/", "lib/"] {
        if let Some(rest) = p.strip_prefix(prefix) {
            p = rest.to_owned();
            break;
        }
    }

    // Handle index files -> parent directory
    let index_suffixes = ["/index.ts", "/index.tsx", "/index.js", "/index.jsx"];
    for suffix in &index_suffixes {
        if let Some(rest) = p.strip_suffix(suffix) {
            return rest.to_owned();
        }
    }
    if p.starts_with("index.") {
        return String::new();
    }

    // Strip extension
    for ext in &[".ts", ".tsx", ".js", ".jsx"] {
        if let Some(rest) = p.strip_suffix(ext) {
            return rest.to_owned();
        }
    }

    p
}

fn derive_module_path_java(path: &str) -> String {
    let mut p = path.to_owned();

    // Strip src/main/java/ or src/test/java/ prefix
    for prefix in &["src/main/java/", "src/test/java/", "src/"] {
        if let Some(rest) = p.strip_prefix(prefix) {
            p = rest.to_owned();
            break;
        }
    }

    // Strip .java extension and take parent path (package)
    if let Some(rest) = p.strip_suffix(".java") {
        p = rest.to_owned();
    }

    // Get the directory part (package) — drop the class name
    if let Some(last_slash) = p.rfind('/') {
        p = p[..last_slash].to_owned();
    } else {
        return String::new(); // default package
    }

    p.replace('/', ".")
}

fn derive_module_path_go(path: &str) -> String {
    let mut p = path.to_owned();

    // Strip common Go prefixes
    for prefix in &["internal/", "pkg/", "cmd/"] {
        if let Some(rest) = p.strip_prefix(prefix) {
            p = rest.to_owned();
            break;
        }
    }

    // Strip .go extension
    if let Some(rest) = p.strip_suffix(".go") {
        p = rest.to_owned();
    }

    // In Go, module path is the directory, not the file
    if let Some(last_slash) = p.rfind('/') {
        p[..last_slash].to_owned()
    } else {
        // Single file at root level
        p
    }
}

fn derive_module_path_default(path: &str) -> String {
    let mut p = path.to_owned();

    // Strip common prefixes
    for prefix in &["src/", "lib/"] {
        if let Some(rest) = p.strip_prefix(prefix) {
            p = rest.to_owned();
            break;
        }
    }

    // Strip extension
    if let Some(dot_pos) = p.rfind('.') {
        p = p[..dot_pos].to_owned();
    }

    p.replace('/', "::")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::progress::QuietProgress;
    use tempfile::TempDir;

    fn create_test_file(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_chunker_creation() {
        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default();
        let chunker = EmbedChunker::new(settings, limits);
        assert!(chunker.settings.max_tokens > 0);
    }

    #[test]
    fn test_chunk_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let rust_code = r#"
/// A test function
fn hello() {
    println!("Hello, world!");
}

fn goodbye() {
    println!("Goodbye!");
}
"#;
        create_test_file(temp_dir.path(), "test.rs", rust_code);

        let settings = EmbedSettings::default();
        let mut chunker = EmbedChunker::with_defaults(settings);
        let progress = QuietProgress;

        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        // Should have at least 2 chunks (hello and goodbye functions)
        assert!(!chunks.is_empty());

        // Check that chunks are sorted
        for i in 1..chunks.len() {
            assert!(chunks[i - 1].source.file <= chunks[i].source.file);
        }
    }

    #[test]
    fn test_determinism() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(temp_dir.path(), "a.rs", "fn foo() {}");
        create_test_file(temp_dir.path(), "b.rs", "fn bar() {}");

        let settings = EmbedSettings::default();
        let progress = QuietProgress;

        let results: Vec<Vec<EmbedChunk>> = (0..3)
            .map(|_| {
                let mut chunker = EmbedChunker::with_defaults(settings.clone());
                chunker
                    .chunk_repository(temp_dir.path(), &progress)
                    .unwrap()
            })
            .collect();

        // All runs should produce identical results
        for i in 1..results.len() {
            assert_eq!(results[0].len(), results[i].len());
            for j in 0..results[0].len() {
                assert_eq!(results[0][j].id, results[i][j].id);
            }
        }
    }

    #[test]
    fn test_streaming_matches_non_streaming_with_cross_batch_dependencies() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "a.rs",
            r#"
use crate::b::callee;

pub fn caller() {
    callee();
}
"#,
        );
        create_test_file(
            temp_dir.path(),
            "b.rs",
            r#"
pub fn callee() {
}
"#,
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            batch_size: 1,
            include_signatures: true,
            enable_hierarchy: true,
            hierarchy_min_children: 1,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;

        let mut non_streaming = EmbedChunker::with_defaults(settings.clone());
        let expected = non_streaming
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let mut streaming = EmbedChunker::with_defaults(settings);
        let (actual, stats) = streaming
            .chunk_repository_streaming_chunks(temp_dir.path(), &progress)
            .unwrap();

        assert_eq!(actual, expected);
        assert_eq!(stats.total_chunks, actual.len());
        assert_eq!(stats.batches_processed, 2);

        let callee = actual
            .iter()
            .find(|chunk| chunk.source.symbol == "callee" && chunk.repr == "code")
            .expect("callee chunk should exist");
        assert!(
            callee
                .context
                .called_by
                .iter()
                .any(|caller| caller.contains("caller")),
            "called_by should include caller across streaming batches: {:?}",
            callee.context.called_by
        );
    }

    #[test]
    fn test_python_import_resolution_avoids_same_name_fallback() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "pkg/a.py",
            r#"
def target(value):
    return value + 1
"#,
        );
        create_test_file(
            temp_dir.path(),
            "pkg/b.py",
            r#"
def target(value):
    return value - 1
"#,
        );
        create_test_file(
            temp_dir.path(),
            "pkg/main.py",
            r#"
from pkg.a import target

def caller(value):
    return target(value)
"#,
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let import_chunk = chunks
            .iter()
            .find(|chunk| chunk.kind == ChunkKind::Imports && chunk.source.file == "pkg/main.py")
            .expect("import chunk should exist");
        assert_eq!(import_chunk.context.imports, vec!["from pkg.a import target".to_owned()]);

        let caller = chunks
            .iter()
            .find(|chunk| chunk.source.file == "pkg/main.py" && chunk.source.symbol == "caller")
            .expect("caller chunk should exist");
        assert_eq!(caller.context.qualified_calls.len(), 1);
        assert!(
            caller.context.qualified_calls[0].ends_with("::pkg::a::target"),
            "caller should resolve target to pkg/a.py: {:?}",
            caller.context.qualified_calls
        );

        let target_a = chunks
            .iter()
            .find(|chunk| chunk.source.file == "pkg/a.py" && chunk.source.symbol == "target")
            .expect("pkg/a.py target should exist");
        assert!(
            target_a
                .context
                .called_by
                .iter()
                .any(|caller| caller.ends_with("::pkg::main::caller")),
            "imported target should receive caller: {:?}",
            target_a.context.called_by
        );

        let target_b = chunks
            .iter()
            .find(|chunk| chunk.source.file == "pkg/b.py" && chunk.source.symbol == "target")
            .expect("pkg/b.py target should exist");
        assert!(
            target_b.context.called_by.is_empty(),
            "same-name unimported target must not receive caller: {:?}",
            target_b.context.called_by
        );

        let graph = crate::embedding::generate_graph_export(&chunks);
        let calls: Vec<_> = graph
            .edges
            .iter()
            .filter(|edge| edge.label == "CALLS")
            .collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].from, caller.id);
        assert_eq!(calls[0].to, target_a.id);
    }

    #[test]
    fn test_python_member_calls_do_not_create_resolved_edges() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "pkg/session.py",
            r#"
class SessionBase:
    def __init__(self):
        self._session = {}

    def get(self, key, default=None):
        return self._session.get(key, default)

    def pop(self, key, default=None):
        return self._session.pop(key, default)

    def update(self, data):
        return self._session.update(data)

class Child(SessionBase):
    def __init__(self):
        super().__init__()

def helper():
    return SessionBase()

def caller():
    return helper()
"#,
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let get_method = chunks
            .iter()
            .find(|chunk| {
                chunk.kind == ChunkKind::Method
                    && chunk.source.file == "pkg/session.py"
                    && chunk.source.parent.as_deref() == Some("SessionBase")
                    && chunk.source.symbol == "get"
            })
            .expect("SessionBase.get chunk should exist");
        assert!(
            get_method.context.calls.is_empty(),
            "member access self._session.get() should not expose a resolvable raw call: {:?}",
            get_method.context.calls
        );
        assert!(
            get_method.context.qualified_calls.is_empty(),
            "member access self._session.get() should not resolve to SessionBase.get: {:?}",
            get_method.context.qualified_calls
        );
        assert!(
            get_method.context.called_by.is_empty(),
            "SessionBase.get should not be called by itself: {:?}",
            get_method.context.called_by
        );

        let child_init = chunks
            .iter()
            .find(|chunk| {
                chunk.kind == ChunkKind::Method
                    && chunk.source.file == "pkg/session.py"
                    && chunk.source.parent.as_deref() == Some("Child")
                    && chunk.source.symbol == "__init__"
            })
            .expect("Child.__init__ chunk should exist");
        assert!(
            child_init.context.calls.is_empty(),
            "super().__init__() should not expose __init__ as a resolvable raw call: {:?}",
            child_init.context.calls
        );
        assert!(
            child_init.context.qualified_calls.is_empty(),
            "super().__init__() should not resolve to the current __init__: {:?}",
            child_init.context.qualified_calls
        );
        assert!(
            child_init.context.called_by.is_empty(),
            "Child.__init__ should not be called by itself: {:?}",
            child_init.context.called_by
        );

        let caller = chunks
            .iter()
            .find(|chunk| chunk.source.file == "pkg/session.py" && chunk.source.symbol == "caller")
            .expect("caller chunk should exist");
        let helper = chunks
            .iter()
            .find(|chunk| chunk.source.file == "pkg/session.py" && chunk.source.symbol == "helper")
            .expect("helper chunk should exist");
        assert_eq!(caller.context.qualified_calls.len(), 1);
        assert!(
            caller.context.qualified_calls[0].ends_with("::pkg::session::helper"),
            "direct same-file call should still resolve: {:?}",
            caller.context.qualified_calls
        );
        assert!(
            helper
                .context
                .called_by
                .iter()
                .any(|caller| caller.ends_with("::pkg::session::caller")),
            "helper should receive called_by from direct same-file call: {:?}",
            helper.context.called_by
        );

        let graph = crate::embedding::generate_graph_export(&chunks);
        assert!(
            graph
                .edges
                .iter()
                .all(|edge| edge.label != "CALLS" || edge.from != edge.to),
            "member-call false positives must not create self CALLS edges"
        );
        assert!(
            !graph
                .edges
                .iter()
                .any(|edge| edge.label == "CALLS" && edge.to == get_method.id),
            "self._session.get() should not produce a CALLS edge to SessionBase.get"
        );
        assert!(
            graph.edges.iter().any(|edge| edge.label == "CALLS"
                && edge.from == caller.id
                && edge.to == helper.id),
            "direct helper() call should still produce a CALLS edge"
        );
    }

    #[test]
    fn test_python_decorated_methods_keep_class_fqn_scope() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "pkg/resources.py",
            r#"
class BufferingHints:
    pass

class Factory:
    @staticmethod
    def BufferingHints():
        return BufferingHints()
"#,
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let class_chunk = chunks
            .iter()
            .find(|chunk| {
                chunk.kind == ChunkKind::Class
                    && chunk.source.file == "pkg/resources.py"
                    && chunk.source.symbol == "BufferingHints"
            })
            .expect("BufferingHints class chunk should exist");
        let method_chunk = chunks
            .iter()
            .find(|chunk| {
                chunk.kind == ChunkKind::Method
                    && chunk.source.file == "pkg/resources.py"
                    && chunk.source.symbol == "BufferingHints"
                    && chunk.source.parent.as_deref() == Some("Factory")
            })
            .expect("decorated BufferingHints method should be scoped under Factory");

        let top_level_function_collision: Vec<_> = chunks
            .iter()
            .filter(|chunk| {
                chunk.kind == ChunkKind::Function
                    && chunk.source.file == "pkg/resources.py"
                    && chunk.source.symbol == "BufferingHints"
                    && chunk.source.parent.is_none()
            })
            .collect();
        assert!(
            top_level_function_collision.is_empty(),
            "decorated class method must not remain as a top-level function alias: {top_level_function_collision:#?}"
        );
        assert_ne!(class_chunk.source.fqn, method_chunk.source.fqn);
        assert!(
            class_chunk
                .source
                .fqn
                .as_deref()
                .unwrap_or_default()
                .ends_with("::pkg::resources::BufferingHints"),
            "class FQN should stay class-scoped: {:?}",
            class_chunk.source.fqn
        );
        assert!(
            method_chunk
                .source
                .fqn
                .as_deref()
                .unwrap_or_default()
                .ends_with("::pkg::resources::Factory::BufferingHints"),
            "decorated method FQN should include class parent: {:?}",
            method_chunk.source.fqn
        );

        let graph = crate::embedding::generate_graph_export(&chunks);
        assert!(
            graph
                .edges
                .iter()
                .all(|edge| edge.label != "CALLS" || !edge.from.is_empty() || !edge.to.is_empty()),
            "graph export should complete for decorated-method chunks"
        );
    }

    #[test]
    fn test_duplicate_fqn_called_by_matches_graph_target() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "pkg/resources.py",
            r#"
def create(**kwargs): return {"type": "A", **kwargs}
def create(**kwargs): return {"type": "B", **kwargs}

def caller():
    return create(name="item")
"#,
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let create_chunks: Vec<_> = chunks
            .iter()
            .filter(|chunk| {
                chunk.source.file == "pkg/resources.py" && chunk.source.symbol == "create"
            })
            .collect();
        assert_eq!(create_chunks.len(), 2, "expected both same-name functions");

        let logical_target = create_chunks
            .iter()
            .min_by(|left, right| EmbedChunker::logical_target_chunk_cmp(left, right))
            .expect("create target should exist");
        let non_target = create_chunks
            .iter()
            .find(|chunk| chunk.id != logical_target.id)
            .expect("second create chunk should exist");
        let caller = chunks
            .iter()
            .find(|chunk| {
                chunk.source.file == "pkg/resources.py" && chunk.source.symbol == "caller"
            })
            .expect("caller chunk should exist");

        assert!(
            logical_target
                .context
                .called_by
                .iter()
                .any(|caller| caller.ends_with("::pkg::resources::caller")),
            "the selected logical target should receive called_by: {:?}",
            logical_target.context.called_by
        );
        assert!(
            non_target.context.called_by.is_empty(),
            "duplicate FQN chunks not selected as graph targets must not receive called_by: {:?}",
            non_target.context.called_by
        );

        let graph = crate::embedding::generate_graph_export(&chunks);
        let calls: Vec<_> = graph
            .edges
            .iter()
            .filter(|edge| edge.label == "CALLS" && edge.from == caller.id)
            .collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, logical_target.id);
    }

    #[test]
    fn test_split_parts_use_fragment_level_relationships() {
        let temp_dir = TempDir::new().unwrap();
        let mut big_body = String::new();
        for index in 0..80 {
            big_body.push_str(&format!(
                "    let value_{index} = callee({index});\n    helper(value_{index});\n"
            ));
        }
        create_test_file(
            temp_dir.path(),
            "lib.rs",
            &format!(
                r#"
pub fn callee(value: i32) -> i32 {{
    value + 1
}}

pub fn helper(value: i32) -> i32 {{
    value * 2
}}

pub fn caller() {{
    big();
}}

pub fn big() {{
{big_body}
}}
"#
            ),
        );

        let settings = EmbedSettings {
            max_tokens: 60,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let big_parts: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.kind == ChunkKind::FunctionPart && chunk.source.symbol == "big")
            .collect();
        assert!(big_parts.len() > 1, "expected big() to be split: {chunks:#?}");

        for part in &big_parts {
            assert_eq!(part.source.symbol, "big");
            assert!(part.tokens <= 60, "split part should respect max_tokens=60");
            assert!(part
                .source
                .fqn
                .as_deref()
                .unwrap_or_default()
                .ends_with("::big"));
            assert!(part.source.parent.is_none());
            assert!(part.part.is_some());
            for call in &part.context.calls {
                assert!(
                    part.content.contains(call),
                    "part call should be present in the fragment: call={call}, content={:?}",
                    part.content
                );
            }
            assert!(
                part.context.calls.contains(&"callee".to_owned())
                    || part.context.calls.contains(&"helper".to_owned()),
                "part should only expose calls present in its own fragment: {:?}",
                part.context.calls
            );
        }

        let entry_part = big_parts
            .iter()
            .find(|part| part.part.as_ref().is_some_and(|part| part.part == 1))
            .expect("split function should have entry part");
        assert!(
            entry_part
                .context
                .called_by
                .iter()
                .any(|caller| caller.ends_with("::caller")),
            "entry part should represent incoming calls to the split symbol: {:?}",
            entry_part.context.called_by
        );
        for part in big_parts
            .iter()
            .filter(|part| part.part.as_ref().is_some_and(|part| part.part > 1))
        {
            assert!(
                part.context.called_by.is_empty(),
                "non-entry parts should not inherit full-symbol called_by: {:?}",
                part.context.called_by
            );
            assert_eq!(part.context.dependents_count, None);
        }

        let callee = chunks
            .iter()
            .find(|chunk| chunk.kind == ChunkKind::Function && chunk.source.symbol == "callee")
            .expect("callee chunk should exist");
        let caller = chunks
            .iter()
            .find(|chunk| chunk.kind == ChunkKind::Function && chunk.source.symbol == "caller")
            .expect("caller chunk should exist");

        let graph = crate::embedding::generate_graph_export(&chunks);
        for part in &big_parts {
            if part.context.calls.contains(&"callee".to_owned()) {
                assert!(
                    graph.edges.iter().any(|edge| {
                        edge.label == "CALLS" && edge.from == part.id && edge.to == callee.id
                    }),
                    "part {} should have outgoing CALLS edge to callee",
                    part.id
                );
            }
        }
        for part in &big_parts {
            let has_incoming = graph
                .edges
                .iter()
                .any(|edge| edge.label == "CALLS" && edge.from == caller.id && edge.to == part.id);
            if part.part.as_ref().is_some_and(|part| part.part == 1) {
                assert!(has_incoming, "entry part should receive incoming CALLS edge");
            } else {
                assert!(!has_incoming, "non-entry part should not receive inherited CALLS edge");
            }
        }
    }

    #[test]
    fn test_split_part_metadata_does_not_inherit_future_calls() {
        let temp_dir = TempDir::new().unwrap();
        let mut body = String::new();
        for index in 0..80 {
            body.push_str(&format!("    later_call({index})\n"));
        }
        create_test_file(
            temp_dir.path(),
            "service.py",
            &format!(
                r#"
def later_call(value):
    return value

def big():
    """
    Explain the function without calling anything.
    """
{body}
"#
            ),
        );

        let settings = EmbedSettings {
            max_tokens: 20,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let first_part = chunks
            .iter()
            .find(|chunk| {
                chunk.kind == ChunkKind::FunctionPart
                    && chunk.source.symbol == "big"
                    && chunk.part.as_ref().is_some_and(|part| part.part == 1)
            })
            .expect("big() should split and expose first part");

        assert!(
            first_part.content.contains("Explain the function"),
            "first part should contain the docstring fragment: {:?}",
            first_part.content
        );
        assert!(
            !first_part.content.contains("later_call("),
            "first part should not contain future body calls: {:?}",
            first_part.content
        );
        assert!(
            !first_part.context.calls.contains(&"later_call".to_owned()),
            "first part metadata must not inherit calls from later fragments: {:?}",
            first_part.context.calls
        );
        assert_eq!(
            first_part.context.signature.as_deref(),
            Some("def big():"),
            "signature is kept only because it appears in this fragment"
        );

        let later_part = chunks
            .iter()
            .find(|chunk| {
                chunk.kind == ChunkKind::FunctionPart
                    && chunk.source.symbol == "big"
                    && chunk.content.contains("later_call(")
            })
            .expect("a later part should contain later_call");
        assert!(
            later_part.context.calls.contains(&"later_call".to_owned()),
            "later part should expose local calls: {:?}",
            later_part.context.calls
        );
        assert_eq!(
            later_part.context.signature, None,
            "later part should not inherit the parent signature"
        );
    }

    #[test]
    fn test_python_split_part_calls_ignore_docstring_continuation() {
        let temp_dir = TempDir::new().unwrap();
        let mut body = String::new();
        for index in 0..40 {
            body.push_str(&format!("    record_metric({index})\n"));
        }
        create_test_file(
            temp_dir.path(),
            "service.py",
            &format!(
                r#"
def record_metric(value):
    return value

def test_layer_global_max_pooling1d(system_dict):
    """
    Args:
        system_dict: Dictionary containing test state (counts, logs, etc.).

    Returns:
        Updated system_dict with results of this test.
    """
    test_name = "test_layer_global_max_pooling1d"
{body}
"#
            ),
        );

        let settings = EmbedSettings {
            max_tokens: 24,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let docstring_part = chunks
            .iter()
            .find(|chunk| {
                chunk.kind == ChunkKind::FunctionPart
                    && chunk.source.symbol == "test_layer_global_max_pooling1d"
                    && chunk.content.contains("Dictionary containing test state")
            })
            .expect("a split part should contain the docstring continuation");
        assert!(
            docstring_part.context.calls.is_empty(),
            "docstring continuation must not produce calls: {:?}",
            docstring_part.context.calls
        );

        let body_part = chunks
            .iter()
            .find(|chunk| {
                chunk.kind == ChunkKind::FunctionPart
                    && chunk.source.symbol == "test_layer_global_max_pooling1d"
                    && chunk.content.contains("record_metric(")
            })
            .expect("a later part should contain a real call");
        assert!(
            body_part
                .context
                .calls
                .contains(&"record_metric".to_owned()),
            "real calls in the split part should remain: {:?}",
            body_part.context.calls
        );
    }

    #[test]
    fn test_split_part_does_not_keep_partial_signature_from_recursive_call() {
        let temp_dir = TempDir::new().unwrap();
        let mut body = String::new();
        for index in 0..60 {
            body.push_str(&format!("    return find_paths(node_{index})\n"));
        }
        create_test_file(
            temp_dir.path(),
            "argtree.py",
            &format!(
                r#"
def find_paths(
    node,
    prefix=None,
):
{body}
"#
            ),
        );

        let settings = EmbedSettings {
            max_tokens: 24,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let recursive_tail = chunks
            .iter()
            .find(|chunk| {
                chunk.kind == ChunkKind::FunctionPart
                    && chunk.source.symbol == "find_paths"
                    && chunk.content.contains("find_paths(")
                    && !chunk.content.contains("def find_paths(")
            })
            .expect("a split tail should contain only the recursive call");

        assert_eq!(
            recursive_tail.context.signature, None,
            "split tail must not keep a partial parent signature from a call expression"
        );
    }

    #[test]
    fn test_top_level_chunks_in_test_named_file_are_marked_test() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "pkg/DistributedTestObject.py",
            "SETUP_VALUE = build_setup()\n\n\ndef keep():\n    return 1\n",
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            include_top_level: true,
            include_tests: true,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let top_level = chunks
            .iter()
            .find(|chunk| chunk.kind == ChunkKind::TopLevel)
            .expect("test-named file should emit a top-level chunk");
        assert!(top_level.source.is_test);
    }

    #[test]
    fn test_overlong_line_segments_include_line_byte_range() {
        let repeated = (0..80)
            .map(|index| format!("item_{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let source_line = format!("VALUES = [{repeated}]");
        let lines = vec![source_line.as_str(), "def keep():", "    return VALUES"];
        let mut symbol = Symbol::new("keep", crate::types::SymbolKind::Function);
        symbol.start_line = 2;
        symbol.end_line = 3;
        let symbols = vec![symbol];

        let settings = EmbedSettings {
            max_tokens: 20,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: true,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let chunks = chunker.extract_top_level(
            &lines,
            &symbols,
            "src/constants.py",
            "Python",
            Some(Language::Python),
            token_model,
            &LineByteRanges::new(),
        );

        let slices: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.source.line_byte_range.is_some())
            .collect();
        assert!(slices.len() > 1, "overlong line should split into byte-addressed slices");
        let mut previous_end = 0;
        for chunk in slices {
            let (start, end) = chunk.source.line_byte_range.expect("byte range");
            assert_eq!(chunk.source.lines, (1, 1));
            assert!(start >= previous_end);
            assert!(end > start);
            assert_eq!(
                chunk.content,
                source_line[start as usize..end as usize],
                "slice content must reconstruct from line_byte_range"
            );
            previous_end = end;
        }
    }

    #[test]
    fn test_overlong_line_redacted_transform_is_byte_slice_level() {
        let safe_prefix = (0..60)
            .map(|index| format!("safe_prefix_{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let safe_suffix = (0..60)
            .map(|index| format!("safe_suffix_{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let before_line =
            format!("VALUES = [{safe_prefix}, 'AKIAIOSFODNN7REALKEY1', {safe_suffix}]");
        let after_line = before_line.replace("AKIAIOSFODNN7REALKEY1", "AKIA****************KEY1");
        let redacted_line_ranges =
            EmbedChunker::changed_line_byte_ranges(&before_line, &after_line);
        let lines = vec![after_line.as_str(), "def keep():", "    return VALUES"];
        let mut symbol = Symbol::new("keep", crate::types::SymbolKind::Function);
        symbol.start_line = 2;
        symbol.end_line = 3;
        let symbols = vec![symbol];

        let settings = EmbedSettings {
            max_tokens: 20,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: true,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let chunks = chunker.extract_top_level(
            &lines,
            &symbols,
            "src/constants.py",
            "Python",
            Some(Language::Python),
            token_model,
            &redacted_line_ranges,
        );

        let line_slices: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.source.lines == (1, 1))
            .collect();
        assert!(line_slices.len() > 1, "overlong redacted line should be split");

        let redacted_slices: Vec<_> = line_slices
            .iter()
            .filter(|chunk| chunk.source.content_transform.as_deref() == Some("redacted_secrets"))
            .collect();
        assert!(
            !redacted_slices.is_empty(),
            "at least one slice should intersect the redacted byte span"
        );
        assert!(redacted_slices
            .iter()
            .all(|chunk| chunk.content.contains("****")));
        assert!(line_slices.iter().any(|chunk| {
            chunk.source.content_transform.is_none() && !chunk.content.contains("****")
        }));
    }

    #[test]
    fn test_redacted_secret_chunks_mark_content_transform() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "config.py",
            "def load_key():\n    return 'AKIAIOSFODNN7REALKEY1'\n",
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: true,
            fail_on_secrets: false,
            redact_secrets: true,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let load_key = chunks
            .iter()
            .find(|chunk| chunk.source.symbol == "load_key")
            .expect("redacted function chunk should exist");
        assert!(!load_key.content.contains("AKIAIOSFODNN7REALKEY1"));
        assert_eq!(load_key.source.content_transform.as_deref(), Some("redacted_secrets"));
    }

    #[test]
    fn test_content_transform_is_current_chunk_level() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "config.py",
            "def load_key():\n    return 'AKIAIOSFODNN7REALKEY1'\n\n\ndef clean_value():\n    return 'public'\n",
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: true,
            fail_on_secrets: false,
            redact_secrets: true,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let load_key = chunks
            .iter()
            .find(|chunk| chunk.source.symbol == "load_key")
            .expect("redacted function chunk should exist");
        let clean_value = chunks
            .iter()
            .find(|chunk| chunk.source.symbol == "clean_value")
            .expect("clean function chunk should exist");

        assert_eq!(load_key.source.content_transform.as_deref(), Some("redacted_secrets"));
        assert_eq!(clean_value.source.content_transform, None);
    }

    #[test]
    fn test_duplicate_canonicalization_keeps_parent_fqn_consistent() {
        let content = "def randomize(self):\n    return helper()".to_owned();
        let full_hash = hash_content(&content).full_hash;
        let mut chunks = vec![
            EmbedChunk {
                id: "function_alias".to_owned(),
                full_hash: full_hash.clone(),
                content: content.clone(),
                tokens: 8,
                kind: ChunkKind::Function,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "effects/LavaBurst.py".to_owned(),
                    lines: (10, 11),
                    symbol: "randomize".to_owned(),
                    fqn: Some("effects::LavaBurst::randomize".to_owned()),
                    language: "Python".to_owned(),
                    parent: None,
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("effects.LavaBurst".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: None,
            },
            EmbedChunk {
                id: "method_alias".to_owned(),
                full_hash,
                content,
                tokens: 8,
                kind: ChunkKind::Function,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "effects/LavaBurst.py".to_owned(),
                    lines: (10, 11),
                    symbol: "randomize".to_owned(),
                    fqn: Some("effects::LavaBurst::LavaBurst::randomize".to_owned()),
                    language: "Python".to_owned(),
                    parent: Some("LavaBurst".to_owned()),
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("effects.LavaBurst".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: None,
            },
        ];
        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let chunker = EmbedChunker::with_defaults(settings);
        let aliases = chunker.canonicalize_duplicate_chunks(&mut chunks);

        assert_eq!(aliases, 1);
        let chunk = &chunks[0];
        assert_eq!(chunk.source.parent.as_deref(), Some("LavaBurst"));
        assert!(
            chunk
                .source
                .fqn
                .as_deref()
                .is_some_and(|fqn| fqn.ends_with("::LavaBurst::randomize")),
            "canonical fqn should preserve the parent scope: {:?}",
            chunk.source.fqn
        );
    }

    #[test]
    fn test_sanitize_chunk_metadata_drops_mismatched_fqn_and_nested_signature() {
        let mut chunks = vec![EmbedChunk {
            id: "outer_part".to_owned(),
            full_hash: "outer_hash".to_owned(),
            content: "    def inner():\n        return 1".to_owned(),
            tokens: 8,
            kind: ChunkKind::FunctionPart,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: "service.py".to_owned(),
                lines: (10, 11),
                symbol: "outer".to_owned(),
                fqn: Some("service::inner".to_owned()),
                language: "Python".to_owned(),
                parent: None,
                visibility: Visibility::Public,
                is_test: false,
                module_path: Some("service".to_owned()),
                parent_chunk_id: None,
                line_byte_range: None,
                content_transform: None,
            },
            context: ChunkContext {
                signature: Some("def inner():".to_owned()),
                type_signature: Some("()".to_owned()),
                return_type: Some("Any".to_owned()),
                parameter_types: vec!["Any".to_owned()],
                ..Default::default()
            },
            children_ids: Vec::new(),
            repr: default_repr(),
            code_chunk_id: None,
            part: None,
        }];

        EmbedChunker::sanitize_chunk_metadata(&mut chunks);

        assert_eq!(chunks[0].source.fqn, None);
        assert_eq!(chunks[0].context.signature, None);
        assert_eq!(chunks[0].context.type_signature, None);
        assert_eq!(chunks[0].context.return_type, None);
        assert!(chunks[0].context.parameter_types.is_empty());
    }

    #[test]
    fn test_python_line_range_calls_ignore_docstring_continuation() {
        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let chunker = EmbedChunker::with_defaults(settings);
        let content = r#"def run(system_dict):
    """
    Args:
        system_dict: Dictionary containing test state (counts, logs, etc.).

    Returns:
        Updated system_dict with results of this test.
    """
    test_name = "run"
    record_metric(test_name)
"#;

        let docstring_calls = chunker.extract_python_calls_in_line_range(content, 1, 3, 8);
        let body_calls = chunker.extract_python_calls_in_line_range(content, 1, 9, 10);

        assert!(
            docstring_calls.is_empty(),
            "docstring continuation should not be parsed as calls: {docstring_calls:?}"
        );
        assert_eq!(body_calls, vec!["record_metric".to_owned()]);
    }

    #[test]
    fn test_split_large_symbol_makes_progress_with_full_overlap_budget() {
        let mut lines = Vec::new();
        for index in 0..80 {
            lines.push(format!("    let value_{index} = callee({index});"));
        }
        let content = format!(
            "pub fn callee(value: i32) -> i32 {{\n    value + 1\n}}\n\npub fn big() {{\n{}\n}}\n",
            lines.join("\n")
        );
        let split_lines: Vec<&str> = content.lines().collect();

        let settings = EmbedSettings {
            max_tokens: 100,
            min_tokens: 1,
            overlap_tokens: 100,
            context_lines: 0,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let segments =
            chunker.split_lines_to_budgeted_segments(&split_lines, 1, token_model, 100, 40);

        assert!(segments.len() > 1);
        for window in segments.windows(2) {
            assert!(
                window[1].end_line > window[0].end_line,
                "segments must make forward progress: {:?}",
                segments
                    .iter()
                    .map(|segment| (segment.start_line, segment.end_line, segment.tokens))
                    .collect::<Vec<_>>()
            );
        }
        assert!(segments.iter().all(|segment| segment.tokens <= 100));
    }

    #[test]
    fn test_split_large_symbol_part_ids_remain_unique_for_repeated_content() {
        let repeated_line = "alpha beta gamma delta epsilon";
        let content = vec![repeated_line; 6].join("\n");
        let mut symbol = Symbol::new("repeat_parts", crate::types::SymbolKind::Function);
        symbol.signature = Some("fn repeat_parts()".to_owned());
        symbol.start_line = 1;
        symbol.end_line = 6;

        let settings = EmbedSettings {
            max_tokens: 0,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let two_line_budget = chunker
            .tokenizer
            .count(&format!("{repeated_line}\n{repeated_line}"), token_model);
        let chunker = EmbedChunker::with_defaults(EmbedSettings {
            max_tokens: two_line_budget,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        });
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let chunks = chunker
            .split_large_symbol(
                &content,
                &symbol,
                "src/repeat_parts.py",
                "Python",
                Path::new("src/repeat_parts.py"),
                1,
                0,
                &[],
                Some(Language::Python),
                token_model,
                &LineByteRanges::new(),
            )
            .unwrap();

        assert!(chunks.len() > 1);
        let unique_ids: std::collections::HashSet<_> =
            chunks.iter().map(|chunk| chunk.id.clone()).collect();
        assert_eq!(unique_ids.len(), chunks.len(), "split parts must have unique IDs");

        let entry_id = chunks[0].id.clone();
        assert!(chunks.iter().all(|chunk| {
            chunk
                .part
                .as_ref()
                .is_some_and(|part| part.parent_id == entry_id)
        }));
    }

    #[test]
    fn test_split_large_import_stays_imports_with_fragment_metadata() {
        let content = "from very.large.module import (\n    first_symbol,\n    second_symbol,\n    third_symbol,\n    fourth_symbol,\n    fifth_symbol,\n    sixth_symbol,\n)";
        let mut symbol = Symbol::new(content, crate::types::SymbolKind::Import);
        symbol.start_line = 1;
        symbol.end_line = content.lines().count() as u32;

        let settings = EmbedSettings {
            max_tokens: 12,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let chunks = chunker
            .split_large_symbol(
                content,
                &symbol,
                "src/imports.py",
                "Python",
                Path::new("src/imports.py"),
                1,
                0,
                &[],
                Some(Language::Python),
                token_model,
                &LineByteRanges::new(),
            )
            .unwrap();

        assert!(chunks.len() > 1, "test import should split: {chunks:#?}");
        let entry_id = chunks[0].id.clone();
        for chunk in chunks {
            assert_eq!(chunk.kind, ChunkKind::Imports);
            let part = chunk.part.expect("split import should keep part metadata");
            assert_eq!(part.parent_id, entry_id);
            assert_eq!(chunk.context.imports, vec![chunk.content.trim().to_owned()]);
            assert!(
                chunk.context.imports[0].as_str() == chunk.content.trim(),
                "fragment import metadata must be current-fragment text"
            );
        }
    }

    #[test]
    fn test_finalize_sanitizes_fragment_metadata_fields() {
        let mut chunks = vec![EmbedChunk {
            id: "import-chunk".to_owned(),
            full_hash: "hash".to_owned(),
            content: "import os\n# fake_comment_call()".to_owned(),
            tokens: 5,
            kind: ChunkKind::Imports,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: "/bin/Python27/Lib/site-packages/easyprocess-0.1.4-py2.7.egg/easyprocess/__init__.py".to_owned(),
                lines: (1, 2),
                symbol: "import os".to_owned(),
                fqn: Some("bad::import os".to_owned()),
                language: "Python".to_owned(),
                parent: None,
                visibility: Visibility::Public,
                is_test: false,
                module_path: Some("bin.Python27.Lib.site-packages.easyprocess-0.1.4-py2.7.egg.easyprocess".to_owned()),
                parent_chunk_id: None,
                line_byte_range: None,
                content_transform: None,
            },
            context: ChunkContext {
                called_by: vec!["caller".to_owned(), "caller".to_owned()],
                dependents_count: Some(99),
                identifiers: Some("fake_comment_call os".to_owned()),
                keywords: vec!["fake".to_owned(), "comment".to_owned()],
                summary: Some("Top-level code in stale parent".to_owned()),
                ..Default::default()
            },
            children_ids: vec!["missing".to_owned()],
            repr: default_repr(),
            code_chunk_id: None,
            part: Some(ChunkPart {
                part: 1,
                of: 2,
                parent_id: String::new(),
                parent_signature: String::new(),
                overlap_lines: 0,
            }),
        }];

        let settings = EmbedSettings {
            enable_hierarchy: false,
            include_signatures: false,
            git_metadata: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let chunker = EmbedChunker::with_defaults(settings);
        chunker.finalize_chunks(&mut chunks, Path::new("."), &progress);

        let chunk = &chunks[0];
        assert_eq!(chunk.source.module_path.as_deref(), Some("easyprocess"));
        assert_eq!(chunk.source.fqn, None);
        assert_eq!(chunk.children_ids, Vec::<String>::new());
        assert_eq!(chunk.context.called_by, Vec::<String>::new());
        assert_eq!(chunk.context.dependents_count, None);
        assert_eq!(chunk.context.summary, None);
        assert!(!chunk
            .context
            .keywords
            .iter()
            .any(|term| term.contains("fake")));
        assert!(!chunk
            .context
            .identifiers
            .as_deref()
            .unwrap_or_default()
            .contains("fake_comment_call"));
        let part = chunk.part.as_ref().expect("part should be repaired");
        assert_eq!(part.parent_id, chunk.id);
        assert!(!part.parent_signature.is_empty());
    }

    #[test]
    fn test_finalize_keywords_are_current_fragment_tokens() {
        let mut chunks = vec![EmbedChunk {
            id: "function-chunk".to_owned(),
            full_hash: "hash".to_owned(),
            content: "def parse_http_response(rawBytes):\n    if rawBytes:\n        return rawBytes\n    raise ValueError(\"comment words hidden\")\n".to_owned(),
            tokens: 8,
            kind: ChunkKind::Function,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: "pkg/parser.py".to_owned(),
                lines: (1, 2),
                symbol: "parse_http_response".to_owned(),
                fqn: Some("pkg::parser::parse_http_response".to_owned()),
                language: "Python".to_owned(),
                parent: None,
                visibility: Visibility::Public,
                is_test: false,
                module_path: Some("pkg.parser".to_owned()),
                parent_chunk_id: None,
                line_byte_range: None,
                content_transform: None,
            },
            context: ChunkContext {
                signature: Some("def parse_http_response(rawBytes):".to_owned()),
                ..Default::default()
            },
            children_ids: Vec::new(),
            repr: default_repr(),
            code_chunk_id: None,
            part: None,
        }];

        let settings = EmbedSettings {
            enable_hierarchy: false,
            include_signatures: false,
            git_metadata: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let chunker = EmbedChunker::with_defaults(settings);
        chunker.finalize_chunks(&mut chunks, Path::new("."), &progress);

        let keywords = &chunks[0].context.keywords;
        assert!(keywords.contains(&"parse_http_response".to_owned()));
        assert!(keywords.contains(&"rawbytes".to_owned()));
        assert!(keywords.contains(&"valueerror".to_owned()));
        assert!(!keywords.contains(&"parse".to_owned()));
        assert!(!keywords.contains(&"http".to_owned()));
        assert!(!keywords.contains(&"response".to_owned()));
        assert!(!keywords.contains(&"return".to_owned()));
        assert!(!keywords.contains(&"raise".to_owned()));
        assert!(!keywords.contains(&"hidden".to_owned()));

        let identifiers = chunks[0].context.identifiers.as_deref().unwrap_or_default();
        assert!(identifiers.contains("parse_http_response"));
        assert!(!identifiers.split_whitespace().any(|term| term == "if"));
        assert!(!identifiers.split_whitespace().any(|term| term == "return"));
        assert!(!identifiers.split_whitespace().any(|term| term == "raise"));
        assert!(!identifiers.split_whitespace().any(|term| term == "hidden"));
    }

    #[test]
    fn test_finalize_python_metadata_ignores_docstring_fragment_prose() {
        let mut chunks = vec![EmbedChunk {
            id: "docstring-part".to_owned(),
            full_hash: "hash".to_owned(),
            content: "    weights to samples from underrepresented classes. The weight for each class is calculated as:\n        weight[class] = total_samples / (number_of_classes * count[class])\n".to_owned(),
            tokens: 20,
            kind: ChunkKind::FunctionPart,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: "monk/gluon/datasets/class_imbalance.py".to_owned(),
                lines: (28, 36),
                symbol: "balance_class_weights".to_owned(),
                fqn: Some("monk::gluon::datasets::class_imbalance::balance_class_weights".to_owned()),
                language: "Python".to_owned(),
                parent: None,
                visibility: Visibility::Public,
                is_test: false,
                module_path: Some("monk.gluon.datasets.class_imbalance".to_owned()),
                parent_chunk_id: None,
                line_byte_range: None,
                content_transform: None,
            },
            context: ChunkContext::default(),
            children_ids: Vec::new(),
            repr: default_repr(),
            code_chunk_id: None,
            part: Some(ChunkPart {
                part: 1,
                of: 2,
                parent_id: "entry".to_owned(),
                parent_signature: "def balance_class_weights".to_owned(),
                overlap_lines: 0,
            }),
        }];

        EmbedChunker::sanitize_chunk_metadata(&mut chunks);

        assert!(chunks[0].context.keywords.is_empty());
        assert_eq!(chunks[0].context.identifiers, None);
    }

    #[test]
    fn test_finalize_python_metadata_ignores_formula_and_non_ascii_prose_fragments() {
        let mut chunks = vec![
            EmbedChunk {
                id: "formula-docstring-part".to_owned(),
                full_hash: "hash".to_owned(),
                content: "    L = -[y * log(σ(x)) + (1-y) * log(1-σ(x))]\n    Where σ(x) is the sigmoid function.\n".to_owned(),
                tokens: 20,
                kind: ChunkKind::FunctionPart,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "convertor/huawei/impl/binary_cross_entropy_grad.py".to_owned(),
                    lines: (145, 154),
                    symbol: "binary_cross_entropy_grad_compute".to_owned(),
                    fqn: Some("pkg::binary_cross_entropy_grad_compute".to_owned()),
                    language: "Python".to_owned(),
                    parent: None,
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("convertor.huawei.impl.binary_cross_entropy_grad".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: None,
            },
            EmbedChunk {
                id: "unicode-docstring-part".to_owned(),
                full_hash: "hash".to_owned(),
                content: "\"\"\"\n月份天数查询程序\n规则：31天\n\"\"\"".to_owned(),
                tokens: 20,
                kind: ChunkKind::TopLevel,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "month01/day03/exercise07.py".to_owned(),
                    lines: (1, 4),
                    symbol: "<top_level>".to_owned(),
                    fqn: Some("bad::top".to_owned()),
                    language: "Python".to_owned(),
                    parent: None,
                    visibility: Visibility::Private,
                    is_test: false,
                    module_path: Some("month01.day03.exercise07".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: None,
            },
        ];

        EmbedChunker::sanitize_chunk_metadata(&mut chunks);

        for chunk in &chunks {
            assert!(
                chunk.context.keywords.is_empty(),
                "prose-only fragment should not produce keywords: {:?}",
                chunk.context.keywords
            );
            assert_eq!(chunk.context.identifiers, None);
        }
    }

    #[test]
    fn test_finalize_python_metadata_does_not_synthesize_none_tokens() {
        let mut chunks = vec![EmbedChunk {
            id: "method-chunk".to_owned(),
            full_hash: "hash".to_owned(),
            content: "    def _print(self, message: str) -> None:\n        \"\"\"Conditionally print messages based on verbosity.\"\"\"\n        if self.verbose >= 1:\n            print(message)\n".to_owned(),
            tokens: 20,
            kind: ChunkKind::Method,
            source: ChunkSource {
                repo: RepoIdentifier::default(),
                file: "monk/gluon/finetune/level_10_schedulers_main.py".to_owned(),
                lines: (39, 42),
                symbol: "_print".to_owned(),
                fqn: Some("monk::gluon::finetune::level_10_schedulers_main::_print".to_owned()),
                language: "Python".to_owned(),
                parent: None,
                visibility: Visibility::Public,
                is_test: false,
                module_path: Some("monk.gluon.finetune.level_10_schedulers_main".to_owned()),
                parent_chunk_id: None,
                line_byte_range: None,
                content_transform: None,
            },
            context: ChunkContext {
                signature: Some("def _print(self, message: str) -> None:".to_owned()),
                docstring: Some("Conditionally print messages based on verbosity.".to_owned()),
                ..Default::default()
            },
            children_ids: Vec::new(),
            repr: default_repr(),
            code_chunk_id: None,
            part: None,
        }];

        EmbedChunker::sanitize_chunk_metadata(&mut chunks);

        let keywords = &chunks[0].context.keywords;
        assert!(keywords.contains(&"_print".to_owned()));
        assert!(keywords.contains(&"message".to_owned()));
        assert!(keywords.contains(&"verbose".to_owned()));
        assert!(!keywords.iter().any(|term| term.contains("none")));
        assert!(!keywords.iter().any(|term| term == "conditionally"));

        let identifiers = chunks[0].context.identifiers.as_deref().unwrap_or_default();
        assert!(identifiers.split_whitespace().any(|term| term == "_print"));
        assert!(!identifiers
            .split_whitespace()
            .any(|term| term.contains("none")));
        assert!(!identifiers
            .split_whitespace()
            .any(|term| term == "conditionally"));
    }

    #[test]
    fn test_fragment_summary_requires_current_docstring_or_signature() {
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "pkg/service.py".to_owned(),
            lines: (10, 20),
            symbol: "worker".to_owned(),
            fqn: Some("pkg::service::worker".to_owned()),
            language: "Python".to_owned(),
            parent: None,
            visibility: Visibility::Public,
            is_test: false,
            module_path: Some("pkg.service".to_owned()),
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };

        assert_eq!(
            generate_fragment_summary(ChunkKind::FunctionPart, &source, &ChunkContext::default()),
            None
        );

        let context =
            ChunkContext { signature: Some("def worker(value):".to_owned()), ..Default::default() };
        assert_eq!(
            generate_fragment_summary(ChunkKind::FunctionPart, &source, &context),
            Some("Public function_part 'worker' -- def worker(value):".to_owned())
        );
    }

    #[test]
    fn test_top_level_split_part_ids_remain_unique_for_repeated_content() {
        let repeated_line = "alpha beta gamma delta epsilon";
        let mut lines = (0..6).map(|_| repeated_line.to_owned()).collect::<Vec<_>>();
        lines.push("def keep():".to_owned());
        lines.push("    return 1".to_owned());
        let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();

        let mut symbol = Symbol::new("keep", crate::types::SymbolKind::Function);
        symbol.start_line = 7;
        symbol.end_line = 8;
        let symbols = vec![symbol];

        let settings = EmbedSettings {
            max_tokens: 0,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: true,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let two_line_budget = chunker
            .tokenizer
            .count(&format!("{repeated_line}\n{repeated_line}"), token_model);
        let chunker = EmbedChunker::with_defaults(EmbedSettings {
            max_tokens: two_line_budget,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: true,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        });
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let chunks = chunker.extract_top_level(
            &line_refs,
            &symbols,
            "src/repeat_top_level.py",
            "Python",
            Some(Language::Python),
            token_model,
            &LineByteRanges::new(),
        );

        assert!(chunks.len() > 1);
        let unique_ids: std::collections::HashSet<_> =
            chunks.iter().map(|chunk| chunk.id.clone()).collect();
        assert_eq!(unique_ids.len(), chunks.len(), "top-level split parts must have unique IDs");

        let entry_id = chunks[0].id.clone();
        assert!(chunks.iter().all(|chunk| {
            chunk
                .part
                .as_ref()
                .is_some_and(|part| part.parent_id == entry_id)
        }));
    }

    #[test]
    fn test_top_level_split_parts_mark_content_transform_per_part() {
        let lines = vec![
            "SAFE_ALPHA = build_alpha()".to_owned(),
            "SECRET_VALUE = '[REDACTED]'".to_owned(),
            "SAFE_BETA = build_beta()".to_owned(),
            "def keep():".to_owned(),
            "    return 1".to_owned(),
        ];
        let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();

        let mut symbol = Symbol::new("keep", crate::types::SymbolKind::Function);
        symbol.start_line = 4;
        symbol.end_line = 5;
        let symbols = vec![symbol];

        let settings = EmbedSettings {
            max_tokens: 10,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: true,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let chunks = chunker.extract_top_level(
            &line_refs,
            &symbols,
            "src/settings.py",
            "Python",
            Some(Language::Python),
            token_model,
            &LineByteRanges::from([(2, vec![(0, 1)])]),
        );

        assert!(
            chunks.len() > 1,
            "top-level span should split so transform can be checked per part"
        );
        let redacted_parts: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.source.content_transform.as_deref() == Some("redacted_secrets"))
            .collect();
        assert_eq!(redacted_parts.len(), 1);
        assert_eq!(redacted_parts[0].source.lines, (2, 2));
        assert!(
            chunks
                .iter()
                .filter(|chunk| chunk.source.lines != (2, 2))
                .all(|chunk| chunk.source.content_transform.is_none()),
            "top-level parts outside the redacted line must not inherit the transform"
        );
    }

    #[test]
    fn test_repair_split_part_parent_ids_keeps_distinct_top_level_sequences() {
        fn top_part(id: &str, lines: (u32, u32), part_no: u32, parent_id: &str) -> EmbedChunk {
            EmbedChunk {
                id: id.to_owned(),
                full_hash: format!("{id}_full"),
                content: format!("line {}", lines.0),
                tokens: 1,
                kind: ChunkKind::TopLevel,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "module.py".to_owned(),
                    lines,
                    symbol: "<top_level>".to_owned(),
                    fqn: None,
                    language: "Python".to_owned(),
                    parent: None,
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("module".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: Some(ChunkPart {
                    part: part_no,
                    of: 2,
                    parent_id: parent_id.to_owned(),
                    parent_signature: String::new(),
                    overlap_lines: 0,
                }),
            }
        }

        let mut chunks = vec![
            top_part("first_entry", (1, 3), 1, "stale"),
            top_part("first_tail", (8, 10), 2, "stale"),
            top_part("second_entry", (20, 22), 1, "stale"),
            top_part("second_tail", (27, 29), 2, "stale"),
        ];
        let live_ids = chunks.iter().map(|chunk| chunk.id.clone()).collect();

        EmbedChunker::repair_split_part_parent_ids(&mut chunks, &live_ids);

        let parent_ids: BTreeMap<_, _> = chunks
            .iter()
            .map(|chunk| {
                (
                    chunk.id.as_str(),
                    chunk
                        .part
                        .as_ref()
                        .expect("part metadata")
                        .parent_id
                        .as_str(),
                )
            })
            .collect();
        assert_eq!(parent_ids["first_entry"], "first_entry");
        assert_eq!(parent_ids["first_tail"], "first_entry");
        assert_eq!(parent_ids["second_entry"], "second_entry");
        assert_eq!(parent_ids["second_tail"], "second_entry");
    }

    #[test]
    fn test_repair_split_part_parent_ids_anchors_orphan_tail_to_live_part() {
        fn part_chunk(id: &str, part_no: u32) -> EmbedChunk {
            EmbedChunk {
                id: id.to_owned(),
                full_hash: format!("{id}_full"),
                content: "tail body".to_owned(),
                tokens: 2,
                kind: ChunkKind::FunctionPart,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "module.py".to_owned(),
                    lines: (20, 25),
                    symbol: "local_callback".to_owned(),
                    fqn: Some("module::local_callback".to_owned()),
                    language: "Python".to_owned(),
                    parent: None,
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("module".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: Some(ChunkPart {
                    part: part_no,
                    of: 2,
                    parent_id: id.to_owned(),
                    parent_signature: "def local_callback(value:".to_owned(),
                    overlap_lines: 0,
                }),
            }
        }

        let mut chunks = vec![part_chunk("tail_only", 2)];
        let live_ids = chunks.iter().map(|chunk| chunk.id.clone()).collect();

        EmbedChunker::repair_split_part_parent_ids(&mut chunks, &live_ids);

        assert_eq!(chunks[0].part.as_ref().unwrap().parent_id, "tail_only");
    }

    #[test]
    fn test_repair_split_part_parent_ids_ignores_dirty_fqn_for_grouping() {
        fn part_chunk(id: &str, part_no: u32, fqn: &str, lines: (u32, u32)) -> EmbedChunk {
            EmbedChunk {
                id: id.to_owned(),
                full_hash: format!("{id}_full"),
                content: format!("part {part_no}"),
                tokens: 2,
                kind: ChunkKind::ClassPart,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "models.py".to_owned(),
                    lines,
                    symbol: "Issue".to_owned(),
                    fqn: Some(fqn.to_owned()),
                    language: "Python".to_owned(),
                    parent: None,
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("models".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: Some(ChunkPart {
                    part: part_no,
                    of: 2,
                    parent_id: "stale".to_owned(),
                    parent_signature: "class Issue(models.Model):".to_owned(),
                    overlap_lines: 0,
                }),
            }
        }

        let mut chunks = vec![
            part_chunk("entry", 1, "models::Issue", (1, 10)),
            part_chunk("tail", 2, "models::Meta", (11, 20)),
        ];
        let live_ids = chunks.iter().map(|chunk| chunk.id.clone()).collect();

        EmbedChunker::repair_split_part_parent_ids(&mut chunks, &live_ids);

        let tail = chunks.iter().find(|chunk| chunk.id == "tail").unwrap();
        assert_eq!(tail.part.as_ref().unwrap().parent_id, "entry");
    }

    #[test]
    fn test_split_class_parts_use_fragment_level_children_hierarchy() {
        fn chunk(
            id: &str,
            kind: ChunkKind,
            symbol: &str,
            parent: Option<&str>,
            lines: (u32, u32),
        ) -> EmbedChunk {
            EmbedChunk {
                id: id.to_owned(),
                full_hash: format!("{id}_full"),
                content: symbol.to_owned(),
                tokens: 1,
                kind,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "service.py".to_owned(),
                    lines,
                    symbol: symbol.to_owned(),
                    fqn: Some(format!("service::{symbol}")),
                    language: "Python".to_owned(),
                    parent: parent.map(str::to_owned),
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("service".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: if kind == ChunkKind::ClassPart {
                    Some(ChunkPart {
                        part: if id.ends_with('1') { 1 } else { 2 },
                        of: 2,
                        parent_id: "class_parent".to_owned(),
                        parent_signature: "class BigService".to_owned(),
                        overlap_lines: 0,
                    })
                } else {
                    None
                },
            }
        }

        let mut chunks = vec![
            chunk("class_part_1", ChunkKind::ClassPart, "BigService", None, (1, 50)),
            chunk("class_part_2", ChunkKind::ClassPart, "BigService", None, (51, 100)),
            chunk(
                "child_method",
                ChunkKind::Method,
                "important_child",
                Some("BigService"),
                (75, 80),
            ),
        ];

        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let progress = QuietProgress;
        let chunker = EmbedChunker::with_defaults(settings);
        chunker.link_parent_children(&mut chunks, &progress);

        let child = chunks
            .iter()
            .find(|chunk| chunk.id == "child_method")
            .expect("child method should exist");
        assert!(
            child.source.parent_chunk_id.as_deref() == Some("class_part_2"),
            "child method should link to the overlapping split class part"
        );

        let first_part = chunks
            .iter()
            .find(|chunk| chunk.id == "class_part_1")
            .expect("first class part should exist");
        assert!(
            !first_part.children_ids.contains(&child.id),
            "non-overlapping class part should not inherit children_ids {:?}",
            first_part.children_ids
        );

        let second_part = chunks
            .iter()
            .find(|chunk| chunk.id == "class_part_2")
            .expect("second class part should exist");
        assert!(
            second_part.children_ids.contains(&child.id),
            "overlapping class part should contain child id {:?}",
            second_part.children_ids
        );
    }

    #[test]
    fn test_finalize_prunes_empty_parts_and_repairs_live_references() {
        fn chunk(
            id: &str,
            kind: ChunkKind,
            symbol: &str,
            parent: Option<&str>,
            lines: (u32, u32),
            content: &str,
            part_no: Option<u32>,
        ) -> EmbedChunk {
            EmbedChunk {
                id: id.to_owned(),
                full_hash: format!("{id}_full"),
                content: content.to_owned(),
                tokens: 1,
                kind,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "service.py".to_owned(),
                    lines,
                    symbol: symbol.to_owned(),
                    fqn: Some(format!("service::{symbol}")),
                    language: "Python".to_owned(),
                    parent: parent.map(str::to_owned),
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("service".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: part_no.map(|part| ChunkPart {
                    part,
                    of: 2,
                    parent_id: "synthetic_missing_parent".to_owned(),
                    parent_signature: "class BigService".to_owned(),
                    overlap_lines: 0,
                }),
            }
        }

        let mut chunks = vec![
            chunk(
                "empty_parent_part",
                ChunkKind::ClassPart,
                "BigService",
                None,
                (1, 10),
                "\n\n",
                Some(1),
            ),
            chunk(
                "live_parent_part",
                ChunkKind::ClassPart,
                "BigService",
                None,
                (11, 20),
                "class BigService:",
                Some(2),
            ),
            chunk(
                "child_method",
                ChunkKind::Method,
                "run",
                Some("BigService"),
                (12, 14),
                "def run(self):\n    return 1",
                None,
            ),
        ];

        let settings = EmbedSettings {
            enable_hierarchy: false,
            include_signatures: false,
            git_metadata: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let chunker = EmbedChunker::with_defaults(settings);
        chunker.finalize_chunks(&mut chunks, Path::new("."), &progress);

        let ids: std::collections::HashSet<_> =
            chunks.iter().map(|chunk| chunk.id.as_str()).collect();
        assert!(!ids.contains("empty_parent_part"));
        assert!(ids.contains("live_parent_part"));
        assert!(ids.contains("child_method"));

        let child = chunks
            .iter()
            .find(|chunk| chunk.id == "child_method")
            .expect("child should remain");
        assert_eq!(child.source.parent_chunk_id.as_deref(), Some("live_parent_part"));

        let live_parent = chunks
            .iter()
            .find(|chunk| chunk.id == "live_parent_part")
            .expect("live parent should remain");
        assert_eq!(live_parent.children_ids, vec!["child_method".to_owned()]);
        assert_eq!(
            live_parent.part.as_ref().unwrap().parent_id,
            "live_parent_part",
            "orphaned non-entry split part should anchor to the first surviving split fragment"
        );
    }

    #[test]
    fn test_duplicate_chunks_canonicalize_and_preserve_alias_relations() {
        fn chunk(id: &str, kind: ChunkKind, parent: Option<&str>) -> EmbedChunk {
            let content = "def run(self):\n    return helper()".to_owned();
            let full_hash = hash_content(&content).full_hash;
            EmbedChunk {
                id: id.to_owned(),
                full_hash,
                content,
                tokens: 8,
                kind,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "service.py".to_owned(),
                    lines: (10, 11),
                    symbol: "run".to_owned(),
                    fqn: Some("service::Worker::run".to_owned()),
                    language: "Python".to_owned(),
                    parent: parent.map(str::to_owned),
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("service".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext {
                    calls: vec![format!("{id}_call")],
                    called_by: vec![format!("{id}_caller")],
                    imports: vec![format!("{id}_import")],
                    qualified_calls: vec![format!("service::{id}_call")],
                    ..Default::default()
                },
                children_ids: vec![format!("{id}_child")],
                repr: default_repr(),
                code_chunk_id: None,
                part: None,
            }
        }

        let mut chunks = vec![
            chunk("function_alias", ChunkKind::Function, None),
            chunk("method_canonical", ChunkKind::Method, Some("Worker")),
        ];
        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let chunker = EmbedChunker::with_defaults(settings);
        let aliases = chunker.canonicalize_duplicate_chunks(&mut chunks);

        assert_eq!(aliases, 1);
        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];
        assert_eq!(chunk.kind, ChunkKind::Method);
        assert_eq!(chunk.context.calls.len(), 2);
        assert_eq!(chunk.context.called_by.len(), 2);
        assert_eq!(chunk.context.imports.len(), 2);
        assert_eq!(chunk.context.qualified_calls.len(), 2);
        assert_eq!(chunk.children_ids.len(), 2);
        assert_eq!(chunk.context.dependents_count, Some(2));
    }

    #[test]
    fn test_python_method_duplicate_and_container_split_source_dedup() {
        let temp_dir = TempDir::new().unwrap();
        let mut class_fields = String::new();
        for index in 0..120 {
            class_fields.push_str(&format!("    field_{index} = 'container field {index}'\n"));
        }
        let method_body = "        return CHILD_BODY_MARKER_RESULT\n";
        create_test_file(
            temp_dir.path(),
            "service.py",
            &format!("class BigContainer:\n{class_fields}\n    def worker(self):\n{method_body}"),
        );

        let settings = EmbedSettings {
            max_tokens: 80,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let worker_chunks: Vec<_> = chunks
            .iter()
            .filter(|chunk| chunk.source.symbol == "worker")
            .collect();
        assert_eq!(worker_chunks.len(), 1, "method/function duplicates should canonicalize");
        let worker = worker_chunks[0];
        assert_eq!(worker.kind, ChunkKind::Method);
        assert!(worker.content.contains("CHILD_BODY_MARKER_RESULT"));

        let class_parts: Vec<_> = chunks
            .iter()
            .filter(|chunk| {
                chunk.kind == ChunkKind::ClassPart && chunk.source.symbol == "BigContainer"
            })
            .collect();
        assert!(!class_parts.is_empty(), "large class should still produce class parts");
        assert!(
            class_parts
                .iter()
                .all(|chunk| !chunk.content.contains("CHILD_BODY_MARKER")),
            "class parts must not duplicate child method bodies: {class_parts:#?}"
        );
        assert!(
            class_parts
                .iter()
                .all(|chunk| chunk.source.content_transform.is_none()),
            "class fragments that do not include masked child lines must not inherit the transform"
        );
    }

    #[test]
    fn test_content_transform_range_intersection_is_fragment_level() {
        let redacted_ranges = LineByteRanges::from([(3, vec![(4, 12)]), (8, vec![(0, 1)])]);
        let masked_lines = BTreeSet::from([5, 6]);

        assert_eq!(
            EmbedChunker::content_transform_for_range(1, 2, None, &redacted_ranges, &masked_lines),
            None
        );
        assert_eq!(
            EmbedChunker::content_transform_for_range(3, 4, None, &redacted_ranges, &masked_lines)
                .as_deref(),
            Some("redacted_secrets")
        );
        assert_eq!(
            EmbedChunker::content_transform_for_range(4, 6, None, &redacted_ranges, &masked_lines)
                .as_deref(),
            Some("masked_container_child_bodies")
        );
        assert_eq!(
            EmbedChunker::content_transform_for_range(3, 6, None, &redacted_ranges, &masked_lines)
                .as_deref(),
            Some("redacted_secrets,masked_container_child_bodies")
        );
        assert_eq!(
            EmbedChunker::content_transform_for_range(
                3,
                3,
                Some((0, 4)),
                &redacted_ranges,
                &masked_lines
            ),
            None
        );
        assert_eq!(
            EmbedChunker::content_transform_for_range(
                3,
                3,
                Some((6, 10)),
                &redacted_ranges,
                &masked_lines
            )
            .as_deref(),
            Some("redacted_secrets")
        );
        assert_eq!(
            EmbedChunker::content_transform_for_range(
                3,
                3,
                None,
                &redacted_ranges,
                &BTreeSet::from([3])
            )
            .as_deref(),
            Some("masked_container_child_bodies")
        );
        assert_eq!(
            EmbedChunker::content_transform_for_range(
                3,
                3,
                Some((6, 10)),
                &redacted_ranges,
                &BTreeSet::from([3])
            )
            .as_deref(),
            Some("masked_container_child_bodies")
        );
    }

    #[test]
    fn test_real_repo_file_split_parts_use_fragment_level_relationships() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let settings = EmbedSettings {
            max_tokens: 120,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            include_patterns: vec!["src/embedding/chunker.rs".to_owned()],
            include_top_level: false,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker.chunk_repository(repo_root, &progress).unwrap();

        let split_large_symbol_parts: Vec<_> = chunks
            .iter()
            .filter(|chunk| {
                chunk.kind == ChunkKind::FunctionPart
                    && chunk.source.symbol == "split_large_symbol"
                    && chunk.repr == "code"
            })
            .collect();
        assert!(
            split_large_symbol_parts.len() > 1,
            "real repository file should split split_large_symbol(): {chunks:#?}"
        );

        for part in &split_large_symbol_parts {
            assert!(part.part.is_some());
            assert!(part.tokens <= 120, "real split part should respect max_tokens=120");
            assert_eq!(part.source.symbol, "split_large_symbol");
            assert!(part
                .source
                .fqn
                .as_deref()
                .unwrap_or_default()
                .ends_with("::split_large_symbol"));
            for call in &part.context.calls {
                assert!(
                    part.content.contains(call),
                    "real split part call should be present in the fragment: call={call}"
                );
            }
            if part.part.as_ref().is_some_and(|part| part.part > 1) {
                assert!(
                    part.context.called_by.is_empty(),
                    "non-entry real split part should not inherit caller relationship: {:?}",
                    part.context.called_by
                );
                assert_eq!(part.context.dependents_count, None);
            }
        }

        let entry_part = split_large_symbol_parts
            .iter()
            .find(|part| part.part.as_ref().is_some_and(|part| part.part == 1))
            .expect("real split symbol should have an entry part");
        assert!(
            !entry_part.context.called_by.is_empty(),
            "entry split part should receive incoming calls to the split symbol: {:?}",
            entry_part.context.called_by
        );
    }

    #[test]
    fn test_extract_top_level_matches_covered_line_semantics() {
        let code = "\
use std::fmt;

fn outer() {
    helper();
}

const VALUE: usize = 1;

impl Service {
    fn run(&self) {}
}

fn trailing() {}
";
        let lines: Vec<&str> = code.lines().collect();
        let mut symbols = Vec::new();
        for (name, kind, start_line, end_line) in [
            ("std::fmt", crate::types::SymbolKind::Import, 1, 1),
            ("outer", crate::types::SymbolKind::Function, 3, 5),
            ("Service", crate::types::SymbolKind::Class, 9, 11),
            ("run", crate::types::SymbolKind::Method, 10, 10),
            ("trailing", crate::types::SymbolKind::Function, 13, 13),
        ] {
            let mut symbol = Symbol::new(name, kind);
            symbol.start_line = start_line;
            symbol.end_line = end_line;
            symbols.push(symbol);
        }

        let mut covered = vec![false; lines.len()];
        for symbol in &symbols {
            let start = symbol.start_line.saturating_sub(1) as usize;
            let end = (symbol.end_line as usize).min(lines.len());
            for item in covered.iter_mut().take(end).skip(start) {
                *item = true;
            }
        }
        let expected = lines
            .iter()
            .enumerate()
            .filter(|(index, _)| !covered[*index])
            .map(|(_, line)| *line)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned();

        let settings = EmbedSettings {
            min_tokens: 1,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let top_levels = chunker.extract_top_level(
            &lines,
            &symbols,
            "src/lib.rs",
            "Rust",
            Some(Language::Rust),
            token_model,
            &LineByteRanges::new(),
        );
        assert_eq!(top_levels.len(), 1);
        let top_level = &top_levels[0];

        assert_eq!(top_level.content, expected);
        assert_eq!(top_level.content, "const VALUE: usize = 1;");
        assert_eq!(top_level.source.lines, (7, 7));
    }

    #[test]
    fn test_real_repo_file_respects_max_tokens_hard_cap() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let content = std::fs::read_to_string(repo_root.join("src/embedding/chunker.rs")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        let mut marker = Symbol::new("module_header", crate::types::SymbolKind::Module);
        marker.start_line = 1;
        marker.end_line = 1;
        let symbols = vec![marker];
        let settings = EmbedSettings {
            max_tokens: 100,
            min_tokens: 1,
            overlap_tokens: 0,
            context_lines: 0,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let chunks = chunker.extract_top_level(
            &lines,
            &symbols,
            "src/embedding/chunker.rs",
            "Rust",
            Some(Language::Rust),
            token_model,
            &LineByteRanges::new(),
        );

        assert!(chunks.len() > 1, "real file top-level content should be split");
        assert!(
            chunks.iter().all(|chunk| chunk.tokens <= 100),
            "all real-file chunks should respect max_tokens=100: {:?}",
            chunks
                .iter()
                .filter(|chunk| chunk.tokens > 100)
                .map(|chunk| (
                    chunk.id.as_str(),
                    chunk.kind.name(),
                    chunk.source.symbol.as_str(),
                    chunk.tokens
                ))
                .collect::<Vec<_>>()
        );
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.source.symbol == "<top_level>" && chunk.part.is_some()),
            "oversized top-level chunks should be represented as split parts"
        );
    }

    #[test]
    fn test_filtered_chunking_populates_repo_identity() {
        use std::collections::HashSet;

        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "a.rs",
            r#"
pub fn caller() {
}
"#,
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            repo_namespace: Some("org".to_owned()),
            repo_name: Some("repo".to_owned()),
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut only_files = HashSet::new();
        only_files.insert(PathBuf::from("a.rs"));

        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository_filtered(temp_dir.path(), &only_files, &progress)
            .unwrap();

        assert!(!chunks.is_empty());
        assert!(chunks
            .iter()
            .all(|chunk| chunk.source.repo.namespace.as_deref() == Some("org")));
        assert!(chunks.iter().all(|chunk| chunk.source.repo.name == "repo"));
    }

    #[test]
    fn test_file_too_large() {
        let temp_dir = TempDir::new().unwrap();
        // Create a file larger than 100 bytes
        let large_content = "x".repeat(200);
        create_test_file(temp_dir.path(), "large.rs", &large_content);

        let settings = EmbedSettings::default();
        let limits = ResourceLimits::default().with_max_file_size(100);
        let mut chunker = EmbedChunker::new(settings, limits);
        let progress = QuietProgress;

        // Should skip the file (warning) and return empty
        let result = chunker.chunk_repository(temp_dir.path(), &progress);

        // The chunker should produce an error about no chunks generated
        // because the only file was skipped
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        let settings = EmbedSettings::default();
        let mut chunker = EmbedChunker::with_defaults(settings);
        let progress = QuietProgress;

        let result = chunker.chunk_repository(temp_dir.path(), &progress);

        assert!(matches!(result, Err(EmbedError::NoChunksGenerated { .. })));
    }

    #[test]
    fn test_language_detection() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        assert_eq!(chunker.detect_language(Path::new("test.rs")), "Rust");
        assert_eq!(chunker.detect_language(Path::new("test.py")), "Python");
        assert_eq!(chunker.detect_language(Path::new("test.unknown")), "unknown");
    }

    #[test]
    fn test_is_test_code() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let test_symbol = Symbol::new("test_foo", crate::types::SymbolKind::Function);
        assert!(chunker.is_test_code(Path::new("foo.rs"), &test_symbol));

        let normal_symbol = Symbol::new("foo", crate::types::SymbolKind::Function);
        assert!(!chunker.is_test_code(Path::new("src/lib.rs"), &normal_symbol));

        // Test path-based detection
        assert!(chunker.is_test_code(Path::new("tests/test_foo.rs"), &normal_symbol));
    }

    #[test]
    fn test_generate_tags() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let mut symbol = Symbol::new("authenticate_user", crate::types::SymbolKind::Function);
        symbol.signature = Some("async fn authenticate_user(password: &str)".to_owned());

        let tags = chunker.generate_tags(&symbol);
        assert!(tags.contains(&"async".to_owned()));
        assert!(tags.contains(&"security".to_owned()));
    }

    #[test]
    fn test_generate_tags_kotlin_suspend() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let mut symbol = Symbol::new("fetchData", crate::types::SymbolKind::Function);
        symbol.signature = Some("suspend fun fetchData(): Result<Data>".to_owned());

        let tags = chunker.generate_tags(&symbol);
        assert!(tags.contains(&"async".to_owned()), "Kotlin suspend should be tagged as async");
    }

    #[test]
    fn test_generate_tags_go_concurrency() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let mut symbol = Symbol::new("processMessages", crate::types::SymbolKind::Function);
        symbol.signature = Some("func processMessages(ch chan string)".to_owned());

        let tags = chunker.generate_tags(&symbol);
        assert!(
            tags.contains(&"concurrency".to_owned()),
            "Go channels should be tagged as concurrency"
        );
    }

    #[test]
    fn test_generate_tags_ml() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Test ML training function
        let mut symbol = Symbol::new("train_model", crate::types::SymbolKind::Function);
        symbol.signature = Some("def train_model(epochs: int, batch_size: int)".to_owned());
        let tags = chunker.generate_tags(&symbol);
        assert!(tags.contains(&"ml".to_owned()), "train_model should be tagged as ml");

        // Test neural network layer
        let mut symbol2 = Symbol::new("forward_pass", crate::types::SymbolKind::Function);
        symbol2.signature = Some("def forward_pass(self, x: torch.Tensor)".to_owned());
        let tags2 = chunker.generate_tags(&symbol2);
        assert!(
            tags2.contains(&"ml".to_owned()),
            "torch.Tensor in signature should be tagged as ml"
        );

        // Test classifier
        let mut symbol3 = Symbol::new("ImageClassifier", crate::types::SymbolKind::Class);
        symbol3.signature = Some("class ImageClassifier(nn.Module)".to_owned());
        let tags3 = chunker.generate_tags(&symbol3);
        assert!(tags3.contains(&"ml".to_owned()), "nn.Module should be tagged as ml");
    }

    #[test]
    fn test_generate_tags_data_science() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Test DataFrame operation
        let mut symbol = Symbol::new("preprocess_dataframe", crate::types::SymbolKind::Function);
        symbol.signature = Some("def preprocess_dataframe(df: pd.DataFrame)".to_owned());
        let tags = chunker.generate_tags(&symbol);
        assert!(
            tags.contains(&"data-science".to_owned()),
            "DataFrame should be tagged as data-science"
        );

        // Test numpy array
        let mut symbol2 = Symbol::new("normalize_array", crate::types::SymbolKind::Function);
        symbol2.signature = Some("def normalize_array(arr: np.ndarray)".to_owned());
        let tags2 = chunker.generate_tags(&symbol2);
        assert!(
            tags2.contains(&"data-science".to_owned()),
            "np.ndarray should be tagged as data-science"
        );

        // Test ETL pipeline
        let symbol3 = Symbol::new("run_etl_pipeline", crate::types::SymbolKind::Function);
        let tags3 = chunker.generate_tags(&symbol3);
        assert!(tags3.contains(&"data-science".to_owned()), "etl should be tagged as data-science");
    }

    #[test]
    fn test_brace_nesting_depth() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Test simple nesting
        let code = "fn foo() { if x { if y { } } }";
        assert_eq!(chunker.calculate_brace_depth(code), 3);

        // Test no nesting
        let flat = "let x = 1;";
        assert_eq!(chunker.calculate_brace_depth(flat), 0);

        // Test deep nesting with all bracket types
        let deep = "fn f() { let a = vec![HashMap::new()]; }";
        assert!(chunker.calculate_brace_depth(deep) >= 2);
    }

    #[test]
    fn test_indent_nesting_depth() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Test Python-style indentation (4 spaces per level)
        let python_code = r#"
def foo():
    if x:
        if y:
            do_something()
        else:
            other()
"#;
        let depth = chunker.calculate_indent_depth(python_code);
        assert!(depth >= 3, "Should detect indentation nesting, got {}", depth);

        // Test flat code
        let flat = "x = 1\ny = 2\n";
        assert!(chunker.calculate_indent_depth(flat) <= 1);
    }

    #[test]
    fn test_combined_nesting_depth() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        // Brace-based should win for languages like Rust
        let rust_code = "fn foo() { if x { match y { A => {}, B => {} } } }";
        let depth = chunker.calculate_nesting_depth(rust_code);
        assert!(depth >= 3, "Should use brace depth for Rust-like code");

        // Indent-based should win for Python-like code (few braces)
        let python_code = "def foo():\n    if x:\n        y()\n";
        let depth = chunker.calculate_nesting_depth(python_code);
        assert!(depth >= 1, "Should use indent depth for Python-like code");
    }

    #[test]
    fn test_lines_of_code() {
        let chunker = EmbedChunker::with_defaults(EmbedSettings::default());

        let code = r#"
// This is a comment
fn foo() {
    let x = 1;

    // Another comment
    let y = 2;
}
"#;
        let loc = chunker.count_lines_of_code(code);
        // Should count: fn foo() {, let x = 1;, let y = 2;, }
        // Should skip: empty lines and comments
        assert!((4..=5).contains(&loc), "LOC should be ~4, got {}", loc);
    }

    #[test]
    fn test_line_too_long_error() {
        let temp_dir = TempDir::new().unwrap();

        // Create a file with a very long line (simulating minified code)
        let long_line = "x".repeat(50_000);
        let content = format!("fn foo() {{ {} }}", long_line);
        create_test_file(temp_dir.path(), "minified.rs", &content);

        let settings = EmbedSettings::default();
        // Use strict line length limit
        let limits = ResourceLimits::default().with_max_line_length(10_000);
        let mut chunker = EmbedChunker::new(settings, limits);
        let progress = QuietProgress;

        let result = chunker.chunk_repository(temp_dir.path(), &progress);

        // Should fail due to line too long
        assert!(result.is_err(), "Should reject files with very long lines");
    }

    #[test]
    fn test_hierarchical_chunking_integration() {
        let temp_dir = TempDir::new().unwrap();

        // Create a Rust file with a struct that has multiple methods
        let rust_code = r#"
/// A user account
pub struct User {
    pub name: String,
    pub email: String,
}

impl User {
    /// Create a new user
    pub fn new(name: String, email: String) -> Self {
        Self { name, email }
    }

    /// Get the user's display name
    pub fn display_name(&self) -> &str {
        &self.name
    }

    /// Validate the user's email
    pub fn validate_email(&self) -> bool {
        self.email.contains('@')
    }
}
"#;
        create_test_file(temp_dir.path(), "user.rs", rust_code);

        // Test WITHOUT hierarchy
        let settings_no_hierarchy = EmbedSettings { enable_hierarchy: false, ..Default::default() };
        let mut chunker_no_hierarchy = EmbedChunker::with_defaults(settings_no_hierarchy);
        let progress = QuietProgress;
        let chunks_no_hierarchy = chunker_no_hierarchy
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        // Test WITH hierarchy
        let settings_with_hierarchy = EmbedSettings {
            enable_hierarchy: true,
            hierarchy_min_children: 2,
            ..Default::default()
        };
        let mut chunker_with_hierarchy = EmbedChunker::with_defaults(settings_with_hierarchy);
        let chunks_with_hierarchy = chunker_with_hierarchy
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        // Hierarchy should produce more chunks (original + summaries)
        assert!(
            chunks_with_hierarchy.len() >= chunks_no_hierarchy.len(),
            "Hierarchy should produce at least as many chunks: {} vs {}",
            chunks_with_hierarchy.len(),
            chunks_no_hierarchy.len()
        );

        // Check for ContainerSummary chunks when hierarchy is enabled
        let summary_chunks: Vec<_> = chunks_with_hierarchy
            .iter()
            .filter(|c| matches!(c.kind, ChunkKind::Module)) // Summary chunks use Module kind
            .collect();

        // If we have container types with enough children, we should have summaries
        // Note: This depends on the parser correctly identifying struct + impl methods
        if !summary_chunks.is_empty() {
            // Summary chunks should have content referencing children
            for summary in &summary_chunks {
                assert!(!summary.content.is_empty(), "Summary chunk should have content");
            }
        }

        // Verify determinism with hierarchy enabled
        let chunks_with_hierarchy_2 = chunker_with_hierarchy
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();
        assert_eq!(
            chunks_with_hierarchy.len(),
            chunks_with_hierarchy_2.len(),
            "Hierarchical chunking should be deterministic"
        );
        for (c1, c2) in chunks_with_hierarchy
            .iter()
            .zip(chunks_with_hierarchy_2.iter())
        {
            assert_eq!(c1.id, c2.id, "Chunk IDs should be identical across runs");
        }
    }

    #[test]
    fn test_duplicate_parent_names_link_child_to_containing_parent_only() {
        fn chunk(
            id: &str,
            kind: ChunkKind,
            symbol: &str,
            parent: Option<&str>,
            lines: (u32, u32),
        ) -> EmbedChunk {
            EmbedChunk {
                id: id.to_owned(),
                full_hash: format!("{id}_full"),
                content: symbol.to_owned(),
                tokens: 1,
                kind,
                source: ChunkSource {
                    repo: RepoIdentifier::default(),
                    file: "service.py".to_owned(),
                    lines,
                    symbol: symbol.to_owned(),
                    fqn: Some(format!("service::{symbol}")),
                    language: "Python".to_owned(),
                    parent: parent.map(str::to_owned),
                    visibility: Visibility::Public,
                    is_test: false,
                    module_path: Some("service".to_owned()),
                    parent_chunk_id: None,
                    line_byte_range: None,
                    content_transform: None,
                },
                context: ChunkContext::default(),
                children_ids: Vec::new(),
                repr: default_repr(),
                code_chunk_id: None,
                part: None,
            }
        }

        let mut chunks = vec![
            chunk("first_parent", ChunkKind::Class, "MockPerson", None, (1, 3)),
            chunk("second_parent", ChunkKind::Class, "MockPerson", None, (10, 12)),
            chunk(
                "child_method",
                ChunkKind::Method,
                "getRelativeUrl",
                Some("MockPerson"),
                (11, 12),
            ),
        ];

        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let progress = QuietProgress;
        let chunker = EmbedChunker::with_defaults(settings);
        chunker.link_parent_children(&mut chunks, &progress);

        let child = chunks
            .iter()
            .find(|chunk| chunk.id == "child_method")
            .expect("child should exist");
        assert_eq!(child.source.parent_chunk_id.as_deref(), Some("second_parent"));

        let first_parent = chunks
            .iter()
            .find(|chunk| chunk.id == "first_parent")
            .expect("first parent should exist");
        assert!(first_parent.children_ids.is_empty());

        let second_parent = chunks
            .iter()
            .find(|chunk| chunk.id == "second_parent")
            .expect("second parent should exist");
        assert_eq!(second_parent.children_ids, vec!["child_method".to_owned()]);
    }

    #[test]
    fn test_trim_blank_segment_edges_keeps_source_lines_current_fragment_only() {
        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let chunker = EmbedChunker::with_defaults(settings);
        let token_model = chunker.parse_token_model(&chunker.settings.token_model);
        let segment = BudgetedSegment {
            content: "useful line\n".to_owned(),
            start_line: 10,
            end_line: 11,
            tokens: 1,
            overlap_lines: 0,
            line_byte_range: None,
        };

        let trimmed = chunker
            .trim_blank_segment_edges(segment, token_model)
            .expect("non-empty segment should remain");

        assert_eq!(trimmed.content, "useful line");
        assert_eq!(trimmed.start_line, 10);
        assert_eq!(trimmed.end_line, 10);
    }

    #[test]
    fn test_python_future_import_is_import_chunk_not_top_level() {
        let temp_dir = TempDir::new().unwrap();
        create_test_file(
            temp_dir.path(),
            "pkg/module.py",
            "from __future__ import annotations\nimport os\n\n\ndef keep():\n    return os.getcwd()\n",
        );

        let settings = EmbedSettings {
            min_tokens: 1,
            context_lines: 0,
            include_top_level: true,
            scan_secrets: false,
            redact_secrets: false,
            ..Default::default()
        };
        let progress = QuietProgress;
        let mut chunker = EmbedChunker::with_defaults(settings);
        let chunks = chunker
            .chunk_repository(temp_dir.path(), &progress)
            .unwrap();

        let future_import = chunks
            .iter()
            .find(|chunk| chunk.content.trim() == "from __future__ import annotations")
            .expect("future import should be emitted");
        assert_eq!(future_import.kind, ChunkKind::Imports);
        assert!(
            chunks.iter().all(|chunk| {
                chunk.kind == ChunkKind::Imports
                    || !chunk.content.contains("from __future__ import annotations")
            }),
            "future import should not leak into top-level chunks: {chunks:#?}"
        );
    }

    #[test]
    fn test_python_fragment_calls_ignore_docstring_text() {
        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let chunker = EmbedChunker::with_defaults(settings);
        let content = "class TransData2D:\n    \"\"\"\n    This class manages:\n    - Memory allocation (GM buffers)\n    - Format handling (NHWC/NCHW)\n    \"\"\"";

        let calls = chunker.extract_local_calls(content, Some(Language::Python));

        assert!(calls.is_empty(), "docstring prose should not become calls: {calls:?}");
    }

    #[test]
    fn test_python_fragment_calls_ignore_unclosed_docstring_text() {
        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let chunker = EmbedChunker::with_defaults(settings);
        let content = "def order_at(a: PolyType) -> Union[int, float]:\n    \"\"\"\n    For a nonzero polynomial `a`, the order ν_p(a) is finite.\n\n    Parameters";

        let calls = chunker.extract_local_calls(content, Some(Language::Python));

        assert!(calls.is_empty(), "truncated docstring prose should not become calls: {calls:?}");
    }

    #[test]
    fn test_python_fragment_calls_dedent_method_body() {
        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let chunker = EmbedChunker::with_defaults(settings);
        let content = "    def compute(self):\n        helper()\n        print('debug')";

        let calls = chunker.extract_local_calls(content, Some(Language::Python));

        assert_eq!(calls, vec!["helper".to_owned()]);
    }

    #[test]
    fn test_python_fragment_calls_dedent_bare_body_statements() {
        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let chunker = EmbedChunker::with_defaults(settings);
        let content = "    later_call(0)\n    later_call(1)\n";

        let calls = chunker.extract_local_calls(content, Some(Language::Python));

        assert_eq!(calls, vec!["later_call".to_owned()]);
    }

    #[test]
    fn test_python_fragment_calls_filter_prose_and_builtins() {
        let settings =
            EmbedSettings { scan_secrets: false, redact_secrets: false, ..Default::default() };
        let chunker = EmbedChunker::with_defaults(settings);
        let content = r#"def build(values):
    # comment_only_call(values)
    text = "RuntimeError(fake_call())"
    if all(check_item(value) for value in values):
        raise ValueError("bad value")
    if any(callable(value) for value in values):
        return make_result(values)
"#;

        let calls = chunker.extract_local_calls(content, Some(Language::Python));

        assert_eq!(calls, vec!["check_item".to_owned(), "make_result".to_owned()]);
    }

    #[test]
    fn test_summary_from_docstring() {
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "src/auth/jwt.rs".to_owned(),
            lines: (10, 20),
            symbol: "verify_token".to_owned(),
            fqn: None,
            language: "Rust".to_owned(),
            parent: None,
            visibility: Visibility::Public,
            is_test: false,
            module_path: None,
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };
        let context = ChunkContext {
            docstring: Some("/// Verify a JWT token and return the claims.".to_owned()),
            signature: Some("pub fn verify_token(token: &str) -> Result<Claims>".to_owned()),
            ..Default::default()
        };

        let summary = generate_summary(ChunkKind::Function, &source, &context);
        assert_eq!(summary, Some("Verify a JWT token and return the claims.".to_owned()));
    }

    #[test]
    fn test_summary_heuristic_for_function() {
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "src/auth/jwt.rs".to_owned(),
            lines: (10, 20),
            symbol: "verify_token".to_owned(),
            fqn: None,
            language: "Rust".to_owned(),
            parent: None,
            visibility: Visibility::Public,
            is_test: false,
            module_path: None,
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };
        let context = ChunkContext {
            signature: Some("pub fn verify_token(token: &str) -> Result<Claims>".to_owned()),
            ..Default::default()
        };

        let summary = generate_summary(ChunkKind::Function, &source, &context);
        assert_eq!(
            summary,
            Some(
                "Public function 'verify_token' in auth::jwt -- pub fn verify_token(token: &str) -> Result<Claims>"
                    .to_owned()
            )
        );
    }

    #[test]
    fn test_summary_heuristic_for_struct() {
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "lib/models/user.py".to_owned(),
            lines: (1, 30),
            symbol: "User".to_owned(),
            fqn: None,
            language: "Python".to_owned(),
            parent: None,
            visibility: Visibility::Public,
            is_test: false,
            module_path: None,
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };
        let context = ChunkContext::default();

        let summary = generate_summary(ChunkKind::Class, &source, &context);
        assert_eq!(summary, Some("Public class 'User' in models::user".to_owned()));
    }

    #[test]
    fn test_summary_none_for_imports() {
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "src/lib.rs".to_owned(),
            lines: (1, 5),
            symbol: "<imports>".to_owned(),
            fqn: None,
            language: "Rust".to_owned(),
            parent: None,
            visibility: Visibility::Public,
            is_test: false,
            module_path: None,
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };
        let context = ChunkContext::default();

        let summary = generate_summary(ChunkKind::Imports, &source, &context);
        assert!(summary.is_none(), "Import chunks should not have a summary");
    }

    #[test]
    fn test_summary_long_signature_truncated() {
        let long_sig = format!(
            "pub fn process({})",
            (0..50)
                .map(|i| format!("arg{}: SomeVeryLongTypeName", i))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "src/processor.rs".to_owned(),
            lines: (1, 100),
            symbol: "process".to_owned(),
            fqn: None,
            language: "Rust".to_owned(),
            parent: None,
            visibility: Visibility::Private,
            is_test: false,
            module_path: None,
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };
        let context = ChunkContext { signature: Some(long_sig), ..Default::default() };

        let summary = generate_summary(ChunkKind::Function, &source, &context).unwrap();
        // The signature part should be truncated to ~200 chars
        assert!(summary.contains("..."), "Long signature should be truncated with ellipsis");
        // The total summary should still be reasonable length
        assert!(summary.len() < 350, "Summary should be concise, got len={}", summary.len());
    }

    #[test]
    fn test_summary_unicode_docstring_truncated_on_char_boundary() {
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "src/audio.c".to_owned(),
            lines: (1, 100),
            symbol: "pcm_RotateByte".to_owned(),
            fqn: None,
            language: "C".to_owned(),
            parent: None,
            visibility: Visibility::Private,
            is_test: false,
            module_path: None,
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };
        let docstring = format!(
            "/** {} */",
            "【機　能】 bsize_shift 分のバッファ中のデータの回転。".repeat(30)
        );
        let context = ChunkContext { docstring: Some(docstring), ..Default::default() };

        let summary = generate_summary(ChunkKind::Function, &source, &context).unwrap();
        assert!(summary.ends_with("..."));
        assert!(summary.len() <= 400);
    }

    #[test]
    fn test_summary_unicode_signature_truncated_on_char_boundary() {
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "characters/characterUtilite.c".to_owned(),
            lines: (1, 100),
            symbol: "CheckItemInBox".to_owned(),
            fqn: None,
            language: "C".to_owned(),
            parent: None,
            visibility: Visibility::Private,
            is_test: false,
            module_path: None,
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };
        let signature = "int CheckItemInBox(string _itemID, string _locationID, string _box) // Addon 2016-1 Jason подсчет указанных предметов в конкретном сундуке конкретной локации".repeat(4);
        let context = ChunkContext { signature: Some(signature), ..Default::default() };

        let summary = generate_summary(ChunkKind::Function, &source, &context).unwrap();
        assert!(summary.contains("..."));
        assert!(summary.len() < 350, "Summary should be concise, got len={}", summary.len());
    }

    #[test]
    fn test_summary_top_level() {
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "src/main.rs".to_owned(),
            lines: (1, 50),
            symbol: "<top_level>".to_owned(),
            fqn: None,
            language: "Rust".to_owned(),
            parent: None,
            visibility: Visibility::Public,
            is_test: false,
            module_path: None,
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };
        let context = ChunkContext::default();

        let summary = generate_summary(ChunkKind::TopLevel, &source, &context);
        assert_eq!(summary, Some("Top-level code in src/main.rs".to_owned()));
    }

    #[test]
    fn test_file_path_to_module() {
        assert_eq!(file_path_to_module("src/auth/jwt.rs"), "auth::jwt");
        assert_eq!(file_path_to_module("lib/models/user.py"), "models::user");
        assert_eq!(file_path_to_module("main/app.ts"), "app");
        assert_eq!(file_path_to_module("other/deep/path.go"), "other::deep::path");
    }

    #[test]
    fn test_strip_doc_markers() {
        assert_eq!(strip_doc_markers("/// Hello world"), "Hello world");
        assert_eq!(strip_doc_markers("//! Module doc"), "Module doc");
        assert_eq!(strip_doc_markers("/** Java doc */"), "Java doc");
        assert_eq!(strip_doc_markers("# Python doc"), "Python doc");
        assert_eq!(strip_doc_markers("\"\"\"Triple quoted\"\"\""), "Triple quoted");
        assert_eq!(strip_doc_markers("  * Javadoc line"), "Javadoc line");
        assert_eq!(strip_doc_markers("Plain text"), "Plain text");
    }

    #[test]
    fn test_summary_with_python_docstring() {
        let source = ChunkSource {
            repo: RepoIdentifier::default(),
            file: "src/utils.py".to_owned(),
            lines: (1, 10),
            symbol: "parse_config".to_owned(),
            fqn: None,
            language: "Python".to_owned(),
            parent: None,
            visibility: Visibility::Public,
            is_test: false,
            module_path: None,
            parent_chunk_id: None,
            line_byte_range: None,
            content_transform: None,
        };
        let context = ChunkContext {
            docstring: Some("\"\"\"Parse configuration from a YAML file.\"\"\"".to_owned()),
            ..Default::default()
        };

        let summary = generate_summary(ChunkKind::Function, &source, &context);
        assert_eq!(summary, Some("Parse configuration from a YAML file.".to_owned()));
    }
}
