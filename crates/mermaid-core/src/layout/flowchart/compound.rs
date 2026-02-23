use std::collections::HashMap;

use crate::ast::common::StyleProperties;
use crate::ast::flowchart::{StyleOverride, SubgraphDef};
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;
use crate::layout::text_measure::TextMeasurer;

const SUBGRAPH_TITLE_SIDE_PADDING: f64 = 18.0;

/// Position subgraphs as bounding boxes around their contained nodes.
/// Recursively processes nested subgraphs from innermost to outermost.
pub fn position_subgraphs(
    subgraphs: &[SubgraphDef],
    positioned_nodes: &[PositionedNode],
    style_overrides: &[StyleOverride],
    measurer: &TextMeasurer<'_>,
    membership: &SubgraphMembership,
) -> Vec<PositionedSubgraph> {
    let node_pos: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let mut result = Vec::new();
    position_subgraphs_recursive(
        subgraphs,
        &node_pos,
        style_overrides,
        measurer,
        &mut result,
        membership,
    );
    result
}

fn position_subgraphs_recursive(
    subgraphs: &[SubgraphDef],
    node_pos: &HashMap<&str, &PositionedNode>,
    style_overrides: &[StyleOverride],
    measurer: &TextMeasurer<'_>,
    result: &mut Vec<PositionedSubgraph>,
    membership: &SubgraphMembership,
) {
    for sg in subgraphs {
        position_subgraphs_recursive(
            &sg.subgraphs,
            node_pos,
            style_overrides,
            measurer,
            result,
            membership,
        );

        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut has_content = false;

        for node in &sg.nodes {
            // Skip nodes that don't actually belong to this subgraph
            // (they were added by cross-subgraph edge link chains)
            if let Some(path) = membership.get(&node.id) {
                if !path.contains(&sg.id) {
                    continue;
                }
            }
            if let Some(pn) = node_pos.get(node.id.as_str()) {
                min_x = min_x.min(pn.x - pn.width / 2.0);
                min_y = min_y.min(pn.y - pn.height / 2.0);
                max_x = max_x.max(pn.x + pn.width / 2.0);
                max_y = max_y.max(pn.y + pn.height / 2.0);
                has_content = true;
            }
        }

        for child_sg in &sg.subgraphs {
            if let Some(child_pos) = result.iter().find(|ps| ps.id == child_sg.id) {
                min_x = min_x.min(child_pos.x);
                min_y = min_y.min(child_pos.y);
                max_x = max_x.max(child_pos.x + child_pos.width);
                max_y = max_y.max(child_pos.y + child_pos.height);
                has_content = true;
            }
        }

        for edge in &sg.edges {
            for id in [&edge.from, &edge.to] {
                // Skip edge endpoints that belong to other subgraphs
                if let Some(path) = membership.get(id.as_str()) {
                    if !path.contains(&sg.id) {
                        continue;
                    }
                }
                if let Some(pn) = node_pos.get(id.as_str()) {
                    min_x = min_x.min(pn.x - pn.width / 2.0);
                    min_y = min_y.min(pn.y - pn.height / 2.0);
                    max_x = max_x.max(pn.x + pn.width / 2.0);
                    max_y = max_y.max(pn.y + pn.height / 2.0);
                    has_content = true;
                }
            }
        }

        if has_content {
            let title_height = if let Some(ref label) = sg.label {
                let normalized = crate::render::html_util::normalize_br(label);
                let line_count = normalized.split('\n').count();
                SUBGRAPH_TITLE_HEIGHT + (line_count.saturating_sub(1) as f64) * 16.0
            } else {
                SUBGRAPH_TITLE_HEIGHT
            };
            let title_text = sg.label.as_deref().unwrap_or(&sg.id);
            let title_width = measure_subgraph_title_width(title_text, measurer);
            let content_width = max_x - min_x;
            let min_required_width = title_width + 2.0 * SUBGRAPH_TITLE_SIDE_PADDING;
            if content_width < min_required_width {
                let extra = (min_required_width - content_width) / 2.0;
                min_x -= extra;
                max_x += extra;
            }

            let mut style = StyleProperties::default();
            for so in style_overrides {
                if so.node_id == sg.id {
                    style = style.merge(&so.properties);
                }
            }

            let label = sg.label.clone().or_else(|| Some(sg.id.clone()));

            result.push(PositionedSubgraph {
                id: sg.id.clone(),
                label,
                x: min_x - SUBGRAPH_PADDING,
                y: min_y - SUBGRAPH_PADDING - title_height,
                width: (max_x - min_x) + 2.0 * SUBGRAPH_PADDING,
                height: (max_y - min_y) + 2.0 * SUBGRAPH_PADDING + title_height,
                style,
            });
        }
    }
}

