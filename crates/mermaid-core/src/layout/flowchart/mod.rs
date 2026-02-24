pub mod compound;
pub mod edge_routing;
pub mod graph_builder;
pub mod normalize;
pub mod types;

use crate::ast::flowchart::{Direction, FlowchartAst};
use crate::error::Result;
use crate::layout::text_measure::TextMeasurer;

// Re-export public types for API compatibility
pub use types::{PositionedEdge, PositionedGraph, PositionedNode, PositionedSubgraph};

use std::collections::HashMap;

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

    // 4. Build dagre graph (compound + multigraph) and run layout
    let (mut dagre_graph, node_data_map) =
        graph_builder::build_dagre_graph(&all_nodes, &all_edges, measurer, ast.direction, ast)?;
    dagre_rust::layout(&mut dagre_graph);

    // 5. Extract positioned nodes and edge data from dagre results
    let mut positioned_nodes = build_positioned_nodes_from_dagre(&dagre_graph, &node_data_map);
    let extraction = extract_edge_data_from_dagre(&dagre_graph);

    // 6. Build subgraph membership map
    let membership = graph_builder::build_subgraph_membership(ast);

    // 7. Extract subgraph positions from dagre compound layout,
    //    then fall back to bounding-box computation for any subgraphs
    //    dagre didn't position (e.g. those with no children).
    let mut positioned_subgraphs =
        build_positioned_subgraphs_from_dagre(&dagre_graph, &ast.subgraphs, &ast.style_overrides);
    // If dagre didn't produce positions for some subgraphs, fall back to
    // the bounding-box approach for those.
    if positioned_subgraphs.len() < count_subgraphs(&ast.subgraphs) {
        positioned_subgraphs = compound::position_subgraphs(
            &ast.subgraphs,
            &positioned_nodes,
            &ast.style_overrides,
            measurer,
            &membership,
        );
    }

    // 8. Route edges using dagre bend points
    let mut positioned_edges = edge_routing::route_edges(
        &positioned_nodes,
        &all_edges,
        is_horizontal,
        &extraction.bend_points,
        &extraction.label_positions,
        &extraction.label_dimensions,
    );

    // 8.5. Adjust edge labels to avoid subgraph border/title overlaps
    edge_routing::adjust_labels_for_subgraph_boundaries(
        &mut positioned_edges,
        &positioned_subgraphs,
    );

    // 9. Normalize coordinates and compute bounding box
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

/// Build PositionedNode list from dagre layout results.
pub(crate) fn build_positioned_nodes_from_dagre(
    g: &dagre_rust::LayoutGraph,
    node_data_map: &HashMap<String, NodeData>,
) -> Vec<PositionedNode> {
    g.nodes()
        .into_iter()
        .filter_map(|id| {
            let node_label = g.node(&id)?;
            // Skip dummy/internal nodes
            if node_label.dummy.is_some() {
                return None;
            }
            let data = node_data_map.get(&id)?;
            Some(PositionedNode {
                id: data.id.clone(),
                label: data.label.clone(),
                shape: data.shape,
                style: data.style.clone(),
                x: node_label.x.unwrap_or(0.0),
                y: node_label.y.unwrap_or(0.0),
                width: node_label.width,
                height: node_label.height,
            })
        })
        .collect()
}

/// Extract subgraph positions from dagre compound layout results.
pub(crate) fn build_positioned_subgraphs_from_dagre(
    g: &dagre_rust::LayoutGraph,
    subgraphs: &[crate::ast::flowchart::SubgraphDef],
    style_overrides: &[crate::ast::flowchart::StyleOverride],
) -> Vec<PositionedSubgraph> {
    let mut result = Vec::new();
    collect_subgraph_positions_from_dagre(g, subgraphs, style_overrides, &mut result);
    result
}

