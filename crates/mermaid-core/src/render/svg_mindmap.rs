use crate::error::Result;
use crate::layout::mindmap::*;
use crate::render::svg_util::{build_basis_curve_path, escape_xml};
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
  .mindmap-edge {{ fill: none; stroke-width: 2; }}
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

fn render_edge(edge: &MindmapEdge) -> String {
    let stroke = section_stroke(edge.section);
    let path = build_basis_curve_path(&edge.points);
    format!(
        r##"<path class="mindmap-edge" d="{}" stroke="{}" stroke-opacity="0.5"/>"##,
        path, stroke,
    ) + "\n"
}

fn render_node(node: &PositionedMindmapNode) -> String {
    let mut s = String::new();

    s.push_str(&format!(
        r#"<g class="mindmap-node" transform="translate({}, {})">"#,
        node.x, node.y,
    ));
    s.push('\n');

    // Draw shape
    let fill = if node.depth == 0 {
        // Root gets a distinct fill
        section_fill(0)
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
            s.push_str(&render_default(node, stroke));
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

fn render_default(node: &PositionedMindmapNode, stroke: &str) -> String {
    // No background, just a bottom line
    let w = node.width;
    let h = node.height;
    format!(
        r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"##,
        -w / 2.0,
        h / 2.0,
        w / 2.0,
        h / 2.0,
        stroke,
    ) + "\n"
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
}
