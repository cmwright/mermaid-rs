use std::fmt::Write;

use crate::ast::flowchart::{ArrowEnd, LineStyle};
use crate::error::Result;
use crate::layout::flowchart::{
    PositionedEdge, PositionedGraph, PositionedNode, PositionedSubgraph,
};
use crate::render::html_util;
use crate::render::svg_util::{build_basis_curve_path, escape_xml};
use crate::render::theme::Theme;

// Architecture nodes combine icon tiles + text labels and have long curved edges
// with markers; use a larger canvas inset so labels/arrowheads do not clip.
const SVG_PADDING: f64 = 20.0;

/// Render a positioned architecture graph to an SVG string.
pub fn render_svg(graph: &PositionedGraph, theme: &Theme) -> Result<String> {
    let view_w = graph.width + 2.0 * SVG_PADDING;
    let view_h = graph.height + 2.0 * SVG_PADDING;

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

    // Defs: arrow markers (same as flowchart)
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
    let text_color = node
        .style
        .color
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.flowchart.primary_text.to_css());
    let line_color = theme.line_color.to_css();

    let _ = write!(svg, r#"<g transform="translate({}, {})">"#, node.x, node.y);
    svg.push('\n');

    if let Some((icon_name, label_text)) = node.label.split_once('\n') {
        // Service node: blue icon box + label below
        let icon_size = 40.0;
        let box_y = -node.height / 2.0;
        let icon_fill = "#3b82f6";

        // Blue icon box
        let _ = write!(
            svg,
            r#"  <rect x="-20" y="{}" width="{}" height="{}" rx="4" fill="{}"/>"#,
            box_y, icon_size, icon_size, icon_fill,
        );
        svg.push('\n');

        // 24×24 Lucide icon centered in the 40×40 box (8px padding each side)
        let icon_x = -12.0;
        let icon_y = box_y + 8.0;
        if let Some(paths) = crate::render::icons::icon_paths(icon_name) {
            let _ = write!(
                svg,
                r#"  <svg x="{}" y="{}" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
                icon_x, icon_y, paths,
            );
        } else {
            // Unknown icon: render name as small text fallback
            let _ = write!(
                svg,
                r#"  <text x="0" y="{}" text-anchor="middle" dominant-baseline="central" font-size="9" fill="white">{}</text>"#,
                box_y + icon_size / 2.0,
                escape_xml(icon_name),
            );
        }
        svg.push('\n');

        // Label text below the box
        let label_y = box_y + icon_size + 4.0 + theme.font_size as f64 / 2.0;
        let _ = write!(
            svg,
            r#"  <text class="node-text" x="0" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
            label_y,
            text_color,
            escape_xml(label_text),
        );
        svg.push('\n');
    } else {
        // Junction node: small dot
        let _ = write!(
            svg,
            r#"  <circle cx="0" cy="0" r="5" fill="{}"/>"#,
            line_color,
        );
        svg.push('\n');
    }

    svg.push_str("</g>\n");
}

