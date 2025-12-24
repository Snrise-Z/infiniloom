//! Info command handler
//!
//! Shows version and configuration information.

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::config::load_config_file;

/// Show version and configuration info
pub fn cmd_info(path: Option<PathBuf>) -> Result<()> {
    println!();
    println!("{}", "Infiniloom - Repository Context Generator".cyan().bold());
    println!("{}", "━".repeat(50).dimmed());
    println!();
    println!("  Version:      {}", env!("CARGO_PKG_VERSION"));
    println!("  Engine:       {}", infiniloom_engine::VERSION);

    // Show project-specific info if path provided
    if let Some(ref project_path) = path {
        let resolved_path = if project_path.as_os_str().is_empty() {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        } else {
            project_path.clone()
        };
        println!();
        println!("  {}:", "Project".yellow());
        println!("    Path:       {}", resolved_path.display());

        // Check for config file
        let config = load_config_file(None, &resolved_path);
        if config.format.is_some() || config.model.is_some() || config.compression.is_some() {
            println!("    Config:     Found (.infiniloom.yaml/.toml/.json)");
            if let Some(fmt) = &config.format {
                println!("      Format:     {:?}", fmt);
            }
            if let Some(model) = &config.model {
                println!("      Model:      {:?}", model);
            }
            if let Some(comp) = &config.compression {
                println!("      Compression: {:?}", comp);
            }
            if let Some(budget) = config.token_budget {
                println!("      Budget:     {} tokens", budget);
            }
        } else {
            println!("    Config:     None (using defaults)");
        }
    }

    println!();
    println!("  {}:", "Supported Formats".yellow());
    println!("    xml       - Claude-optimized (with cache hints)");
    println!("    markdown  - GPT-optimized (with code blocks)");
    println!("    json      - Generic structured format");
    println!("    yaml      - Gemini-optimized (query at end)");
    println!("    toon      - Most token-efficient (~40% smaller)");
    println!("    plain     - Simple plain text (no markup)");
    println!();
    println!("  {}:", "Supported Models".yellow());
    println!("    claude      - Anthropic Claude (default)");
    println!("    gpt52       - OpenAI GPT-5.2 (o200k_base encoding)");
    println!("    gpt51       - OpenAI GPT-5.1 (o200k_base encoding)");
    println!("    gpt5        - OpenAI GPT-5 (o200k_base encoding)");
    println!("    o4-mini     - OpenAI O4-mini reasoning model");
    println!("    o3          - OpenAI O3 reasoning model");
    println!("    o1          - OpenAI O1 reasoning model");
    println!("    gpt4o       - OpenAI GPT-4o (o200k_base encoding)");
    println!("    gpt4o-mini  - OpenAI GPT-4o-mini (o200k_base encoding)");
    println!("    gpt4        - OpenAI GPT-4/GPT-4 Turbo (cl100k_base, legacy)");
    println!("    gpt35-turbo - OpenAI GPT-3.5-turbo (cl100k_base, legacy)");
    println!("    gemini      - Google Gemini");
    println!("    llama       - Meta Llama 3/4");
    println!("    codellama   - Meta CodeLlama (optimized for code)");
    println!("    mistral     - Mistral AI (Large, Medium, Codestral)");
    println!("    deepseek    - DeepSeek (V3, R1, Coder)");
    println!("    qwen        - Alibaba Qwen (Qwen3, Qwen2.5)");
    println!("    cohere      - Cohere (Command R+, Command R)");
    println!("    grok        - xAI Grok (Grok 2, Grok 3)");
    println!();
    println!("  {}:", "Compression Levels".yellow());
    println!("    none      - No compression (0%)");
    println!("    minimal   - Whitespace only (~15%)");
    println!("    balanced  - Remove comments (~35%)");
    println!("    aggressive - Signatures only (~60%)");
    println!("    extreme   - Key symbols only (~80%)");
    println!("    focused   - Key symbols with context (~75%)");
    println!("    semantic  - Heuristic chunking (~65%, NOT neural)");
    println!();

    Ok(())
}
