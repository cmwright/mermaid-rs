use std::fmt::Write;

use crate::ast::flowchart::{ArrowEnd, LineStyle, NodeShape};
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

    // Sort elements for deterministic output ordering.
    let mut subgraphs: Vec<&PositionedSubgraph> = graph.subgraphs.iter().collect();
    subgraphs.sort_by(|a, b| {
        a.y.total_cmp(&b.y)
            .then(a.x.total_cmp(&b.x))
            .then(a.id.cmp(&b.id))
    });

    let mut edges: Vec<&PositionedEdge> = graph.edges.iter().collect();
    edges.sort_by(|a, b| {
        edge_sort_key_y(a)
            .total_cmp(&edge_sort_key_y(b))
            .then(edge_sort_key_x(a).total_cmp(&edge_sort_key_x(b)))
            .then(a.from_id.cmp(&b.from_id))
            .then(a.to_id.cmp(&b.to_id))
    });

    let mut nodes: Vec<&PositionedNode> = graph.nodes.iter().collect();
    nodes.sort_by(|a, b| {
        a.y.total_cmp(&b.y)
            .then(a.x.total_cmp(&b.x))
            .then(a.id.cmp(&b.id))
    });

    // Mermaid-like layer grouping: clusters, edge paths, edge labels, nodes.
    svg.push_str(r#"<g class="clusters">"#);
    svg.push('\n');
    for sg in &subgraphs {
        render_subgraph(&mut svg, sg, theme);
    }
    svg.push_str("</g>\n");

    svg.push_str(r#"<g class="edgePaths">"#);
    svg.push('\n');
    for edge in &edges {
        render_edge_path(&mut svg, edge, theme);
    }
    svg.push_str("</g>\n");

    svg.push_str(r#"<g class="edgeLabels">"#);
    svg.push('\n');
    for edge in &edges {
        render_edge_label(&mut svg, edge, theme);
    }
    svg.push_str("</g>\n");

    svg.push_str(r#"<g class="nodes">"#);
    svg.push('\n');
    for node in &nodes {
        render_node(&mut svg, node, theme);
    }
    svg.push_str("</g>\n");

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
  <marker id="arrowhead-start" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="1.5" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 10 0 L 0 5 L 10 10 z" fill="{line_color}"/>
  </marker>
  <marker id="circle-end" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="9" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <circle cx="5" cy="5" r="4" stroke="{line_color}" stroke-width="1" fill="none"/>
  </marker>
  <marker id="circle-start" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="1" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <circle cx="5" cy="5" r="4" stroke="{line_color}" stroke-width="1" fill="none"/>
  </marker>
  <marker id="cross-end" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="9" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 1 1 L 9 9 M 9 1 L 1 9" stroke="{line_color}" stroke-width="1.5" fill="none"/>
  </marker>
  <marker id="cross-start" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="1" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 1 1 L 9 9 M 9 1 L 1 9" stroke="{line_color}" stroke-width="1.5" fill="none"/>
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

fn marker_end_attr(arrow: ArrowEnd) -> &'static str {
    match arrow {
        ArrowEnd::None => "",
        ArrowEnd::Arrow => r#" marker-end="url(#arrowhead)""#,
        ArrowEnd::Circle => r#" marker-end="url(#circle-end)""#,
        ArrowEnd::Cross => r#" marker-end="url(#cross-end)""#,
    }
}

fn marker_start_attr(arrow: ArrowEnd) -> &'static str {
    match arrow {
        ArrowEnd::None => "",
        ArrowEnd::Arrow => r#" marker-start="url(#arrowhead-start)""#,
        ArrowEnd::Circle => r#" marker-start="url(#circle-start)""#,
        ArrowEnd::Cross => r#" marker-start="url(#cross-start)""#,
    }
}

fn edge_sort_key_y(edge: &PositionedEdge) -> f64 {
    if let Some(y) = edge.label_y {
        return y;
    }
    if edge.points.is_empty() {
        return 0.0;
    }
    edge.points.iter().map(|p| p.1).sum::<f64>() / edge.points.len() as f64
}

fn edge_sort_key_x(edge: &PositionedEdge) -> f64 {
    if let Some(x) = edge.label_x {
        return x;
    }
    if edge.points.is_empty() {
        return 0.0;
    }
    edge.points.iter().map(|p| p.0).sum::<f64>() / edge.points.len() as f64
}

fn render_edge_path(svg: &mut String, edge: &PositionedEdge, theme: &Theme) {
    if edge.points.len() < 2 {
        return;
    }

    if edge.line_style == LineStyle::Invisible {
        return;
    }

    let line_color = theme.line_color.to_css();
    let path_d = build_basis_curve_path(&edge.points);

    let stroke_width = match edge.line_style {
        LineStyle::Thick => theme.flowchart.edge_width * 2.0,
        _ => theme.flowchart.edge_width.max(1.75),
    };

    let dasharray = match edge.line_style {
        LineStyle::Dotted => r#" stroke-dasharray="3,3""#,
        _ => "",
    };

    let m_end = marker_end_attr(edge.arrow_end);
    let m_start = marker_start_attr(edge.arrow_start);

    let _ = write!(
        svg,
        r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"{}{}{}/>"#,
        path_d, line_color, stroke_width, dasharray, m_end, m_start,
    );
    svg.push('\n');
}

fn render_edge_label(svg: &mut String, edge: &PositionedEdge, theme: &Theme) {
    if edge.line_style == LineStyle::Invisible || edge.points.len() < 2 {
        return;
    }

    if let (Some(label), Some(lx), Some(ly)) = (&edge.label, edge.label_x, edge.label_y) {
        // Use measured dimensions if available, otherwise fall back to rough approximation
        let label_w = edge.label_width.unwrap_or(label.len() as f64 * 8.0 + 10.0);
        let label_h = edge.label_height.unwrap_or(20.0);
        let _ = write!(
            svg,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{}"/>"#,
            lx - label_w / 2.0,
            ly - label_h / 2.0,
            label_w,
            label_h,
            theme.edge_label_background.to_css(),
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
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="1.5" stroke-dasharray="5,5"/>"#,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::common::StyleProperties;
    use crate::ast::flowchart::{ArrowEnd, Direction, EdgeType, LineStyle, NodeShape};
    use crate::layout::flowchart::{
        PositionedEdge, PositionedGraph, PositionedNode, PositionedSubgraph,
    };
    use crate::render::theme::Theme;

    /// Helper: build a minimal PositionedGraph with the given nodes, edges, and subgraphs.
    fn make_graph(
        nodes: Vec<PositionedNode>,
        edges: Vec<PositionedEdge>,
        subgraphs: Vec<PositionedSubgraph>,
    ) -> PositionedGraph {
        PositionedGraph {
            nodes,
            edges,
            subgraphs,
            width: 400.0,
            height: 400.0,
            direction: Direction::TopToBottom,
        }
    }

    /// Helper: build a single node with given shape and label.
    fn make_node(id: &str, label: &str, shape: NodeShape) -> PositionedNode {
        PositionedNode {
            id: id.to_string(),
            label: label.to_string(),
            shape,
            style: StyleProperties::default(),
            x: 100.0,
            y: 100.0,
            width: 80.0,
            height: 40.0,
        }
    }

    /// Helper: build an edge with the given type and optional label.
    fn make_edge(from: &str, to: &str, edge_type: EdgeType, label: Option<&str>) -> PositionedEdge {
        let has_label = label.is_some();
        let (line_style, arrow_end) = edge_type.to_parts();
        PositionedEdge {
            from_id: from.to_string(),
            to_id: to.to_string(),
            line_style,
            arrow_start: ArrowEnd::None,
            arrow_end,
            label: label.map(|s| s.to_string()),
            label_x: if has_label { Some(150.0) } else { None },
            label_y: if has_label { Some(150.0) } else { None },
            label_width: if has_label { Some(60.0) } else { None },
            label_height: if has_label { Some(20.0) } else { None },
            points: vec![(50.0, 50.0), (100.0, 100.0), (150.0, 150.0)],
        }
    }

    // ---------------------------------------------------------------
    // 1. All node shapes
    // ---------------------------------------------------------------

    #[test]
    fn test_all_node_shapes() {
        let shapes = vec![
            (NodeShape::Rectangle, "rect"),
            (NodeShape::RoundedRectangle, "rounded"),
            (NodeShape::Stadium, "stadium"),
            (NodeShape::Circle, "circle"),
            (NodeShape::DoubleCircle, "dblcircle"),
            (NodeShape::Diamond, "diamond"),
            (NodeShape::Hexagon, "hexagon"),
            (NodeShape::Subroutine, "subroutine"),
            (NodeShape::Cylinder, "cylinder"),
            (NodeShape::Asymmetric, "asymmetric"),
            (NodeShape::Trapezoid, "trapezoid"),
            (NodeShape::TrapezoidAlt, "trapezoidalt"),
            (NodeShape::Parallelogram, "parallelogram"),
            (NodeShape::ParallelogramAlt, "parallelogramalt"),
        ];

        let nodes: Vec<PositionedNode> = shapes
            .iter()
            .enumerate()
            .map(|(i, (shape, label))| {
                let mut node = make_node(&format!("n{}", i), label, *shape);
                node.y = (i as f64) * 60.0;
                node
            })
            .collect();

        let graph = make_graph(nodes, vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        // Rectangle: plain <rect> without rx
        assert!(
            svg.contains("<rect x="),
            "SVG should contain <rect> for Rectangle"
        );
        // RoundedRectangle: <rect> with rx attribute
        assert!(
            svg.contains("rx="),
            "SVG should contain rx= for RoundedRectangle"
        );
        // Circle: <circle> element
        assert!(
            svg.contains("<circle"),
            "SVG should contain <circle> for Circle"
        );
        // Diamond: <polygon> element
        assert!(
            svg.contains("<polygon"),
            "SVG should contain <polygon> for Diamond"
        );
        // Subroutine: <rect> and <line> elements
        assert!(
            svg.contains("<line"),
            "SVG should contain <line> for Subroutine"
        );
        // Cylinder: <ellipse> element
        assert!(
            svg.contains("<ellipse"),
            "SVG should contain <ellipse> for Cylinder"
        );
        // Verify the SVG is well-formed
        assert!(svg.contains("</svg>"), "SVG should be well-formed");
    }

    // ---------------------------------------------------------------
    // 2. Asymmetric text offset
    // ---------------------------------------------------------------

    #[test]
    fn test_asymmetric_text_offset() {
        let node = make_node("asym", "Flag", NodeShape::Asymmetric);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        // The asymmetric shape should produce a <text> with an x= attribute
        // because text_x_offset > 0 for Asymmetric nodes
        assert!(
            svg.contains(r#"<text class="node-text" x="#),
            "Asymmetric node should have text with x offset attribute. SVG: {}",
            svg
        );
    }

    // ---------------------------------------------------------------
    // 3. Multi-line text
    // ---------------------------------------------------------------

    #[test]
    fn test_multi_line_text() {
        let node = make_node("ml", "Line1\nLine2\nLine3", NodeShape::Rectangle);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        // Multi-line labels should produce <tspan> elements
        assert!(
            svg.contains("<tspan"),
            "Multi-line text should contain <tspan> elements. SVG: {}",
            svg
        );

        // Should contain dominant-baseline on tspan
        assert!(
            svg.contains(r#"dominant-baseline="central""#),
            "tspan elements should have dominant-baseline attribute"
        );
    }

    // ---------------------------------------------------------------
    // 4. HTML bold text
    // ---------------------------------------------------------------

    #[test]
    fn test_html_bold_text() {
        let node = make_node("bold", "<b>Bold</b> normal", NodeShape::Rectangle);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        // HTML-formatted text should produce tspan with font-weight="bold"
        assert!(
            svg.contains(r#"font-weight="bold""#),
            "HTML bold text should produce font-weight=\"bold\" in SVG. SVG: {}",
            svg
        );

        // The bold text content should appear
        assert!(
            svg.contains(">Bold</tspan>"),
            "Bold text content should appear in tspan. SVG: {}",
            svg
        );
    }

    // ---------------------------------------------------------------
    // 5. Multi-line edge labels
    // ---------------------------------------------------------------

    #[test]
    fn test_multi_line_edge_label() {
        let edge = make_edge("a", "b", EdgeType::SolidArrow, Some("Line1\nLine2"));
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        // Multi-line edge labels should produce <tspan> elements
        assert!(
            svg.contains("<tspan"),
            "Multi-line edge label should contain <tspan> elements. SVG: {}",
            svg
        );
    }

    // ---------------------------------------------------------------
    // 6. All edge types
    // ---------------------------------------------------------------

    #[test]
    fn test_all_edge_types() {
        let edge_types = vec![
            EdgeType::SolidArrow,
            EdgeType::DottedArrow,
            EdgeType::ThickArrow,
            EdgeType::SolidLine,
            EdgeType::DottedLine,
            EdgeType::ThickLine,
        ];

        let mut nodes = vec![];
        let mut edges = vec![];

        for (i, et) in edge_types.iter().enumerate() {
            let from_id = format!("s{}", i);
            let to_id = format!("t{}", i);
            let mut from_node = make_node(&from_id, &format!("S{}", i), NodeShape::Rectangle);
            let mut to_node = make_node(&to_id, &format!("T{}", i), NodeShape::Rectangle);
            from_node.y = (i as f64) * 80.0;
            to_node.y = (i as f64) * 80.0 + 40.0;
            nodes.push(from_node);
            nodes.push(to_node);
            edges.push(make_edge(&from_id, &to_id, *et, None));
        }

        let graph = make_graph(nodes, edges, vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        // SolidArrow and DottedArrow should have arrowhead marker
        assert!(
            svg.contains(r#"marker-end="url(#arrowhead)""#),
            "Arrow edges should have arrowhead marker. SVG: {}",
            svg
        );

        // DottedArrow and DottedLine should have stroke-dasharray
        assert!(
            svg.contains(r#"stroke-dasharray="3,3""#),
            "Dotted edges should have stroke-dasharray. SVG: {}",
            svg
        );

        // All should have <path> elements
        let path_count = svg.matches("<path d=").count();
        // 6 edges + arrowhead marker paths in defs
        assert!(
            path_count >= 6,
            "Should have at least 6 edge paths, found {}",
            path_count
        );
    }

    #[test]
    fn test_solid_arrow_has_marker() {
        let edge = make_edge("a", "b", EdgeType::SolidArrow, None);
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains(r#"marker-end="url(#arrowhead)""#));
        assert!(!svg.contains(r#"stroke-dasharray="3,3""#));
    }

    #[test]
    fn test_dotted_arrow_has_dasharray_and_marker() {
        let edge = make_edge("a", "b", EdgeType::DottedArrow, None);
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains(r#"stroke-dasharray="3,3""#));
        assert!(svg.contains(r#"marker-end="url(#arrowhead)""#));
    }

    #[test]
    fn test_thick_arrow_has_marker_no_dasharray() {
        let edge = make_edge("a", "b", EdgeType::ThickArrow, None);
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains(r#"marker-end="url(#arrowhead)""#));
        let after_defs = svg.split("</defs>").nth(1).unwrap_or("");
        assert!(
            !after_defs.contains(r#"stroke-dasharray="3,3""#),
            "ThickArrow should not have stroke-dasharray"
        );
    }

    #[test]
    fn test_solid_line_no_marker_no_dasharray() {
        let edge = make_edge("a", "b", EdgeType::SolidLine, None);
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        let after_defs = svg.split("</defs>").nth(1).unwrap_or("");
        let edge_paths: Vec<&str> = after_defs
            .lines()
            .filter(|l| l.contains("<path d="))
            .collect();
        assert!(!edge_paths.is_empty(), "Should have at least one edge path");
        for path in &edge_paths {
            assert!(
                !path.contains("marker-end"),
                "SolidLine should not have marker-end"
            );
            assert!(
                !path.contains("stroke-dasharray"),
                "SolidLine should not have stroke-dasharray"
            );
        }
    }

    #[test]
    fn test_dotted_line_dasharray_no_marker() {
        let edge = make_edge("a", "b", EdgeType::DottedLine, None);
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        let after_defs = svg.split("</defs>").nth(1).unwrap_or("");
        let edge_paths: Vec<&str> = after_defs
            .lines()
            .filter(|l| l.contains("<path d="))
            .collect();
        assert!(!edge_paths.is_empty(), "Should have at least one edge path");
        for path in &edge_paths {
            assert!(
                !path.contains("marker-end"),
                "DottedLine should not have marker-end"
            );
            assert!(
                path.contains(r#"stroke-dasharray="3,3""#),
                "DottedLine should have stroke-dasharray"
            );
        }
    }

    #[test]
    fn test_thick_line_no_marker_no_dasharray() {
        let edge = make_edge("a", "b", EdgeType::ThickLine, None);
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        let after_defs = svg.split("</defs>").nth(1).unwrap_or("");
        let edge_paths: Vec<&str> = after_defs
            .lines()
            .filter(|l| l.contains("<path d="))
            .collect();
        assert!(!edge_paths.is_empty(), "Should have at least one edge path");
        for path in &edge_paths {
            assert!(
                !path.contains("marker-end"),
                "ThickLine should not have marker-end"
            );
            assert!(
                !path.contains("stroke-dasharray"),
                "ThickLine should not have stroke-dasharray"
            );
        }
    }

    // ---------------------------------------------------------------
    // 7. Multi-line subgraph label
    // ---------------------------------------------------------------

    #[test]
    fn test_multi_line_subgraph_label() {
        let sg = PositionedSubgraph {
            id: "sg1".to_string(),
            label: Some("Title Line1\nTitle Line2".to_string()),
            x: 10.0,
            y: 10.0,
            width: 200.0,
            height: 200.0,
            style: StyleProperties::default(),
        };
        let graph = make_graph(vec![], vec![], vec![sg]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("<tspan"),
            "Multi-line subgraph label should contain <tspan> elements. SVG: {}",
            svg
        );

        assert!(
            svg.contains(r#"dy="1.2em""#),
            "Second line should have dy=\"1.2em\". SVG: {}",
            svg
        );

        assert!(
            svg.contains(r#"dy="0""#),
            "First line should have dy=\"0\". SVG: {}",
            svg
        );
    }

    // ---------------------------------------------------------------
    // 8. Empty edge label segments
    // ---------------------------------------------------------------

    #[test]
    fn test_empty_edge_label_segment() {
        let edge = make_edge("a", "b", EdgeType::SolidArrow, Some("Top\n\nBottom"));
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("<tspan"),
            "Edge label with empty line should contain tspan. SVG: {}",
            svg
        );
    }

    // ---------------------------------------------------------------
    // 9. Edge with no label
    // ---------------------------------------------------------------

    #[test]
    fn test_edge_no_label() {
        let edge = make_edge("a", "b", EdgeType::SolidArrow, None);
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            !svg.contains(r#"class="edge-label""#),
            "Edge without label should not have edge-label text element. SVG: {}",
            svg
        );
    }

    // ---------------------------------------------------------------
    // 10. DoubleCircle inner ring
    // ---------------------------------------------------------------

    #[test]
    fn test_double_circle_has_two_circles() {
        let node = make_node("dc", "State", NodeShape::DoubleCircle);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        let after_defs = svg.split("</defs>").nth(1).unwrap_or("");
        let circle_count = after_defs.matches("<circle").count();
        assert_eq!(
            circle_count, 2,
            "DoubleCircle should produce exactly 2 <circle> elements in content, found {}. SVG: {}",
            circle_count, svg
        );

        assert!(
            svg.contains(r#"fill="none""#),
            "Inner circle of DoubleCircle should have fill=\"none\". SVG: {}",
            svg
        );
    }

    // ---------------------------------------------------------------
    // Individual shape tests for targeted coverage
    // ---------------------------------------------------------------

    #[test]
    fn test_rectangle_shape() {
        let node = make_node("r", "Box", NodeShape::Rectangle);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains("<rect x="));
        assert!(svg.contains("Box"));
    }

    #[test]
    fn test_rounded_rectangle_shape() {
        let node = make_node("rr", "Rounded", NodeShape::RoundedRectangle);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains("rx="));
        assert!(svg.contains("ry="));
    }

    #[test]
    fn test_stadium_shape() {
        let node = make_node("st", "Stadium", NodeShape::Stadium);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains("rx="));
    }

    #[test]
    fn test_circle_shape() {
        let node = make_node("c", "Circle", NodeShape::Circle);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        let after_defs = svg.split("</defs>").nth(1).unwrap_or("");
        let circle_count = after_defs.matches("<circle").count();
        assert_eq!(
            circle_count, 1,
            "Circle should produce exactly 1 <circle> element in content"
        );
    }

    #[test]
    fn test_diamond_shape() {
        let node = make_node("d", "Decision", NodeShape::Diamond);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains("<polygon"), "Diamond should use <polygon>");
    }

    #[test]
    fn test_hexagon_shape() {
        let node = make_node("h", "Hex", NodeShape::Hexagon);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains("<polygon"), "Hexagon should use <polygon>");
    }

    #[test]
    fn test_subroutine_shape() {
        let node = make_node("sub", "Subroutine", NodeShape::Subroutine);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains("<rect"), "Subroutine should have a <rect>");
        let line_count = svg.matches("<line").count();
        assert_eq!(
            line_count, 2,
            "Subroutine should have 2 <line> elements, found {}",
            line_count
        );
    }

    #[test]
    fn test_cylinder_shape() {
        let node = make_node("cyl", "DB", NodeShape::Cylinder);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("<ellipse"),
            "Cylinder should have <ellipse> elements"
        );
        let ellipse_count = svg.matches("<ellipse").count();
        assert_eq!(
            ellipse_count, 2,
            "Cylinder should have 2 <ellipse> elements, found {}",
            ellipse_count
        );
    }

    #[test]
    fn test_asymmetric_shape() {
        let node = make_node("as", "Flag", NodeShape::Asymmetric);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains("<polygon"), "Asymmetric should use <polygon>");
    }

    #[test]
    fn test_trapezoid_shape() {
        let node = make_node("tr", "Trap", NodeShape::Trapezoid);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains("<polygon"), "Trapezoid should use <polygon>");
    }

    #[test]
    fn test_trapezoid_alt_shape() {
        let node = make_node("tra", "TrapAlt", NodeShape::TrapezoidAlt);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("<polygon"),
            "TrapezoidAlt should use <polygon>"
        );
    }

    #[test]
    fn test_parallelogram_shape() {
        let node = make_node("p", "Para", NodeShape::Parallelogram);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("<polygon"),
            "Parallelogram should use <polygon>"
        );
    }

    #[test]
    fn test_parallelogram_alt_shape() {
        let node = make_node("pa", "ParaAlt", NodeShape::ParallelogramAlt);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("<polygon"),
            "ParallelogramAlt should use <polygon>"
        );
    }

    // ---------------------------------------------------------------
    // Multi-line node text with asymmetric x offset in tspan
    // ---------------------------------------------------------------

    #[test]
    fn test_multi_line_asymmetric_text_offset_in_tspan() {
        let node = make_node("asml", "Line1\nLine2", NodeShape::Asymmetric);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("<tspan"),
            "Multi-line asymmetric should have tspan"
        );
        // The x attribute on tspan should NOT be "0" for asymmetric shape
        assert!(
            !svg.contains(r#"<tspan x="0""#),
            "Asymmetric multi-line text tspan x should not be \"0\". SVG: {}",
            svg
        );
    }

    // ---------------------------------------------------------------
    // Empty multi-line node text segment (empty line)
    // ---------------------------------------------------------------

    #[test]
    fn test_empty_line_in_multiline_node_label() {
        let node = make_node("empty", "Top\n\nBottom", NodeShape::Rectangle);
        let graph = make_graph(vec![node], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("&#160;"),
            "Empty line in multi-line label should produce &#160;. SVG: {}",
            svg
        );
    }

    // ---------------------------------------------------------------
    // Subgraph with single-line label
    // ---------------------------------------------------------------

    #[test]
    fn test_single_line_subgraph_label() {
        let sg = PositionedSubgraph {
            id: "sg".to_string(),
            label: Some("My Subgraph".to_string()),
            x: 10.0,
            y: 10.0,
            width: 200.0,
            height: 200.0,
            style: StyleProperties::default(),
        };
        let graph = make_graph(vec![], vec![], vec![sg]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("My Subgraph"),
            "Subgraph label should appear in SVG"
        );
        assert!(
            !svg.contains("<tspan"),
            "Single-line subgraph label should not use tspan"
        );
    }

    // ---------------------------------------------------------------
    // Subgraph with no label
    // ---------------------------------------------------------------

    #[test]
    fn test_subgraph_no_label() {
        let sg = PositionedSubgraph {
            id: "sg".to_string(),
            label: None,
            x: 10.0,
            y: 10.0,
            width: 200.0,
            height: 200.0,
            style: StyleProperties::default(),
        };
        let graph = make_graph(vec![], vec![], vec![sg]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(svg.contains("<rect"));
        let after_style = svg.split("</style>").nth(1).unwrap_or("");
        let text_with_anchor: Vec<&str> = after_style
            .lines()
            .filter(|l| l.contains("text-anchor=\"middle\" font-family"))
            .collect();
        assert!(
            text_with_anchor.is_empty(),
            "Subgraph without label should not have label text elements"
        );
    }

    // ---------------------------------------------------------------
    // Edge with single-line label (non-multi-line path)
    // ---------------------------------------------------------------

    #[test]
    fn test_single_line_edge_label() {
        let edge = make_edge("a", "b", EdgeType::SolidArrow, Some("yes"));
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains(r#"class="edge-label""#),
            "Edge with label should have edge-label class"
        );
        assert!(svg.contains("yes"), "Edge label text should appear");
    }

    // ---------------------------------------------------------------
    // Edge with BR tag in label (normalized to newline)
    // ---------------------------------------------------------------

    #[test]
    fn test_edge_with_single_point_skipped() {
        let edge = PositionedEdge {
            from_id: "a".to_string(),
            to_id: "b".to_string(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![(50.0, 50.0)],
        };
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();
        let after_defs = svg.split("</defs>").nth(1).unwrap_or("");
        assert!(
            !after_defs.contains("<path d=\"M"),
            "Edge with < 2 points should not render a path"
        );
    }

    #[test]
    fn test_edge_label_without_measured_dimensions() {
        let edge = PositionedEdge {
            from_id: "a".to_string(),
            to_id: "b".to_string(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("fallback".to_string()),
            label_x: Some(150.0),
            label_y: Some(150.0),
            label_width: None,
            label_height: None,
            points: vec![(50.0, 50.0), (100.0, 100.0), (150.0, 150.0)],
        };
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();
        assert!(
            svg.contains("fallback"),
            "edge label should appear even without measured dims"
        );
    }

    #[test]
    fn test_build_defs_uses_min_arrowhead_size() {
        // build_defs uses arrowhead_size.max(8.0) - when theme has smaller value, uses 8
        let mut theme = Theme::default();
        theme.flowchart.arrowhead_size = 5.0;
        let edge = make_edge("a", "b", EdgeType::SolidArrow, None);
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let svg = render_svg(&graph, &theme).unwrap();
        // markerWidth/markerHeight should be at least 8 (6.4 from 8*0.8)
        assert!(
            svg.contains("markerWidth="),
            "defs should contain marker dimensions"
        );
    }

    #[test]
    fn test_edge_label_with_br_tag() {
        let edge = make_edge("a", "b", EdgeType::SolidArrow, Some("Top<br/>Bottom"));
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("<tspan"),
            "Edge label with <br/> should produce tspan elements. SVG: {}",
            svg
        );
    }

    #[test]
    fn test_edge_label_with_html_tags_stripped() {
        // Single-line edge label with HTML - exercises strip_html_tags in the
        // lines.len() <= 1 branch of edge label rendering
        let edge = make_edge("a", "b", EdgeType::SolidArrow, Some("<b>bold</b> text"));
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains("bold text"),
            "HTML tags should be stripped from single-line edge label. SVG: {}",
            svg
        );
        assert!(
            !svg.contains("<b>"),
            "Raw HTML tags should not appear in output. SVG: {}",
            svg
        );
    }

    // ── New marker and edge style tests ─────────────────────────

    #[test]
    fn test_circle_end_marker() {
        let mut edge = make_edge("a", "b", EdgeType::SolidArrow, None);
        edge.arrow_end = ArrowEnd::Circle;
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains(r#"marker-end="url(#circle-end)""#),
            "Circle edge should have circle-end marker. SVG: {}",
            svg
        );
    }

    #[test]
    fn test_cross_end_marker() {
        let mut edge = make_edge("a", "b", EdgeType::SolidArrow, None);
        edge.arrow_end = ArrowEnd::Cross;
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains(r#"marker-end="url(#cross-end)""#),
            "Cross edge should have cross-end marker. SVG: {}",
            svg
        );
    }

    #[test]
    fn test_bidirectional_arrow_markers() {
        let mut edge = make_edge("a", "b", EdgeType::SolidArrow, None);
        edge.arrow_start = ArrowEnd::Arrow;
        edge.arrow_end = ArrowEnd::Arrow;
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains(r#"marker-end="url(#arrowhead)""#),
            "Bidirectional edge should have arrowhead marker-end. SVG: {}",
            svg
        );
        assert!(
            svg.contains(r#"marker-start="url(#arrowhead-start)""#),
            "Bidirectional edge should have arrowhead-start marker-start. SVG: {}",
            svg
        );
    }

    #[test]
    fn test_invisible_edge_not_rendered() {
        let edge = PositionedEdge {
            from_id: "a".to_string(),
            to_id: "b".to_string(),
            line_style: LineStyle::Invisible,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::None,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![(50.0, 50.0), (100.0, 100.0), (150.0, 150.0)],
        };
        let nodes = vec![
            make_node("a", "A", NodeShape::Rectangle),
            make_node("b", "B", NodeShape::Rectangle),
        ];
        let graph = make_graph(nodes, vec![edge], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        let after_defs = svg.split("</defs>").nth(1).unwrap_or("");
        assert!(
            !after_defs.contains("<path d=\"M"),
            "Invisible edge should not render a path in content. SVG: {}",
            svg
        );
    }

    #[test]
    fn test_defs_contain_all_markers() {
        let graph = make_graph(vec![], vec![], vec![]);
        let theme = Theme::default();
        let svg = render_svg(&graph, &theme).unwrap();

        assert!(
            svg.contains(r#"id="arrowhead""#),
            "should have arrowhead marker"
        );
        assert!(
            svg.contains(r#"id="arrowhead-start""#),
            "should have arrowhead-start marker"
        );
        assert!(
            svg.contains(r#"id="circle-end""#),
            "should have circle-end marker"
        );
        assert!(
            svg.contains(r#"id="circle-start""#),
            "should have circle-start marker"
        );
        assert!(
            svg.contains(r#"id="cross-end""#),
            "should have cross-end marker"
        );
        assert!(
            svg.contains(r#"id="cross-start""#),
            "should have cross-start marker"
        );
    }
}
