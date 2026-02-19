use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

mod args;
use args::CliArgs;

fn main() -> Result<()> {
    let args = CliArgs::parse();
    run(args)
}

/// Runs the mermaid CLI with the provided arguments
pub fn run(args: CliArgs) -> Result<()> {
    // Initialize logging
    init_logging(args.verbose);

    // Read input
    let source = read_input(&args.input)?;

    // Determine output format
    let output_format = determine_output_format(&args.format, &args.output);

    // Build render config
    let config = build_render_config(&args, output_format)?;

    // Render
    let render_output =
        mermaid_core::render(&source, &config).map_err(|e| anyhow::anyhow!("{}", e))?;

    // Determine output path
    let output_path = determine_output_path(&args.input, &args.output, output_format);

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

/// Initialize logging based on verbosity
fn init_logging(verbose: bool) {
    if verbose {
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::new()
            .filter_level(log::LevelFilter::Warn)
            .init();
    }
}

/// Read input from stdin or file
fn read_input(input: &PathBuf) -> Result<String> {
    if input.to_str() == Some("-") {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read from stdin")?;
        Ok(buf)
    } else {
        fs::read_to_string(input)
            .with_context(|| format!("Failed to read input file: {}", input.display()))
    }
}

/// Determine output format based on explicit flag or file extension
fn determine_output_format(
    format_arg: &Option<String>,
    output_path: &Option<PathBuf>,
) -> mermaid_core::OutputFormat {
    if let Some(fmt) = format_arg {
        match fmt.as_str() {
            "png" => mermaid_core::OutputFormat::Png,
            _ => mermaid_core::OutputFormat::Svg,
        }
    } else if let Some(ref path) = output_path {
        // Auto-detect from extension
        match path.extension().and_then(|e| e.to_str()) {
            Some("png") => mermaid_core::OutputFormat::Png,
            _ => mermaid_core::OutputFormat::Svg,
        }
    } else {
        mermaid_core::OutputFormat::Svg
    }
}

/// Build the render configuration from CLI arguments
fn build_render_config(
    args: &CliArgs,
    output_format: mermaid_core::OutputFormat,
) -> Result<mermaid_core::RenderConfig> {
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

    Ok(mermaid_core::RenderConfig {
        theme,
        font_provider,
        output_format,
        width: args.width,
        background,
    })
}

/// Determine the output file path
fn determine_output_path(
    input: &PathBuf,
    output: &Option<PathBuf>,
    output_format: mermaid_core::OutputFormat,
) -> PathBuf {
    output.clone().unwrap_or_else(|| {
        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let ext = match output_format {
            mermaid_core::OutputFormat::Png => "png",
            _ => "svg",
        };
        PathBuf::from(format!("{}.{}", stem, ext))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_output_format_explicit_png() {
        let format = determine_output_format(&Some("png".to_string()), &None);
        assert!(matches!(format, mermaid_core::OutputFormat::Png));
    }

    #[test]
    fn test_determine_output_format_explicit_svg() {
        let format = determine_output_format(&Some("svg".to_string()), &None);
        assert!(matches!(format, mermaid_core::OutputFormat::Svg));
    }

    #[test]
    fn test_determine_output_format_from_png_extension() {
        let output = Some(PathBuf::from("output.png"));
        let format = determine_output_format(&None, &output);
        assert!(matches!(format, mermaid_core::OutputFormat::Png));
    }

    #[test]
    fn test_determine_output_format_from_svg_extension() {
        let output = Some(PathBuf::from("output.svg"));
        let format = determine_output_format(&None, &output);
        assert!(matches!(format, mermaid_core::OutputFormat::Svg));
    }

    #[test]
    fn test_determine_output_format_default() {
        let format = determine_output_format(&None, &None);
        assert!(matches!(format, mermaid_core::OutputFormat::Svg));
    }

    #[test]
    fn test_determine_output_format_explicit_overrides_extension() {
        // Explicit format should override file extension
        let output = Some(PathBuf::from("output.png"));
        let format = determine_output_format(&Some("svg".to_string()), &output);
        assert!(matches!(format, mermaid_core::OutputFormat::Svg));
    }

    #[test]
    fn test_determine_output_path_with_explicit_output() {
        let input = PathBuf::from("input.mmd");
        let output = Some(PathBuf::from("custom.svg"));
        let path = determine_output_path(&input, &output, mermaid_core::OutputFormat::Svg);
        assert_eq!(path, PathBuf::from("custom.svg"));
    }

    #[test]
    fn test_determine_output_path_default_svg() {
        let input = PathBuf::from("diagram.mmd");
        let path = determine_output_path(&input, &None, mermaid_core::OutputFormat::Svg);
        assert_eq!(path, PathBuf::from("diagram.svg"));
    }

    #[test]
    fn test_determine_output_path_default_png() {
        let input = PathBuf::from("diagram.mmd");
        let path = determine_output_path(&input, &None, mermaid_core::OutputFormat::Png);
        assert_eq!(path, PathBuf::from("diagram.png"));
    }

    #[test]
    fn test_determine_output_path_with_complex_input_name() {
        let input = PathBuf::from("/path/to/my-diagram.v1.mmd");
        let path = determine_output_path(&input, &None, mermaid_core::OutputFormat::Svg);
        assert_eq!(path, PathBuf::from("my-diagram.v1.svg"));
    }

    #[test]
    fn test_determine_output_path_with_empty_stem() {
        // A file named ".mmd" has an empty stem, which becomes ".mmd.svg"
        let input = PathBuf::from(".mmd");
        let path = determine_output_path(&input, &None, mermaid_core::OutputFormat::Svg);
        assert_eq!(path, PathBuf::from(".mmd.svg"));
    }

    #[test]
    fn test_read_input_from_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!("test_read_{}.mmd", std::process::id()));
        fs::write(&test_file, "test content").unwrap();

        let result = read_input(&test_file);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test content");

        fs::remove_file(&test_file).ok();
    }

    #[test]
    fn test_read_input_from_nonexistent_file() {
        let result = read_input(&PathBuf::from("/nonexistent/file.mmd"));
        assert!(result.is_err());
    }

    #[test]
    fn test_build_render_config_default() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd"]);
        let config = build_render_config(&args, mermaid_core::OutputFormat::Svg).unwrap();

        assert!(matches!(
            config.output_format,
            mermaid_core::OutputFormat::Svg
        ));
        assert!(config.width.is_none());
        assert!(config.background.is_none());
    }

    #[test]
    fn test_build_render_config_with_width() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-w", "800"]);
        let config = build_render_config(&args, mermaid_core::OutputFormat::Svg).unwrap();

        assert_eq!(config.width, Some(800));
    }

    #[test]
    fn test_build_render_config_with_background() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-b", "#ff0000"]);
        let config = build_render_config(&args, mermaid_core::OutputFormat::Svg).unwrap();

        match config.background {
            Some(mermaid_core::ast::common::Color::Hex(hex)) => assert_eq!(hex, "#ff0000"),
            _ => panic!("Expected Hex color"),
        }
    }

    #[test]
    fn test_build_render_config_with_transparent_background() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-b", "transparent"]);
        let config = build_render_config(&args, mermaid_core::OutputFormat::Svg).unwrap();

        match config.background {
            Some(mermaid_core::ast::common::Color::None) => (),
            _ => panic!("Expected None color (transparent)"),
        }
    }

    #[test]
    fn test_build_render_config_with_theme() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-t", "dark"]);
        let config = build_render_config(&args, mermaid_core::OutputFormat::Svg).unwrap();

        // Just verify it doesn't panic and builds successfully
        assert!(matches!(
            config.output_format,
            mermaid_core::OutputFormat::Svg
        ));
    }

    #[test]
    fn test_build_render_config_png_format() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd"]);
        let config = build_render_config(&args, mermaid_core::OutputFormat::Png).unwrap();

        assert!(matches!(
            config.output_format,
            mermaid_core::OutputFormat::Png
        ));
    }

    #[test]
    fn test_build_render_config_invalid_font() {
        let args = CliArgs::parse_from([
            "mermaid",
            "-i",
            "test.mmd",
            "--font",
            "/nonexistent/font.ttf",
        ]);
        let result = build_render_config(&args, mermaid_core::OutputFormat::Svg);

        assert!(result.is_err());
    }
}