fn render_subgraph(svg: &mut String, sg: &PositionedSubgraph, theme: &Theme) {
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

    // Title label (with optional icon prefix encoded as "icon_name\nlabel_text")
    if let Some(label) = &sg.label {
        let text_color = theme.flowchart.subgraph_text.to_css();
        let label_y = sg.y + 18.0;

        // Split icon prefix from display label
        let (icon_name, display_label) = label
            .split_once('\n')
            .map(|(i, l)| (Some(i), l))
            .unwrap_or((None, label.as_str()));

        // Render small inline icon to the left of the label, if known
        let text_x = if let Some(icon_name) = icon_name {
            let icon_x = sg.x + 6.0;
            let icon_y = sg.y + 4.0;
            if let Some(paths) = crate::render::icons::icon_paths(icon_name) {
                let _ = write!(
                    svg,
                    r#"<svg x="{}" y="{}" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="{}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
                    icon_x, icon_y, text_color, paths,
                );
                svg.push('\n');
            }
            sg.x + 28.0 // icon (6 + 18) + 4px gap
        } else {
            sg.x + 8.0
        };

        let display_text = html_util::strip_html_tags(display_label);
        let _ = write!(
            svg,
            r#"<text x="{}" y="{}" text-anchor="start" font-family="{}" font-size="{}" font-weight="bold" fill="{}">{}</text>"#,
            text_x,
            label_y,
            theme.font_family,
            theme.font_size,
            text_color,
            escape_xml(&display_text),
        );
        svg.push('\n');
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

fn render_edge(svg: &mut String, edge: &PositionedEdge, theme: &Theme) {
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

    // Edge label
    if let (Some(label), Some(lx), Some(ly)) = (&edge.label, edge.label_x, edge.label_y) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::common::StyleProperties;
    use crate::ast::flowchart::{ArrowEnd, Direction, LineStyle, NodeShape};
    use crate::layout::flowchart::{
        PositionedEdge, PositionedGraph, PositionedNode, PositionedSubgraph,
    };
    use crate::render::theme::Theme;

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
            height: 200.0,
            direction: Direction::LeftToRight,
        }
    }

    #[test]
    fn viewbox_includes_architecture_padding() {
        let graph = make_graph(vec![], vec![], vec![]);
        let svg = render_svg(&graph, &Theme::default()).unwrap();
        // 400x200 graph + 20px inset on each side.
        assert!(svg.contains(r#"viewBox="0 0 440 240""#), "svg: {svg}");
    }

    fn make_node(id: &str, label: &str) -> PositionedNode {
        PositionedNode {
            id: id.to_string(),
            label: label.to_string(),
            shape: NodeShape::RoundedRectangle,
            style: StyleProperties::default(),
            x: 100.0,
            y: 100.0,
            width: 80.0,
            height: 60.0,
        }
    }

    fn content_after_defs(svg: &str) -> &str {
        svg.split("</defs>").nth(1).unwrap_or("")
    }

    #[test]
    fn service_node_renders_icon_box() {
        let node = make_node("srv", "server\nWeb Server");
        let graph = make_graph(vec![node], vec![], vec![]);
        let svg = render_svg(&graph, &Theme::default()).unwrap();
        let content = content_after_defs(&svg);

        // Blue icon box
        assert!(
            content.contains(r##"fill="#3b82f6""##),
            "missing blue box: {svg}"
        );
        // Nested icon SVG with white stroke
        assert!(
            content.contains(r#"stroke="white""#),
            "missing white stroke icon: {svg}"
        );
        // 24×24 icon dimensions
        assert!(
            content.contains(r#"width="24" height="24""#),
            "missing 24x24 icon svg: {svg}"
        );
        // Server icon path data (first rect of the server icon)
        assert!(
            content.contains(r#"<rect width="20" height="8""#),
            "missing server icon paths: {svg}"
        );
        // Label text
        assert!(
            content.contains(">Web Server<"),
            "missing label text: {svg}"
        );
    }

    #[test]
    fn service_node_unknown_icon_fallback() {
        let node = make_node("x", "notanicon\nMy Service");
        let graph = make_graph(vec![node], vec![], vec![]);
        let svg = render_svg(&graph, &Theme::default()).unwrap();
        let content = content_after_defs(&svg);

        // Falls back to small text, not a nested SVG icon
        assert!(
            content.contains(r#"font-size="9""#),
            "missing text fallback: {svg}"
        );
        assert!(
            !content.contains(r#"stroke="white""#),
            "should not render icon svg for unknown icon: {svg}"
        );
    }

    #[test]
    fn junction_node_renders_circle() {
        let node = make_node("j1", "j1");
        let graph = make_graph(vec![node], vec![], vec![]);
        let svg = render_svg(&graph, &Theme::default()).unwrap();
        let content = content_after_defs(&svg);

        assert!(
            content.contains("<circle"),
            "missing junction circle: {svg}"
        );
        // Should not have the blue icon box
        assert!(
            !content.contains(r##"fill="#3b82f6""##),
            "junction should not have blue icon box: {svg}"
        );
    }

    #[test]
    fn group_with_icon_renders_inline_svg() {
        let sg = PositionedSubgraph {
            id: "api".to_string(),
            label: Some("cloud\nAPI Layer".to_string()),
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
            style: StyleProperties::default(),
        };
        let graph = make_graph(vec![], vec![], vec![sg]);
        let svg = render_svg(&graph, &Theme::default()).unwrap();
        let content = content_after_defs(&svg);

        // 18×18 inline icon
        assert!(
            content.contains(r#"width="18" height="18""#),
            "missing 18x18 group icon: {svg}"
        );
        // Cloud icon path data
        assert!(
            content.contains("M17.5 19H9"),
            "missing cloud icon paths: {svg}"
        );
        // Label text
        assert!(
            content.contains(">API Layer<"),
            "missing group label: {svg}"
        );
    }

    #[test]
    fn group_without_icon_renders_text_only() {
        let sg = PositionedSubgraph {
            id: "g1".to_string(),
            label: Some("Plain Group".to_string()),
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
            style: StyleProperties::default(),
        };
        let graph = make_graph(vec![], vec![], vec![sg]);
        let svg = render_svg(&graph, &Theme::default()).unwrap();
        let content = content_after_defs(&svg);

        assert!(
            content.contains(">Plain Group<"),
            "missing group label: {svg}"
        );
        // No inline icon SVG
        assert!(
            !content.contains(r#"width="18" height="18""#),
            "plain group should not have icon svg: {svg}"
        );
    }

    #[test]
    fn edge_with_arrow_renders_marker() {
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
            points: vec![(50.0, 50.0), (100.0, 100.0), (150.0, 150.0)],
        };
        let graph = make_graph(vec![], vec![edge], vec![]);
        let svg = render_svg(&graph, &Theme::default()).unwrap();
        let content = content_after_defs(&svg);

        assert!(
            content.contains(r#"marker-end="url(#arrowhead)""#),
            "missing arrowhead marker: {svg}"
        );
        assert!(
            !content.contains("marker-start="),
            "should not have start marker: {svg}"
        );
    }
}
