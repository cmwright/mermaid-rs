use crate::ast::flowchart::{Direction, FlowchartAst};
use crate::error::Result;
use crate::layout::text_measure::TextMeasurer;

// Re-export public types for API compatibility
pub use crate::layout::types::{
    PositionedEdge, PositionedGraph, PositionedNode, PositionedSubgraph,
};

use crate::layout::compound;
use crate::layout::edge_routing;
use crate::layout::graph_builder;
use crate::layout::normalize;
use crate::layout::sugiyama;
use crate::layout::types::*;

/// Compute layout positions for a flowchart AST.
pub fn layout_flowchart(
    ast: &FlowchartAst,
    measurer: &TextMeasurer<'_>,
) -> Result<PositionedGraph> {
    let is_horizontal = matches!(
        ast.direction,
        Direction::LeftToRight | Direction::RightToLeft
    );

    // 1. Build class definitions map
    let class_defs = graph_builder::build_class_map(&ast.class_defs);

    // 2. Collect all nodes, merging style information
    let all_nodes = graph_builder::collect_all_nodes(ast, &class_defs);

    // 3. Collect all edges (including from subgraphs)
    let all_edges = graph_builder::collect_all_edges(ast);

    // 4. Build petgraph
    let (mut graph, _index_map) = graph_builder::build_petgraph(&all_nodes, &all_edges, measurer)?;

    // 5. Build subgraph membership map
    let membership = graph_builder::build_subgraph_membership(ast);

    // 6. Run Sugiyama layout pipeline
    let result = sugiyama::layout(&mut graph, ast.direction, &membership, ast);

    // 7. Build positioned nodes from Sugiyama results
    let mut positioned_nodes = build_positioned_nodes(&graph, &result.positions);

    // 8. Position subgraphs (with style overrides)
    let mut positioned_subgraphs =
        compound::position_subgraphs(&ast.subgraphs, &positioned_nodes, &ast.style_overrides);

    // 9. Ensure sibling subgraphs do not overlap
    compound::separate_overlapping_sibling_subgraphs(
        ast,
        &membership,
        &mut positioned_nodes,
        &positioned_subgraphs,
        &all_edges,
        is_horizontal,
    );
    positioned_subgraphs =
        compound::position_subgraphs(&ast.subgraphs, &positioned_nodes, &ast.style_overrides);

    // 10. Route edges after final node positions
    let mut positioned_edges = edge_routing::route_edges(&positioned_nodes, &all_edges, is_horizontal);

    // 11. Normalize coordinates and compute bounding box
    let (width, height) = normalize::normalize_and_compute_bounds(
        &mut positioned_nodes,
        &mut positioned_edges,
        &mut positioned_subgraphs,
    );

    Ok(PositionedGraph {
        nodes: positioned_nodes,
        edges: positioned_edges,
        subgraphs: positioned_subgraphs,
        width,
        height,
        direction: ast.direction,
    })
}

/// Build PositionedNode list from the graph and computed positions.
fn build_positioned_nodes(
    graph: &petgraph::graph::DiGraph<NodeData, EdgeData>,
    positions: &std::collections::HashMap<petgraph::graph::NodeIndex, (f64, f64)>,
) -> Vec<PositionedNode> {
    graph
        .node_indices()
        .filter_map(|idx| {
            let node = &graph[idx];
            // Skip dummy nodes
            if node.id.starts_with("__dummy_") {
                return None;
            }
            let &(x, y) = positions.get(&idx)?;
            Some(PositionedNode {
                id: node.id.clone(),
                label: node.label.clone(),
                shape: node.shape,
                style: node.style.clone(),
                x,
                y,
                width: node.width,
                height: node.height,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{EdgeDef, EdgeType, NodeDef, NodeShape};
    use crate::font::FontProvider;

    fn make_measurer(provider: &FontProvider) -> TextMeasurer<'_> {
        let font = provider.font_ref().unwrap();
        TextMeasurer::new(font, 14.0)
    }

    #[test]
    fn test_layout_simple() {
        let ast = FlowchartAst {
            direction: Direction::TopToBottom,
            nodes: vec![
                NodeDef {
                    id: "A".into(),
                    label: Some("Start".into()),
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                },
                NodeDef {
                    id: "B".into(),
                    label: Some("End".into()),
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                },
            ],
            edges: vec![EdgeDef {
                from: "A".into(),
                to: "B".into(),
                edge_type: EdgeType::SolidArrow,
                label: None,
            }],
            subgraphs: vec![],
            class_defs: vec![],
            class_assignments: vec![],
            style_overrides: vec![],
        };

        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.edges.len(), 1);

        // In TB direction, B should be below A
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let b = result.nodes.iter().find(|n| n.id == "B").unwrap();
        assert!(b.y > a.y, "B should be below A in TD layout");
    }

    #[test]
    fn test_layout_lr() {
        let ast = FlowchartAst {
            direction: Direction::LeftToRight,
            nodes: vec![
                NodeDef {
                    id: "A".into(),
                    label: Some("Left".into()),
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                },
                NodeDef {
                    id: "B".into(),
                    label: Some("Right".into()),
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                },
            ],
            edges: vec![EdgeDef {
                from: "A".into(),
                to: "B".into(),
                edge_type: EdgeType::SolidArrow,
                label: None,
            }],
            subgraphs: vec![],
            class_defs: vec![],
            class_assignments: vec![],
            style_overrides: vec![],
        };

        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let b = result.nodes.iter().find(|n| n.id == "B").unwrap();
        assert!(b.x > a.x, "B should be to the right of A in LR layout");
    }

    #[test]
    fn test_no_overlap() {
        let ast = FlowchartAst {
            direction: Direction::TopToBottom,
            nodes: vec![
                NodeDef {
                    id: "A".into(),
                    label: Some("Node A".into()),
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                },
                NodeDef {
                    id: "B".into(),
                    label: Some("Node B".into()),
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                },
                NodeDef {
                    id: "C".into(),
                    label: Some("Node C".into()),
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                },
            ],
            edges: vec![
                EdgeDef {
                    from: "A".into(),
                    to: "B".into(),
                    edge_type: EdgeType::SolidArrow,
                    label: None,
                },
                EdgeDef {
                    from: "A".into(),
                    to: "C".into(),
                    edge_type: EdgeType::SolidArrow,
                    label: None,
                },
            ],
            subgraphs: vec![],
            class_defs: vec![],
            class_assignments: vec![],
            style_overrides: vec![],
        };

        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        // Check no two nodes overlap
        for (i, a) in result.nodes.iter().enumerate() {
            for b in result.nodes.iter().skip(i + 1) {
                let overlap_x = (a.x - b.x).abs() < (a.width + b.width) / 2.0;
                let overlap_y = (a.y - b.y).abs() < (a.height + b.height) / 2.0;
                assert!(
                    !(overlap_x && overlap_y),
                    "Nodes {} and {} overlap at ({},{}) and ({},{})",
                    a.id,
                    b.id,
                    a.x,
                    a.y,
                    b.x,
                    b.y
                );
            }
        }
    }
}
