use crate::ast::flowchart::{EdgeType, NodeShape};
use crate::error::Result;
use crate::layout::flowchart_layout::{
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

    let mut svg = String::with_capacity(4096);

    // SVG header
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        view_w as i64, view_h as i64, view_w as i64, view_h as i64,
    ));
    svg.push('\n');

    // Style block
    svg.push_str(&format!(
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
    ));
    svg.push('\n');

    // Defs: arrow markers
    svg.push_str(&build_defs(theme));

    // Content group with padding offset
    svg.push_str(&format!(
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    ));
    svg.push('\n');

    // Subgraph backgrounds (behind everything)
    for sg in &graph.subgraphs {
        svg.push_str(&render_subgraph(sg, theme));
    }

    // Edges (behind nodes)
    for edge in &graph.edges {
        svg.push_str(&render_edge(edge, theme));
    }

    // Nodes (on top)
    for node in &graph.nodes {
        svg.push_str(&render_node(node, theme));
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn build_defs(theme: &Theme) -> String {
    let line_color = theme.line_color.to_css();
    let sz = theme.arrowhead_size.max(8.0);

    format!(
        r#"<defs>
  <marker id="arrowhead" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="8.5" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 0 0 L 10 5 L 0 10 z" fill="{line_color}"/>
  </marker>
  <marker id="arrowhead-thick" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="8.5" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 0 0 L 10 5 L 0 10 z" fill="{line_color}"/>
  </marker>
</defs>
"#,
        mw = sz * 0.8,
        line_color = line_color,
    )
}

fn render_node(node: &PositionedNode, theme: &Theme) -> String {
    let fill = node
        .style
        .fill
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.primary_color.to_css());
    let stroke = node
        .style
        .stroke
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.primary_border.to_css());
    let stroke_width = node.style.stroke_width.unwrap_or(theme.node_border_width);
    let text_color = node
        .style
        .color
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.primary_text.to_css());

    let mut s = format!(r#"<g transform="translate({}, {})">"#, node.x, node.y);
    s.push('\n');

    // Draw shape
    s.push_str(&draw_shape(
        node.shape,
        node.width,
        node.height,
        &fill,
        &stroke,
        stroke_width,
    ));

    // Draw label text
    let label_lines: Vec<String> = html_util::normalize_br(&node.label)
        .lines()
        .map(|l| l.to_string())
        .collect();

    if label_lines.len() <= 1 && !html_util::has_html(&node.label) {
        // Simple single-line text (fast path)
        s.push_str(&format!(
            r#"  <text class="node-text" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
            text_color,
            escape_xml(&node.label),
        ));
        s.push('\n');
    } else {
        // Multi-line or HTML-formatted text
        let line_height = 1.2_f64; // em units
        let total_lines = label_lines.len();
        let start_dy = -((total_lines as f64 - 1.0) / 2.0) * line_height;

        s.push_str(&format!(
            r#"  <text class="node-text" text-anchor="middle" fill="{}">"#,
            text_color,
        ));
        s.push('\n');

        for (i, line) in label_lines.iter().enumerate() {
            let dy = if i == 0 {
                format!("{}em", start_dy)
            } else {
                format!("{}em", line_height)
            };

            let segments = html_util::parse_segments(line);
            if segments.is_empty() {
                s.push_str(&format!(
                    r#"    <tspan x="0" dy="{}" dominant-baseline="central">&#160;</tspan>"#,
                    dy,
                ));
                s.push('\n');
            } else {
                let mut first_in_line = true;
                for seg in &segments {
                    let mut attrs = String::new();
                    if first_in_line {
                        attrs.push_str(&format!(r#" x="0" dy="{}""#, dy));
                        first_in_line = false;
                    }
                    attrs.push_str(r#" dominant-baseline="central""#);
                    if seg.bold {
                        attrs.push_str(r#" font-weight="bold""#);
                    }
                    s.push_str(&format!(
                        r#"    <tspan{}>{}</tspan>"#,
                        attrs,
                        escape_xml(&seg.text),
                    ));
                    s.push('\n');
                }
            }
        }

        s.push_str("  </text>\n");
    }

    s.push_str("</g>\n");
    s
}

fn draw_shape(
    shape: NodeShape,
    w: f64,
    h: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f64,
) -> String {
    let hw = w / 2.0;
    let hh = h / 2.0;

    match shape {
        NodeShape::Rectangle => {
            format!(
                r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, -hh, w, h, fill, stroke, stroke_width,
            ) + "\n"
        }
        NodeShape::RoundedRectangle => {
            let rx = h.min(w) * 0.2;
            format!(
                r#"  <rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, -hh, w, h, rx, rx, fill, stroke, stroke_width,
            ) + "\n"
        }
        NodeShape::Stadium => {
            let rx = hh;
            format!(
                r#"  <rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, -hh, w, h, rx, rx, fill, stroke, stroke_width,
            ) + "\n"
        }
        NodeShape::Circle | NodeShape::DoubleCircle => {
            let r = hw.max(hh);
            let mut s = format!(
                r#"  <circle cx="0" cy="0" r="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                r, fill, stroke, stroke_width,
            ) + "\n";
            if shape == NodeShape::DoubleCircle {
                s += &format!(
                    r#"  <circle cx="0" cy="0" r="{}" fill="none" stroke="{}" stroke-width="{}"/>"#,
                    r - 5.0,
                    stroke,
                    stroke_width,
                );
                s.push('\n');
            }
            s
        }
        NodeShape::Diamond => {
            let points = format!("0,{} {},0 0,{} {},0", -hh, hw, hh, -hw);
            format!(
                r#"  <polygon points="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                points, fill, stroke, stroke_width,
            ) + "\n"
        }
        NodeShape::Hexagon => {
            let offset = hh * 0.5;
            let points = format!(
                "{},{} {},{} {},{} {},{} {},{} {},{}",
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
            );
            format!(
                r#"  <polygon points="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                points, fill, stroke, stroke_width,
            ) + "\n"
        }
        NodeShape::Subroutine => {
            // Rectangle with vertical lines near left and right edges
            let inset = 8.0;
            let mut s = format!(
                r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, -hh, w, h, fill, stroke, stroke_width,
            ) + "\n";
            s += &format!(
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw + inset,
                -hh,
                -hw + inset,
                hh,
                stroke,
                stroke_width,
            );
            s.push('\n');
            s += &format!(
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                hw - inset,
                -hh,
                hw - inset,
                hh,
                stroke,
                stroke_width,
            );
            s.push('\n');
            s
        }
        NodeShape::Cylinder => {
            // Cylinder using ellipses at top and bottom
            let ry = 8.0;
            let body_top = -hh + ry;
            let body_bot = hh - ry;
            let mut s = String::new();
            // Body rectangle
            s += &format!(
                r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="none"/>"#,
                -hw,
                body_top,
                w,
                body_bot - body_top,
                fill,
            );
            s.push('\n');
            // Left and right edges
            s += &format!(
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, body_top, -hw, body_bot, stroke, stroke_width,
            );
            s.push('\n');
            s += &format!(
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                hw, body_top, hw, body_bot, stroke, stroke_width,
            );
            s.push('\n');
            // Top ellipse
            s += &format!(
                r#"  <ellipse cx="0" cy="{}" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                body_top, hw, ry, fill, stroke, stroke_width,
            );
            s.push('\n');
            // Bottom ellipse
            s += &format!(
                r#"  <ellipse cx="0" cy="{}" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                body_bot, hw, ry, fill, stroke, stroke_width,
            );
            s.push('\n');
            s
        }
        NodeShape::Asymmetric => {
            // Flag-like shape: rectangle with pointed right edge
            let notch = hh * 0.6;
            let points = format!(
                "{},{} {},{} {},{} {},{} {},{}",
                -hw,
                -hh,
                hw - notch,
                -hh,
                hw,
                0.0,
                hw - notch,
                hh,
                -hw,
                hh,
            );
            format!(
                r#"  <polygon points="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                points, fill, stroke, stroke_width,
            ) + "\n"
        }
        NodeShape::Trapezoid => {
            let offset = hw * 0.2;
            let points = format!(
                "{},{} {},{} {},{} {},{}",
                -hw + offset,
                -hh,
                hw - offset,
                -hh,
                hw,
                hh,
                -hw,
                hh,
            );
            format!(
                r#"  <polygon points="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                points, fill, stroke, stroke_width,
            ) + "\n"
        }
        NodeShape::TrapezoidAlt => {
            let offset = hw * 0.2;
            let points = format!(
                "{},{} {},{} {},{} {},{}",
                -hw,
                -hh,
                hw,
                -hh,
                hw - offset,
                hh,
                -hw + offset,
                hh,
            );
            format!(
                r#"  <polygon points="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                points, fill, stroke, stroke_width,
            ) + "\n"
        }
        NodeShape::Parallelogram => {
            let offset = hw * 0.2;
            let points = format!(
                "{},{} {},{} {},{} {},{}",
                -hw + offset,
                -hh,
                hw,
                -hh,
                hw - offset,
                hh,
                -hw,
                hh,
            );
            format!(
                r#"  <polygon points="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                points, fill, stroke, stroke_width,
            ) + "\n"
        }
        NodeShape::ParallelogramAlt => {
            let offset = hw * 0.2;
            let points = format!(
                "{},{} {},{} {},{} {},{}",
                -hw,
                -hh,
                hw - offset,
                -hh,
                hw,
                hh,
                -hw + offset,
                hh,
            );
            format!(
                r#"  <polygon points="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                points, fill, stroke, stroke_width,
            ) + "\n"
        }
    }
}

fn render_edge(edge: &PositionedEdge, theme: &Theme) -> String {
    if edge.points.len() < 2 {
        return String::new();
    }

    let line_color = theme.line_color.to_css();
    let mut s = String::new();

    // Build path using B-spline interpolation (same as d3.curveBasis)
    let path_d = build_basis_curve_path(&edge.points);

    let mut attrs = format!(
        r#"d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round""#,
        path_d,
        line_color,
        theme.edge_width.max(1.75),
    );

    // Edge type styling
    match edge.edge_type {
        EdgeType::SolidArrow => {
            attrs.push_str(r#" marker-end="url(#arrowhead)""#);
        }
        EdgeType::DottedArrow => {
            attrs.push_str(r#" stroke-dasharray="3,3" marker-end="url(#arrowhead)""#);
        }
        EdgeType::ThickArrow => {
            // Override stroke-width for thick
            attrs = format!(
                r#"d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round" marker-end="url(#arrowhead)""#,
                path_d,
                line_color,
                theme.edge_width * 2.0,
            );
        }
        EdgeType::SolidLine => {}
        EdgeType::DottedLine => {
            attrs.push_str(r#" stroke-dasharray="3,3""#);
        }
        EdgeType::ThickLine => {
            attrs = format!(
                r#"d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round""#,
                path_d,
                line_color,
                theme.edge_width * 2.0,
            );
        }
    }

    s.push_str(&format!("<path {}/>\n", attrs));

    // Edge label
    if let (Some(label), Some(lx), Some(ly)) = (&edge.label, edge.label_x, edge.label_y) {
        // Use measured dimensions if available, otherwise fall back to rough approximation
        let label_w = edge.label_width.unwrap_or_else(|| label.len() as f64 * 8.0 + 10.0);
        let label_h = edge.label_height.unwrap_or(20.0);
        s.push_str(&format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="rgba(232,232,232,0.8)"/>"#,
            lx - label_w / 2.0,
            ly - label_h / 2.0,
            label_w,
            label_h,
        ));
        s.push('\n');
        s.push_str(&format!(
            r#"<text class="edge-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
            lx,
            ly,
            theme.text_color.to_css(),
            escape_xml(label),
        ));
        s.push('\n');
    }

    s
}

fn render_subgraph(sg: &PositionedSubgraph, theme: &Theme) -> String {
    let mut s = String::new();

    // Resolve fill/stroke from subgraph style overrides or theme defaults
    let fill = sg
        .style
        .fill
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.subgraph_fill.to_css());
    let stroke = sg
        .style
        .stroke
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.subgraph_border.to_css());

    // Background rectangle
    s.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="1" stroke-dasharray="5,5"/>"#,
        sg.x, sg.y, sg.width, sg.height,
        fill, stroke,
    ));
    s.push('\n');

    // Title label (handle <br/> as line breaks, strip other HTML)
    if let Some(label) = &sg.label {
        let clean = html_util::normalize_br(label);
        let lines: Vec<&str> = clean.split('\n').collect();
        let label_x = sg.x + sg.width / 2.0;
        let label_y = sg.y + 18.0;
        if lines.len() == 1 {
            let text = html_util::strip_html_tags(&lines[0]);
            s.push_str(&format!(
                r#"<text x="{}" y="{}" text-anchor="middle" font-family="{}" font-size="{}" font-weight="bold" fill="{}">{}</text>"#,
                label_x,
                label_y,
                theme.font_family,
                theme.font_size,
                theme.subgraph_text.to_css(),
                escape_xml(&text),
            ));
        } else {
            s.push_str(&format!(
                r#"<text x="{}" y="{}" text-anchor="middle" font-family="{}" font-size="{}" font-weight="bold" fill="{}">"#,
                label_x,
                label_y,
                theme.font_family,
                theme.font_size,
                theme.subgraph_text.to_css(),
            ));
            for (i, line) in lines.iter().enumerate() {
                let text = html_util::strip_html_tags(line);
                if i == 0 {
                    s.push_str(&format!(
                        r#"<tspan x="{}" dy="0">{}</tspan>"#,
                        label_x,
                        escape_xml(&text),
                    ));
                } else {
                    s.push_str(&format!(
                        r#"<tspan x="{}" dy="1.2em">{}</tspan>"#,
                        label_x,
                        escape_xml(&text),
                    ));
                }
            }
            s.push_str("</text>");
        }
        s.push('\n');
    }

    s
}

