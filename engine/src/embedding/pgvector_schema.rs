//! pgvector-optimized PostgreSQL schema generation
//!
//! Generates a complete SQL schema for storing embedding chunks in PostgreSQL
//! with pgvector, pg_trgm, full-text search, and hybrid search support.
//!
//! # Usage
//!
//! ```rust
//! use infiniloom_engine::embedding::pgvector_schema::generate_pgvector_schema;
//!
//! let schema = generate_pgvector_schema(1536);
//! println!("{schema}");
//! ```

/// Default embedding vector dimensions (OpenAI text-embedding-3-small)
pub const DEFAULT_EMBEDDING_DIMS: u32 = 1536;

/// Minimum supported embedding dimensions
pub const MIN_EMBEDDING_DIMS: u32 = 2;

/// Maximum supported embedding dimensions (pgvector limit)
pub const MAX_EMBEDDING_DIMS: u32 = 16000;

/// Generate a complete pgvector-optimized PostgreSQL schema.
///
/// The schema includes:
/// - Extensions: `vector` and `pg_trgm`
/// - Main `chunks` table with all fields from `EmbedChunk`/`ChunkSource`/`ChunkContext`
/// - IVFFlat vector index for similarity search
/// - Full-text search via generated `tsvector` column
/// - Trigram indexes for fuzzy text matching
/// - Filtering indexes for common query patterns
/// - `hybrid_search()` function combining vector + text search
///
/// # Arguments
///
/// * `embedding_dims` - Dimension of the embedding vectors (e.g., 1536 for OpenAI, 1024 for Voyage)
///
/// # Panics
///
/// Does not panic. Returns valid SQL for any `u32` dimension value.
/// Production users should validate dimensions are within `MIN_EMBEDDING_DIMS..=MAX_EMBEDDING_DIMS`.
pub fn generate_pgvector_schema(embedding_dims: u32) -> String {
    format!(
        r#"-- Infiniloom pgvector schema
-- Generated for embedding dimension: {dims}
-- https://github.com/infiniloom/infiniloom

-- ============================================================
-- 1. Extensions
-- ============================================================

CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- ============================================================
-- 2. Main chunks table
-- ============================================================

CREATE TABLE IF NOT EXISTS chunks (
    -- Identity
    id                  TEXT PRIMARY KEY,
    full_hash           TEXT NOT NULL,
    content             TEXT NOT NULL,
    embedding           vector({dims}),
    tokens              INTEGER NOT NULL,
    kind                TEXT NOT NULL,
    repr                TEXT DEFAULT 'code',

    -- Source location
    repo_namespace      TEXT,
    repo_name           TEXT,
    file                TEXT NOT NULL,
    lines_start         INTEGER,
    lines_end           INTEGER,
    symbol              TEXT,
    fqn                 TEXT,
    language            TEXT,
    parent_symbol       TEXT,
    parent_chunk_id     TEXT,
    visibility          TEXT,
    module_path         TEXT,
    is_test             BOOLEAN DEFAULT false,

    -- Context (retrieval enhancement)
    summary             TEXT,
    docstring           TEXT,
    signature           TEXT,
    type_signature      TEXT,
    return_type         TEXT,
    parameter_types     TEXT[],
    error_types         TEXT[],
    calls               TEXT[],
    called_by           TEXT[],
    qualified_calls     TEXT[],
    unresolved_calls    TEXT[],
    imports             TEXT[],
    tags                TEXT[],
    identifiers         TEXT,
    keywords            TEXT[],
    side_effects        TEXT[],

    -- Metrics
    complexity_score    INTEGER,
    lines_of_code       INTEGER,
    max_nesting_depth   INTEGER,
    dependents_count    INTEGER,

    -- Hierarchy
    children_ids        TEXT[],
    code_chunk_id       TEXT,

    -- Git metadata
    git_last_modified   TIMESTAMPTZ,
    git_change_frequency INTEGER,
    git_total_commits   INTEGER,
    git_authors         TEXT[],
    git_age_days        INTEGER,

    -- Housekeeping
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW()
);

-- ============================================================
-- 3. Vector index (IVFFlat, cosine distance)
-- ============================================================

CREATE INDEX IF NOT EXISTS idx_chunks_embedding
    ON chunks USING ivfflat (embedding vector_cosine_ops)
    WITH (lists = 1000);

-- ============================================================
-- 4. Full-text search
-- ============================================================

ALTER TABLE chunks
    ADD COLUMN IF NOT EXISTS identifiers_tsvector TSVECTOR
    GENERATED ALWAYS AS (to_tsvector('simple', COALESCE(identifiers, ''))) STORED;

CREATE INDEX IF NOT EXISTS idx_chunks_fts
    ON chunks USING GIN (identifiers_tsvector);

-- ============================================================
-- 5. Trigram indexes (fuzzy text matching)
-- ============================================================

CREATE INDEX IF NOT EXISTS idx_chunks_summary_trgm
    ON chunks USING GIN (summary gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_chunks_fqn_trgm
    ON chunks USING GIN (fqn gin_trgm_ops);

-- ============================================================
-- 6. Filtering indexes
-- ============================================================

CREATE INDEX IF NOT EXISTS idx_chunks_repo
    ON chunks (repo_namespace, repo_name);

CREATE INDEX IF NOT EXISTS idx_chunks_file
    ON chunks (file);

CREATE INDEX IF NOT EXISTS idx_chunks_kind
    ON chunks (kind);

CREATE INDEX IF NOT EXISTS idx_chunks_language
    ON chunks (language);

CREATE INDEX IF NOT EXISTS idx_chunks_module
    ON chunks USING btree (module_path text_pattern_ops);

CREATE INDEX IF NOT EXISTS idx_chunks_repr
    ON chunks (repr);

CREATE INDEX IF NOT EXISTS idx_chunks_tags
    ON chunks USING GIN (tags);

CREATE INDEX IF NOT EXISTS idx_chunks_calls
    ON chunks USING GIN (calls);

-- ============================================================
-- 7. Hybrid search function (vector + full-text)
-- ============================================================

CREATE OR REPLACE FUNCTION hybrid_search(
    query_embedding vector({dims}),
    query_text TEXT,
    result_limit INTEGER DEFAULT 20
)
RETURNS TABLE (chunk_id TEXT, content TEXT, file TEXT, symbol TEXT, score FLOAT)
AS $$
    SELECT id, content, file, symbol,
        (embedding <=> query_embedding) * 0.6
        - ts_rank(identifiers_tsvector, plainto_tsquery('simple', query_text)) * 0.4
        AS score
    FROM chunks
    WHERE repr = 'code'
      AND (identifiers_tsvector @@ plainto_tsquery('simple', query_text)
           OR (embedding <=> query_embedding) < 0.5)
    ORDER BY score
    LIMIT result_limit;
$$ LANGUAGE SQL;
"#,
        dims = embedding_dims
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_schema_contains_extensions() {
        let schema = generate_pgvector_schema(1536);
        assert!(schema.contains("CREATE EXTENSION IF NOT EXISTS vector;"));
        assert!(schema.contains("CREATE EXTENSION IF NOT EXISTS pg_trgm;"));
    }

    #[test]
    fn test_generate_schema_contains_table() {
        let schema = generate_pgvector_schema(1536);
        assert!(schema.contains("CREATE TABLE IF NOT EXISTS chunks"));
        assert!(schema.contains("id                  TEXT PRIMARY KEY"));
        assert!(schema.contains("embedding           vector(1536)"));
    }

    #[test]
    fn test_generate_schema_custom_dims() {
        let schema = generate_pgvector_schema(768);
        assert!(schema.contains("vector(768)"));
        // Should appear in table, index, and function
        assert_eq!(schema.matches("vector(768)").count(), 3);
    }

    #[test]
    fn test_generate_schema_contains_indexes() {
        let schema = generate_pgvector_schema(1536);
        assert!(schema.contains("idx_chunks_embedding"));
        assert!(schema.contains("idx_chunks_fts"));
        assert!(schema.contains("idx_chunks_summary_trgm"));
        assert!(schema.contains("idx_chunks_fqn_trgm"));
        assert!(schema.contains("idx_chunks_repo"));
        assert!(schema.contains("idx_chunks_file"));
        assert!(schema.contains("idx_chunks_kind"));
        assert!(schema.contains("idx_chunks_language"));
        assert!(schema.contains("idx_chunks_module"));
        assert!(schema.contains("idx_chunks_repr"));
        assert!(schema.contains("idx_chunks_tags"));
        assert!(schema.contains("idx_chunks_calls"));
    }

    #[test]
    fn test_generate_schema_contains_hybrid_search() {
        let schema = generate_pgvector_schema(1536);
        assert!(schema.contains("CREATE OR REPLACE FUNCTION hybrid_search"));
        assert!(schema.contains("vector_cosine_ops"));
        assert!(schema.contains("plainto_tsquery"));
    }

    #[test]
    fn test_generate_schema_contains_all_chunk_fields() {
        let schema = generate_pgvector_schema(1536);
        // Source fields
        assert!(schema.contains("repo_namespace"));
        assert!(schema.contains("repo_name"));
        assert!(schema.contains("file"));
        assert!(schema.contains("lines_start"));
        assert!(schema.contains("symbol"));
        assert!(schema.contains("fqn"));
        assert!(schema.contains("language"));
        assert!(schema.contains("parent_symbol"));
        assert!(schema.contains("visibility"));
        assert!(schema.contains("is_test"));
        // Context fields
        assert!(schema.contains("docstring"));
        assert!(schema.contains("signature"));
        assert!(schema.contains("calls"));
        assert!(schema.contains("called_by"));
        assert!(schema.contains("imports"));
        assert!(schema.contains("tags"));
        assert!(schema.contains("keywords"));
        assert!(schema.contains("lines_of_code"));
        assert!(schema.contains("max_nesting_depth"));
        // Git fields
        assert!(schema.contains("git_last_modified"));
        assert!(schema.contains("git_authors"));
    }

    #[test]
    fn test_generate_schema_contains_tsvector() {
        let schema = generate_pgvector_schema(1536);
        assert!(schema.contains("identifiers_tsvector TSVECTOR"));
        assert!(schema.contains("GENERATED ALWAYS AS"));
    }
}
