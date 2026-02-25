use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "mermaid",
    version,
    about = "Render Mermaid diagrams to SVG without a browser",
    long_about = "A pure-Rust Mermaid diagram renderer. Converts .mmd files to SVG \
                  without requiring a browser, Node.js, or Puppeteer."
)]
pub struct CliArgs {
    /// Input Mermaid file (.mmd) or '-' for stdin
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,

    /// Output file path. Format inferred from extension if -f not specified.
    #[arg(short = 'o', long = "output")]
    pub output: Option<PathBuf>,

    /// Output format: svg, png, or ascii
    #[arg(short = 'f', long = "format", value_parser = ["svg", "png", "ascii"])]
    pub format: Option<String>,

    /// Theme: default, dark, forest, neutral
    #[arg(short = 't', long = "theme", default_value = "default")]
    pub theme: String,

    /// Background color (CSS hex, e.g. '#ffffff' or 'transparent')
    #[arg(short = 'b', long = "background")]
    pub background: Option<String>,

    /// Custom font file (.ttf or .otf)
    #[arg(long = "font")]
    pub font: Option<PathBuf>,

    /// Maximum width in pixels
    #[arg(short = 'w', long = "width")]
    pub width: Option<u32>,

    /// Verbose logging
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_default_args() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd"]);
        assert_eq!(args.input, PathBuf::from("test.mmd"));
        assert!(args.output.is_none());
        assert!(args.format.is_none());
        assert_eq!(args.theme, "default");
        assert!(args.background.is_none());
        assert!(args.font.is_none());
        assert!(args.width.is_none());
        assert!(!args.verbose);
    }

    #[test]
    fn test_output_arg() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-o", "output.svg"]);
        assert_eq!(args.input, PathBuf::from("test.mmd"));
        assert_eq!(args.output, Some(PathBuf::from("output.svg")));
    }

    #[test]
    fn test_output_arg_long() {
        let args =
            CliArgs::parse_from(["mermaid", "--input", "test.mmd", "--output", "output.png"]);
        assert_eq!(args.input, PathBuf::from("test.mmd"));
        assert_eq!(args.output, Some(PathBuf::from("output.png")));
    }

    #[test]
    fn test_format_svg() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-f", "svg"]);
        assert_eq!(args.format, Some("svg".to_string()));
    }

    #[test]
    fn test_format_png() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "--format", "png"]);
        assert_eq!(args.format, Some("png".to_string()));
    }

    #[test]
    fn test_theme_arg() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-t", "dark"]);
        assert_eq!(args.theme, "dark");
    }

    #[test]
    fn test_theme_arg_long() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "--theme", "forest"]);
        assert_eq!(args.theme, "forest");
    }

    #[test]
    fn test_background_arg() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-b", "#ffffff"]);
        assert_eq!(args.background, Some("#ffffff".to_string()));
    }

    #[test]
    fn test_background_transparent() {
        let args =
            CliArgs::parse_from(["mermaid", "-i", "test.mmd", "--background", "transparent"]);
        assert_eq!(args.background, Some("transparent".to_string()));
    }

    #[test]
    fn test_font_arg() {
        let args =
            CliArgs::parse_from(["mermaid", "-i", "test.mmd", "--font", "/path/to/font.ttf"]);
        assert_eq!(args.font, Some(PathBuf::from("/path/to/font.ttf")));
    }

    #[test]
    fn test_width_arg() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-w", "800"]);
        assert_eq!(args.width, Some(800));
    }

    #[test]
    fn test_width_arg_long() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "--width", "1200"]);
        assert_eq!(args.width, Some(1200));
    }

    #[test]
    fn test_verbose_arg() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "-v"]);
        assert!(args.verbose);
    }

    #[test]
    fn test_verbose_arg_long() {
        let args = CliArgs::parse_from(["mermaid", "-i", "test.mmd", "--verbose"]);
        assert!(args.verbose);
    }

    #[test]
    fn test_stdin_input() {
        let args = CliArgs::parse_from(["mermaid", "-i", "-"]);
        assert_eq!(args.input, PathBuf::from("-"));
    }

    #[test]
    fn test_all_args_combined() {
        let args = CliArgs::parse_from([
            "mermaid",
            "-i",
            "input.mmd",
            "-o",
            "output.png",
            "-f",
            "png",
            "-t",
            "dark",
            "-b",
            "transparent",
            "--font",
            "custom.ttf",
            "-w",
            "1024",
            "-v",
        ]);
        assert_eq!(args.input, PathBuf::from("input.mmd"));
        assert_eq!(args.output, Some(PathBuf::from("output.png")));
        assert_eq!(args.format, Some("png".to_string()));
        assert_eq!(args.theme, "dark");
        assert_eq!(args.background, Some("transparent".to_string()));
        assert_eq!(args.font, Some(PathBuf::from("custom.ttf")));
        assert_eq!(args.width, Some(1024));
        assert!(args.verbose);
    }
}
