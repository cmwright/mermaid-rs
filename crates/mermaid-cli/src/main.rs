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

    // Determine output format
    let output_format = if let Some(fmt) = &args.format {
        match fmt.as_str() {
            "png" => mermaid_core::OutputFormat::Png,
            _ => mermaid_core::OutputFormat::Svg,
        }
    } else if let Some(ref path) = args.output {
        // Auto-detect from extension
        match path.extension().and_then(|e| e.to_str()) {
            Some("png") => mermaid_core::OutputFormat::Png,
            _ => mermaid_core::OutputFormat::Svg,
        }
    } else {
        mermaid_core::OutputFormat::Svg
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

    // Parse background color if provided
    let background = args.background.as_ref().and_then(|bg| {
        if bg == "transparent" {
            Some(mermaid_core::ast::common::Color::None)
        } else {
            Some(mermaid_core::ast::common::Color::Hex(bg.clone()))
        }
    });

    let config = mermaid_core::RenderConfig {
        theme,
        font_provider,
        output_format,
        width: args.width,
        background,
    };

    // Render
    let render_output =
        mermaid_core::render(&source, &config).map_err(|e| anyhow::anyhow!("{}", e))?;

    // Determine output path
    let output_path = args.output.unwrap_or_else(|| {
        let stem = args
            .input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let ext = match output_format {
            mermaid_core::OutputFormat::Png => "png",
            _ => "svg",
        };
        PathBuf::from(format!("{}.{}", stem, ext))
    });

    // Write output (binary for PNG, text for SVG)
    let output_bytes = render_output.into_bytes();
    fs::write(&output_path, &output_bytes)
        .with_context(|| format!("Failed to write output: {}", output_path.display()))?;

    eprintln!(
        "Rendered {} -> {}",
        args.input.display(),
        output_path.display()
    );
    Ok(())
}
