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

    /// Output format (currently: svg)
    #[arg(short = 'f', long = "format", value_parser = ["svg"])]
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