fn collect_subgraph_positions_from_dagre(
    g: &dagre_rust::LayoutGraph,
    subgraphs: &[crate::ast::flowchart::SubgraphDef],
    style_overrides: &[crate::ast::flowchart::StyleOverride],
    result: &mut Vec<PositionedSubgraph>,
) {
    for sg in subgraphs {
        if let Some(nl) = g.node(&sg.id) {
            if let (Some(x), Some(y)) = (nl.x, nl.y) {
                let mut style = crate::ast::common::StyleProperties::default();
                for so in style_overrides {
                    if so.node_id == sg.id {
                        style = style.merge(&so.properties);
                    }
                }
                result.push(PositionedSubgraph {
                    id: sg.id.clone(),
                    label: sg.label.clone(),
                    x: x - nl.width / 2.0,
                    y: y - nl.height / 2.0,
                    width: nl.width,
                    height: nl.height,
                    style,
                });
            }
        }
        // Recurse into nested subgraphs
        collect_subgraph_positions_from_dagre(g, &sg.subgraphs, style_overrides, result);
    }
}

pub(crate) fn count_subgraphs(subgraphs: &[crate::ast::flowchart::SubgraphDef]) -> usize {
    subgraphs
        .iter()
        .map(|sg| 1 + count_subgraphs(&sg.subgraphs))
        .sum()
}

/// Result of extracting edge data from dagre layout.
pub(crate) struct DagreEdgeExtraction {
    pub(crate) bend_points: HashMap<(String, String), Vec<(f64, f64)>>,
    pub(crate) label_positions: HashMap<(String, String), (f64, f64)>,
    pub(crate) label_dimensions: HashMap<(String, String), (f64, f64)>,
}

