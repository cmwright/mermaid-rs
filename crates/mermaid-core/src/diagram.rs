use crate::ast::common::Color;
use crate::error::Result;
use crate::font::FontProvider;
use crate::layout::flowchart;
use crate::layout::gantt;
use crate::layout::gitgraph;
use crate::layout::mindmap;
use crate::layout::pie;
use crate::layout::sequence;
use crate::layout::text_measure::TextMeasurer;
use crate::parser::{self, DiagramKind};
#[cfg(feature = "png")]
use crate::render::png;
use crate::render::svg_architecture;
use crate::render::svg_flowchart;
use crate::render::svg_gantt;
use crate::render::svg_gitgraph;
use crate::render::svg_mindmap;
use crate::render::svg_pie;
use crate::render::svg_sequence;
use crate::render::theme::Theme;

/// Output format for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Svg,
    #[cfg(feature = "png")]
    Png,
}

/// The output of rendering, which can be either text (SVG) or binary (PNG).
#[derive(Debug, Clone)]
pub enum RenderOutput {
    Svg(String),
    #[cfg(feature = "png")]
    Png(Vec<u8>),
}

impl RenderOutput {
    /// Get the SVG string if this is SVG output.
    pub fn as_svg(&self) -> Option<&str> {
        match self {
            RenderOutput::Svg(s) => Some(s),
            #[cfg(feature = "png")]
            RenderOutput::Png(_) => None,
        }
    }

    /// Get the PNG bytes if this is PNG output.
    #[cfg(feature = "png")]
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
            #[cfg(feature = "png")]
            RenderOutput::Png(b) => b,
        }
    }

    /// Convert to SVG string, returning an error if this is PNG output.
    pub fn into_svg(self) -> crate::error::Result<String> {
        match self {
            RenderOutput::Svg(s) => Ok(s),
            #[cfg(feature = "png")]
            RenderOutput::Png(_) => Err(crate::error::MermaidError::Render(
                "Expected SVG output, but got PNG".to_string(),
            )),
        }
    }

    /// Convert to PNG bytes, returning an error if this is SVG output.
    #[cfg(feature = "png")]
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
        DiagramKind::Architecture => render_architecture_svg(source, config)?,
        DiagramKind::Flowchart => render_flowchart_svg(source, config)?,
        DiagramKind::Gantt => render_gantt_svg(source, config)?,
        DiagramKind::GitGraph => render_gitgraph_svg(source, config)?,
        DiagramKind::Pie => render_pie_svg(source, config)?,
        DiagramKind::Mindmap => render_mindmap_svg(source, config)?,
        DiagramKind::Sequence => render_sequence_svg(source, config)?,
    };

    match config.output_format {
        OutputFormat::Svg => Ok(RenderOutput::Svg(svg)),
        #[cfg(feature = "png")]
        OutputFormat::Png => {
            let png = png::render_png(&svg, config)?;
            Ok(RenderOutput::Png(png))
        }
    }
}

fn render_architecture_svg(source: &str, config: &RenderConfig) -> Result<String> {
    let arch_ast = crate::parser::architecture::parse_architecture(source)?;
    let fc_ast = arch_ast.to_flowchart_ast();
    let font_ref = config.font_provider.font_ref()?;
    let measurer = TextMeasurer::new(font_ref, config.theme.font_size as f32);
    let positioned = flowchart::layout_flowchart(&fc_ast, &measurer)?;
    svg_architecture::render_svg(&positioned, &config.theme)
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

fn render_gantt_svg(source: &str, config: &RenderConfig) -> Result<String> {
    // 1. Parse
    let ast = crate::parser::gantt::parse_gantt(source)?;

    // 2. Layout
    let font_ref = config.font_provider.font_ref()?;
    let measurer = TextMeasurer::new(font_ref, config.theme.font_size as f32);
    let layout = gantt::layout_gantt(&ast, &measurer, &config.theme)?;

    // 3. Render SVG
    let svg = svg_gantt::render_svg(&layout, &config.theme)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_config_default() {
        let config = RenderConfig::default();
        assert_eq!(config.output_format, OutputFormat::Svg);
        assert!(config.width.is_none());
        assert!(config.background.is_none());
    }

    #[test]
    fn render_output_as_svg() {
        let out = RenderOutput::Svg("<svg></svg>".to_string());
        assert_eq!(out.as_svg(), Some("<svg></svg>"));
        assert!(out.as_png().is_none());
    }

    #[cfg(feature = "png")]
    #[test]
    fn render_output_as_png() {
        let out = RenderOutput::Png(vec![0x89, 0x50, 0x4e]);
        assert!(out.as_svg().is_none());
        assert_eq!(out.as_png().unwrap(), &[0x89, 0x50, 0x4e]);
    }

    #[test]
    fn render_output_into_bytes_svg() {
        let out = RenderOutput::Svg("<svg></svg>".to_string());
        let bytes = out.into_bytes();
        assert_eq!(bytes, b"<svg></svg>");
    }

    #[cfg(feature = "png")]
    #[test]
    fn render_output_into_bytes_png() {
        let out = RenderOutput::Png(vec![1, 2, 3]);
        let bytes = out.into_bytes();
        assert_eq!(bytes, vec![1, 2, 3]);
    }

    #[test]
    fn render_output_into_svg_ok() {
        let out = RenderOutput::Svg("svg".to_string());
        assert!(out.into_svg().is_ok());
    }

    #[cfg(feature = "png")]
    #[test]
    fn render_output_into_svg_err_on_png() {
        let out = RenderOutput::Png(vec![]);
        let err = out.into_svg().unwrap_err();
        assert!(err.to_string().contains("PNG"));
    }

    #[cfg(feature = "png")]
    #[test]
    fn render_output_into_png_ok() {
        let out = RenderOutput::Png(vec![1, 2, 3]);
        assert!(out.into_png().is_ok());
    }

    #[cfg(feature = "png")]
    #[test]
    fn render_output_into_png_err_on_svg() {
        let out = RenderOutput::Svg("x".to_string());
        let err = out.into_png().unwrap_err();
        assert!(err.to_string().contains("SVG"));
    }

    #[test]
    fn render_flowchart() {
        let config = RenderConfig::default();
        let out = render("flowchart LR\n  A --> B", &config).unwrap();
        let svg = out.as_svg().unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("A") || svg.contains("B"));
    }

    #[test]
    fn render_sequence() {
        let config = RenderConfig::default();
        let out = render("sequenceDiagram\n  A->>B: hi", &config).unwrap();
        let svg = out.as_svg().unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn render_pie() {
        let config = RenderConfig::default();
        let out = render(r#"pie
    "a" : 1
    "b" : 2"#, &config).unwrap();
        let svg = out.as_svg().unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn render_gantt() {
        let config = RenderConfig::default();
        let out = render("gantt\n  title X\n  section S\n  A: 2024-01-01, 1d", &config).unwrap();
        let svg = out.as_svg().unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn render_gitgraph() {
        let config = RenderConfig::default();
        let out = render("gitGraph\n  commit", &config).unwrap();
        let svg = out.as_svg().unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn render_mindmap() {
        let config = RenderConfig::default();
        let out = render("mindmap\n  root\n    a\n    b", &config).unwrap();
        let svg = out.as_svg().unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn render_architecture() {
        let config = RenderConfig::default();
        let out = render(
            "architecture-beta\n  service s(server)[My Service]\n",
            &config,
        )
        .unwrap();
        let svg = out.as_svg().unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("My Service"));
    }
}
