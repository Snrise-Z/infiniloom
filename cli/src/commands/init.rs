//! Init command handler
//!
//! Creates a new configuration file with project-specific templates.

use anyhow::{Context, Result};
use clap::ValueEnum;
use colored::Colorize;
use std::path::PathBuf;

/// Configuration file format
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ConfigFormat {
    /// YAML format
    Yaml,
    /// TOML format
    Toml,
    /// JSON format
    Json,
}

/// Configuration template for common project types
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ConfigTemplate {
    /// Generic template (default)
    Generic,
    /// Rust project (Cargo.toml, *.rs)
    Rust,
    /// Python project (*.py, requirements.txt)
    Python,
    /// TypeScript/JavaScript project (*.ts, *.tsx, package.json)
    Typescript,
    /// Go project (*.go, go.mod)
    Go,
    /// Java project (*.java, pom.xml/build.gradle)
    Java,
}

/// Initialize a new configuration file
pub fn cmd_init(
    path: PathBuf,
    format: ConfigFormat,
    template: ConfigTemplate,
    output: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let (ext, format_name) = match format {
        ConfigFormat::Yaml => ("yaml", "yaml"),
        ConfigFormat::Toml => ("toml", "toml"),
        ConfigFormat::Json => ("json", "json"),
    };

    // If explicit output path is given, use it; otherwise create in the given directory
    let output_path = output.unwrap_or_else(|| path.join(format!(".infiniloom.{}", ext)));

    // Check if file exists
    if output_path.exists() && !force {
        eprintln!(
            "{} Configuration file already exists: {}",
            "Error:".red().bold(),
            output_path.display()
        );
        eprintln!("Use --force to overwrite");
        std::process::exit(1);
    }

    // Generate config based on template
    let config_content = generate_template_config(format_name, template);

    // Write config file
    std::fs::write(&output_path, &config_content)
        .with_context(|| format!("Failed to write config file: {}", output_path.display()))?;

    let template_name = match template {
        ConfigTemplate::Generic => "generic",
        ConfigTemplate::Rust => "Rust",
        ConfigTemplate::Python => "Python",
        ConfigTemplate::Typescript => "TypeScript",
        ConfigTemplate::Go => "Go",
        ConfigTemplate::Java => "Java",
    };

    println!(
        "{} Created {} configuration file: {}",
        "✓".green(),
        template_name,
        output_path.display()
    );
    println!();
    println!("Edit this file to customize Infiniloom behavior.");
    println!("See https://toposlabs.ai/infiniloom/ for options.");

    Ok(())
}

/// Generate configuration content based on template and format
fn generate_template_config(format: &str, template: ConfigTemplate) -> String {
    match template {
        ConfigTemplate::Generic => infiniloom_engine::Config::generate_default(format),
        ConfigTemplate::Rust => generate_rust_template(format),
        ConfigTemplate::Python => generate_python_template(format),
        ConfigTemplate::Typescript => generate_typescript_template(format),
        ConfigTemplate::Go => generate_go_template(format),
        ConfigTemplate::Java => generate_java_template(format),
    }
}

/// Generate Rust project configuration template
fn generate_rust_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - Rust Project Template
# Documentation: https://toposlabs.ai/infiniloom/

output:
  format: xml
  model: claude
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # Rust source files and configuration
  include:
    - "*.rs"
    - "Cargo.toml"
    - "Cargo.lock"
    - "build.rs"
  # Exclude build artifacts and dependencies
  exclude:
    - "target/*"
    - "target/**"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
"#
        .to_owned(),
        "toml" => r#"# Infiniloom Configuration - Rust Project Template

