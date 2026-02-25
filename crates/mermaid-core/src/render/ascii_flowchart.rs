//! ASCII renderer for flowchart diagrams.
//!
//! Converts a `PositionedGraph` (the same data consumed by `svg_flowchart`) into
//! a Unicode text art representation.

use crate::ast::flowchart::{ArrowEnd, Direction, LineStyle, NodeShape};
use crate::error::Result;
use crate::layout::flowchart::types::PositionedGraph;
use crate::render::ascii_canvas::{ArrowDirection, TextCanvas};

/// Render a positioned flowchart graph as ASCII/Unicode text art.
pub fn render_ascii(graph: &PositionedGraph) -> Result<String> {
    let mut canvas = TextCanvas::from_pixel_size(graph.width, graph.height);

    // 1. Draw subgraphs first (background layer)
    for sg in &graph.subgraphs {
        draw_subgraph(&mut canvas, sg);
    }

    // 2. Draw edges (behind nodes)
    for edge in &graph.edges {
        draw_edge(&mut canvas, edge, graph.direction);
    }

    // 3. Draw nodes (foreground)
    for node in &graph.nodes {
        draw_node(&mut canvas, node);
    }

    Ok(canvas.to_string())
}

fn draw_subgraph(
    canvas: &mut TextCanvas,
    sg: &crate::layout::flowchart::types::PositionedSubgraph,
) {
    let left = canvas.px_to_col(sg.x);
    let top = canvas.px_to_row(sg.y);
    let right = canvas.px_to_col(sg.x + sg.width);
    let bottom = canvas.px_to_row(sg.y + sg.height);
    canvas.draw_box(left, top, right, bottom);

    // Draw label in top-left corner
    if let Some(ref label) = sg.label {
        let truncated = truncate_label(label, (right - left).saturating_sub(2));
        canvas.draw_text(left + 1, top, &truncated);
    }
}

fn draw_node(canvas: &mut TextCanvas, node: &crate::layout::flowchart::types::PositionedNode) {
    let cx = node.x;
    let cy = node.y;
    let half_w = node.width / 2.0;
    let half_h = node.height / 2.0;

    let left = canvas.px_to_col(cx - half_w);
    let top = canvas.px_to_row(cy - half_h);
    let right = canvas.px_to_col(cx + half_w);
    let bottom = canvas.px_to_row(cy + half_h);

    match node.shape {
        NodeShape::RoundedRectangle | NodeShape::Stadium => {
            canvas.draw_rounded_box(left, top, right, bottom);
        }
        NodeShape::Diamond => {
            let center_col = canvas.px_to_col(cx);
            let center_row = canvas.px_to_row(cy);
            let hw = (right - left) / 2;
            let hh = (bottom - top) / 2;
            canvas.draw_diamond(center_col, center_row, hw.max(1), hh.max(1));
        }
        NodeShape::Circle | NodeShape::DoubleCircle => {
            // Render circles as rounded boxes with parentheses
            canvas.draw_rounded_box(left, top, right, bottom);
        }
        NodeShape::Hexagon => {
            // Draw as a box with angle markers
            canvas.draw_box(left, top, right, bottom);
            // Overwrite left/right middle with angle brackets
            let mid_row = (top + bottom) / 2;
            canvas.put(left, mid_row, '⟨');
            canvas.put(right, mid_row, '⟩');
        }
        NodeShape::Parallelogram | NodeShape::ParallelogramAlt => {
            // Draw as a box with slash markers
            canvas.draw_box(left, top, right, bottom);
            canvas.put(left, top, '╱');
            canvas.put(right, bottom, '╱');
        }
        NodeShape::Cylinder => {
            // Draw as box with special top/bottom
            canvas.draw_box(left, top, right, bottom);
            // Mark top and bottom to suggest cylinder shape
            for c in (left + 1)..right {
                canvas.put(c, top, '⌒');
            }
        }
        NodeShape::Subroutine => {
            // Double vertical bars on sides
            canvas.draw_box(left, top, right, bottom);
            for r in (top + 1)..bottom {
                canvas.put(left + 1, r, '│');
                if right > 0 {
                    canvas.put(right - 1, r, '│');
                }
            }
        }
        NodeShape::Asymmetric => {
            // Flag shape: box with right side pointed
            canvas.draw_box(left, top, right, bottom);
            let mid_row = (top + bottom) / 2;
            canvas.put(right, mid_row, '▶');
        }
        NodeShape::Trapezoid | NodeShape::TrapezoidAlt => {
            canvas.draw_box(left, top, right, bottom);
        }
        // Default: rectangle
        NodeShape::Rectangle => {
            canvas.draw_box(left, top, right, bottom);
        }
    }

    // Draw label centered in the node
    let label = strip_html_basic(&node.label);
    let available_width = right.saturating_sub(left).saturating_sub(2);
    if available_width > 0 {
        let lines = wrap_text(&label, available_width);
        let total_lines = lines.len();
        let start_row = if total_lines == 1 {
            (top + bottom) / 2
        } else {
            let mid = (top + bottom) / 2;
            mid.saturating_sub(total_lines / 2)
        };
        for (i, line) in lines.iter().enumerate() {
            let row = start_row + i;
            if row > top && row < bottom {
                let truncated = truncate_label(line, available_width);
                let text_len = truncated.chars().count();
                let start_col = left + 1 + (available_width.saturating_sub(text_len)) / 2;
                canvas.draw_text(start_col, row, &truncated);
            }
        }
    }
}

