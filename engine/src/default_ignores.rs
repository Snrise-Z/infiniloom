//! Default ignore patterns for Infiniloom
//!
//! These patterns are applied by default to exclude common non-essential files
//! that waste tokens without adding value for LLM context.

/// Default patterns to ignore (dependencies, build outputs, etc.)
pub const DEFAULT_IGNORES: &[&str] = &[
    // === Dependencies ===
    "node_modules/**",
    "vendor/**",
    ".venv/**",
    "venv/**",
    "__pycache__/**",
    ".cache/**",
    ".npm/**",
    ".yarn/**",
    ".pnpm/**",
    "bower_components/**",
    "jspm_packages/**",
    // === Build outputs ===
    "dist/**",
    "build/**",
    "out/**",
    "target/**",
    "_build/**",
    ".next/**",
    ".nuxt/**",
    ".output/**",
    ".svelte-kit/**",
    ".vercel/**",
    ".netlify/**",
    // === Minified/bundled files ===
    "*.min.js",
    "*.min.css",
    "*.bundle.js",
    "*.chunk.js",
    "*.min.map",
    // === Generated code ===
    "*.generated.*",
    "*.pb.go",
    "*_generated.go",
    "*.g.dart",
    "generated/**",
    "*.gen.ts",
    "*.gen.js",
    "__generated__/**",
    // === Lock files (large, not useful for understanding) ===
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "poetry.lock",
    "Gemfile.lock",
    "composer.lock",
    "Pipfile.lock",
    "bun.lockb",
    "flake.lock",
    // === Assets (binary or not code) ===
    "*.svg",
    "*.png",
    "*.jpg",
    "*.jpeg",
    "*.gif",
    "*.ico",
    "*.webp",
    "*.avif",
    "*.bmp",
    "*.tiff",
    "*.psd",
    "*.ai",
    "*.sketch",
    "*.fig",
    "*.woff",
    "*.woff2",
    "*.ttf",
    "*.eot",
    "*.otf",
    "*.mp3",
    "*.mp4",
    "*.wav",
    "*.ogg",
    "*.webm",
    "*.mov",
    "*.avi",
    "*.mkv",
    "*.flv",
    "*.pdf",
    // === IDE/Editor ===
    ".idea/**",
    ".vscode/**",
    "*.swp",
    "*.swo",
    "*~",
    ".DS_Store",
    "Thumbs.db",
    "*.iml",
    // === Coverage/Reports ===
    "coverage/**",
    ".nyc_output/**",
    "htmlcov/**",
    ".coverage",
    "lcov.info",
    "*.lcov",
    // === Logs ===
    "*.log",
    "logs/**",
    "npm-debug.log*",
    "yarn-debug.log*",
    "yarn-error.log*",
    // === Temporary files ===
    "tmp/**",
    "temp/**",
    ".tmp/**",
    ".temp/**",
    // === Database files ===
    "*.db",
    "*.sqlite",
    "*.sqlite3",
    // === Large data files ===
    "*.csv",
    "*.parquet",
    "*.arrow",
    "*.feather",
    // === Snapshots (usually large, auto-generated) ===
    "__snapshots__/**",
    "*.snap",
    // === Type definition bundles ===
    "*.d.ts.map",
    // === WASM ===
    "*.wasm",
    // === Compiled Python ===
    "*.pyc",
    "*.pyo",
    "*.pyd",
    // === Misc ===
    ".git/**",
    ".hg/**",
    ".svn/**",
    ".env",
    ".env.*",
    "*.bak",
    "*.backup",
];

/// Patterns for test files (can be optionally excluded)
pub const TEST_IGNORES: &[&str] = &[
    "**/test/**",
    "**/tests/**",
    "**/__tests__/**",
    "**/spec/**",
    "**/specs/**",
    "**/*_test.*",
    "**/*.test.*",
    "**/*.spec.*",
    "**/*.fixture.*",
    "**/*_fixture.*",
    "**/test_*.*",
    "**/conftest.py",
    "**/fixtures/**",
    "**/mocks/**",
    "**/__mocks__/**",
    "**/__fixtures__/**",
    "**/testdata/**",
    "**/test-data/**",
    "**/*_test/**",
    "**/*.stories.*",
    "**/*.story.*",
    // E2E and integration test patterns
    "**/e2e/**",
    "**/integration/**",
    "**/cypress/**",
    "**/playwright/**",
];

