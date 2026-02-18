use crate::ast::common::Color;
use crate::error::Result;
use crate::font::FontProvider;
use crate::layout::flowchart_layout;
use crate::layout::sequence_layout;
use crate::layout::text_measure::TextMeasurer;
use crate::parser::{self, DiagramKind};
use crate::render::svg_flowchart;
use crate::render::svg_sequence;
use crate::render::theme::Theme;

/// Output format for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Svg,
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
pub fn render(source: &str, config: &RenderConfig) -> Result<String> {
    let kind = parser::detect_diagram_kind(source)?;
    match kind {
        DiagramKind::Flowchart => render_flowchart(source, config),
        DiagramKind::Sequence => render_sequence(source, config),
    }
}

fn render_flowchart(source: &str, config: &RenderConfig) -> Result<String> {
    // 1. Parse
    let ast = crate::parser::flowchart::parse_flowchart(source)?;

    // 2. Layout
    let font_ref = config.font_provider.font_ref()?;
    let measurer = TextMeasurer::new(font_ref, config.theme.font_size as f32);
    let positioned = flowchart_layout::layout_flowchart(&ast, &measurer)?;

    // 3. Render SVG
    let svg = svg_flowchart::render_svg(&positioned, &config.theme)?;

    Ok(svg)
}

fn render_sequence(source: &str, config: &RenderConfig) -> Result<String> {
    // 1. Parse
    let ast = crate::parser::sequence::parse_sequence(source)?;

    // 2. Layout
    let font_ref = config.font_provider.font_ref()?;
    let measurer = TextMeasurer::new(font_ref, config.theme.font_size as f32);
    let layout = sequence_layout::layout_sequence(&ast, &measurer, &config.theme)?;

    // 3. Render SVG
    let svg = svg_sequence::render_svg(&layout, &config.theme)?;

    Ok(svg)
}