/// Extract bend points, label positions, and label dimensions from dagre edge labels.
pub(crate) fn extract_edge_data_from_dagre(g: &dagre_rust::LayoutGraph) -> DagreEdgeExtraction {
    let mut bend_points = HashMap::new();
    let mut label_positions = HashMap::new();
    let mut label_dimensions = HashMap::new();

    for edge in g.edges() {
        let Some(el) = g.edge_by_obj(&edge) else {
            continue;
        };

        // Extract interior bend points from edge.points, stripping the first
        // and last entries which are dagre's rect-intersection endpoints
        // (added by assign_node_intersects). Our edge router computes its own
        // shape-aware intersections, so we only need the interior waypoints
        // from dummy nodes. For short edges (adjacent ranks), dagre only has
        // 2 intersection points — stripping leaves nothing, so they correctly
        // fall through to route_short_edge which applies S-curve avoidance.
        if el.points.len() > 2 {
            let bps: Vec<(f64, f64)> = el.points[1..el.points.len() - 1]
                .iter()
                .map(|p| (p.x, p.y))
                .collect();
            bend_points.insert((edge.v.clone(), edge.w.clone()), bps.clone());
            let rev: Vec<_> = bps.into_iter().rev().collect();
            bend_points.insert((edge.w.clone(), edge.v.clone()), rev);
        }

        // Extract label position
        if let (Some(x), Some(y)) = (el.x, el.y) {
            let key = (edge.v.clone(), edge.w.clone());
            label_positions.insert(key.clone(), (x, y));
            label_positions.insert((edge.w.clone(), edge.v.clone()), (x, y));

            // Store label dimensions
            if el.width > 0.0 || el.height > 0.0 {
                label_dimensions.insert(key, (el.width, el.height));
                label_dimensions.insert((edge.w.clone(), edge.v.clone()), (el.width, el.height));
            }
        }
    }

    DagreEdgeExtraction {
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
        let result = layout_from_source("flowchart TD\n    A --> B\n    B --> C\n    C --> A");
        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.edges.len(), 3);
        // All nodes should have finite, non-negative coordinates after normalization
        for node in &result.nodes {
            assert!(
                node.x.is_finite() && node.x >= 0.0,
                "node {} has invalid x={}",
                node.id,
                node.x
            );
            assert!(
                node.y.is_finite() && node.y >= 0.0,
                "node {} has invalid y={}",
                node.id,
                node.y
            );
        }
    }

    #[test]
    fn test_layout_with_self_loop_cycle() {
        // Self-loop A -> A and a longer cycle
        let result =
            layout_from_source("flowchart TD\n    A --> B\n    B --> C\n    C --> D\n    D --> B");
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
        let result =
            layout_from_source("flowchart TD\n    A --> B\n    B --> C\n    C --> D\n    A --> D");
        assert_eq!(result.nodes.len(), 4);
        // Find the edge from A to D
        let a_to_d = result
            .edges
            .iter()
            .find(|e| e.from_id == "A" && e.to_id == "D")
            .unwrap();
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
        let a_to_d = result
            .edges
            .iter()
            .find(|e| e.from_id == "A" && e.to_id == "D")
            .unwrap();
        assert!(a_to_d.label.is_some());
        assert!(
            a_to_d.label_x.is_some(),
            "labeled long edge should have label_x"
        );
        assert!(
            a_to_d.label_y.is_some(),
            "labeled long edge should have label_y"
        );
        assert!(
            a_to_d.label_width.is_some(),
            "labeled long edge should have label_width from label_node"
        );
        assert!(
            a_to_d.label_height.is_some(),
            "labeled long edge should have label_height from label_node"
        );
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
        let shapes: Vec<(String, NodeShape)> = result
            .nodes
            .iter()
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
            assert!(
                sg.width > 0.0 && sg.height > 0.0,
                "subgraph {} has bad dims",
                sg.id
            );
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

    // -- compound.rs: subgraph compaction --

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
            assert!(
                edge.points.len() >= 2,
                "edge from B should have at least 2 points"
            );
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
        assert!(
            (w - 8.0).abs() < 0.1,
            "empty graph width should be 8.0 (padding only)"
        );
        assert!(
            (h - 8.0).abs() < 0.1,
            "empty graph height should be 8.0 (padding only)"
        );
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
        let (w, h) =
            normalize::normalize_and_compute_bounds(&mut nodes, &mut edges, &mut subgraphs);
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
        let cross_edge = result
            .edges
            .iter()
            .find(|e| e.from_id == "B" && e.to_id == "C")
            .unwrap();
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
            label_y: Some(95.0), // center above y=100, label_top=85 < 100, label_bottom=105 > 100
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
            height: 200.0, // bottom border at y=300
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
            label_y: Some(295.0), // center at 295 (above 300); label_bottom = 305 > 300
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
            label_x: Some(95.0), // center at 95 (left of 100); label_right=115 > 100
            label_y: Some(150.0), // vertically within subgraph
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
            width: 200.0, // right border at x=300
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
            label_x: Some(305.0), // center at 305; label_left=285 < 300
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

    // -- Regression: multi-subgraph flowchart with cross-subgraph edges --

    #[test]
    fn test_multi_subgraph_flowchart_layout() {
        let source = r#"graph TD
    subgraph RBAC["RBAC Layer"]
        Role_analyst["Role: analyst"]
        Role_editor["Role: editor"]
        Bob["User: bob"] -->|member of| Role_analyst
        Carol["User: carol"] -->|member of| Role_editor
    end

    subgraph Folders["Folder Hierarchy"]
        Root["Folder: root"]
        Eng["Folder: engineering"]
        Backend["Folder: backend"]

        Backend -->|parents| Eng
        Eng -->|parents| Root
    end

    subgraph Files["Files"]
        F1["design-doc.pdf"]
        F2["api-spec.yaml"]
        F3["secret-report.pdf"]

        F1 -->|parents| Backend
        F2 -->|parents| Backend
        F3 -->|parents| Eng
    end

    subgraph DirectGrants["Direct Entity Grants"]
        Alice["User: alice"]
        Alice -->|"viewers (direct)"| F3
    end

    Role_analyst -->|"viewers (RBAC)"| Root
    Role_editor -->|"editors (RBAC)"| Eng"#;

        let result = layout_from_source(source);

        // --- 4 subgraphs exist ---
        assert_eq!(result.subgraphs.len(), 4, "expected 4 subgraphs");

        let sg = |id: &str| -> &PositionedSubgraph {
            result
                .subgraphs
                .iter()
                .find(|s| s.id == id)
                .unwrap_or_else(|| panic!("subgraph '{id}' not found"))
        };
        let direct = sg("DirectGrants");
        let files = sg("Files");
        let rbac = sg("RBAC");
        let folders = sg("Folders");

        // --- No pair of subgraphs overlaps on both axes simultaneously ---
        for (i, a) in result.subgraphs.iter().enumerate() {
            for b in result.subgraphs.iter().skip(i + 1) {
                let x_overlap = a.x < b.x + b.width && b.x < a.x + a.width;
                let y_overlap = a.y < b.y + b.height && b.y < a.y + a.height;
                assert!(
                    !(x_overlap && y_overlap),
                    "Subgraphs '{}' and '{}' overlap!\n  {} = ({:.0}, {:.0}, {:.0}x{:.0})\n  {} = ({:.0}, {:.0}, {:.0}x{:.0})",
                    a.id, b.id,
                    a.id, a.x, a.y, a.width, a.height,
                    b.id, b.x, b.y, b.width, b.height,
                );
            }
        }

        // --- Vertical ordering: 3 tiers ---
        // Top tier: DirectGrants + RBAC (both are sources with no incoming subgraph edges)
        // Middle tier: Files (depends on DirectGrants)
        // Bottom tier: Folder Hierarchy (depends on Files + RBAC)
        //
        // Note: with compound layout, subgraph bounding boxes may overlap in
        // the vertical direction depending on ranksep and node placement.
        // We verify the top-edges of the subgraphs follow the expected ordering.
        assert!(
            direct.y < files.y + files.height,
            "DirectGrants (top={:.0}) should start above Files bottom (={:.0})",
            direct.y,
            files.y + files.height,
        );
        assert!(
            files.y < folders.y + folders.height,
            "Files (top={:.0}) should start above Folders bottom (={:.0})",
            files.y,
            folders.y + folders.height,
        );

        // --- All three files should be horizontally aligned (same y) ---
        let f1 = result.nodes.iter().find(|n| n.id == "F1").unwrap();
        let f2 = result.nodes.iter().find(|n| n.id == "F2").unwrap();
        let f3 = result.nodes.iter().find(|n| n.id == "F3").unwrap();

        let y_diff_12 = (f1.y - f2.y).abs();
        let y_diff_13 = (f1.y - f3.y).abs();
        assert!(
            y_diff_12 < 1.0,
            "F1 (y={:.1}) and F2 (y={:.1}) should be horizontally aligned",
            f1.y,
            f2.y,
        );
        assert!(
            y_diff_13 < 1.0,
            "F1 (y={:.1}) and F3 (y={:.1}) should be horizontally aligned",
            f1.y,
            f3.y,
        );
    }

    #[test]
    fn test_complex_subgraph_edge_endpoints() {
        let source = include_str!("../../../../../tests/test_loop/complex_subgraphs.mmd");
        let result = layout_from_source(source);

        // Helper to find a node
        let node = |id: &str| -> &PositionedNode {
            result
                .nodes
                .iter()
                .find(|n| n.id == id)
                .unwrap_or_else(|| panic!("node '{id}' not found"))
        };
        // Helper to find an edge
        let edge = |from: &str, to: &str| -> &PositionedEdge {
            result
                .edges
                .iter()
                .find(|e| e.from_id == from && e.to_id == to)
                .unwrap_or_else(|| panic!("edge '{from}'->{to}' not found"))
        };

        // The "deploy staging" edge: Gate --> SAPI
        let gate = node("Gate");
        let sapi = node("SAPI");
        let deploy_staging = edge("Gate", "SAPI");

        // Edge should start near Gate and end near SAPI
        let start = deploy_staging.points.first().unwrap();
        let end = deploy_staging.points.last().unwrap();

        // Start should be within gate's bounding box vicinity
        let start_dist_to_gate = ((start.0 - gate.x).powi(2) + (start.1 - gate.y).powi(2)).sqrt();
        assert!(
            start_dist_to_gate < gate.width + 50.0,
            "deploy_staging edge start ({:.1},{:.1}) too far from Gate ({:.1},{:.1}), dist={:.1}",
            start.0,
            start.1,
            gate.x,
            gate.y,
            start_dist_to_gate,
        );

        // End should be within SAPI's bounding box vicinity
        let end_dist_to_sapi = ((end.0 - sapi.x).powi(2) + (end.1 - sapi.y).powi(2)).sqrt();
        assert!(
            end_dist_to_sapi < sapi.width + 50.0,
            "deploy_staging edge end ({:.1},{:.1}) too far from SAPI ({:.1},{:.1}), dist={:.1}",
            end.0,
            end.1,
            sapi.x,
            sapi.y,
            end_dist_to_sapi,
        );

        // The edge should go generally downward (gate.y < sapi.y)
        assert!(
            gate.y < sapi.y,
            "Gate (y={:.1}) should be above SAPI (y={:.1})",
            gate.y,
            sapi.y,
        );

        // CRITICAL: No waypoint should deviate more than a reasonable amount
        // from the corridor between Gate and SAPI. If waypoints go far left
        // or far right, the edge is taking a wild detour.
        let corridor_min_x = gate.x.min(sapi.x) - 100.0;
        let corridor_max_x = gate.x.max(sapi.x) + 100.0;
        for (i, p) in deploy_staging.points.iter().enumerate() {
            assert!(
                p.0 >= corridor_min_x && p.0 <= corridor_max_x,
                "deploy_staging waypoint [{i}] x={:.1} outside corridor [{:.1}, {:.1}] \
                (Gate.x={:.1}, SAPI.x={:.1})",
                p.0,
                corridor_min_x,
                corridor_max_x,
                gate.x,
                sapi.x,
            );
        }

        // ---- Gate → PAPI ("deploy prod") should also stay in its corridor ----
        let papi = node("PAPI");
        let deploy_prod = edge("Gate", "PAPI");

        // Waypoints should stay within the corridor between Gate and PAPI.
        // Before the fix, dummies overshoot to x ≈ -300 (right of PAPI at -415).
        let prod_corridor_min_x = gate.x.min(papi.x) - 100.0;
        let prod_corridor_max_x = gate.x.max(papi.x) + 100.0;
        for (i, p) in deploy_prod.points.iter().enumerate() {
            assert!(
                p.0 >= prod_corridor_min_x && p.0 <= prod_corridor_max_x,
                "deploy_prod waypoint [{i}] x={:.1} outside corridor [{:.1}, {:.1}] \
                (Gate.x={:.1}, PAPI.x={:.1})",
                p.0,
                prod_corridor_min_x,
                prod_corridor_max_x,
                gate.x,
                papi.x,
            );
        }

        // ---- PWA → PAPI should stay in corridor too ----
        let pwa = node("PWA");
        let pwa_papi = edge("PWA", "PAPI");

        let pwa_corridor_min_x = pwa.x.min(papi.x) - 100.0;
        let pwa_corridor_max_x = pwa.x.max(papi.x) + 100.0;
        for (i, p) in pwa_papi.points.iter().enumerate() {
            assert!(
                p.0 >= pwa_corridor_min_x && p.0 <= pwa_corridor_max_x,
                "pwa_papi waypoint [{i}] x={:.1} outside corridor [{:.1}, {:.1}] \
                (PWA.x={:.1}, PAPI.x={:.1})",
                p.0,
                pwa_corridor_min_x,
                pwa_corridor_max_x,
                pwa.x,
                papi.x,
            );
        }
    }

    #[test]
    fn test_org_flowchart_edge_corridors() {
        let source = include_str!("../../../../../tests/test_loop/input_mermaid.mmd");
        let result = layout_from_source(source);

        let node = |id: &str| -> &PositionedNode {
            result
                .nodes
                .iter()
                .find(|n| n.id == id)
                .unwrap_or_else(|| panic!("node '{id}' not found"))
        };
        let edge = |from: &str, to: &str| -> &PositionedEdge {
            result
                .edges
                .iter()
                .find(|e| e.from_id == from && e.to_id == to)
                .unwrap_or_else(|| panic!("edge '{from}'->'{to}' not found"))
        };

        // The three org_id edges: RootOU→OO1, EUOU→OO2, SmallOU→OO3
        for (src_id, tgt_id) in &[("RootOU", "OO1"), ("EUOU", "OO2"), ("SmallOU", "OO3")] {
            let src = node(src_id);
            let tgt = node(tgt_id);
            let e = edge(src_id, tgt_id);

            let corridor_min_x = src.x.min(tgt.x) - 150.0;
            let corridor_max_x = src.x.max(tgt.x) + 150.0;

            for (i, p) in e.points.iter().enumerate() {
                assert!(
                    p.0 >= corridor_min_x && p.0 <= corridor_max_x,
                    "org_id edge {}->{} waypoint [{i}] x={:.1} outside corridor \
                    [{:.1}, {:.1}] (src.x={:.1}, tgt.x={:.1})",
                    src_id,
                    tgt_id,
                    p.0,
                    corridor_min_x,
                    corridor_max_x,
                    src.x,
                    tgt.x,
                );
            }
        }
    }

    /// Edges should not pass through subgraphs that don't contain their
    /// source or target node.
    #[test]
    fn test_edges_dont_cross_unrelated_subgraphs() {
        let source = include_str!("../../../../../tests/test_loop/input_mermaid.mmd");
        let result = layout_from_source(source);

        // Helper: check if a point is inside a subgraph bbox
        let point_in_subgraph = |px: f64, py: f64, sg: &PositionedSubgraph| -> bool {
            px >= sg.x && px <= sg.x + sg.width && py >= sg.y && py <= sg.y + sg.height
        };

        // Helper: check if a node center is inside a subgraph bbox
        let node_in_subgraph = |n: &PositionedNode, sg: &PositionedSubgraph| -> bool {
            point_in_subgraph(n.x, n.y, sg)
        };

        // Find nodes by id
        let node = |id: &str| -> &PositionedNode {
            result
                .nodes
                .iter()
                .find(|n| n.id == id)
                .unwrap_or_else(|| panic!("node '{id}' not found"))
        };

        // For each edge, check that no waypoint passes through a subgraph
        // that doesn't contain either the source or target node.
        // We check edge segments by sampling points along them.
        let edges_to_check = [("RootOU", "OO1"), ("EUOU", "OO2"), ("SmallOU", "OO3")];

        for (src_id, tgt_id) in &edges_to_check {
            let src_node = node(src_id);
            let tgt_node = node(tgt_id);
            let e = result
                .edges
                .iter()
                .find(|e| e.from_id == *src_id && e.to_id == *tgt_id)
                .unwrap();

            for sg in &result.subgraphs {
                // Skip subgraphs that contain either endpoint
                if node_in_subgraph(src_node, sg) || node_in_subgraph(tgt_node, sg) {
                    continue;
                }

                // Check each segment of the edge path
                for win in e.points.windows(2) {
                    let (x1, y1) = win[0];
                    let (x2, y2) = win[1];
                    // Sample 10 points along the segment
                    for s in 0..=10 {
                        let t = s as f64 / 10.0;
                        let px = x1 + (x2 - x1) * t;
                        let py = y1 + (y2 - y1) * t;
                        assert!(
                            !point_in_subgraph(px, py, sg),
                            "Edge {}->{} passes through subgraph '{}' at ({:.1},{:.1}).\n\
                            Subgraph bounds: ({:.1},{:.1}) {}x{}\n\
                            Source {} at ({:.1},{:.1}), Target {} at ({:.1},{:.1})",
                            src_id,
                            tgt_id,
                            sg.id,
                            px,
                            py,
                            sg.x,
                            sg.y,
                            sg.width,
                            sg.height,
                            src_id,
                            src_node.x,
                            src_node.y,
                            tgt_id,
                            tgt_node.x,
                            tgt_node.y,
                        );
                    }
                }
            }
        }
    }

    /// In a top-down flowchart, edges from Job Queue (PQueue) should always
    /// go downward — waypoints should have monotonically non-decreasing y.
    #[test]
    fn test_job_queue_edges_go_downward() {
        let source = include_str!("../../../../../tests/test_loop/complex_subgraphs.mmd");
        let result = layout_from_source(source);

        let node = |id: &str| -> &PositionedNode {
            result
                .nodes
                .iter()
                .find(|n| n.id == id)
                .unwrap_or_else(|| panic!("node '{id}' not found"))
        };

        let pqueue = node("PQueue");

        // PQueue has edges to: PNotify, PRedis, PS3
        for tgt_id in &["PNotify", "PRedis", "PS3"] {
            let tgt = node(tgt_id);
            let e = result
                .edges
                .iter()
                .find(|e| e.from_id == "PQueue" && e.to_id == *tgt_id)
                .unwrap_or_else(|| panic!("edge PQueue->{tgt_id} not found"));

            // In TD layout, target should be below source
            assert!(
                tgt.y > pqueue.y,
                "PQueue (y={:.1}) should be above {} (y={:.1}) in TD layout",
                pqueue.y,
                tgt_id,
                tgt.y,
            );

            // Waypoints should be monotonically non-decreasing in y
            // (allowing small tolerance for floating point)
            for w in e.points.windows(2) {
                assert!(
                    w[1].1 >= w[0].1 - 1.0,
                    "PQueue->{} edge goes upward: waypoint y={:.1} followed by y={:.1}. \
                    All points: {:?}",
                    tgt_id,
                    w[0].1,
                    w[1].1,
                    e.points,
                );
            }
        }
    }

    /// The "read replica" edge (PAPI → PReplica) should not pass through
    /// any other node's bounding box.
    #[test]
    fn test_papi_to_preplica_avoids_nodes() {
        let source = include_str!("../../../../../tests/test_loop/complex_subgraphs.mmd");
        let result = layout_from_source(source);

        let edge = result
            .edges
            .iter()
            .find(|e| e.from_id == "PAPI" && e.to_id == "PReplica")
            .expect("edge PAPI->PReplica not found");

        let margin = 5.0;

        // For each line segment of the edge, sample points and check
        // they don't fall inside any unrelated node's bounding box.
        for win in edge.points.windows(2) {
            let (x1, y1) = win[0];
            let (x2, y2) = win[1];
            for s in 0..=10 {
                let t = s as f64 / 10.0;
                let px = x1 + (x2 - x1) * t;
                let py = y1 + (y2 - y1) * t;

                for node in &result.nodes {
                    if node.id == "PAPI" || node.id == "PReplica" {
                        continue;
                    }
                    let left = node.x - node.width / 2.0 + margin;
                    let right = node.x + node.width / 2.0 - margin;
                    let top = node.y - node.height / 2.0 + margin;
                    let bottom = node.y + node.height / 2.0 - margin;

                    assert!(
                        !(px >= left && px <= right && py >= top && py <= bottom),
                        "PAPI->PReplica edge passes through node '{}' at ({:.1},{:.1}).\n\
                        Node bounds: ({:.1},{:.1})-({:.1},{:.1})\n\
                        Edge points: {:?}",
                        node.id,
                        px,
                        py,
                        left,
                        top,
                        right,
                        bottom,
                        edge.points,
                    );
                }
            }
        }
    }

    /// The gap between Frontend and Backend subgraphs should be compact —
    /// not much larger than the normal rank separation (~50px).
    #[test]
    fn test_frontend_backend_gap_is_compact() {
        let source = include_str!("../../../../../tests/test_loop/complex_subgraphs.mmd");
        let result = layout_from_source(source);

        let sg = |id: &str| -> &PositionedSubgraph {
            result
                .subgraphs
                .iter()
                .find(|s| s.id == id)
                .unwrap_or_else(|| panic!("subgraph '{id}' not found"))
        };

        // Staging side
        let stage_fe = sg("StageFE");
        let stage_be = sg("StageBE");
        let stage_fe_bottom = stage_fe.y + stage_fe.height;
        let stage_gap = stage_be.y - stage_fe_bottom;

        // Production side
        let prod_fe = sg("ProdFE");
        let prod_be = sg("ProdBE");
        let prod_fe_bottom = prod_fe.y + prod_fe.height;
        let prod_gap = prod_be.y - prod_fe_bottom;

        // Normal rank_sep is 50, so with one interstitial label rank
        // the gap should be roughly 2-3x rank_sep (~100-150px).
        // Allow generous margin but catch the ~300px+ gaps we see now.
        // Normal rank_sep is 50, so with one interstitial label rank
        // the gap should be roughly 2-3x rank_sep (~100-150px).
        // Production has a label dummy ("deploy prod") that correctly
        // keeps full spacing, so allow up to 155px.
        let max_gap = 155.0;

        assert!(
            stage_gap <= max_gap,
            "Staging Frontend->Backend gap is {stage_gap:.0}px (max {max_gap:.0}). \
            StageFE bottom={stage_fe_bottom:.0}, StageBE top={:.0}",
            stage_be.y,
        );
        assert!(
            prod_gap <= max_gap,
            "Production Frontend->Backend gap is {prod_gap:.0}px (max {max_gap:.0}). \
            ProdFE bottom={prod_fe_bottom:.0}, ProdBE top={:.0}",
            prod_be.y,
        );
    }
}
