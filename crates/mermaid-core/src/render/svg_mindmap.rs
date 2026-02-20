use crate::error::Result;
use crate::layout::mindmap::*;
use crate::render::svg_util::escape_xml;
use crate::render::theme::Theme;

use crate::ast::mindmap::MindmapNodeShape;

const SVG_PADDING: f64 = 8.0;

// 12-color section palette matching mermaid.js mindmap default theme
const SECTION_COLORS: &[(&str, &str)] = &[
    ("#f0f0ff", "#9370DB"), // 0: light purple
    ("#ffffde", "#aaaa33"), // 1: light yellow
    ("#cdffb2", "#55aa55"), // 2: light green
    ("#ffc7c7", "#cc5555"), // 3: light red
    ("#c7e8ff", "#5588cc"), // 4: light blue
    ("#ffe4c7", "#cc8844"), // 5: light orange
    ("#e8c7ff", "#9955cc"), // 6: light magenta
    ("#c7ffe8", "#44aa88"), // 7: light cyan
    ("#ffd4e5", "#cc5588"), // 8: pink
    ("#d4e5ff", "#5566cc"), // 9: light steel
    ("#e5ffd4", "#66aa44"), // 10: lime
    ("#ffe5d4", "#cc7744"), // 11: peach
];

fn section_fill(section: usize) -> &'static str {
    SECTION_COLORS[section % SECTION_COLORS.len()].0
}

fn section_stroke(section: usize) -> &'static str {
    SECTION_COLORS[section % SECTION_COLORS.len()].1
}

/// Render a positioned mindmap layout to an SVG string.
pub fn render_svg(layout: &MindmapLayout, theme: &Theme) -> Result<String> {
    let view_w = (layout.width + 2.0 * SVG_PADDING).ceil();
    let view_h = (layout.height + 2.0 * SVG_PADDING).ceil();

    let mut svg = String::with_capacity(8192);

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
  .mindmap-node text {{ font-family: {}; font-size: {}px; fill: {}; }}
  .mindmap-root text {{ fill: white; }}
  .mindmap-edge {{ fill: none; }}
</style>"#,
        theme.background.to_css(),
        theme.font_family,
        theme.font_size,
        theme.text_color.to_css(),
    ));
    svg.push('\n');

    // Content group
    svg.push_str(&format!(
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    ));
    svg.push('\n');

    // Layer 1: Edges (behind everything)
    for edge in &layout.edges {
        svg.push_str(&render_edge(edge));
    }

    // Layer 2: Node shapes
    for node in &layout.nodes {
        svg.push_str(&render_node(node));
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn edge_stroke_width(depth: usize) -> f64 {
    // Thicker near root, tapering toward leaves (matches mermaid.js)
    match depth {
        0 => 6.0,
        1 => 4.0,
        2 => 2.5,
        _ => 1.5,
    }
}

fn render_edge(edge: &MindmapEdge) -> String {
    let stroke = section_stroke(edge.section);
    let width = edge_stroke_width(edge.depth);
    let (x1, y1) = edge.points[0];
    let (x2, y2) = *edge.points.last().unwrap();
    format!(
        r##"<line class="mindmap-edge" x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-opacity="0.7"/>"##,
        x1, y1, x2, y2, stroke, width,
    ) + "\n"
}

fn render_node(node: &PositionedMindmapNode) -> String {
    let mut s = String::new();

    let class = if node.depth == 0 {
        "mindmap-node mindmap-root"
    } else {
        "mindmap-node"
    };
    s.push_str(&format!(
        r#"<g class="{}" transform="translate({}, {})">"#,
        class, node.x, node.y,
    ));
    s.push('\n');

    // Draw shape — root uses dark fill (stroke color) for prominence
    let fill = if node.depth == 0 {
        section_stroke(0)
    } else {
        section_fill(node.section)
    };
    let stroke = if node.depth == 0 {
        section_stroke(0)
    } else {
        section_stroke(node.section)
    };

    match node.shape {
        MindmapNodeShape::Default => {
            // In mindmaps, bare text nodes get a rounded rect (matches mermaid.js)
            s.push_str(&render_rounded_rect(node, fill, stroke));
        }
        MindmapNodeShape::Rect => {
            s.push_str(&render_rect(node, fill, stroke));
        }
        MindmapNodeShape::RoundedRect => {
            s.push_str(&render_rounded_rect(node, fill, stroke));
        }
        MindmapNodeShape::Circle => {
            s.push_str(&render_circle(node, fill, stroke));
        }
        MindmapNodeShape::Cloud => {
            s.push_str(&render_cloud(node, fill, stroke));
        }
        MindmapNodeShape::Bang => {
            s.push_str(&render_bang(node, fill, stroke));
        }
        MindmapNodeShape::Hexagon => {
            s.push_str(&render_hexagon(node, fill, stroke));
        }
    }

    // Draw text label
    s.push_str(&render_text(node));

    s.push_str("</g>\n");
    s
}

fn render_rect(node: &PositionedMindmapNode, fill: &str, stroke: &str) -> String {
    let w = node.width;
    let h = node.height;
    format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" rx="0" fill="{}" stroke="{}" stroke-width="2"/>"##,
        -w / 2.0,
        -h / 2.0,
        w,
        h,
        fill,
        stroke,
    ) + "\n"
}