[output]
format = "xml"
model = "claude"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.rs", "Cargo.toml", "Cargo.lock", "build.rs"]
exclude = ["target/*", "target/**"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_"]
"#
        .to_owned(),
        "json" => r#"{
  "output": {
    "format": "xml",
    "model": "claude",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.rs", "Cargo.toml", "Cargo.lock", "build.rs"],
    "exclude": ["target/*", "target/**"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_"]
  }
}"#
        .to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

/// Generate Python project configuration template
fn generate_python_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - Python Project Template
# Documentation: https://toposlabs.ai/infiniloom/

output:
  format: markdown
  model: gpt4o
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # Python source files and configuration
  include:
    - "*.py"
    - "*.pyi"
    - "requirements.txt"
    - "pyproject.toml"
    - "setup.py"
    - "setup.cfg"
    - "Pipfile"
  # Exclude virtual environments and cache
  exclude:
    - "venv/*"
    - ".venv/*"
    - "__pycache__/*"
    - "*.pyc"
    - ".pytest_cache/*"
    - ".mypy_cache/*"
    - "*.egg-info/*"
    - "dist/*"
    - "build/*"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
    - "localhost"
"#
        .to_owned(),
        "toml" => r#"# Infiniloom Configuration - Python Project Template

[output]
format = "markdown"
model = "gpt4o"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.py", "*.pyi", "requirements.txt", "pyproject.toml", "setup.py", "setup.cfg", "Pipfile"]
exclude = ["venv/*", ".venv/*", "__pycache__/*", "*.pyc", ".pytest_cache/*", ".mypy_cache/*", "*.egg-info/*", "dist/*", "build/*"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_", "localhost"]
"#
        .to_owned(),
        "json" => r#"{
  "output": {
    "format": "markdown",
    "model": "gpt4o",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.py", "*.pyi", "requirements.txt", "pyproject.toml", "setup.py", "setup.cfg", "Pipfile"],
    "exclude": ["venv/*", ".venv/*", "__pycache__/*", "*.pyc", ".pytest_cache/*", ".mypy_cache/*", "*.egg-info/*", "dist/*", "build/*"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_", "localhost"]
  }
}"#
        .to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

/// Generate TypeScript/JavaScript project configuration template
fn generate_typescript_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - TypeScript/JavaScript Project Template
# Documentation: https://toposlabs.ai/infiniloom/

output:
  format: xml
  model: claude
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # TypeScript/JavaScript source files and configuration
  include:
    - "*.ts"
    - "*.tsx"
    - "*.js"
    - "*.jsx"
    - "*.mjs"
    - "*.cjs"
    - "package.json"
    - "tsconfig.json"
    - "*.config.js"
    - "*.config.ts"
  # Exclude dependencies and build outputs
  exclude:
    - "node_modules/*"
    - "node_modules/**"
    - "dist/*"
    - "build/*"
    - ".next/*"
    - ".nuxt/*"
    - "coverage/*"
    - "*.test.ts"
    - "*.test.tsx"
    - "*.spec.ts"
    - "*.spec.tsx"
    - "*.min.js"
    - "*.bundle.js"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
    - "localhost"
    - "127.0.0.1"
"#
        .to_owned(),
        "toml" => r#"# Infiniloom Configuration - TypeScript/JavaScript Project Template

[output]
format = "xml"
model = "claude"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.ts", "*.tsx", "*.js", "*.jsx", "*.mjs", "*.cjs", "package.json", "tsconfig.json", "*.config.js", "*.config.ts"]
exclude = ["node_modules/*", "node_modules/**", "dist/*", "build/*", ".next/*", ".nuxt/*", "coverage/*", "*.test.ts", "*.test.tsx", "*.spec.ts", "*.spec.tsx", "*.min.js", "*.bundle.js"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_", "localhost", "127.0.0.1"]
"#
        .to_owned(),
        "json" => r#"{
  "output": {
    "format": "xml",
    "model": "claude",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.ts", "*.tsx", "*.js", "*.jsx", "*.mjs", "*.cjs", "package.json", "tsconfig.json", "*.config.js", "*.config.ts"],
    "exclude": ["node_modules/*", "node_modules/**", "dist/*", "build/*", ".next/*", ".nuxt/*", "coverage/*", "*.test.ts", "*.test.tsx", "*.spec.ts", "*.spec.tsx", "*.min.js", "*.bundle.js"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_", "localhost", "127.0.0.1"]
  }
}"#
        .to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

/// Generate Go project configuration template
fn generate_go_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - Go Project Template
# Documentation: https://toposlabs.ai/infiniloom/

output:
  format: xml
  model: claude
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # Go source files and configuration
  include:
    - "*.go"
    - "go.mod"
    - "go.sum"
  # Exclude vendor and build outputs
  exclude:
    - "vendor/*"
    - "vendor/**"
    - "*_test.go"
    - "testdata/*"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
    - "localhost"
"#
        .to_owned(),
        "toml" => r#"# Infiniloom Configuration - Go Project Template

[output]
format = "xml"
model = "claude"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.go", "go.mod", "go.sum"]
exclude = ["vendor/*", "vendor/**", "*_test.go", "testdata/*"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_", "localhost"]
"#
        .to_owned(),
        "json" => r#"{
  "output": {
    "format": "xml",
    "model": "claude",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.go", "go.mod", "go.sum"],
    "exclude": ["vendor/*", "vendor/**", "*_test.go", "testdata/*"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_", "localhost"]
  }
}"#
        .to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

/// Generate Java project configuration template
fn generate_java_template(format: &str) -> String {
    match format {
        "yaml" => r#"# Infiniloom Configuration - Java Project Template
# Documentation: https://toposlabs.ai/infiniloom/

output:
  format: xml
  model: claude
  compression: balanced
  line_numbers: true
  show_file_summary: true
  show_directory_structure: true

scan:
  # Java source files and build configuration
  include:
    - "*.java"
    - "pom.xml"
    - "build.gradle"
    - "build.gradle.kts"
    - "settings.gradle"
    - "settings.gradle.kts"
    - "gradle.properties"
  # Exclude build outputs and IDE files
  exclude:
    - "target/*"
    - "target/**"
    - "build/*"
    - "build/**"
    - ".gradle/*"
    - ".idea/*"
    - "*.class"
    - "*Test.java"
    - "*Tests.java"
    - "*IT.java"
  include_hidden: false
  include_tests: false
  include_docs: false

security:
  scan_secrets: true
  fail_on_secrets: false
  redact_secrets: true
  allowlist:
    - "EXAMPLE_"
    - "test_"
    - "localhost"
"#
        .to_owned(),
        "toml" => r#"# Infiniloom Configuration - Java Project Template

[output]
format = "xml"
model = "claude"
compression = "balanced"
line_numbers = true
show_file_summary = true
show_directory_structure = true

[scan]
include = ["*.java", "pom.xml", "build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts", "gradle.properties"]
exclude = ["target/*", "target/**", "build/*", "build/**", ".gradle/*", ".idea/*", "*.class", "*Test.java", "*Tests.java", "*IT.java"]
include_hidden = false
include_tests = false
include_docs = false

[security]
scan_secrets = true
fail_on_secrets = false
redact_secrets = true
allowlist = ["EXAMPLE_", "test_", "localhost"]
"#
        .to_owned(),
        "json" => r#"{
  "output": {
    "format": "xml",
    "model": "claude",
    "compression": "balanced",
    "line_numbers": true,
    "show_file_summary": true,
    "show_directory_structure": true
  },
  "scan": {
    "include": ["*.java", "pom.xml", "build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts", "gradle.properties"],
    "exclude": ["target/*", "target/**", "build/*", "build/**", ".gradle/*", ".idea/*", "*.class", "*Test.java", "*Tests.java", "*IT.java"],
    "include_hidden": false,
    "include_tests": false,
    "include_docs": false
  },
  "security": {
    "scan_secrets": true,
    "fail_on_secrets": false,
    "redact_secrets": true,
    "allowlist": ["EXAMPLE_", "test_", "localhost"]
  }
}"#
        .to_owned(),
        _ => infiniloom_engine::Config::generate_default(format),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // Template Generation Tests - YAML Format
    // ============================================

    #[test]
    fn test_rust_template_yaml() {
        let template = generate_rust_template("yaml");
        assert!(template.contains("# Infiniloom Configuration - Rust Project Template"));
        assert!(template.contains("\"*.rs\""));
        assert!(template.contains("\"Cargo.toml\""));
        assert!(template.contains("\"target/*\""));
        assert!(template.contains("format: xml"));
        assert!(template.contains("model: claude"));
        assert!(template.contains("scan_secrets: true"));
    }

    #[test]
    fn test_python_template_yaml() {
        let template = generate_python_template("yaml");
        assert!(template.contains("# Infiniloom Configuration - Python Project Template"));
        assert!(template.contains("\"*.py\""));
        assert!(template.contains("\"*.pyi\""));
        assert!(template.contains("\"requirements.txt\""));
        assert!(template.contains("\"pyproject.toml\""));
        assert!(template.contains("\"venv/*\""));
        assert!(template.contains("\"__pycache__/*\""));
        assert!(template.contains("format: markdown"));
        assert!(template.contains("model: gpt4o"));
    }

    #[test]
    fn test_typescript_template_yaml() {
        let template = generate_typescript_template("yaml");
        assert!(template
            .contains("# Infiniloom Configuration - TypeScript/JavaScript Project Template"));
        assert!(template.contains("\"*.ts\""));
        assert!(template.contains("\"*.tsx\""));
        assert!(template.contains("\"*.js\""));
        assert!(template.contains("\"package.json\""));
        assert!(template.contains("\"node_modules/*\""));
        assert!(template.contains("\"dist/*\""));
        assert!(template.contains("\"*.test.ts\""));
    }

    #[test]
    fn test_go_template_yaml() {
        let template = generate_go_template("yaml");
        assert!(template.contains("# Infiniloom Configuration - Go Project Template"));
        assert!(template.contains("\"*.go\""));
        assert!(template.contains("\"go.mod\""));
        assert!(template.contains("\"go.sum\""));
        assert!(template.contains("\"vendor/*\""));
        assert!(template.contains("\"*_test.go\""));
    }

    #[test]
    fn test_java_template_yaml() {
        let template = generate_java_template("yaml");
        assert!(template.contains("# Infiniloom Configuration - Java Project Template"));
        assert!(template.contains("\"*.java\""));
        assert!(template.contains("\"pom.xml\""));
        assert!(template.contains("\"build.gradle\""));
        assert!(template.contains("\"target/*\""));
        assert!(template.contains("\".gradle/*\""));
        assert!(template.contains("\"*Test.java\""));
    }

    // ============================================
    // Template Generation Tests - TOML Format
    // ============================================

    #[test]
    fn test_rust_template_toml() {
        let template = generate_rust_template("toml");
        assert!(template.contains("# Infiniloom Configuration - Rust Project Template"));
        assert!(template.contains("[output]"));
        assert!(template.contains("[scan]"));
        assert!(template.contains("[security]"));
        assert!(template.contains("format = \"xml\""));
        assert!(template.contains("include = [\"*.rs\""));
        assert!(template.contains("exclude = [\"target/*\""));
    }

    #[test]
    fn test_python_template_toml() {
        let template = generate_python_template("toml");
        assert!(template.contains("# Infiniloom Configuration - Python Project Template"));
        assert!(template.contains("format = \"markdown\""));
        assert!(template.contains("model = \"gpt4o\""));
        assert!(template.contains("\"*.py\""));
        assert!(template.contains("\"venv/*\""));
    }

    #[test]
    fn test_typescript_template_toml() {
        let template = generate_typescript_template("toml");
        assert!(template
            .contains("# Infiniloom Configuration - TypeScript/JavaScript Project Template"));
        assert!(template.contains("\"*.ts\""));
        assert!(template.contains("\"node_modules/*\""));
    }

    #[test]
    fn test_go_template_toml() {
        let template = generate_go_template("toml");
        assert!(template.contains("# Infiniloom Configuration - Go Project Template"));
        assert!(template.contains("\"*.go\""));
        assert!(template.contains("\"vendor/*\""));
    }

    #[test]
    fn test_java_template_toml() {
        let template = generate_java_template("toml");
        assert!(template.contains("# Infiniloom Configuration - Java Project Template"));
        assert!(template.contains("\"*.java\""));
        assert!(template.contains("\"pom.xml\""));
    }

    // ============================================
    // Template Generation Tests - JSON Format
    // ============================================

    #[test]
    fn test_rust_template_json() {
        let template = generate_rust_template("json");
        // Verify it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&template).expect("Invalid JSON");
        assert_eq!(parsed["output"]["format"], "xml");
        assert_eq!(parsed["output"]["model"], "claude");
        assert!(parsed["scan"]["include"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "*.rs"));
        assert!(parsed["scan"]["exclude"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "target/*"));
    }

    #[test]
    fn test_python_template_json() {
        let template = generate_python_template("json");
        let parsed: serde_json::Value = serde_json::from_str(&template).expect("Invalid JSON");
        assert_eq!(parsed["output"]["format"], "markdown");
        assert_eq!(parsed["output"]["model"], "gpt4o");
        assert!(parsed["scan"]["include"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "*.py"));
    }

    #[test]
    fn test_typescript_template_json() {
        let template = generate_typescript_template("json");
        let parsed: serde_json::Value = serde_json::from_str(&template).expect("Invalid JSON");
        assert!(parsed["scan"]["include"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "*.ts"));
        assert!(parsed["scan"]["exclude"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "node_modules/*"));
    }

    #[test]
    fn test_go_template_json() {
        let template = generate_go_template("json");
        let parsed: serde_json::Value = serde_json::from_str(&template).expect("Invalid JSON");
        assert!(parsed["scan"]["include"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "*.go"));
    }

    #[test]
    fn test_java_template_json() {
        let template = generate_java_template("json");
        let parsed: serde_json::Value = serde_json::from_str(&template).expect("Invalid JSON");
        assert!(parsed["scan"]["include"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "*.java"));
    }

    // ============================================
    // Template Generation Tests - Unknown Format
    // ============================================

    #[test]
    fn test_rust_template_unknown_format() {
        let template = generate_rust_template("unknown");
        // Should fall back to engine's default
        assert!(!template.is_empty());
    }

    #[test]
    fn test_python_template_unknown_format() {
        let template = generate_python_template("unknown");
        assert!(!template.is_empty());
    }

    #[test]
    fn test_typescript_template_unknown_format() {
        let template = generate_typescript_template("unknown");
        assert!(!template.is_empty());
    }

    #[test]
    fn test_go_template_unknown_format() {
        let template = generate_go_template("unknown");
        assert!(!template.is_empty());
    }

    #[test]
    fn test_java_template_unknown_format() {
        let template = generate_java_template("unknown");
        assert!(!template.is_empty());
    }

    // ============================================
    // generate_template_config Tests
    // ============================================

    #[test]
    fn test_generate_template_config_generic() {
        let config = generate_template_config("yaml", ConfigTemplate::Generic);
        assert!(!config.is_empty());
        // Generic should use engine's default
    }

    #[test]
    fn test_generate_template_config_rust() {
        let config = generate_template_config("yaml", ConfigTemplate::Rust);
        assert!(config.contains("Rust"));
        assert!(config.contains("*.rs"));
    }

    #[test]
    fn test_generate_template_config_python() {
        let config = generate_template_config("toml", ConfigTemplate::Python);
        assert!(config.contains("Python"));
        assert!(config.contains("*.py"));
    }

    #[test]
    fn test_generate_template_config_typescript() {
        let config = generate_template_config("json", ConfigTemplate::Typescript);
        assert!(config.contains("*.ts"));
    }

    #[test]
    fn test_generate_template_config_go() {
        let config = generate_template_config("yaml", ConfigTemplate::Go);
        assert!(config.contains("Go"));
        assert!(config.contains("*.go"));
    }

    #[test]
    fn test_generate_template_config_java() {
        let config = generate_template_config("toml", ConfigTemplate::Java);
        assert!(config.contains("Java"));
        assert!(config.contains("*.java"));
    }

    // ============================================
    // cmd_init Tests
    // ============================================

    #[test]
    fn test_cmd_init_creates_yaml_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = cmd_init(
            temp_dir.path().to_path_buf(),
            ConfigFormat::Yaml,
            ConfigTemplate::Rust,
            None,
            false,
        );
        assert!(result.is_ok());

        let config_path = temp_dir.path().join(".infiniloom.yaml");
        assert!(config_path.exists());

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("Rust"));
    }

    #[test]
    fn test_cmd_init_creates_toml_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = cmd_init(
            temp_dir.path().to_path_buf(),
            ConfigFormat::Toml,
            ConfigTemplate::Python,
            None,
            false,
        );
        assert!(result.is_ok());

        let config_path = temp_dir.path().join(".infiniloom.toml");
        assert!(config_path.exists());
    }

    #[test]
    fn test_cmd_init_creates_json_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = cmd_init(
            temp_dir.path().to_path_buf(),
            ConfigFormat::Json,
            ConfigTemplate::Typescript,
            None,
            false,
        );
        assert!(result.is_ok());

        let config_path = temp_dir.path().join(".infiniloom.json");
        assert!(config_path.exists());

        // Verify it's valid JSON
        let content = std::fs::read_to_string(&config_path).unwrap();
        let _: serde_json::Value = serde_json::from_str(&content).expect("Invalid JSON created");
    }

    #[test]
    fn test_cmd_init_custom_output_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let custom_path = temp_dir.path().join("custom-config.yaml");

        let result = cmd_init(
            temp_dir.path().to_path_buf(),
            ConfigFormat::Yaml,
            ConfigTemplate::Go,
            Some(custom_path.clone()),
            false,
        );
        assert!(result.is_ok());
        assert!(custom_path.exists());
    }

    #[test]
    fn test_cmd_init_force_overwrites() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join(".infiniloom.yaml");

        // Create initial file
        std::fs::write(&config_path, "original content").unwrap();

        // Run with force
        let result = cmd_init(
            temp_dir.path().to_path_buf(),
            ConfigFormat::Yaml,
            ConfigTemplate::Java,
            None,
            true,
        );
        assert!(result.is_ok());

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("Java"));
        assert!(!content.contains("original content"));
    }

    #[test]
    fn test_cmd_init_all_templates_yaml() {
        let templates = [
            ConfigTemplate::Generic,
            ConfigTemplate::Rust,
            ConfigTemplate::Python,
            ConfigTemplate::Typescript,
            ConfigTemplate::Go,
            ConfigTemplate::Java,
        ];

        for template in templates {
            let temp_dir = tempfile::tempdir().unwrap();
            let result =
                cmd_init(temp_dir.path().to_path_buf(), ConfigFormat::Yaml, template, None, false);
            assert!(result.is_ok(), "Failed for template {:?}", template);
        }
    }

    #[test]
    fn test_cmd_init_all_formats() {
        let formats = [ConfigFormat::Yaml, ConfigFormat::Toml, ConfigFormat::Json];

        for format in formats {
            let temp_dir = tempfile::tempdir().unwrap();
            let result =
                cmd_init(temp_dir.path().to_path_buf(), format, ConfigTemplate::Rust, None, false);
            assert!(result.is_ok(), "Failed for format {:?}", format);
        }
    }
}