/// Patterns for documentation (can be optionally excluded)
pub const DOC_IGNORES: &[&str] = &[
    "docs/**",
    "doc/**",
    "documentation/**",
    "*.md",
    "*.mdx",
    "*.rst",
    "CHANGELOG*",
    "HISTORY*",
    "AUTHORS*",
    "CONTRIBUTORS*",
    "CONTRIBUTING*",
    "CODE_OF_CONDUCT*",
];

/// Check if a path matches any of the given glob patterns
pub fn matches_any(path: &str, patterns: &[&str]) -> bool {
    for pattern in patterns {
        if let Ok(glob) = glob::Pattern::new(pattern) {
            if glob.matches(path) {
                return true;
            }
        }
        // Also check if pattern matches any path component
        if let Some(suffix) = pattern.strip_prefix("**/") {
            if let Ok(glob) = glob::Pattern::new(suffix) {
                // Check against each component and suffix of path
                for (i, _) in path.match_indices('/') {
                    if glob.matches(&path[i + 1..]) {
                        return true;
                    }
                }
                if glob.matches(path) {
                    return true;
                }
            }
        }
    }
    false
}

/// Filter files based on default ignore patterns
pub fn filter_default_ignores<'a>(
    files: impl Iterator<Item = &'a str>,
    include_tests: bool,
    include_docs: bool,
) -> Vec<&'a str> {
    files
        .filter(|path| {
            // Always apply default ignores
            if matches_any(path, DEFAULT_IGNORES) {
                return false;
            }

            // Optionally filter tests
            if !include_tests && matches_any(path, TEST_IGNORES) {
                return false;
            }

            // Optionally filter docs
            if !include_docs && matches_any(path, DOC_IGNORES) {
                return false;
            }

            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ignores() {
        assert!(matches_any("node_modules/foo/bar.js", DEFAULT_IGNORES));
        assert!(matches_any("dist/bundle.js", DEFAULT_IGNORES));
        assert!(matches_any("package-lock.json", DEFAULT_IGNORES));
        assert!(matches_any("foo.min.js", DEFAULT_IGNORES));
        assert!(matches_any("generated/types.ts", DEFAULT_IGNORES));

        assert!(!matches_any("src/index.ts", DEFAULT_IGNORES));
        assert!(!matches_any("lib/utils.py", DEFAULT_IGNORES));
    }

    #[test]
    fn test_test_ignores() {
        assert!(matches_any("src/__tests__/foo.test.ts", TEST_IGNORES));
        assert!(matches_any("tests/unit/test_foo.py", TEST_IGNORES));
        assert!(matches_any("spec/models/user_spec.rb", TEST_IGNORES));

        // Fixture file patterns (issue: .fixture.go files appearing in results)
        assert!(matches_any("pkg/tools/ReadFile.fixture.go", TEST_IGNORES));
        assert!(matches_any("internal/something_fixture.ts", TEST_IGNORES));
        assert!(matches_any("src/api.fixture.json", TEST_IGNORES));

        // E2E and integration patterns
        assert!(matches_any("e2e/login.spec.ts", TEST_IGNORES));
        assert!(matches_any("cypress/integration/app.cy.ts", TEST_IGNORES));
        assert!(matches_any("playwright/tests/smoke.ts", TEST_IGNORES));

        assert!(!matches_any("src/index.ts", TEST_IGNORES));
    }

    #[test]
    fn test_filter() {
        let files = vec![
            "src/index.ts",
            "src/utils.ts",
            "node_modules/foo/index.js",
            "tests/test_main.py",
            "docs/README.md",
            "package-lock.json",
        ];

        let filtered = filter_default_ignores(files.into_iter(), false, true);
        assert_eq!(filtered, vec!["src/index.ts", "src/utils.ts", "docs/README.md"]);
    }
}
