use crate::ast::common::Color;
use crate::error::Result;
use crate::font::FontProvider;
use crate::layout::flowchart;
use crate::layout::gitgraph;
use crate::layout::mindmap;
use crate::layout::pie;
use crate::layout::sequence;
use crate::layout::text_measure::TextMeasurer;
use crate::parser::{self, DiagramKind};
use crate::render::png;
use crate::render::svg_flowchart;
use crate::render::svg_gitgraph;
use crate::render::svg_mindmap;
use crate::render::svg_pie;
use crate::render::svg_sequence;
use crate::render::theme::Theme;

/// Output format for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Svg,
    Png,
}

/// The output of rendering, which can be either text (SVG) or binary (PNG).
#[derive(Debug, Clone)]
pub enum RenderOutput {
    Svg(String),
    Png(Vec<u8>),
}

impl RenderOutput {
    /// Get the SVG string if this is SVG output.
    pub fn as_svg(&self) -> Option<&str> {
        match self {
            RenderOutput::Svg(s) => Some(s),
            RenderOutput::Png(_) => None,
        }
    }

    /// Get the PNG bytes if this is PNG output.
    pub fn as_png(&self) -> Option<&[u8]> {
        match self {
            RenderOutput::Svg(_) => None,
            RenderOutput::Png(b) => Some(b),
        }
    }

    /// Convert to bytes (UTF-8 for SVG, raw bytes for PNG).
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            RenderOutput::Svg(s) => s.into_bytes(),
            RenderOutput::Png(b) => b,
        }
    }

    /// Convert to SVG string, returning an error if this is PNG output.
    pub fn into_svg(self) -> crate::error::Result<String> {
        match self {
            RenderOutput::Svg(s) => Ok(s),
            RenderOutput::Png(_) => Err(crate::error::MermaidError::Render(
                "Expected SVG output, but got PNG".to_string(),
            )),
        }
    }

    /// Convert to PNG bytes, returning an error if this is SVG output.
    pub fn into_png(self) -> crate::error::Result<Vec<u8>> {
        match self {
            RenderOutput::Svg(_) => Err(crate::error::MermaidError::Render(
                "Expected PNG output, but got SVG".to_string(),
            )),
            RenderOutput::Png(b) => Ok(b),
        }
    }
}

/// Configuration for rendering a diagram.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub theme: Theme,
    pub font_provider: FontProvider,
    pub output_format: OutputFormat,
    pub width: Option<u32>,
    pub background: Option<Color>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            font_provider: FontProvider::default_font(),
            output_format: OutputFormat::Svg,
            width: None,
            background: None,
        }
    }
}

/// Top-level render function: detect diagram type, parse, layout, render.
pub fn render(source: &str, config: &RenderConfig) -> Result<RenderOutput> {
    let kind = parser::detect_diagram_kind(source)?;
    let svg = match kind {
        DiagramKind::Flowchart => render_flowchart_svg(source, config)?,
        DiagramKind::GitGraph => render_gitgraph_svg(source, config)?,
        DiagramKind::Pie => render_pie_svg(source, config)?,
        DiagramKind::Mindmap => render_mindmap_svg(source, config)?,
        DiagramKind::Sequence => render_sequence_svg(source, config)?,
    };

    match config.output_format {
        OutputFormat::Svg => Ok(RenderOutput::Svg(svg)),
        OutputFormat::Png => {
            let png = png::render_png(&svg, config)?;
            Ok(RenderOutput::Png(png))
        }
    }
}

fn render_gitgraph_svg(source: &str, config: &RenderConfig) -> Result<String> {
    let ast = crate::parser::gitgraph::parse_gitgraph(source)?;
    let font_ref = config.font_provider.font_ref()?;
    let measurer = TextMeasurer::new(font_ref, config.theme.font_size as f32);
    let layout = gitgraph::layout_gitgraph(&ast, &measurer, &config.theme)?;
    let svg = svg_gitgraph::render_svg(&layout, &config.theme)?;
    Ok(svg)
}

fn render_flowchart_svg(source: &str, config: &RenderConfig) -> Result<String> {
    // 1. Parse
    let ast = crate::parser::flowchart::parse_flowchart(source)?;

    // 2. Layout
    let font_ref = config.font_provider.font_ref()?;
    let measurer = TextMeasurer::new(font_ref, config.theme.font_size as f32);
    let positioned = flowchart::layout_flowchart(&ast, &measurer)?;

    // 3. Render SVG
    let svg = svg_flowchart::render_svg(&positioned, &config.theme)?;

    Ok(svg)
}

fn render_pie_svg(source: &str, config: &RenderConfig) -> Result<String> {
    // 1. Parse
    let ast = crate::parser::pie::parse_pie(source)?;

    // 2. Layout
    let font_ref = config.font_provider.font_ref()?;
    let measurer = TextMeasurer::new(font_ref, config.theme.font_size as f32);
    let layout = pie::layout_pie(&ast, &measurer, &config.theme)?;

    // 3. Render SVG
    let svg = svg_pie::render_svg(&layout, &config.theme)?;

    Ok(svg)
}

fn render_sequence_svg(source: &str, config: &RenderConfig) -> Result<String> {
    // 1. Parse
    let ast = crate::parser::sequence::parse_sequence(source)?;

    // 2. Layout
    let font_ref = config.font_provider.font_ref()?;
    let measurer = TextMeasurer::new(font_ref, config.theme.font_size as f32);
    let layout = sequence::layout_sequence(&ast, &measurer, &config.theme)?;

    // 3. Render SVG
    let svg = svg_sequence::render_svg(&layout, &config.theme)?;

    Ok(svg)
}

fn render_mindmap_svg(source: &str, config: &RenderConfig) -> Result<String> {
    // 1. Parse
    let ast = crate::parser::mindmap::parse_mindmap(source)?;

    // 2. Layout
    let font_ref = config.font_provider.font_ref()?;
    let measurer = TextMeasurer::new(font_ref, config.theme.font_size as f32);
    let layout = mindmap::layout_mindmap(&ast, &measurer, &config.theme)?;

    // 3. Render SVG
    let svg = svg_mindmap::render_svg(&layout, &config.theme)?;

    Ok(svg)
}