fn draw_edge(
    canvas: &mut TextCanvas,
    edge: &crate::layout::flowchart::types::PositionedEdge,
    direction: Direction,
) {
    if edge.points.len() < 2 {
        return;
    }

    // Draw the line
    match edge.line_style {
        LineStyle::Dotted => {
            canvas.draw_polyline_dashed(&edge.points);
        }
        LineStyle::Thick => {
            canvas.draw_polyline_thick(&edge.points);
        }
        LineStyle::Invisible => {
            // Don't draw anything
            return;
        }
        _ => {
            canvas.draw_polyline(&edge.points);
        }
    }

    // Draw arrowhead at the end
    if edge.arrow_end != ArrowEnd::None {
        let last = edge.points[edge.points.len() - 1];
        let prev = edge.points[edge.points.len() - 2];
        let arrow_dir = infer_arrow_direction(prev, last, direction);
        let (ax, ay) = last;
        let ch = match edge.arrow_end {
            ArrowEnd::Arrow => match arrow_dir {
                ArrowDirection::Right => '▶',
                ArrowDirection::Left => '◀',
                ArrowDirection::Down => '▼',
                ArrowDirection::Up => '▲',
            },
            ArrowEnd::Circle => '●',
            ArrowEnd::Cross => '✕',
            ArrowEnd::None => ' ',
        };
        let c = canvas.px_to_col(ax);
        let r = canvas.px_to_row(ay);
        canvas.put(c, r, ch);
    }

    // Draw arrowhead at the start (for bidirectional edges)
    if edge.arrow_start != ArrowEnd::None {
        let first = edge.points[0];
        let second = edge.points[1];
        // Arrow points away from second toward first
        let arrow_dir = infer_arrow_direction(second, first, direction);
        let ch = match edge.arrow_start {
            ArrowEnd::Arrow => match arrow_dir {
                ArrowDirection::Right => '▶',
                ArrowDirection::Left => '◀',
                ArrowDirection::Down => '▼',
                ArrowDirection::Up => '▲',
            },
            ArrowEnd::Circle => '●',
            ArrowEnd::Cross => '✕',
            ArrowEnd::None => ' ',
        };
        let c = canvas.px_to_col(first.0);
        let r = canvas.px_to_row(first.1);
        canvas.put(c, r, ch);
    }

    // Draw edge label if present
    if let Some(ref label) = edge.label {
        if let (Some(lx), Some(ly)) = (edge.label_x, edge.label_y) {
            canvas.draw_text_centered_px(lx, ly, label);
        }
    }
}

/// Infer arrow direction from two consecutive points.
fn infer_arrow_direction(
    from: (f64, f64),
    to: (f64, f64),
    default_direction: Direction,
) -> ArrowDirection {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;

    if dx.abs() < 0.1 && dy.abs() < 0.1 {
        // Points are the same; use default flow direction
        return match default_direction {
            Direction::TopToBottom => ArrowDirection::Down,
            Direction::BottomToTop => ArrowDirection::Up,
            Direction::LeftToRight => ArrowDirection::Right,
            Direction::RightToLeft => ArrowDirection::Left,
        };
    }

    if dx.abs() > dy.abs() {
        if dx > 0.0 {
            ArrowDirection::Right
        } else {
            ArrowDirection::Left
        }
    } else if dy > 0.0 {
        ArrowDirection::Down
    } else {
        ArrowDirection::Up
    }
}