fn render_rounded_rect(node: &PositionedMindmapNode, fill: &str, stroke: &str) -> String {
    let w = node.width;
    let h = node.height;
    format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="2"/>"##,
        -w / 2.0,
        -h / 2.0,
        w,
        h,
        fill,
        stroke,
    ) + "\n"
}

fn render_circle(node: &PositionedMindmapNode, fill: &str, stroke: &str) -> String {
    let rx = node.width / 2.0;
    let ry = node.height / 2.0;
    format!(
        r##"<ellipse cx="0" cy="0" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="2"/>"##,
        rx, ry, fill, stroke,
    ) + "\n"
}

fn render_hexagon(node: &PositionedMindmapNode, fill: &str, stroke: &str) -> String {
    let w = node.width;
    let h = node.height;
    let inset = h / 4.0; // hexagon point inset

    let points = format!(
        "{},{} {},{} {},{} {},{} {},{} {},{}",
        -w / 2.0 + inset,
        -h / 2.0, // top-left
        w / 2.0 - inset,
        -h / 2.0, // top-right
        w / 2.0,
        0.0, // right
        w / 2.0 - inset,
        h / 2.0, // bottom-right
        -w / 2.0 + inset,
        h / 2.0, // bottom-left
        -w / 2.0,
        0.0, // left
    );

    format!(
        r##"<polygon points="{}" fill="{}" stroke="{}" stroke-width="2"/>"##,
        points, fill, stroke,
    ) + "\n"
}

fn render_cloud(node: &PositionedMindmapNode, fill: &str, stroke: &str) -> String {
    let w = node.width;
    let h = node.height;

    // Cloud shape using arcs — simplified version of mermaid.js cloudBkg
    let hw = w / 2.0;
    let hh = h / 2.0;
    let r = hh * 0.5;

    let mut path = format!("M {},{}", -hw, 0.0);
    // top-left arc
    path.push_str(&format!(" a {},{} 0 0,1 {},{}", r, r, hw * 0.5, -hh));
    // top-right arc
    path.push_str(&format!(" a {},{} 0 0,1 {},{}", r, r, hw * 0.5, 0.0));
    // right arc
    path.push_str(&format!(" a {},{} 0 0,1 {},{}", r, r, 0.0, hh * 0.5));
    // right-down arc
    path.push_str(&format!(" a {},{} 0 0,1 {},{}", r, r, 0.0, hh * 0.5));
    // bottom-right arc
    path.push_str(&format!(" a {},{} 0 0,1 {},{}", r, r, -hw * 0.5, hh * 0.0));
    // bottom-left arc
    path.push_str(&format!(" a {},{} 0 0,1 {},{}", r, r, -hw * 0.5, 0.0));
    // left-down arc
    path.push_str(&format!(" a {},{} 0 0,1 {},{}", r, r, 0.0, -hh * 0.5));
    // left-up arc
    path.push_str(&format!(" a {},{} 0 0,1 {},{}", r, r, 0.0, -hh * 0.5));
    path.push_str(" Z");

    format!(
        r##"<path d="{}" fill="{}" stroke="{}" stroke-width="2"/>"##,
        path, fill, stroke,
    ) + "\n"
}

fn render_bang(node: &PositionedMindmapNode, fill: &str, stroke: &str) -> String {
    let w = node.width;
    let h = node.height;

    // Bang (explosion) shape — jagged edges using arcs
    let hw = w / 2.0;
    let hh = h / 2.0;
    let jag = 6.0; // jaggedness

    let mut path = format!("M {},0", -hw);

    // Generate jagged outline
    let steps = 12;
    for i in 0..steps {
        let angle = std::f64::consts::PI * 2.0 * (i as f64) / (steps as f64);
        let r = if i % 2 == 0 {
            1.0
        } else {
            1.0 + jag / hw.max(hh)
        };
        let x = angle.cos() * hw * r;
        let y = angle.sin() * hh * r;
        path.push_str(&format!(" L {},{}", x, y));
    }
    path.push_str(" Z");

    format!(
        r##"<path d="{}" fill="{}" stroke="{}" stroke-width="2"/>"##,
        path, fill, stroke,
    ) + "\n"
}

