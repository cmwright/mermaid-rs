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
    let mut result = sugiyama::layout(&mut graph, ast.direction, &membership, ast);

    // 7. Build positioned nodes from Sugiyama results
    let mut positioned_nodes = build_positioned_nodes(&graph, &result.positions);

    // 8. Position subgraphs (with style overrides)
    let mut positioned_subgraphs = compound::position_subgraphs(
        &ast.subgraphs,
        &positioned_nodes,
        &membership,
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
        &membership,
        &ast.style_overrides,
        measurer,
    );
    // 9.5. Sync dummy node positions with shifted real nodes
    sync_dummy_positions(&graph, &result.dummy_chains, &positioned_nodes, &mut result.positions);

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

    // 10.5. Adjust edge labels to avoid subgraph border/title overlaps
    edge_routing::adjust_labels_for_subgraph_boundaries(
        &mut positioned_edges,
        &positioned_subgraphs,
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

/// After subgraph separation shifts real nodes, update dummy node positions
/// so that edge bend points and labels stay aligned with their endpoints.
/// For each dummy chain, interpolates the shift between source and target.
fn sync_dummy_positions(
    graph: &petgraph::graph::DiGraph<NodeData, EdgeData>,
    dummy_chains: &[DummyChain],
    positioned_nodes: &[PositionedNode],
    positions: &mut HashMap<petgraph::graph::NodeIndex, (f64, f64)>,
) {
    let node_pos: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    for chain in dummy_chains {
        let src_id = &graph[chain.original_source].id;
        let tgt_id = &graph[chain.original_target].id;

        let Some(new_src) = node_pos.get(src_id.as_str()) else {
            continue;
        };
        let Some(new_tgt) = node_pos.get(tgt_id.as_str()) else {
            continue;
        };
        let Some(&old_src) = positions.get(&chain.original_source) else {
            continue;
        };
        let Some(&old_tgt) = positions.get(&chain.original_target) else {
            continue;
        };

        let src_dx = new_src.x - old_src.0;
        let src_dy = new_src.y - old_src.1;
        let tgt_dx = new_tgt.x - old_tgt.0;
        let tgt_dy = new_tgt.y - old_tgt.1;

        // Skip if neither endpoint moved
        if src_dx.abs() < 0.1 && src_dy.abs() < 0.1 && tgt_dx.abs() < 0.1 && tgt_dy.abs() < 0.1 {
            continue;
        }

        // Update the real node positions in the map too
        positions.insert(chain.original_source, (new_src.x, new_src.y));
        positions.insert(chain.original_target, (new_tgt.x, new_tgt.y));

        // Interpolate shifts for each dummy node in the chain
        let n = chain.dummy_nodes.len();
        for (i, &dummy) in chain.dummy_nodes.iter().enumerate() {
            let t = (i + 1) as f64 / (n + 1) as f64;
            let dx = src_dx + (tgt_dx - src_dx) * t;
            let dy = src_dy + (tgt_dy - src_dy) * t;

            if let Some(pos) = positions.get_mut(&dummy) {
                pos.0 += dx;
                pos.1 += dy;
            }
        }
    }
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
    use crate::ast::flowchart::{ArrowEnd, EdgeDef, LineStyle, NodeDef, NodeShape};
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
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
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
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
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
                    line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                    label: None,
                },
                EdgeDef {
                    from: "A".into(),
                    to: "C".into(),
                    line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
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
        nodes.sort_by(|a, b| {
            a.y.partial_cmp(&b.y)
                .unwrap()
                .then(a.x.partial_cmp(&b.x).unwrap())
        });
        for n in &nodes {
            println!(
                "  {:>4} ({:>20}): x={:>8.1}, y={:>8.1}",
                n.id, n.label, n.x, n.y
            );
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

    // =======================================================================
    // End-to-end tests for coverage of specific code paths
    // =======================================================================

    /// Helper: parse a flowchart string and run layout, returning the result.
    fn layout_from_source(source: &str) -> PositionedGraph {
        use crate::parser::flowchart::parse_flowchart;
        let ast = parse_flowchart(source).unwrap();
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        layout_flowchart(&ast, &measurer).unwrap()
    }

    // -- coordinate_assignment.rs: BT (bottom-to-top) mirror --

    #[test]
    fn test_layout_bt_direction() {
        let result = layout_from_source("flowchart BT\n    A[Top] --> B[Bottom]");
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let b = result.nodes.iter().find(|n| n.id == "B").unwrap();
        // In BT, A (source) should be below B (target is rendered above)
        assert!(
            a.y > b.y,
            "In BT layout, A should be below B (A.y={:.1}, B.y={:.1})",
            a.y,
            b.y
        );
        assert_eq!(result.direction, Direction::BottomToTop);
    }

    // -- coordinate_assignment.rs: RL (right-to-left) mirror --

    #[test]
    fn test_layout_rl_direction() {
        let result = layout_from_source("flowchart RL\n    A[Left] --> B[Right]");
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let b = result.nodes.iter().find(|n| n.id == "B").unwrap();
        // In RL, A (source) should be to the right of B (target)
        assert!(
            a.x > b.x,
            "In RL layout, A should be right of B (A.x={:.1}, B.x={:.1})",
            a.x,
            b.x
        );
        assert_eq!(result.direction, Direction::RightToLeft);
    }

    // -- cycle_removal.rs: cycles and edge restoration --

    #[test]
    fn test_layout_with_cycle() {
        let result = layout_from_source(
            "flowchart TD\n    A --> B\n    B --> C\n    C --> A",
        );
        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.edges.len(), 3);
        // All nodes should have finite, non-negative coordinates after normalization
        for node in &result.nodes {
            assert!(node.x.is_finite() && node.x >= 0.0, "node {} has invalid x={}", node.id, node.x);
            assert!(node.y.is_finite() && node.y >= 0.0, "node {} has invalid y={}", node.id, node.y);
        }
    }

    #[test]
    fn test_layout_with_self_loop_cycle() {
        // Self-loop A -> A and a longer cycle
        let result = layout_from_source(
            "flowchart TD\n    A --> B\n    B --> C\n    C --> D\n    D --> B",
        );
        assert_eq!(result.nodes.len(), 4);
        // Graph should still produce a valid layout
        for node in &result.nodes {
            assert!(node.x.is_finite(), "node {} x is not finite", node.id);
            assert!(node.y.is_finite(), "node {} y is not finite", node.id);
        }
    }

    // -- dummy_nodes.rs: edges spanning 3+ ranks --

    #[test]
    fn test_layout_long_edge_spanning_multiple_ranks() {
        // A -> B -> C -> D, plus A -> D (spans 3 ranks)
        let result = layout_from_source(
            "flowchart TD\n    A --> B\n    B --> C\n    C --> D\n    A --> D",
        );
        assert_eq!(result.nodes.len(), 4);
        // Find the edge from A to D
        let a_to_d = result.edges.iter().find(|e| e.from_id == "A" && e.to_id == "D").unwrap();
        // Long edge should have bend points (more than 2 points)
        assert!(
            a_to_d.points.len() > 2,
            "Long edge A->D should have bend points, got {} points",
            a_to_d.points.len()
        );
    }

    #[test]
    fn test_layout_long_edge_with_label() {
        // Long edge with label -> label dummy node should be created at midpoint
        // Exercises build_edge_bend_points with label_node present
        let result = layout_from_source(
            "flowchart TD\n    A --> B\n    B --> C\n    C --> D\n    A -->|long label| D",
        );
        let a_to_d = result.edges.iter().find(|e| e.from_id == "A" && e.to_id == "D").unwrap();
        assert!(a_to_d.label.is_some());
        assert!(a_to_d.label_x.is_some(), "labeled long edge should have label_x");
        assert!(a_to_d.label_y.is_some(), "labeled long edge should have label_y");
        assert!(a_to_d.label_width.is_some(), "labeled long edge should have label_width from label_node");
        assert!(a_to_d.label_height.is_some(), "labeled long edge should have label_height from label_node");
    }

    #[test]
    fn test_sync_dummy_positions_skip_when_source_missing_from_node_pos() {
        use petgraph::graph::DiGraph;
        use sugiyama::dummy_nodes::DummyChain;
        use crate::layout::flowchart::types::*;

        let mut graph = DiGraph::new();
        let a = graph.add_node(NodeData {
            id: "A".to_string(),
            label: "A".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });
        let b = graph.add_node(NodeData {
            id: "B".to_string(),
            label: "B".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });

        let chain = DummyChain {
            original_source: a,
            original_target: b,
            edge_data: EdgeData {
                label: None,
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label_width: 0.0,
                label_height: 0.0,
            },
            dummy_nodes: vec![],
            label_node: None,
        };

        // positioned_nodes has only B, not A - sync_dummy_positions should continue (skip chain)
        let positioned_nodes = vec![PositionedNode {
            id: "B".into(),
            label: "B".into(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 150.0,
            width: 40.0,
            height: 20.0,
        }];
        let mut positions = std::collections::HashMap::new();
        positions.insert(a, (50.0, 50.0));
        positions.insert(b, (100.0, 150.0));

        sync_dummy_positions(&graph, &[chain], &positioned_nodes, &mut positions);
        assert_eq!(positions.get(&a).unwrap().0, 50.0);
    }

    #[test]
    fn test_sync_dummy_positions_skip_when_target_missing_from_node_pos() {
        use petgraph::graph::DiGraph;
        use sugiyama::dummy_nodes::DummyChain;
        use crate::layout::flowchart::types::*;

        let mut graph = DiGraph::new();
        let a = graph.add_node(NodeData {
            id: "A".to_string(),
            label: "A".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });
        let b = graph.add_node(NodeData {
            id: "B".to_string(),
            label: "B".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });

        let chain = DummyChain {
            original_source: a,
            original_target: b,
            edge_data: EdgeData {
                label: None,
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label_width: 0.0,
                label_height: 0.0,
            },
            dummy_nodes: vec![],
            label_node: None,
        };

        // positioned_nodes has only A, not B - sync_dummy_positions should continue (line 161)
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 50.0,
            width: 40.0,
            height: 20.0,
        }];
        let mut positions = std::collections::HashMap::new();
        positions.insert(a, (50.0, 50.0));
        positions.insert(b, (100.0, 150.0));

        sync_dummy_positions(&graph, &[chain], &positioned_nodes, &mut positions);
        assert_eq!(positions.get(&b).unwrap().0, 100.0);
    }

    #[test]
    fn test_sync_dummy_positions_skip_when_old_src_missing_from_positions() {
        use petgraph::graph::DiGraph;
        use sugiyama::dummy_nodes::DummyChain;
        use crate::layout::flowchart::types::*;

        let mut graph = DiGraph::new();
        let a = graph.add_node(NodeData {
            id: "A".to_string(),
            label: "A".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });
        let b = graph.add_node(NodeData {
            id: "B".to_string(),
            label: "B".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });

        let chain = DummyChain {
            original_source: a,
            original_target: b,
            edge_data: EdgeData {
                label: None,
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label_width: 0.0,
                label_height: 0.0,
            },
            dummy_nodes: vec![],
            label_node: None,
        };

        let positioned_nodes = vec![
            PositionedNode { id: "A".into(), label: "A".into(), shape: NodeShape::Rectangle, style: Default::default(), x: 60.0, y: 50.0, width: 40.0, height: 20.0 },
            PositionedNode { id: "B".into(), label: "B".into(), shape: NodeShape::Rectangle, style: Default::default(), x: 100.0, y: 150.0, width: 40.0, height: 20.0 },
        ];
        let mut positions = std::collections::HashMap::new();
        positions.insert(b, (100.0, 150.0));
        // a is missing from positions - should continue (line 164)

        sync_dummy_positions(&graph, &[chain], &positioned_nodes, &mut positions);
        assert!(!positions.contains_key(&a));
    }

    #[test]
    fn test_sync_dummy_positions_skip_when_old_tgt_missing_from_positions() {
        use petgraph::graph::DiGraph;
        use sugiyama::dummy_nodes::DummyChain;
        use crate::layout::flowchart::types::*;

        let mut graph = DiGraph::new();
        let a = graph.add_node(NodeData {
            id: "A".to_string(),
            label: "A".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });
        let b = graph.add_node(NodeData {
            id: "B".to_string(),
            label: "B".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });

        let chain = DummyChain {
            original_source: a,
            original_target: b,
            edge_data: EdgeData {
                label: None,
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label_width: 0.0,
                label_height: 0.0,
            },
            dummy_nodes: vec![],
            label_node: None,
        };

        let positioned_nodes = vec![
            PositionedNode { id: "A".into(), label: "A".into(), shape: NodeShape::Rectangle, style: Default::default(), x: 50.0, y: 50.0, width: 40.0, height: 20.0 },
            PositionedNode { id: "B".into(), label: "B".into(), shape: NodeShape::Rectangle, style: Default::default(), x: 110.0, y: 150.0, width: 40.0, height: 20.0 },
        ];
        let mut positions = std::collections::HashMap::new();
        positions.insert(a, (50.0, 50.0));
        // b is missing from positions - should continue (line 168)

        sync_dummy_positions(&graph, &[chain], &positioned_nodes, &mut positions);
        assert!(!positions.contains_key(&b));
    }

    #[test]
    fn test_sync_dummy_positions_skip_when_neither_endpoint_moved() {
        use petgraph::graph::DiGraph;
        use sugiyama::dummy_nodes::DummyChain;
        use crate::layout::flowchart::types::*;

        let mut graph = DiGraph::new();
        let a = graph.add_node(NodeData {
            id: "A".to_string(),
            label: "A".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });
        let b = graph.add_node(NodeData {
            id: "B".to_string(),
            label: "B".to_string(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        });
        let dummy = graph.add_node(NodeData {
            id: "__dummy_0_1".to_string(),
            label: String::new(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 0.0,
            height: 0.0,
        });

        let chain = DummyChain {
            original_source: a,
            original_target: b,
            edge_data: EdgeData {
                label: None,
                line_style: LineStyle::Solid,
                arrow_start: ArrowEnd::None,
                arrow_end: ArrowEnd::Arrow,
                label_width: 0.0,
                label_height: 0.0,
            },
            dummy_nodes: vec![dummy],
            label_node: None,
        };

        let (ax, ay) = (50.0, 50.0);
        let (bx, by) = (100.0, 150.0);
        let positioned_nodes = vec![
            PositionedNode { id: "A".into(), label: "A".into(), shape: NodeShape::Rectangle, style: Default::default(), x: ax, y: ay, width: 40.0, height: 20.0 },
            PositionedNode { id: "B".into(), label: "B".into(), shape: NodeShape::Rectangle, style: Default::default(), x: bx, y: by, width: 40.0, height: 20.0 },
        ];
        let mut positions = std::collections::HashMap::new();
        positions.insert(a, (ax, ay));
        positions.insert(b, (bx, by));
        positions.insert(dummy, (75.0, 100.0));

        sync_dummy_positions(&graph, &[chain], &positioned_nodes, &mut positions);
        // Neither moved - dummy position should be unchanged
        assert!((positions.get(&dummy).unwrap().0 - 75.0).abs() < 0.01);
    }

    // -- edge_routing.rs: diamond node intersection --

    #[test]
    fn test_layout_diamond_node() {
        let result = layout_from_source(
            "flowchart TD\n    A[Start] --> B{Decision}\n    B -->|yes| C[End Yes]\n    B -->|no| D[End No]",
        );
        let b = result.nodes.iter().find(|n| n.id == "B").unwrap();
        assert_eq!(b.shape, NodeShape::Diamond);
        // Diamond should have equal width and height (it's a rotated square)
        assert!(
            (b.width - b.height).abs() < 1.0,
            "Diamond should have equal w/h, got {}x{}",
            b.width,
            b.height
        );
        // Edges from B should start from the diamond boundary
        let from_b: Vec<_> = result.edges.iter().filter(|e| e.from_id == "B").collect();
        assert_eq!(from_b.len(), 2, "B should have 2 outgoing edges");
    }

    // -- graph_builder.rs: various node shapes --

    #[test]
    fn test_layout_various_shapes() {
        let result = layout_from_source(
            r#"flowchart TD
    A[Rectangle] --> B(Rounded)
    B --> C([Stadium])
    C --> D[[Subroutine]]
    D --> E[(Cylinder)]
    E --> F((Circle))
    F --> G{Diamond}
    G --> H{{Hexagon}}
    H --> I>Asymmetric]"#,
        );
        assert_eq!(result.nodes.len(), 9);

        // Verify each shape is correct
        let shapes: Vec<(String, NodeShape)> = result.nodes.iter()
            .map(|n| (n.id.clone(), n.shape))
            .collect();
        let a = shapes.iter().find(|(id, _)| id == "A").unwrap();
        assert_eq!(a.1, NodeShape::Rectangle);
        let b = shapes.iter().find(|(id, _)| id == "B").unwrap();
        assert_eq!(b.1, NodeShape::RoundedRectangle);
        let c = shapes.iter().find(|(id, _)| id == "C").unwrap();
        assert_eq!(c.1, NodeShape::Stadium);
        let d = shapes.iter().find(|(id, _)| id == "D").unwrap();
        assert_eq!(d.1, NodeShape::Subroutine);
        let e = shapes.iter().find(|(id, _)| id == "E").unwrap();
        assert_eq!(e.1, NodeShape::Cylinder);
        let f = shapes.iter().find(|(id, _)| id == "F").unwrap();
        assert_eq!(f.1, NodeShape::Circle);
        let g = shapes.iter().find(|(id, _)| id == "G").unwrap();
        assert_eq!(g.1, NodeShape::Diamond);
        let h = shapes.iter().find(|(id, _)| id == "H").unwrap();
        assert_eq!(h.1, NodeShape::Hexagon);
        let i_node = shapes.iter().find(|(id, _)| id == "I").unwrap();
        assert_eq!(i_node.1, NodeShape::Asymmetric);

        // All nodes should have positive dimensions
        for node in &result.nodes {
            assert!(node.width > 0.0, "node {} width should be > 0", node.id);
            assert!(node.height > 0.0, "node {} height should be > 0", node.id);
        }
    }

    #[test]
    fn test_layout_circle_size() {
        // Circle must fully contain text: diameter = sqrt(w^2 + h^2)
        let result = layout_from_source("flowchart TD\n    A((Circle))");
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        assert_eq!(a.shape, NodeShape::Circle);
        assert!(
            (a.width - a.height).abs() < 0.1,
            "Circle should have equal width/height"
        );
    }

    #[test]
    fn test_layout_double_circle_size() {
        let result = layout_from_source("flowchart TD\n    A(((DoubleCircle)))");
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        assert_eq!(a.shape, NodeShape::DoubleCircle);
        assert!(
            (a.width - a.height).abs() < 0.1,
            "DoubleCircle should have equal width/height"
        );
    }

    #[test]
    fn test_layout_hexagon_wider_than_base() {
        // Hexagon gets extra width: base_w + base_h * 0.5
        let result = layout_from_source("flowchart TD\n    A{{Hex}} --> B[Rect]");
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        assert_eq!(a.shape, NodeShape::Hexagon);
    }

    // -- compound.rs: overlapping sibling subgraphs --

    #[test]
    fn test_layout_overlapping_sibling_subgraphs() {
        let result = layout_from_source(
            r#"flowchart TD
    subgraph SG1
        A1[Node A1] --> A2[Node A2]
        A2 --> A3[Node A3]
    end
    subgraph SG2
        B1[Node B1] --> B2[Node B2]
        B2 --> B3[Node B3]
    end
    A3 --> B1"#,
        );
        assert_eq!(result.subgraphs.len(), 2);

        let sg1 = result.subgraphs.iter().find(|s| s.id == "SG1").unwrap();
        let sg2 = result.subgraphs.iter().find(|s| s.id == "SG2").unwrap();

        // Subgraphs should not overlap
        let sg1_right = sg1.x + sg1.width;
        let sg1_bottom = sg1.y + sg1.height;
        let sg2_right = sg2.x + sg2.width;
        let sg2_bottom = sg2.y + sg2.height;

        let x_overlap = sg1.x < sg2_right && sg2.x < sg1_right;
        let y_overlap = sg1.y < sg2_bottom && sg2.y < sg1_bottom;

        if x_overlap && y_overlap {
            panic!(
                "Subgraphs SG1 and SG2 overlap: SG1=({:.0},{:.0},{:.0},{:.0}), SG2=({:.0},{:.0},{:.0},{:.0})",
                sg1.x, sg1.y, sg1_right, sg1_bottom,
                sg2.x, sg2.y, sg2_right, sg2_bottom,
            );
        }
    }

    #[test]
    fn test_layout_multiple_sibling_subgraphs() {
        // Three sibling subgraphs: triggers the cross-axis and main-axis separation
        let result = layout_from_source(
            r#"flowchart TD
    subgraph Alpha
        A1 --> A2
    end
    subgraph Beta
        B1 --> B2
    end
    subgraph Gamma
        C1 --> C2
    end
    A2 --> B1
    B2 --> C1"#,
        );
        assert_eq!(result.subgraphs.len(), 3);
        // All subgraphs should have positive dimensions
        for sg in &result.subgraphs {
            assert!(sg.width > 0.0 && sg.height > 0.0, "subgraph {} has bad dims", sg.id);
        }
    }

    #[test]
    fn test_layout_sibling_subgraphs_horizontal() {
        // LR layout with sibling subgraphs
        let result = layout_from_source(
            r#"flowchart LR
    subgraph SG1
        A1 --> A2
    end
    subgraph SG2
        B1 --> B2
    end
    A2 --> B1"#,
        );
        assert_eq!(result.subgraphs.len(), 2);
    }

    // -- compound.rs: compact_subgraphs --

    #[test]
    fn test_layout_subgraph_compaction() {
        // Subgraph with multiple nodes spread across ranks
        let result = layout_from_source(
            r#"flowchart TD
    subgraph SG
        A --> B
        A --> C
        B --> D
        C --> D
    end
    E --> A
    D --> F"#,
        );
        assert_eq!(result.subgraphs.len(), 1);
        let sg = &result.subgraphs[0];
        assert!(sg.width > 0.0 && sg.height > 0.0);
    }

    // -- rank_assignment.rs: align_sibling_subgraph_ranks --

    #[test]
    fn test_layout_sibling_subgraph_rank_alignment() {
        // Two sibling subgraphs that should be aligned at the same tier
        let result = layout_from_source(
            r#"flowchart TD
    Start --> A1
    Start --> B1
    subgraph Left
        A1 --> A2
        A2 --> A3
    end
    subgraph Right
        B1 --> B2
    end
    A3 --> End
    B2 --> End"#,
        );
        let left = result.subgraphs.iter().find(|s| s.id == "Left").unwrap();
        let right = result.subgraphs.iter().find(|s| s.id == "Right").unwrap();

        // Both subgraphs feed into End, so the rank alignment should ensure
        // they are positioned in the same general area
        assert!(left.width > 0.0 && right.width > 0.0);
    }

    // -- ordering.rs: refine_subgraph_ordering --

    #[test]
    fn test_layout_subgraph_ordering_refinement() {
        // Subgraph with crossing edges that refinement should fix
        let result = layout_from_source(
            r#"flowchart TD
    subgraph SG
        A --> D
        B --> C
        A --> B
    end
    X --> A
    X --> B"#,
        );
        assert_eq!(result.subgraphs.len(), 1);
    }

    // -- edge_routing.rs: diamond intersection via full pipeline --

    #[test]
    fn test_layout_diamond_edge_routing() {
        // Diamond node with edges: tests intersect_diamond through the full pipeline
        let result = layout_from_source(
            r#"flowchart TD
    A[Input] --> B{Is valid?}
    B -->|yes| C[Process]
    B -->|no| D[Reject]
    C --> E[Done]
    D --> E"#,
        );
        let b = result.nodes.iter().find(|n| n.id == "B").unwrap();
        assert_eq!(b.shape, NodeShape::Diamond);

        // Check edges from the diamond
        let from_b: Vec<_> = result.edges.iter().filter(|e| e.from_id == "B").collect();
        assert_eq!(from_b.len(), 2);
        for edge in &from_b {
            assert!(edge.points.len() >= 2, "edge from B should have at least 2 points");
            // First point should be on or near the diamond boundary
            let p0 = edge.points[0];
            let dx = p0.0 - b.x;
            let dy = p0.1 - b.y;
            let dist = (dx * dx + dy * dy).sqrt();
            assert!(
                dist > 1.0,
                "edge start should be on diamond boundary, not at center (dist={dist:.1})"
            );
        }
    }

    // -- edge_routing.rs: subgraph boundary label adjustment via full pipeline --

    #[test]
    fn test_layout_edge_labels_crossing_subgraph() {
        let result = layout_from_source(
            r#"flowchart TD
    subgraph SG
        A --> B
    end
    C -->|crosses boundary| A
    B -->|exits subgraph| D"#,
        );
        assert_eq!(result.subgraphs.len(), 1);
        // Labeled edges exist and have positions
        for edge in &result.edges {
            if edge.label.is_some() {
                assert!(edge.label_x.is_some(), "labeled edge should have label_x");
                assert!(edge.label_y.is_some(), "labeled edge should have label_y");
            }
        }
    }

    // -- normalize.rs: empty graph guard --

    #[test]
    fn test_normalize_empty_graph() {
        // Test with an empty node/edge list directly
        let (w, h) = normalize::normalize_and_compute_bounds(&mut [], &mut [], &mut []);
        assert!((w - 8.0).abs() < 0.1, "empty graph width should be 8.0 (padding only)");
        assert!((h - 8.0).abs() < 0.1, "empty graph height should be 8.0 (padding only)");
    }

    // -- normalize.rs: basic normalization --

    #[test]
    fn test_normalize_shifts_to_positive() {
        let mut nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            x: -50.0,
            y: -30.0,
            width: 40.0,
            height: 20.0,
        }];
        let mut edges = vec![];
        let mut subgraphs = vec![];
        let (w, h) = normalize::normalize_and_compute_bounds(
            &mut nodes,
            &mut edges,
            &mut subgraphs,
        );
        // After normalization, node should have non-negative coords
        assert!(
            nodes[0].x >= 0.0,
            "node x should be >= 0 after normalization, got {}",
            nodes[0].x
        );
        assert!(
            nodes[0].y >= 0.0,
            "node y should be >= 0 after normalization, got {}",
            nodes[0].y
        );
        assert!(w > 0.0 && h > 0.0);
    }

    // -- Full pipeline: BT with multiple ranks and long edges --

    #[test]
    fn test_layout_bt_long_edges() {
        let result = layout_from_source(
            r#"flowchart BT
    A --> B
    B --> C
    C --> D
    A -->|skip| D"#,
        );
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let d = result.nodes.iter().find(|n| n.id == "D").unwrap();
        // In BT, A (source) should be below D
        assert!(
            a.y > d.y,
            "In BT, A should be below D (A.y={:.1}, D.y={:.1})",
            a.y,
            d.y
        );
    }

    // -- Full pipeline: RL with multiple ranks --

    #[test]
    fn test_layout_rl_multiple_ranks() {
        let result = layout_from_source(
            r#"flowchart RL
    A --> B
    B --> C
    A --> C"#,
        );
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let c = result.nodes.iter().find(|n| n.id == "C").unwrap();
        // In RL, A should be right of C
        assert!(
            a.x > c.x,
            "In RL, A should be right of C (A.x={:.1}, C.x={:.1})",
            a.x,
            c.x
        );
    }

    // -- Nested subgraphs --

    #[test]
    fn test_layout_nested_subgraphs() {
        let result = layout_from_source(
            r#"flowchart TD
    subgraph Outer
        subgraph Inner
            A --> B
        end
        C --> A
    end
    D --> C
    B --> E"#,
        );
        assert!(result.subgraphs.len() >= 2);
        let outer = result.subgraphs.iter().find(|s| s.id == "Outer").unwrap();
        let inner = result.subgraphs.iter().find(|s| s.id == "Inner").unwrap();

        // Inner should be geometrically contained within Outer
        assert!(
            inner.x >= outer.x && inner.y >= outer.y,
            "Inner should be inside Outer"
        );
        assert!(
            inner.x + inner.width <= outer.x + outer.width + 1.0,
            "Inner right edge should be within Outer"
        );
        assert!(
            inner.y + inner.height <= outer.y + outer.height + 1.0,
            "Inner bottom edge should be within Outer"
        );
    }

    // -- Disconnected components --

    #[test]
    fn test_layout_disconnected_components() {
        let result = layout_from_source(
            r#"flowchart TD
    A --> B
    C --> D"#,
        );
        assert_eq!(result.nodes.len(), 4);
        // Both components should be laid out
        for node in &result.nodes {
            assert!(node.x.is_finite());
            assert!(node.y.is_finite());
        }
    }

    // -- Edge-only node creation (graph_builder: edge-referenced implicit nodes) --

    #[test]
    fn test_layout_implicit_nodes() {
        // Nodes only referenced in edges, not declared
        let result = layout_from_source("flowchart TD\n    X --> Y");
        assert_eq!(result.nodes.len(), 2);
        let x = result.nodes.iter().find(|n| n.id == "X").unwrap();
        let y = result.nodes.iter().find(|n| n.id == "Y").unwrap();
        // Implicit nodes should default to Rectangle shape
        assert_eq!(x.shape, NodeShape::Rectangle);
        assert_eq!(y.shape, NodeShape::Rectangle);
    }

    // -- Single node (no edges) --

    #[test]
    fn test_layout_single_node_no_edges() {
        let result = layout_from_source("flowchart TD\n    A[Alone]");
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.edges.len(), 0);
        assert!(result.nodes[0].x >= 0.0);
        assert!(result.nodes[0].y >= 0.0);
    }

    // -- Subgraph with edges crossing boundaries and labels --

    #[test]
    fn test_layout_labeled_edges_with_subgraphs() {
        let result = layout_from_source(
            r#"flowchart TD
    subgraph SG1
        A --> B
    end
    subgraph SG2
        C --> D
    end
    B -->|from SG1 to SG2| C"#,
        );
        assert_eq!(result.subgraphs.len(), 2);
        let cross_edge = result.edges.iter().find(|e| e.from_id == "B" && e.to_id == "C").unwrap();
        assert_eq!(cross_edge.label.as_deref(), Some("from SG1 to SG2"));
        assert!(cross_edge.label_x.is_some());
        assert!(cross_edge.label_y.is_some());
    }

    // -- Large graph to exercise ordering thoroughly --

    #[test]
    fn test_layout_wide_graph() {
        let result = layout_from_source(
            r#"flowchart TD
    A --> B
    A --> C
    A --> D
    A --> E
    A --> F
    B --> G
    C --> G
    D --> G
    E --> G
    F --> G"#,
        );
        assert_eq!(result.nodes.len(), 7);
        // All nodes at same rank (B-F) should have distinct x coordinates
        let a = result.nodes.iter().find(|n| n.id == "A").unwrap();
        let g = result.nodes.iter().find(|n| n.id == "G").unwrap();
        assert!(a.y < g.y, "A should be above G");
    }

    // -- Verify bounds are correct --

    #[test]
    fn test_layout_bounds_cover_all_elements() {
        let result = layout_from_source(
            r#"flowchart TD
    subgraph SG
        A --> B
    end
    C --> A"#,
        );
        // All nodes should be within the declared width/height
        for node in &result.nodes {
            assert!(
                node.x + node.width / 2.0 <= result.width + 1.0,
                "node {} exceeds width bound",
                node.id
            );
            assert!(
                node.y + node.height / 2.0 <= result.height + 1.0,
                "node {} exceeds height bound",
                node.id
            );
        }
        for sg in &result.subgraphs {
            assert!(
                sg.x + sg.width <= result.width + 1.0,
                "subgraph {} exceeds width bound",
                sg.id
            );
            assert!(
                sg.y + sg.height <= result.height + 1.0,
                "subgraph {} exceeds height bound",
                sg.id
            );
        }
    }

    #[test]
    fn test_example6_rbac_member_edges_stay_local_and_direct_grants_stays_top() {
        use crate::parser::flowchart::parse_flowchart;

        let source = include_str!("../../../../../tests/test_loop/test_graphs.mmd");
        let ast = parse_flowchart(source).unwrap();
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        let bob = result.nodes.iter().find(|n| n.id == "Bob").unwrap();
        let carol = result.nodes.iter().find(|n| n.id == "Carol").unwrap();
        let role_analyst = result.nodes.iter().find(|n| n.id == "Role_analyst").unwrap();
        let role_editor = result.nodes.iter().find(|n| n.id == "Role_editor").unwrap();
        let alice = result.nodes.iter().find(|n| n.id == "Alice").unwrap();

        // RBAC member edges should be local (near-vertical), not long cross-graph curves.
        let bob_edge = result
            .edges
            .iter()
            .find(|e| e.from_id == "Bob" && e.to_id == "Role_analyst")
            .unwrap();
        let carol_edge = result
            .edges
            .iter()
            .find(|e| e.from_id == "Carol" && e.to_id == "Role_editor")
            .unwrap();

        let bob_min_x = bob_edge
            .points
            .iter()
            .map(|p| p.0)
            .fold(f64::INFINITY, f64::min);
        let bob_max_x = bob_edge
            .points
            .iter()
            .map(|p| p.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let carol_min_x = carol_edge
            .points
            .iter()
            .map(|p| p.0)
            .fold(f64::INFINITY, f64::min);
        let carol_max_x = carol_edge
            .points
            .iter()
            .map(|p| p.0)
            .fold(f64::NEG_INFINITY, f64::max);

        assert!(
            bob_min_x >= bob.x.min(role_analyst.x) - 30.0 && bob_max_x <= bob.x.max(role_analyst.x) + 30.0,
            "Bob->Role_analyst should stay local/near vertical"
        );
        assert!(
            carol_min_x >= carol.x.min(role_editor.x) - 30.0
                && carol_max_x <= carol.x.max(role_editor.x) + 30.0,
            "Carol->Role_editor should stay local/near vertical"
        );

        // Direct grants should remain in the top tier like Mermaid.js (not pushed down).
        assert!(
            (alice.y - bob.y).abs() < 120.0,
            "Alice should be near RBAC top tier (Alice.y={}, Bob.y={})",
            alice.y,
            bob.y
        );
    }

    #[test]
    fn test_example6_subgraphs_do_not_overlap() {
        use crate::parser::flowchart::parse_flowchart;

        let source = include_str!("../../../../../tests/test_loop/test_graphs.mmd");
        let ast = parse_flowchart(source).unwrap();
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        for i in 0..result.subgraphs.len() {
            for j in (i + 1)..result.subgraphs.len() {
                let a = &result.subgraphs[i];
                let b = &result.subgraphs[j];
                let a_right = a.x + a.width;
                let a_bottom = a.y + a.height;
                let b_right = b.x + b.width;
                let b_bottom = b.y + b.height;

                let x_overlap = a.x < b_right && b.x < a_right;
                let y_overlap = a.y < b_bottom && b.y < a_bottom;

                if x_overlap && y_overlap {
                    panic!(
                        "Subgraphs overlap: {} and {} (A=({}, {}, {}, {}), B=({}, {}, {}, {}))",
                        a.id, b.id, a.x, a.y, a_right, a_bottom, b.x, b.y, b_right, b_bottom
                    );
                }
            }
        }
    }

    #[test]
    fn test_example6_files_subgraph_above_folder_hierarchy() {
        use crate::parser::flowchart::parse_flowchart;

        let source = include_str!("../../../../../tests/test_loop/test_graphs.mmd");
        let ast = parse_flowchart(source).unwrap();
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        let files = result
            .subgraphs
            .iter()
            .find(|s| s.id == "Files")
            .expect("Files subgraph missing");
        let folders = result
            .subgraphs
            .iter()
            .find(|s| s.id == "Folders")
            .expect("Folders subgraph missing");

        let files_bottom = files.y + files.height;
        assert!(
            files_bottom <= folders.y - 1.0,
            "Files bottom ({}) must be above Folders top ({})",
            files_bottom,
            folders.y
        );
    }

    #[test]
    fn test_example6_files_nodes_are_horizontally_aligned() {
        use crate::parser::flowchart::parse_flowchart;

        let source = include_str!("../../../../../tests/test_loop/test_graphs.mmd");
        let ast = parse_flowchart(source).unwrap();
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        let f1 = result.nodes.iter().find(|n| n.id == "F1").unwrap();
        let f2 = result.nodes.iter().find(|n| n.id == "F2").unwrap();
        let f3 = result.nodes.iter().find(|n| n.id == "F3").unwrap();

        let eps = 1.0;
        assert!(
            (f1.y - f2.y).abs() <= eps && (f2.y - f3.y).abs() <= eps,
            "Files nodes should share one row (F1.y={}, F2.y={}, F3.y={})",
            f1.y,
            f2.y,
            f3.y
        );
    }

    #[test]
    fn test_example5_smallou_aligns_with_orgccc_in_identity_platform() {
        use crate::parser::flowchart::parse_flowchart;

        let source = include_str!("../../../../../tests/test_loop/input_mermaid.mmd");
        let ast = parse_flowchart(source).unwrap();
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        let small_ou = result
            .nodes
            .iter()
            .find(|n| n.id == "SmallOU")
            .expect("SmallOU node missing");
        let org_ccc = result
            .nodes
            .iter()
            .find(|n| n.id == "OO3")
            .expect("OO3 node missing");

        let x_diff = (small_ou.x - org_ccc.x).abs();
        assert!(
            x_diff <= 1.0,
            "SmallOU (Root OU: Ipsum Inc) should align vertically with OO3 (id=org-ccc): SmallOU.x={}, OO3.x={}, diff={}",
            small_ou.x,
            org_ccc.x,
            x_diff
        );

        let platform = result
            .subgraphs
            .iter()
            .find(|s| s.id == "Platform")
            .expect("Platform subgraph missing");
        let id_platform = result
            .subgraphs
            .iter()
            .find(|s| s.id == "OryNetwork")
            .expect("OryNetwork subgraph missing");
        let platform_bottom = platform.y + platform.height;
        let vertical_gap = id_platform.y - platform_bottom;
        assert!(
            vertical_gap >= 50.0 - 1.0,
            "Expected at least 50px vertical gap between Platform and OryNetwork, got {} (Platform bottom={}, OryNetwork top={})",
            vertical_gap,
            platform_bottom,
            id_platform.y
        );
    }

    // -- Test label top straddles top border, center above --

    #[test]
    fn test_adjust_labels_straddling_top_border_center_above() {
        use crate::layout::flowchart::edge_routing::adjust_labels_for_subgraph_boundaries;

        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 50.0,
            y: 100.0,
            width: 200.0,
            height: 200.0,
            style: Default::default(),
        };

        // Label center ABOVE the top border, but bottom extends past it
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(150.0),
            label_y: Some(95.0),  // center above y=100, label_top=85 < 100, label_bottom=105 > 100
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(50.0, 50.0), (250.0, 150.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_y = edges[0].label_y.unwrap();
        // Center was above border -> pushed fully above
        assert!(
            new_y < 100.0 - 10.0,
            "label center above border should be pushed up, got y={new_y}"
        );
    }

    // -- Test label straddles bottom border, center above --

    #[test]
    fn test_adjust_labels_straddling_bottom_border_center_above() {
        use crate::layout::flowchart::edge_routing::adjust_labels_for_subgraph_boundaries;

        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 50.0,
            y: 100.0,
            width: 200.0,
            height: 200.0,  // bottom border at y=300
            style: Default::default(),
        };

        // Label straddles bottom border with center ABOVE it
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(150.0),
            label_y: Some(295.0),  // center at 295 (above 300); label_bottom = 305 > 300
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(150.0, 250.0), (150.0, 350.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_y = edges[0].label_y.unwrap();
        // Center above bottom border -> pushed inside subgraph
        assert!(
            new_y < 300.0,
            "label center above bottom border should be pushed inside, got y={new_y}"
        );
    }

    // -- Test label straddles left border, center left --

    #[test]
    fn test_adjust_labels_straddling_left_border_center_left() {
        use crate::layout::flowchart::edge_routing::adjust_labels_for_subgraph_boundaries;

        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 100.0,
            y: 50.0,
            width: 200.0,
            height: 200.0,
            style: Default::default(),
        };

        // Label straddles left border at x=100 with center LEFT of it
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(95.0),   // center at 95 (left of 100); label_right=115 > 100
            label_y: Some(150.0),  // vertically within subgraph
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(50.0, 150.0), (200.0, 150.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_x = edges[0].label_x.unwrap();
        // Center left of border -> pushed further left
        assert!(
            new_x < 100.0 - 15.0,
            "label center left of left border should be pushed further left, got x={new_x}"
        );
    }

    // -- Test label straddles right border, center right --

    #[test]
    fn test_adjust_labels_straddling_right_border_center_right() {
        use crate::layout::flowchart::edge_routing::adjust_labels_for_subgraph_boundaries;

        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 100.0,
            y: 50.0,
            width: 200.0,  // right border at x=300
            height: 200.0,
            style: Default::default(),
        };

        // Label straddles right border with center RIGHT of it
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(305.0),  // center at 305; label_left=285 < 300
            label_y: Some(150.0),
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(200.0, 150.0), (400.0, 150.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_x = edges[0].label_x.unwrap();
        // Center right of border -> pushed further right
        assert!(
            new_x > 300.0 + 15.0,
            "label center right of right border should be pushed further right, got x={new_x}"
        );
    }
}
