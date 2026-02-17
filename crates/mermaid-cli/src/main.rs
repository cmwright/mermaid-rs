use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

mod args;
use args::CliArgs;

fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Initialize logging
    if args.verbose {
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Warn)
            .init();
    }

    // Read input
    let source = if args.input.to_str() == Some("-") {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read from stdin")?;
        buf
    } else {
        fs::read_to_string(&args.input)
            .with_context(|| format!("Failed to read input file: {}", args.input.display()))?
    };

    // Build render config
    let theme = mermaid_core::render::theme::Theme::by_name(&args.theme);

    let font_provider = if let Some(font_path) = &args.font {
        let font_data = fs::read(font_path)
            .with_context(|| format!("Failed to read font: {}", font_path.display()))?;
        mermaid_core::font::FontProvider::from_bytes(font_data)?
    } else {
        mermaid_core::font::FontProvider::default_font()
    };

    let config = mermaid_core::RenderConfig {
        theme,
        font_provider,
        output_format: mermaid_core::OutputFormat::Svg,
        width: args.width,
        background: None,
    };

    // Render
    let svg_output = mermaid_core::render(&source, &config)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Determine output path
    let output_path = args.output.unwrap_or_else(|| {
        let stem = args
            .input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        PathBuf::from(format!("{}.svg", stem))
    });

    // Write output
    fs::write(&output_path, &svg_output)
        .with_context(|| format!("Failed to write output: {}", output_path.display()))?;

    eprintln!("Rendered {} -> {}", args.input.display(), output_path.display());
    Ok(())
}