fn render_text(node: &PositionedMindmapNode) -> String {
    let label = &node.label;
    let lines: Vec<&str> = label.lines().collect();
    let n = lines.len();

    if n == 1 {
        format!(
            r#"<text text-anchor="middle" dominant-baseline="central">{}</text>"#,
            escape_xml(lines[0]),
        ) + "\n"
    } else {
        let line_height = 1.2;
        let start_dy = -(((n - 1) as f64) / 2.0) * line_height;

        let mut s = String::from(r#"<text text-anchor="middle">"#);
        for (i, line) in lines.iter().enumerate() {
            let dy = if i == 0 {
                format!("{}em", start_dy)
            } else {
                format!("{}em", line_height)
            };
            s.push_str(&format!(
                r#"<tspan x="0" dy="{}" dominant-baseline="central">{}</tspan>"#,
                dy,
                escape_xml(line),
            ));
        }
        s.push_str("</text>\n");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a single-node layout with the given shape, depth, and section.
    fn single_node_layout(
        label: &str,
        shape: MindmapNodeShape,
        depth: usize,
        section: usize,
    ) -> MindmapLayout {
        MindmapLayout {
            width: 400.0,
            height: 300.0,
            nodes: vec![PositionedMindmapNode {
                id: "n1".into(),
                label: label.into(),
                shape,
                x: 200.0,
                y: 150.0,
                width: 100.0,
                height: 50.0,
                section,
                depth,
                icon: None,
                css_class: None,
            }],
            edges: vec![],
        }
    }

    #[test]
    fn test_render_simple_mindmap() {
        let layout = MindmapLayout {
            width: 400.0,
            height: 300.0,
            nodes: vec![PositionedMindmapNode {
                id: "root".into(),
                label: "Root".into(),
                shape: MindmapNodeShape::Circle,
                x: 200.0,
                y: 150.0,
                width: 80.0,
                height: 40.0,
                section: 0,
                depth: 0,
                icon: None,
                css_class: None,
            }],
            edges: vec![],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>\n"));
        assert!(svg.contains("Root"));
        assert!(svg.contains("ellipse")); // circle shape
    }

    #[test]
    fn test_cloud_shape() {
        let layout = single_node_layout("Clouds", MindmapNodeShape::Cloud, 1, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // Cloud shape is rendered as a <path> with arc commands
        assert!(svg.contains("<path"), "cloud must render a <path> element");
        assert!(svg.contains(" a "), "cloud path must contain arc ('a') commands");
        assert!(svg.contains("Clouds"));
    }

    #[test]
    fn test_bang_shape() {
        let layout = single_node_layout("Boom", MindmapNodeShape::Bang, 1, 1);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // Bang shape is rendered as a <path> with L (line-to) commands for jagged edges
        assert!(svg.contains("<path"), "bang must render a <path> element");
        assert!(svg.contains(" L "), "bang path must contain line-to ('L') commands");
        assert!(svg.contains("Boom"));
    }

    #[test]
    fn test_hexagon_shape() {
        let layout = single_node_layout("Hex", MindmapNodeShape::Hexagon, 1, 2);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(
            svg.contains("<polygon"),
            "hexagon must render a <polygon> element"
        );
        assert!(svg.contains("points="), "polygon must have a points attribute");
        assert!(svg.contains("Hex"));
    }

    #[test]
    fn test_rect_shape() {
        let layout = single_node_layout("Box", MindmapNodeShape::Rect, 1, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(svg.contains("<rect"), "rect must render a <rect> element");
        assert!(
            svg.contains(r#"rx="0""#),
            "rect shape must have rx=\"0\" (sharp corners)"
        );
        assert!(svg.contains("Box"));
    }

    #[test]
    fn test_rounded_rect_shape() {
        let layout = single_node_layout("Rounded", MindmapNodeShape::RoundedRect, 1, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(svg.contains("<rect"), "rounded rect must render a <rect> element");
        assert!(
            svg.contains(r#"rx="5""#),
            "rounded rect shape must have rx=\"5\""
        );
        assert!(svg.contains("Rounded"));
    }

    #[test]
    fn test_default_shape_renders_as_rounded_rect() {
        let layout = single_node_layout("Plain", MindmapNodeShape::Default, 1, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // Default shape renders as a rounded rect in mindmaps
        assert!(svg.contains("<rect"), "default shape must render a <rect> element");
        assert!(
            svg.contains(r#"rx="5""#),
            "default shape must render as rounded rect with rx=\"5\""
        );
        assert!(svg.contains("Plain"));
    }

    #[test]
    fn test_multiline_text_renders_tspan_elements() {
        let layout = single_node_layout("Line1\nLine2", MindmapNodeShape::RoundedRect, 1, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(
            svg.contains("<tspan"),
            "multi-line label must produce <tspan> elements"
        );
        assert!(svg.contains("Line1"), "first line text must be present");
        assert!(svg.contains("Line2"), "second line text must be present");
        // Should contain dy attributes for positioning
        assert!(svg.contains("dy="), "tspan elements must have dy attributes");
        assert!(
            svg.contains(r#"x="0""#),
            "tspan elements must reset x to 0"
        );
    }

    #[test]
    fn test_single_line_text_no_tspan() {
        let layout = single_node_layout("Single", MindmapNodeShape::RoundedRect, 1, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(
            !svg.contains("<tspan"),
            "single-line label should not produce <tspan> elements"
        );
        assert!(svg.contains("Single"));
        assert!(svg.contains("text-anchor=\"middle\""));
        assert!(svg.contains("dominant-baseline=\"central\""));
    }

    #[test]
    fn test_edge_rendering_depth_0() {
        let layout = MindmapLayout {
            width: 400.0,
            height: 300.0,
            nodes: vec![],
            edges: vec![MindmapEdge {
                from_id: "root".into(),
                to_id: "child".into(),
                points: vec![(100.0, 150.0), (300.0, 150.0)],
                section: 0,
                depth: 0,
            }],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(svg.contains("mindmap-edge"), "edge must have mindmap-edge class");
        // depth 0 => stroke-width 6
        assert!(
            svg.contains(r#"stroke-width="6""#),
            "depth-0 edge must have stroke-width 6"
        );
    }

    #[test]
    fn test_edge_rendering_depth_1() {
        let layout = MindmapLayout {
            width: 400.0,
            height: 300.0,
            nodes: vec![],
            edges: vec![MindmapEdge {
                from_id: "a".into(),
                to_id: "b".into(),
                points: vec![(50.0, 50.0), (200.0, 100.0)],
                section: 1,
                depth: 1,
            }],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // depth 1 => stroke-width 4
        assert!(
            svg.contains(r#"stroke-width="4""#),
            "depth-1 edge must have stroke-width 4"
        );
    }

    #[test]
    fn test_edge_rendering_depth_2() {
        let layout = MindmapLayout {
            width: 400.0,
            height: 300.0,
            nodes: vec![],
            edges: vec![MindmapEdge {
                from_id: "a".into(),
                to_id: "b".into(),
                points: vec![(50.0, 50.0), (200.0, 100.0)],
                section: 2,
                depth: 2,
            }],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // depth 2 => stroke-width 2.5
        assert!(
            svg.contains(r#"stroke-width="2.5""#),
            "depth-2 edge must have stroke-width 2.5"
        );
    }

    #[test]
    fn test_edge_rendering_depth_3_plus() {
        let layout = MindmapLayout {
            width: 400.0,
            height: 300.0,
            nodes: vec![],
            edges: vec![
                MindmapEdge {
                    from_id: "a".into(),
                    to_id: "b".into(),
                    points: vec![(50.0, 50.0), (200.0, 100.0)],
                    section: 0,
                    depth: 3,
                },
                MindmapEdge {
                    from_id: "b".into(),
                    to_id: "c".into(),
                    points: vec![(200.0, 100.0), (350.0, 150.0)],
                    section: 0,
                    depth: 5,
                },
            ],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // depth >= 3 => stroke-width 1.5
        assert!(
            svg.contains(r#"stroke-width="1.5""#),
            "depth-3+ edge must have stroke-width 1.5"
        );
    }

    #[test]
    fn test_root_node_has_mindmap_root_class() {
        let layout = single_node_layout("Root", MindmapNodeShape::Circle, 0, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(
            svg.contains("mindmap-root"),
            "depth-0 node must have mindmap-root class"
        );
        assert!(
            svg.contains("mindmap-node mindmap-root"),
            "root must have both mindmap-node and mindmap-root classes"
        );
    }

    #[test]
    fn test_non_root_node_no_mindmap_root_class() {
        let layout = single_node_layout("Child", MindmapNodeShape::RoundedRect, 1, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // The node group should contain "mindmap-node" but not "mindmap-root"
        assert!(svg.contains(r#"class="mindmap-node""#));
        // Make sure there is no "mindmap-root" on the node group
        assert!(
            !svg.contains(r#"class="mindmap-node mindmap-root""#),
            "non-root node must not have mindmap-root class"
        );
    }

    #[test]
    fn test_section_colors_cycling() {
        // Build nodes with sections 0..12 to verify cycling through all SECTION_COLORS
        let nodes: Vec<PositionedMindmapNode> = (0..13)
            .map(|i| PositionedMindmapNode {
                id: format!("n{}", i),
                label: format!("Section{}", i),
                shape: MindmapNodeShape::RoundedRect,
                x: 50.0 * i as f64,
                y: 100.0,
                width: 80.0,
                height: 40.0,
                section: i,
                depth: 1,
                icon: None,
                css_class: None,
            })
            .collect();

        let layout = MindmapLayout {
            width: 700.0,
            height: 300.0,
            nodes,
            edges: vec![],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Verify that section fills and strokes from the palette appear in the SVG.
        // Section 0 fill = #f0f0ff, stroke = #9370DB
        assert!(svg.contains("#f0f0ff"), "section 0 fill must be present");
        assert!(svg.contains("#9370DB"), "section 0 stroke must be present");
        // Section 1 fill = #ffffde, stroke = #aaaa33
        assert!(svg.contains("#ffffde"), "section 1 fill must be present");
        assert!(svg.contains("#aaaa33"), "section 1 stroke must be present");
        // Section 2 fill = #cdffb2, stroke = #55aa55
        assert!(svg.contains("#cdffb2"), "section 2 fill must be present");
        assert!(svg.contains("#55aa55"), "section 2 stroke must be present");
        // Section 3 fill = #ffc7c7, stroke = #cc5555
        assert!(svg.contains("#ffc7c7"), "section 3 fill must be present");
        // Section 12 wraps around to index 0 (12 % 12 == 0), so #f0f0ff appears again
        // (already verified above)

        // All 13 node labels should be present
        for i in 0..13 {
            assert!(
                svg.contains(&format!("Section{}", i)),
                "section {} label must be present",
                i
            );
        }
    }

    #[test]
    fn test_root_node_uses_stroke_color_as_fill() {
        // Root node (depth 0) should use section_stroke(0) for both fill and stroke
        let layout = single_node_layout("Root", MindmapNodeShape::RoundedRect, 0, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        let stroke_color = SECTION_COLORS[0].1; // "#9370DB"
        // The rect's fill and stroke should both be the stroke color
        let expected_fill = format!(r#"fill="{}""#, stroke_color);
        let expected_stroke = format!(r#"stroke="{}""#, stroke_color);
        assert!(
            svg.contains(&expected_fill),
            "root node fill must be the stroke color ({})",
            stroke_color
        );
        assert!(
            svg.contains(&expected_stroke),
            "root node stroke must be ({})",
            stroke_color
        );
    }

    #[test]
    fn test_nonroot_node_uses_section_fill_and_stroke() {
        // Non-root node (depth 1, section 2) should use section_fill(2) and section_stroke(2)
        let layout = single_node_layout("Child", MindmapNodeShape::RoundedRect, 1, 2);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        let fill_color = SECTION_COLORS[2].0; // "#cdffb2"
        let stroke_color = SECTION_COLORS[2].1; // "#55aa55"
        assert!(
            svg.contains(&format!(r#"fill="{}""#, fill_color)),
            "non-root node fill must be section fill color ({})",
            fill_color
        );
        assert!(
            svg.contains(&format!(r#"stroke="{}""#, stroke_color)),
            "non-root node stroke must be section stroke color ({})",
            stroke_color
        );
    }

    #[test]
    fn test_edge_uses_section_stroke_color() {
        let layout = MindmapLayout {
            width: 400.0,
            height: 300.0,
            nodes: vec![],
            edges: vec![MindmapEdge {
                from_id: "a".into(),
                to_id: "b".into(),
                points: vec![(0.0, 0.0), (100.0, 100.0)],
                section: 4,
                depth: 0,
            }],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        let edge_stroke = SECTION_COLORS[4].1; // "#5588cc"
        assert!(
            svg.contains(&format!(r#"stroke="{}""#, edge_stroke)),
            "edge must use section stroke color"
        );
    }

    #[test]
    fn test_multiline_three_lines() {
        let layout = single_node_layout("A\nB\nC", MindmapNodeShape::Rect, 1, 0);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Count tspan occurrences — should be 3 (one per line)
        let tspan_count = svg.matches("<tspan").count();
        assert_eq!(
            tspan_count, 3,
            "three-line label must produce exactly 3 <tspan> elements"
        );
        assert!(svg.contains(">A</tspan>"));
        assert!(svg.contains(">B</tspan>"));
        assert!(svg.contains(">C</tspan>"));
    }
}
