use std::fmt::Write;

use crate::ast::flowchart::{EdgeType, NodeShape};
use crate::error::Result;
use crate::layout::flowchart::{
    PositionedEdge, PositionedGraph, PositionedNode, PositionedSubgraph,
};
use crate::render::html_util;
use crate::render::svg_util::{build_basis_curve_path, escape_xml};
use crate::render::theme::Theme;

const SVG_PADDING: f64 = 8.0;

/// Render a positioned flowchart graph to an SVG string.
pub fn render_svg(graph: &PositionedGraph, theme: &Theme) -> Result<String> {
    let view_w = graph.width + 2.0 * SVG_PADDING;
    let view_h = graph.height + 2.0 * SVG_PADDING;

    // Estimate capacity: ~200 bytes per node, ~300 per edge, ~200 per subgraph, plus overhead
    let est_capacity =
        1024 + graph.nodes.len() * 200 + graph.edges.len() * 300 + graph.subgraphs.len() * 200;
    let mut svg = String::with_capacity(est_capacity);

    // SVG header
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        view_w as i64, view_h as i64, view_w as i64, view_h as i64,
    );
    svg.push('\n');

    // Style block
    let _ = write!(
        svg,
        r#"<style>
  svg {{ background: {}; }}
  .node-text {{ font-family: {}; font-size: {}px; }}
  .edge-label {{ font-family: {}; font-size: {}px; }}
</style>"#,
        theme.background.to_css(),
        theme.font_family,
        theme.font_size,
        theme.font_family,
        theme.font_size * 0.85,
    );
    svg.push('\n');

    // Defs: arrow markers
    build_defs(&mut svg, theme);

    // Content group with padding offset
    let _ = write!(
        svg,
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    );
    svg.push('\n');

    // Subgraph backgrounds (behind everything)
    for sg in &graph.subgraphs {
        render_subgraph(&mut svg, sg, theme);
    }

    // Edges (behind nodes)
    for edge in &graph.edges {
        render_edge(&mut svg, edge, theme);
    }

    // Nodes (on top)
    for node in &graph.nodes {
        render_node(&mut svg, node, theme);
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn build_defs(svg: &mut String, theme: &Theme) {
    let line_color = theme.line_color.to_css();
    let sz = theme.flowchart.arrowhead_size.max(8.0);
    let mw = sz * 0.8;

    let _ = write!(
        svg,
        r#"<defs>
  <marker id="arrowhead" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="8.5" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 0 0 L 10 5 L 0 10 z" fill="{line_color}"/>
  </marker>
  <marker id="arrowhead-thick" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="8.5" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 0 0 L 10 5 L 0 10 z" fill="{line_color}"/>
  </marker>
</defs>
"#,
    );
}

fn render_node(svg: &mut String, node: &PositionedNode, theme: &Theme) {
    let fill = node
        .style
        .fill
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.flowchart.primary_color.to_css());
    let stroke = node
        .style
        .stroke
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.flowchart.primary_border.to_css());
    let stroke_width = node
        .style
        .stroke_width
        .unwrap_or(theme.flowchart.node_border_width);
    let text_color = node
        .style
        .color
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.flowchart.primary_text.to_css());

    let _ = write!(svg, r#"<g transform="translate({}, {})">"#, node.x, node.y);
    svg.push('\n');

    // Draw shape
    draw_shape(
        svg,
        node.shape,
        node.width,
        node.height,
        &fill,
        &stroke,
        stroke_width,
    );

    // Draw label text
    // For asymmetric shape, offset text right to center in the flat area
    let text_x_offset = if node.shape == NodeShape::Asymmetric {
        let notch = (node.height / 2.0) * 0.6;
        notch / 2.0
    } else {
        0.0
    };

    let label_lines: Vec<String> = html_util::normalize_br(&node.label)
        .lines()
        .map(|l| l.to_string())
        .collect();

    if label_lines.len() <= 1 && !html_util::has_html(&node.label) {
        // Simple single-line text (fast path)
        if text_x_offset.abs() < 0.01 {
            let _ = write!(
                svg,
                r#"  <text class="node-text" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
                text_color,
                escape_xml(&node.label),
            );
        } else {
            let _ = write!(
                svg,
                r#"  <text class="node-text" x="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
                text_x_offset,
                text_color,
                escape_xml(&node.label),
            );
        }
        svg.push('\n');
    } else {
        // Multi-line or HTML-formatted text
        let line_height = 1.2_f64; // em units
        let total_lines = label_lines.len();
        let start_dy = -((total_lines as f64 - 1.0) / 2.0) * line_height;

        let _ = write!(
            svg,
            r#"  <text class="node-text" text-anchor="middle" fill="{}">"#,
            text_color,
        );
        svg.push('\n');

        let tspan_x = if text_x_offset.abs() < 0.01 {
            "0".to_string()
        } else {
            format!("{}", text_x_offset)
        };

        for (i, line) in label_lines.iter().enumerate() {
            let dy = if i == 0 {
                format!("{}em", start_dy)
            } else {
                format!("{}em", line_height)
            };

            let segments = html_util::parse_segments(line);
            if segments.is_empty() {
                let _ = write!(
                    svg,
                    r#"    <tspan x="{}" dy="{}" dominant-baseline="central">&#160;</tspan>"#,
                    tspan_x, dy,
                );
                svg.push('\n');
            } else {
                let mut first_in_line = true;
                for seg in &segments {
                    svg.push_str("    <tspan");
                    if first_in_line {
                        let _ = write!(svg, r#" x="{}" dy="{}""#, tspan_x, dy);
                        first_in_line = false;
                    }
                    svg.push_str(r#" dominant-baseline="central""#);
                    if seg.bold {
                        svg.push_str(r#" font-weight="bold""#);
                    }
                    let _ = write!(svg, ">{}</tspan>", escape_xml(&seg.text));
                    svg.push('\n');
                }
            }
        }

        svg.push_str("  </text>\n");
    }

    svg.push_str("</g>\n");
}

fn draw_shape(
    svg: &mut String,
    shape: NodeShape,
    w: f64,
    h: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f64,
) {
    let hw = w / 2.0;
    let hh = h / 2.0;

    match shape {
        NodeShape::Rectangle => {
            let _ = write!(
                svg,
                r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, -hh, w, h, fill, stroke, stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::RoundedRectangle => {
            let rx = h.min(w) * 0.2;
            let _ = write!(
                svg,
                r#"  <rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, -hh, w, h, rx, rx, fill, stroke, stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::Stadium => {
            let rx = hh;
            let _ = write!(
                svg,
                r#"  <rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, -hh, w, h, rx, rx, fill, stroke, stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::Circle | NodeShape::DoubleCircle => {
            let r = hw.max(hh);
            let _ = write!(
                svg,
                r#"  <circle cx="0" cy="0" r="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                r, fill, stroke, stroke_width,
            );
            svg.push('\n');
            if shape == NodeShape::DoubleCircle {
                let _ = write!(
                    svg,
                    r#"  <circle cx="0" cy="0" r="{}" fill="none" stroke="{}" stroke-width="{}"/>"#,
                    r - 5.0,
                    stroke,
                    stroke_width,
                );
                svg.push('\n');
            }
        }
        NodeShape::Diamond => {
            let _ = write!(
                svg,
                r#"  <polygon points="0,{} {},0 0,{} {},0" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hh, hw, hh, -hw, fill, stroke, stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::Hexagon => {
            let offset = hh * 0.5;
            let _ = write!(
                svg,
                r#"  <polygon points="{},{} {},{} {},{} {},{} {},{} {},{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw + offset,
                -hh,
                hw - offset,
                -hh,
                hw,
                0.0,
                hw - offset,
                hh,
                -hw + offset,
                hh,
                -hw,
                0.0,
                fill,
                stroke,
                stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::Subroutine => {
            // Rectangle with vertical lines near left and right edges
            let inset = 8.0;
            let _ = write!(
                svg,
                r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, -hh, w, h, fill, stroke, stroke_width,
            );
            svg.push('\n');
            let _ = write!(
                svg,
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw + inset,
                -hh,
                -hw + inset,
                hh,
                stroke,
                stroke_width,
            );
            svg.push('\n');
            let _ = write!(
                svg,
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                hw - inset,
                -hh,
                hw - inset,
                hh,
                stroke,
                stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::Cylinder => {
            // Cylinder using ellipses at top and bottom
            let ry = 8.0;
            let body_top = -hh + ry;
            let body_bot = hh - ry;
            // Body rectangle
            let _ = write!(
                svg,
                r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="none"/>"#,
                -hw,
                body_top,
                w,
                body_bot - body_top,
                fill,
            );
            svg.push('\n');
            // Left and right edges
            let _ = write!(
                svg,
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, body_top, -hw, body_bot, stroke, stroke_width,
            );
            svg.push('\n');
            let _ = write!(
                svg,
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                hw, body_top, hw, body_bot, stroke, stroke_width,
            );
            svg.push('\n');
            // Top ellipse
            let _ = write!(
                svg,
                r#"  <ellipse cx="0" cy="{}" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                body_top, hw, ry, fill, stroke, stroke_width,
            );
            svg.push('\n');
            // Bottom ellipse
            let _ = write!(
                svg,
                r#"  <ellipse cx="0" cy="{}" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                body_bot, hw, ry, fill, stroke, stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::Asymmetric => {
            // Flag/banner shape: rectangle with V-notch cut into left side
            let notch = hh * 0.6;
            let _ = write!(
                svg,
                r#"  <polygon points="{},{} {},{} {},{} {},{} {},{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw,
                -hh, // top left corner
                -hw + notch,
                0.0, // center left (inward notch)
                -hw,
                hh, // bottom left corner
                hw,
                hh, // bottom right corner
                hw,
                -hh, // top right corner
                fill,
                stroke,
                stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::Trapezoid => {
            let offset = hw * 0.2;
            let _ = write!(
                svg,
                r#"  <polygon points="{},{} {},{} {},{} {},{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw + offset,
                -hh,
                hw - offset,
                -hh,
                hw,
                hh,
                -hw,
                hh,
                fill,
                stroke,
                stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::TrapezoidAlt => {
            let offset = hw * 0.2;
            let _ = write!(
                svg,
                r#"  <polygon points="{},{} {},{} {},{} {},{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw,
                -hh,
                hw,
                -hh,
                hw - offset,
                hh,
                -hw + offset,
                hh,
                fill,
                stroke,
                stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::Parallelogram => {
            let offset = hw * 0.2;
            let _ = write!(
                svg,
                r#"  <polygon points="{},{} {},{} {},{} {},{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw + offset,
                -hh,
                hw,
                -hh,
                hw - offset,
                hh,
                -hw,
                hh,
                fill,
                stroke,
                stroke_width,
            );
            svg.push('\n');
        }
        NodeShape::ParallelogramAlt => {
            let offset = hw * 0.2;
            let _ = write!(
                svg,
                r#"  <polygon points="{},{} {},{} {},{} {},{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw,
                -hh,
                hw - offset,
                -hh,
                hw,
                hh,
                -hw + offset,
                hh,
                fill,
                stroke,
                stroke_width,
            );
            svg.push('\n');
        }
    }
}

fn render_edge(svg: &mut String, edge: &PositionedEdge, theme: &Theme) {
    if edge.points.len() < 2 {
        return;
    }

    let line_color = theme.line_color.to_css();

    // Build path using B-spline interpolation (same as d3.curveBasis)
    let path_d = build_basis_curve_path(&edge.points);

    // Edge type styling
    match edge.edge_type {
        EdgeType::SolidArrow => {
            let _ = write!(
                svg,
                r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round" marker-end="url(#arrowhead)"/>"#,
                path_d,
                line_color,
                theme.flowchart.edge_width.max(1.75),
            );
        }
        EdgeType::DottedArrow => {
            let _ = write!(
                svg,
                r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round" stroke-dasharray="3,3" marker-end="url(#arrowhead)"/>"#,
                path_d,
                line_color,
                theme.flowchart.edge_width.max(1.75),
            );
        }
        EdgeType::ThickArrow => {
            let _ = write!(
                svg,
                r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round" marker-end="url(#arrowhead)"/>"#,
                path_d,
                line_color,
                theme.flowchart.edge_width * 2.0,
            );
        }
        EdgeType::SolidLine => {
            let _ = write!(
                svg,
                r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                path_d,
                line_color,
                theme.flowchart.edge_width.max(1.75),
            );
        }
        EdgeType::DottedLine => {
            let _ = write!(
                svg,
                r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round" stroke-dasharray="3,3"/>"#,
                path_d,
                line_color,
                theme.flowchart.edge_width.max(1.75),
            );
        }
        EdgeType::ThickLine => {
            let _ = write!(
                svg,
                r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                path_d,
                line_color,
                theme.flowchart.edge_width * 2.0,
            );
        }
    }
    svg.push('\n');

    // Edge label
    if let (Some(label), Some(lx), Some(ly)) = (&edge.label, edge.label_x, edge.label_y) {
        // Use measured dimensions if available, otherwise fall back to rough approximation
        let label_w = edge.label_width.unwrap_or(label.len() as f64 * 8.0 + 10.0);
        let label_h = edge.label_height.unwrap_or(20.0);
        let _ = write!(
            svg,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="rgba(232,232,232,0.8)"/>"#,
            lx - label_w / 2.0,
            ly - label_h / 2.0,
            label_w,
            label_h,
        );
        svg.push('\n');

        // Handle <br/> line breaks in edge labels
        let clean = html_util::normalize_br(label);
        let lines: Vec<&str> = clean.lines().collect();
        if lines.len() <= 1 {
            let _ = write!(
                svg,
                r#"<text class="edge-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
                lx,
                ly,
                theme.text_color.to_css(),
                escape_xml(&html_util::strip_html_tags(&clean)),
            );
        } else {
            let line_height = 1.2_f64;
            let start_dy = -((lines.len() as f64 - 1.0) / 2.0) * line_height;
            let _ = write!(
                svg,
                r#"<text class="edge-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">"#,
                lx,
                ly,
                theme.text_color.to_css(),
            );
            for (i, line) in lines.iter().enumerate() {
                let dy = if i == 0 {
                    format!("{}em", start_dy)
                } else {
                    format!("{}em", line_height)
                };
                let _ = write!(
                    svg,
                    r#"<tspan x="{}" dy="{}">{}</tspan>"#,
                    lx,
                    dy,
                    escape_xml(&html_util::strip_html_tags(line)),
                );
            }
            svg.push_str("</text>");
        }
        svg.push('\n');
    }
}

fn render_subgraph(svg: &mut String, sg: &PositionedSubgraph, theme: &Theme) {
    // Resolve fill/stroke from subgraph style overrides or theme defaults
    let fill = sg
        .style
        .fill
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.flowchart.subgraph_fill.to_css());
    let stroke = sg
        .style
        .stroke
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.flowchart.subgraph_border.to_css());

    // Background rectangle
    let _ = write!(
        svg,
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="1" stroke-dasharray="5,5"/>"#,
        sg.x, sg.y, sg.width, sg.height, fill, stroke,
    );
    svg.push('\n');

    // Title label (handle <br/> as line breaks, strip other HTML)
    if let Some(label) = &sg.label {
        let clean = html_util::normalize_br(label);
        let lines: Vec<&str> = clean.split('\n').collect();
        let label_x = sg.x + sg.width / 2.0;
        let label_y = sg.y + 18.0;
        if lines.len() == 1 {
            let text = html_util::strip_html_tags(lines[0]);
            let _ = write!(
                svg,
                r#"<text x="{}" y="{}" text-anchor="middle" font-family="{}" font-size="{}" font-weight="bold" fill="{}">{}</text>"#,
                label_x,
                label_y,
                theme.font_family,
                theme.font_size,
                theme.flowchart.subgraph_text.to_css(),
                escape_xml(&text),
            );
        } else {
            let _ = write!(
                svg,
                r#"<text x="{}" y="{}" text-anchor="middle" font-family="{}" font-size="{}" font-weight="bold" fill="{}">"#,
                label_x,
                label_y,
                theme.font_family,
                theme.font_size,
                theme.flowchart.subgraph_text.to_css(),
            );
            for (i, line) in lines.iter().enumerate() {
                let text = html_util::strip_html_tags(line);
                if i == 0 {
                    let _ = write!(
                        svg,
                        r#"<tspan x="{}" dy="0">{}</tspan>"#,
                        label_x,
                        escape_xml(&text),
                    );
                } else {
                    let _ = write!(
                        svg,
                        r#"<tspan x="{}" dy="1.2em">{}</tspan>"#,
                        label_x,
                        escape_xml(&text),
                    );
                }
            }
            svg.push_str("</text>");
        }
        svg.push('\n');
    }
}