fn measure_subgraph_title_width(label: &str, measurer: &TextMeasurer<'_>) -> f64 {
    let normalized = crate::render::html_util::normalize_br(label);
    normalized
        .split('\n')
        .map(crate::render::html_util::strip_html_tags)
        .map(|line| measurer.measure(&line).width)
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::SubgraphDef;
    use crate::font::FontProvider;

    #[test]
    fn test_position_subgraphs() {
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: Some("Subgraph".to_string()),
            direction: None,
            nodes: vec![crate::ast::flowchart::NodeDef {
                id: "A".into(),
                label: None,
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                class_shorthand: None,
            }],
            edges: vec![],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 50.0,
            width: 40.0,
            height: 20.0,
        }];
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(
            &subgraphs,
            &positioned_nodes,
            &[],
            &measurer,
            &SubgraphMembership::new(),
        );
        assert_eq!(result.len(), 1);
        assert!(result[0].width > 0.0);
        assert!(result[0].height > 0.0);
    }

    #[test]
    fn test_position_subgraphs_nested() {
        let subgraphs = vec![SubgraphDef {
            id: "Outer".to_string(),
            label: None,
            direction: None,
            nodes: vec![],
            edges: vec![],
            subgraphs: vec![SubgraphDef {
                id: "Inner".to_string(),
                label: Some("Inner".to_string()),
                direction: None,
                nodes: vec![crate::ast::flowchart::NodeDef {
                    id: "A".into(),
                    label: None,
                    shape: crate::ast::flowchart::NodeShape::Rectangle,
                    class_shorthand: None,
                }],
                edges: vec![],
                subgraphs: vec![],
            }],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 80.0,
            width: 40.0,
            height: 20.0,
        }];
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(
            &subgraphs,
            &positioned_nodes,
            &[],
            &measurer,
            &SubgraphMembership::new(),
        );
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_measure_subgraph_title_width_multiline() {
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let w = measure_subgraph_title_width("Line1\nLine2\nLine3", &measurer);
        assert!(w > 0.0);
    }

    #[test]
    fn test_position_subgraphs_multiline_label() {
        // Subgraph with multiline label (line 113 - title_height with line_count > 1)
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: Some("Line1\nLine2\nLine3".to_string()),
            direction: None,
            nodes: vec![crate::ast::flowchart::NodeDef {
                id: "A".into(),
                label: None,
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                class_shorthand: None,
            }],
            edges: vec![],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 50.0,
            width: 40.0,
            height: 20.0,
        }];
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(
            &subgraphs,
            &positioned_nodes,
            &[],
            &measurer,
            &SubgraphMembership::new(),
        );
        assert_eq!(result.len(), 1);
        assert!(result[0].height > 0.0);
    }

    #[test]
    fn test_position_subgraphs_title_wider_than_content() {
        // content_width < min_required_width -> expand min_x/max_x (lines 87-91)
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: Some("Very Long Subgraph Title That Exceeds Content".to_string()),
            direction: None,
            nodes: vec![crate::ast::flowchart::NodeDef {
                id: "A".into(),
                label: None,
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                class_shorthand: None,
            }],
            edges: vec![],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 50.0,
            width: 10.0,
            height: 10.0,
        }];
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(
            &subgraphs,
            &positioned_nodes,
            &[],
            &measurer,
            &SubgraphMembership::new(),
        );
        assert_eq!(result.len(), 1);
        assert!(result[0].width >= 2.0 * SUBGRAPH_TITLE_SIDE_PADDING);
    }

    #[test]
    fn test_position_subgraphs_has_content_from_edges_only() {
        // Subgraph with has_content from edges (nodes in edges but not in node_pos)
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: None,
            direction: None,
            nodes: vec![],
            edges: vec![crate::ast::flowchart::EdgeDef {
                from: "A".into(),
                to: "B".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
            }],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![
            PositionedNode {
                id: "A".into(),
                label: "A".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B".into(),
                label: "B".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 150.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(
            &subgraphs,
            &positioned_nodes,
            &[],
            &measurer,
            &SubgraphMembership::new(),
        );
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_subgraph_style_overrides() {
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: Some("Styled".to_string()),
            direction: None,
            nodes: vec![crate::ast::flowchart::NodeDef {
                id: "A".into(),
                label: None,
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                class_shorthand: None,
            }],
            edges: vec![],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 50.0,
            width: 40.0,
            height: 20.0,
        }];
        let style_overrides = vec![crate::ast::flowchart::StyleOverride {
            node_id: "SG".into(),
            properties: crate::ast::common::parse_style_string("fill:#f96"),
        }];
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(
            &subgraphs,
            &positioned_nodes,
            &style_overrides,
            &measurer,
            &SubgraphMembership::new(),
        );
        assert_eq!(result.len(), 1);
        assert!(result[0].style.fill.is_some());
    }
}
