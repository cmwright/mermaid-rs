pub mod compound;
pub mod edge_routing;
pub mod graph_builder;
pub mod normalize;
pub mod sugiyama;
pub mod types;

use crate::ast::flowchart::{Direction, FlowchartAst};
use crate::error::Result;
use crate::layout::text_measure::TextMeasurer;

// Re-export public types for API compatibility
pub use types::{PositionedEdge, PositionedGraph, PositionedNode, PositionedSubgraph};

use std::collections::HashMap;

use sugiyama::dummy_nodes::DummyChain;
use types::*;

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
    let mut positioned_subgraphs = compound::position_subgraphs(
        &ast.subgraphs,
        &positioned_nodes,
        &ast.style_overrides,
        measurer,
    );

    // 9. Ensure sibling subgraphs do not overlap
    compound::separate_overlapping_sibling_subgraphs(
        ast,
        &membership,
        &mut positioned_nodes,
        &positioned_subgraphs,
        &all_edges,
        is_horizontal,
    );
    positioned_subgraphs = compound::position_subgraphs(
        &ast.subgraphs,
        &positioned_nodes,
        &ast.style_overrides,
        measurer,
    );

    // 10. Extract bend points and label positions from dummy node positions, then route edges
    let extraction = build_edge_bend_points(&graph, &result.dummy_chains, &result.positions);
    let mut positioned_edges = edge_routing::route_edges(
        &positioned_nodes,
        &all_edges,
        is_horizontal,
        &extraction.bend_points,
        &extraction.label_positions,
        &extraction.label_dimensions,
    );

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

/// Result of extracting bend points and label positions from dummy chains.
struct DummyExtractionResult {
    bend_points: HashMap<(String, String), Vec<(f64, f64)>>,
    label_positions: HashMap<(String, String), (f64, f64)>,
    label_dimensions: HashMap<(String, String), (f64, f64)>,
}

/// Build a map from (source_id, target_id) → bend points for long edges,
/// and extract label positions from label dummy nodes.
///
/// Dummy nodes now participate in coordinate assignment (like dagre),
/// so we simply extract their positions as edge waypoints.
fn build_edge_bend_points(
    graph: &petgraph::graph::DiGraph<NodeData, EdgeData>,
    dummy_chains: &[DummyChain],
    positions: &HashMap<petgraph::graph::NodeIndex, (f64, f64)>,
) -> DummyExtractionResult {
    let mut bend_points = HashMap::new();
    let mut label_positions = HashMap::new();
    let mut label_dimensions = HashMap::new();

    for chain in dummy_chains {
        let src_id = graph[chain.original_source].id.clone();
        let tgt_id = graph[chain.original_target].id.clone();

        let bps: Vec<(f64, f64)> = chain
            .dummy_nodes
            .iter()
            .filter_map(|&dummy| positions.get(&dummy).copied())
            .collect();

        if !bps.is_empty() {
            // Store under both directions for reversed-edge lookup
            bend_points.insert((src_id.clone(), tgt_id.clone()), bps.clone());
            let rev: Vec<_> = bps.into_iter().rev().collect();
            bend_points.insert((tgt_id.clone(), src_id.clone()), rev);
        }

        // Extract label position from the label dummy node
        if let Some(label_dummy) = chain.label_node {
            if let Some(&pos) = positions.get(&label_dummy) {
                let key = (src_id.clone(), tgt_id.clone());
                label_positions.insert(key.clone(), pos);
                // Also store for reversed-edge lookup
                label_positions.insert((tgt_id.clone(), src_id.clone()), pos);
                // Store label dimensions
                let lw = chain.edge_data.label_width;
                let lh = chain.edge_data.label_height;
                label_dimensions.insert(key, (lw, lh));
                label_dimensions.insert((tgt_id, src_id), (lw, lh));
            }
        }
    }

    DummyExtractionResult {
        bend_points,
        label_positions,
        label_dimensions,
    }
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

    /// Regression test: in the "Larger flowchart with styling" example,
    /// sq→ci should be a straight vertical edge (same x-coordinate).
    #[test]
    fn test_sq_ci_vertical_alignment() {
        use crate::parser::flowchart::parse_flowchart;

        let source = r#"graph TB
    sq[Square shape] --> ci((Circle shape))

    subgraph A
        od>Odd shape]-- Two line<br/>edge comment --> ro
        di{Diamond with <br/> line break} -.-> ro(Rounded<br>square<br>shape)
        di==>ro2(Rounded square shape)
    end

    e --> od3>Really long text with linebreak<br>in an Odd shape]

    e((Inner / circle<br>and some odd <br>special characters)) --> f(,.?!+-*ز)

    cyr[Cyrillic]-->cyr2((Circle shape Начало));

     classDef green fill:#9f6,stroke:#333,stroke-width:2px;
     classDef orange fill:#f96,stroke:#333,stroke-width:4px;
     class sq,e green
     class di orange"#;

        let ast = parse_flowchart(source).unwrap();
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        let sq = result.nodes.iter().find(|n| n.id == "sq").unwrap();
        let ci = result.nodes.iter().find(|n| n.id == "ci").unwrap();

        // Print all node positions for debugging
        println!("\n=== Node positions ===");
        let mut nodes: Vec<_> = result.nodes.iter().collect();
        nodes.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap().then(a.x.partial_cmp(&b.x).unwrap()));
        for n in &nodes {
            println!("  {:>4} ({:>20}): x={:>8.1}, y={:>8.1}", n.id, n.label, n.x, n.y);
        }

        // sq should be directly above ci (same x-coordinate)
        let x_diff = (sq.x - ci.x).abs();
        assert!(
            x_diff < 1.0,
            "sq (x={:.1}) and ci (x={:.1}) should be vertically aligned (diff={:.1})",
            sq.x,
            ci.x,
            x_diff
        );
    }
}