/// Strip basic HTML tags from a label (e.g., `<b>text</b>` -> `text`).
fn strip_html_basic(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    // Normalize <br> variants that were converted to newlines
    result.replace("<br>", "\n").replace("<br/>", "\n")
}

/// Truncate a label to fit within a character width, adding ellipsis if needed.
fn truncate_label(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        chars[..max_width].iter().collect()
    } else {
        let mut s: String = chars[..max_width - 1].iter().collect();
        s.push('…');
        s
    }
}

/// Simple word-wrap for text within a given character width.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![];
    }
    let mut lines = Vec::new();
    for line in text.split('\n') {
        if line.chars().count() <= max_width {
            lines.push(line.to_string());
        } else {
            // Simple character-based wrapping
            let chars: Vec<char> = line.chars().collect();
            let mut start = 0;
            while start < chars.len() {
                let end = (start + max_width).min(chars.len());
                lines.push(chars[start..end].iter().collect());
                start = end;
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::common::StyleProperties;
    use crate::layout::flowchart::types::{PositionedEdge, PositionedGraph, PositionedNode};

    fn simple_graph() -> PositionedGraph {
        PositionedGraph {
            width: 300.0,
            height: 200.0,
            direction: Direction::TopToBottom,
            subgraphs: vec![],
            nodes: vec![
                PositionedNode {
                    id: "A".to_string(),
                    label: "Start".to_string(),
                    shape: NodeShape::RoundedRectangle,
                    style: StyleProperties::default(),
                    x: 100.0,
                    y: 40.0,
                    width: 80.0,
                    height: 40.0,
                },
                PositionedNode {
                    id: "B".to_string(),
                    label: "End".to_string(),
                    shape: NodeShape::Rectangle,
                    style: StyleProperties::default(),
                    x: 100.0,
                    y: 140.0,
                    width: 80.0,
                    height: 40.0,
                },
            ],
            edges: vec![PositionedEdge {
                from_id: "A".to_string(),
                to_id: "B".to_string(),
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label: None,
                label_x: None,
                label_y: None,
                label_width: None,
                label_height: None,
                points: vec![(100.0, 60.0), (100.0, 120.0)],
            }],
        }
    }

    #[test]
    fn test_render_simple_flowchart() {
        let graph = simple_graph();
        let result = render_ascii(&graph).unwrap();
        assert!(!result.is_empty());
        // Should contain the labels
        assert!(
            result.contains("Start"),
            "Output should contain 'Start':\n{}",
            result
        );
        assert!(
            result.contains("End"),
            "Output should contain 'End':\n{}",
            result
        );
    }

    #[test]
    fn test_render_contains_box_chars() {
        let graph = simple_graph();
        let result = render_ascii(&graph).unwrap();
        // Should contain box-drawing characters
        assert!(
            result.contains('┌') || result.contains('╭'),
            "Output should contain box-drawing chars:\n{}",
            result
        );
    }

    #[test]
    fn test_render_contains_arrow() {
        let graph = simple_graph();
        let result = render_ascii(&graph).unwrap();
        // Should contain an arrow
        assert!(
            result.contains('▼') || result.contains('▶') || result.contains('│'),
            "Output should contain arrow or line chars:\n{}",
            result
        );
    }

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html_basic("<b>bold</b>"), "bold");
        assert_eq!(strip_html_basic("plain"), "plain");
        assert_eq!(strip_html_basic("<i>a</i> & <b>b</b>"), "a & b");
    }

    #[test]
    fn test_truncate_label() {
        assert_eq!(truncate_label("Hello", 10), "Hello");
        assert_eq!(truncate_label("Hello World", 5), "Hell…");
        assert_eq!(truncate_label("Hi", 2), "Hi");
    }

    #[test]
    fn test_truncate_label_zero_width() {
        assert_eq!(truncate_label("Hello", 0), "");
    }

    #[test]
    fn test_truncate_label_very_short_max() {
        assert_eq!(truncate_label("Hello", 3), "Hel");
        assert_eq!(truncate_label("Hello", 1), "H");
    }

    #[test]
    fn test_wrap_text_simple() {
        let result = wrap_text("Hello", 10);
        assert_eq!(result, vec!["Hello"]);
    }

    #[test]
    fn test_wrap_text_wraps_long_line() {
        let result = wrap_text("HelloWorld", 5);
        assert_eq!(result, vec!["Hello", "World"]);
    }

    #[test]
    fn test_wrap_text_preserves_newlines() {
        let result = wrap_text("Hi\nBye", 10);
        assert_eq!(result, vec!["Hi", "Bye"]);
    }

    #[test]
    fn test_wrap_text_zero_width() {
        let result = wrap_text("Hello", 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_strip_html_basic_nested() {
        assert_eq!(strip_html_basic("<div><b>inner</b></div>"), "inner");
    }

    #[test]
    fn test_strip_html_basic_no_tags() {
        assert_eq!(strip_html_basic("no tags here"), "no tags here");
    }

    #[test]
    fn test_infer_arrow_direction_right() {
        let dir = infer_arrow_direction((0.0, 0.0), (100.0, 0.0), Direction::TopToBottom);
        assert_eq!(dir, ArrowDirection::Right);
    }

    #[test]
    fn test_infer_arrow_direction_left() {
        let dir = infer_arrow_direction((100.0, 0.0), (0.0, 0.0), Direction::TopToBottom);
        assert_eq!(dir, ArrowDirection::Left);
    }

    #[test]
    fn test_infer_arrow_direction_down() {
        let dir = infer_arrow_direction((0.0, 0.0), (0.0, 100.0), Direction::TopToBottom);
        assert_eq!(dir, ArrowDirection::Down);
    }

    #[test]
    fn test_infer_arrow_direction_up() {
        let dir = infer_arrow_direction((0.0, 100.0), (0.0, 0.0), Direction::TopToBottom);
        assert_eq!(dir, ArrowDirection::Up);
    }

    #[test]
    fn test_infer_arrow_direction_same_point_uses_default() {
        let dir = infer_arrow_direction((50.0, 50.0), (50.0, 50.0), Direction::LeftToRight);
        assert_eq!(dir, ArrowDirection::Right);
        let dir = infer_arrow_direction((50.0, 50.0), (50.0, 50.0), Direction::BottomToTop);
        assert_eq!(dir, ArrowDirection::Up);
        let dir = infer_arrow_direction((50.0, 50.0), (50.0, 50.0), Direction::RightToLeft);
        assert_eq!(dir, ArrowDirection::Left);
    }

    #[test]
    fn test_render_diamond_node() {
        let graph = PositionedGraph {
            width: 200.0,
            height: 200.0,
            direction: Direction::TopToBottom,
            subgraphs: vec![],
            nodes: vec![PositionedNode {
                id: "D".to_string(),
                label: "Yes?".to_string(),
                shape: NodeShape::Diamond,
                style: StyleProperties::default(),
                x: 100.0,
                y: 100.0,
                width: 80.0,
                height: 60.0,
            }],
            edges: vec![],
        };
        let result = render_ascii(&graph).unwrap();
        assert!(result.contains("Yes?"));
    }

    #[test]
    fn test_render_various_node_shapes() {
        let shapes = vec![
            NodeShape::Circle,
            NodeShape::Hexagon,
            NodeShape::Cylinder,
            NodeShape::Subroutine,
            NodeShape::Asymmetric,
            NodeShape::Parallelogram,
            NodeShape::Trapezoid,
            NodeShape::Stadium,
        ];
        for shape in shapes {
            let graph = PositionedGraph {
                width: 200.0,
                height: 100.0,
                direction: Direction::TopToBottom,
                subgraphs: vec![],
                nodes: vec![PositionedNode {
                    id: "N".to_string(),
                    label: "Test".to_string(),
                    shape,
                    style: StyleProperties::default(),
                    x: 100.0,
                    y: 50.0,
                    width: 80.0,
                    height: 40.0,
                }],
                edges: vec![],
            };
            let result = render_ascii(&graph).unwrap();
            assert!(!result.is_empty(), "shape {:?} should render", shape);
        }
    }

    #[test]
    fn test_render_dotted_edge() {
        let graph = PositionedGraph {
            width: 200.0,
            height: 200.0,
            direction: Direction::TopToBottom,
            subgraphs: vec![],
            nodes: vec![],
            edges: vec![PositionedEdge {
                from_id: "A".to_string(),
                to_id: "B".to_string(),
                line_style: LineStyle::Dotted,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label: None,
                label_x: None,
                label_y: None,
                label_width: None,
                label_height: None,
                points: vec![(100.0, 20.0), (100.0, 180.0)],
            }],
        };
        let result = render_ascii(&graph).unwrap();
        assert!(
            result.contains('╎') || result.contains('╌'),
            "dotted edge should use dash chars"
        );
    }

    #[test]
    fn test_render_thick_edge() {
        let graph = PositionedGraph {
            width: 200.0,
            height: 200.0,
            direction: Direction::TopToBottom,
            subgraphs: vec![],
            nodes: vec![],
            edges: vec![PositionedEdge {
                from_id: "A".to_string(),
                to_id: "B".to_string(),
                line_style: LineStyle::Thick,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label: None,
                label_x: None,
                label_y: None,
                label_width: None,
                label_height: None,
                points: vec![(100.0, 20.0), (100.0, 180.0)],
            }],
        };
        let result = render_ascii(&graph).unwrap();
        assert!(
            result.contains('┃') || result.contains('━'),
            "thick edge should use thick chars"
        );
    }

    #[test]
    fn test_render_invisible_edge() {
        let graph = PositionedGraph {
            width: 200.0,
            height: 200.0,
            direction: Direction::TopToBottom,
            subgraphs: vec![],
            nodes: vec![],
            edges: vec![PositionedEdge {
                from_id: "A".to_string(),
                to_id: "B".to_string(),
                line_style: LineStyle::Invisible,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label: None,
                label_x: None,
                label_y: None,
                label_width: None,
                label_height: None,
                points: vec![(100.0, 20.0), (100.0, 180.0)],
            }],
        };
        let result = render_ascii(&graph).unwrap();
        // Invisible edge should not draw any line characters
        assert!(!result.contains('│') && !result.contains('─'));
    }

    #[test]
    fn test_render_bidirectional_edge() {
        let graph = PositionedGraph {
            width: 300.0,
            height: 100.0,
            direction: Direction::LeftToRight,
            subgraphs: vec![],
            nodes: vec![],
            edges: vec![PositionedEdge {
                from_id: "A".to_string(),
                to_id: "B".to_string(),
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::Arrow,
                arrow_end: ArrowEnd::Arrow,
                label: None,
                label_x: None,
                label_y: None,
                label_width: None,
                label_height: None,
                points: vec![(20.0, 50.0), (280.0, 50.0)],
            }],
        };
        let result = render_ascii(&graph).unwrap();
        // Should have arrows at both ends
        assert!(result.contains('▶') || result.contains('◀'));
    }

    #[test]
    fn test_render_edge_with_label() {
        let graph = PositionedGraph {
            width: 300.0,
            height: 200.0,
            direction: Direction::TopToBottom,
            subgraphs: vec![],
            nodes: vec![],
            edges: vec![PositionedEdge {
                from_id: "A".to_string(),
                to_id: "B".to_string(),
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label: Some("yes".to_string()),
                label_x: Some(150.0),
                label_y: Some(100.0),
                label_width: None,
                label_height: None,
                points: vec![(150.0, 20.0), (150.0, 180.0)],
            }],
        };
        let result = render_ascii(&graph).unwrap();
        assert!(result.contains("yes"), "edge label should be rendered");
    }

    #[test]
    fn test_render_circle_and_cross_arrows() {
        for (arrow, expected) in [(ArrowEnd::Circle, '●'), (ArrowEnd::Cross, '✕')] {
            let graph = PositionedGraph {
                width: 200.0,
                height: 200.0,
                direction: Direction::TopToBottom,
                subgraphs: vec![],
                nodes: vec![],
                edges: vec![PositionedEdge {
                    from_id: "A".to_string(),
                    to_id: "B".to_string(),
                    line_style: LineStyle::Solid,
                    arrow_start: ArrowEnd::None,
                    arrow_end: arrow,
                    label: None,
                    label_x: None,
                    label_y: None,
                    label_width: None,
                    label_height: None,
                    points: vec![(100.0, 20.0), (100.0, 180.0)],
                }],
            };
            let result = render_ascii(&graph).unwrap();
            assert!(
                result.contains(expected),
                "should contain {:?} for {:?}",
                expected,
                arrow
            );
        }
    }

    #[test]
    fn test_render_subgraph_with_label() {
        use crate::layout::flowchart::types::PositionedSubgraph;
        let graph = PositionedGraph {
            width: 300.0,
            height: 200.0,
            direction: Direction::TopToBottom,
            subgraphs: vec![PositionedSubgraph {
                id: "sg1".to_string(),
                label: Some("Group".to_string()),
                x: 10.0,
                y: 10.0,
                width: 200.0,
                height: 150.0,
                style: StyleProperties::default(),
            }],
            nodes: vec![],
            edges: vec![],
        };
        let result = render_ascii(&graph).unwrap();
        assert!(
            result.contains("Group"),
            "subgraph label should be rendered"
        );
        assert!(result.contains('┌'), "subgraph should have box");
    }
}
