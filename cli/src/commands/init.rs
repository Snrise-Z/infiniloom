//! Init command handler
//!
//! Creates a new configuration file with project-specific templates.

use anyhow::{Context, Result};
use clap::ValueEnum;
use colored::Colorize;
use std::path::PathBuf;

/// Configuration file format
#[derive(ValueEnum, Clone, Copy)]
pub enum ConfigFormat {
    /// YAML format
    Yaml,
    /// TOML format
    Toml,
    /// JSON format
    Json,
}

/// Configuration template for common project types
#[derive(ValueEnum, Clone, Copy)]
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
