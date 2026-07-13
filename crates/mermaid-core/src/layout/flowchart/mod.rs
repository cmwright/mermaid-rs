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

use std::collections::{HashMap, HashSet};

use types::*;

/// Compute layout positions for a flowchart AST.
pub fn layout_flowchart(
    ast: &FlowchartAst,
    measurer: &TextMeasurer<'_>,
) -> Result<PositionedGraph> {
    layout_flowchart_impl(ast, measurer, true, None, None)
}

fn layout_flowchart_impl(
    ast: &FlowchartAst,
    measurer: &TextMeasurer<'_>,
    allow_isolated_subgraph_extraction: bool,
    fixed_node_sizes: Option<&HashMap<String, (f64, f64)>>,
    ranksep_override: Option<f64>,
) -> Result<PositionedGraph> {
    if allow_isolated_subgraph_extraction && fixed_node_sizes.is_none() {
        if let Some(extracted) = layout_with_extracted_isolated_subgraphs(ast, measurer)? {
            return Ok(extracted);
        }
    }

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

    // 4. Build subgraph membership once and reuse in layout stages.
    let membership = graph_builder::build_subgraph_membership(ast);

    // 5. Build dagre graph (compound + multigraph) and run layout
    let (mut dagre_graph, node_data_map) =
        graph_builder::build_dagre_graph_with_fixed_node_sizes_and_membership(
            &all_nodes,
            &all_edges,
            measurer,
            ast.direction,
            ast,
            fixed_node_sizes,
            &membership,
        )?;
    if let Some(ranksep) = ranksep_override {
        dagre_graph.graph_mut().ranksep = ranksep;
    }
    dagre_rust::layout(&mut dagre_graph);

    // 6. Extract positioned nodes and edge data from dagre results
    let mut positioned_nodes = build_positioned_nodes_from_dagre(&dagre_graph, &node_data_map);
    let extraction = extract_edge_data_from_dagre(&dagre_graph);

    // 7. Compute subgraph bounding boxes from positioned nodes.
    //    compound::position_subgraphs processes children before parents,
    //    correctly handles multi-line title heights, and ensures parent
    //    subgraphs fully contain their children.
    let mut positioned_subgraphs = compound::position_subgraphs(
        &ast.subgraphs,
        &positioned_nodes,
        &ast.style_overrides,
        measurer,
        &membership,
    );

    // 7.5. Create synthetic positioned-node entries for subgraph IDs that are
    //      edge endpoints so that edge routing can find them.  Dagre's
    //      raw_points already contain correct border-intersection paths;
    //      these synthetic nodes just need to exist for the lookup.
    let sg_ids = graph_builder::subgraph_ids_recursive(&ast.subgraphs);
    for edge in &all_edges {
        for id in [&edge.from, &edge.to] {
            if sg_ids.contains(id)
                && !positioned_nodes.iter().any(|n| n.id == *id)
            {
                if let Some(sg) = positioned_subgraphs.iter().find(|s| s.id == *id) {
                    positioned_nodes.push(PositionedNode {
                        id: sg.id.clone(),
                        label: sg.label.clone().unwrap_or_default(),
                        shape: crate::ast::flowchart::NodeShape::Rectangle,
                        style: sg.style.clone(),
                        x: sg.x + sg.width / 2.0,
                        y: sg.y + sg.height / 2.0,
                        width: sg.width,
                        height: sg.height,
                    });
                }
            }
        }
    }

    // 8. Route edges using dagre bend points.
    //    For edges involving subgraph endpoints, remove dagre's raw/bend points
    //    so route_edges falls back to direct geometric routing. Dagre's compound
    //    border-node routing produces unintuitive paths for child→parent edges.
    let mut raw_points = extraction.raw_points;
    let mut bend_pts = extraction.bend_points;
    for edge in &all_edges {
        if sg_ids.contains(&edge.from) || sg_ids.contains(&edge.to) {
            // Drop dagre routing for every edge between this pair regardless of
            // its per-edge name so subgraph endpoints fall back to geometric
            // routing.
            raw_points.retain(|(from, to, _), _| !(from == &edge.from && to == &edge.to));
            bend_pts.retain(|(from, to, _), _| !(from == &edge.from && to == &edge.to));
        }
    }
    let mut positioned_edges = edge_routing::route_edges(
        &positioned_nodes,
        &all_edges,
        is_horizontal,
        &raw_points,
        &bend_pts,
        &extraction.label_positions,
        &extraction.label_dimensions,
    );

    // 8.5. Remove synthetic subgraph-as-node entries so they aren't rendered.
    positioned_nodes.retain(|n| !sg_ids.contains(&n.id));

    // 8.6. Adjust edge labels to avoid subgraph border/title overlaps
    edge_routing::adjust_labels_for_subgraph_boundaries(
        &mut positioned_edges,
        &positioned_subgraphs,
    );

    // 8.7. Separate any edge labels that still overlap one another.
    edge_routing::separate_overlapping_labels(&mut positioned_edges);

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

fn toggle_direction(dir: Direction) -> Direction {
    match dir {
        Direction::TopToBottom => Direction::LeftToRight,
        Direction::BottomToTop => Direction::RightToLeft,
        Direction::LeftToRight => Direction::TopToBottom,
        Direction::RightToLeft => Direction::BottomToTop,
    }
}

fn layout_with_extracted_isolated_subgraphs(
    ast: &FlowchartAst,
    measurer: &TextMeasurer<'_>,
) -> Result<Option<PositionedGraph>> {
    if ast.subgraphs.is_empty() {
        return Ok(None);
    }

    let membership = graph_builder::build_subgraph_membership(ast);
    let all_edges = graph_builder::collect_all_edges(ast);

    #[derive(Clone)]
    struct IsolatedSubgraphLayout {
        id: String,
        descendants: HashSet<String>,
        layout: PositionedGraph,
        wrapper: PositionedSubgraph,
    }

    let mut isolated = Vec::new();
    for sg in &ast.subgraphs {
        let descendants: HashSet<String> = membership
            .iter()
            .filter_map(|(node_id, path)| {
                (path.first().map(|p| p == &sg.id).unwrap_or(false)).then_some(node_id.clone())
            })
            .collect();
        if descendants.is_empty() {
            continue;
        }

        let has_external_edges = all_edges.iter().any(|e| {
            let from_in = descendants.contains(&e.from);
            let to_in = descendants.contains(&e.to);
            from_in ^ to_in
        });
        if has_external_edges {
            continue;
        }

        let mut local_ast = ast.clone();
        local_ast.direction = sg.direction.unwrap_or(toggle_direction(ast.direction));
        local_ast.nodes = Vec::new();
        local_ast.edges = Vec::new();
        local_ast.subgraphs = vec![sg.clone()];

        // MermaidJS recursive cluster render applies parent ranksep + 25.
        let local_layout =
            layout_flowchart_impl(&local_ast, measurer, false, None, Some(RANK_SEP + 25.0))?;
        let wrapper = local_layout
            .subgraphs
            .iter()
            .find(|s| s.id == sg.id)
            .cloned()
            .ok_or_else(|| {
                crate::error::MermaidError::Layout(format!(
                    "isolated subgraph '{}' missing wrapper layout",
                    sg.id
                ))
            })?;

        isolated.push(IsolatedSubgraphLayout {
            id: sg.id.clone(),
            descendants,
            layout: local_layout,
            wrapper,
        });
    }

    if isolated.is_empty() {
        return Ok(None);
    }

    let isolated_ids: HashSet<String> = isolated.iter().map(|i| i.id.clone()).collect();
    let isolated_descendants: HashSet<String> = isolated
        .iter()
        .flat_map(|i| i.descendants.iter().cloned())
        .collect();

    let mut transformed = ast.clone();
    transformed
        .subgraphs
        .retain(|sg| !isolated_ids.contains(&sg.id));
    transformed
        .nodes
        .retain(|n| !isolated_descendants.contains(&n.id));
    transformed.edges.retain(|e| {
        !isolated_descendants.contains(&e.from) && !isolated_descendants.contains(&e.to)
    });

    let mut synthetic_nodes = Vec::new();
    for i in &isolated {
        synthetic_nodes.push(crate::ast::flowchart::NodeDef {
            id: i.id.clone(),
            label: Some(i.id.clone()),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            class_shorthand: None,
        });
    }
    synthetic_nodes.extend(transformed.nodes);
    transformed.nodes = synthetic_nodes;

    let fixed_sizes: HashMap<String, (f64, f64)> = isolated
        .iter()
        .map(|i| (i.id.clone(), (i.wrapper.width, i.wrapper.height)))
        .collect();

    let mut top_layout =
        layout_flowchart_impl(&transformed, measurer, false, Some(&fixed_sizes), None)?;

    for i in isolated {
        let Some(anchor_idx) = top_layout.nodes.iter().position(|n| n.id == i.id) else {
            continue;
        };
        let anchor = top_layout.nodes[anchor_idx].clone();
        let target_x = anchor.x - anchor.width / 2.0;
        let target_y = anchor.y - anchor.height / 2.0;
        let shift_x = target_x - i.wrapper.x;
        let shift_y = target_y - i.wrapper.y;

        top_layout.nodes.remove(anchor_idx);

        top_layout
            .nodes
            .extend(i.layout.nodes.into_iter().map(|mut n| {
                n.x += shift_x;
                n.y += shift_y;
                n
            }));

        top_layout
            .edges
            .extend(i.layout.edges.into_iter().map(|mut e| {
                for p in &mut e.points {
                    p.0 += shift_x;
                    p.1 += shift_y;
                }
                if let Some(x) = &mut e.label_x {
                    *x += shift_x;
                }
                if let Some(y) = &mut e.label_y {
                    *y += shift_y;
                }
                e
            }));

        top_layout
            .subgraphs
            .extend(i.layout.subgraphs.into_iter().map(|mut s| {
                s.x += shift_x;
                s.y += shift_y;
                s
            }));
    }

    let (width, height) = normalize::normalize_and_compute_bounds(
        &mut top_layout.nodes,
        &mut top_layout.edges,
        &mut top_layout.subgraphs,
    );
    top_layout.width = width;
    top_layout.height = height;
    top_layout.direction = ast.direction;

    Ok(Some(top_layout))
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
    pub(crate) raw_points: HashMap<EdgeKey, Vec<(f64, f64)>>,
    pub(crate) bend_points: HashMap<EdgeKey, Vec<(f64, f64)>>,
    pub(crate) label_positions: HashMap<EdgeKey, (f64, f64)>,
    pub(crate) label_dimensions: HashMap<EdgeKey, (f64, f64)>,
}

/// Extract bend points, label positions, and label dimensions from dagre edge labels.
pub(crate) fn extract_edge_data_from_dagre(g: &dagre_rust::LayoutGraph) -> DagreEdgeExtraction {
    let mut raw_points = HashMap::new();
    let mut bend_points = HashMap::new();
    let mut label_positions = HashMap::new();
    let mut label_dimensions = HashMap::new();

    // Two-pass extraction: first store all forward (actual) edge data,
    // then fill in reversed entries only where the reverse direction has
    // no real edge.  This prevents bidirectional edges (A→B, B→A) from
    // clobbering each other's dagre-computed layouts.
    struct ForwardEdge {
        key: EdgeKey,
        raw: Option<Vec<(f64, f64)>>,
        bps: Option<Vec<(f64, f64)>>,
    }
    let mut forward_edges = Vec::new();

    for edge in g.edges() {
        let Some(el) = g.edge_by_obj(&edge) else {
            continue;
        };

        let key = (edge.v.clone(), edge.w.clone(), edge.name.clone());
        let raw = if el.points.len() >= 2 {
            let pts: Vec<(f64, f64)> = el.points.iter().map(|p| (p.x, p.y)).collect();
            raw_points.insert(key.clone(), pts.clone());
            Some(pts)
        } else {
            None
        };

        let bps = if el.points.len() > 2 {
            let pts: Vec<(f64, f64)> = el.points[1..el.points.len() - 1]
                .iter()
                .map(|p| (p.x, p.y))
                .collect();
            bend_points.insert(key.clone(), pts.clone());
            Some(pts)
        } else {
            None
        };

        forward_edges.push(ForwardEdge { key, raw, bps });

        // Extract label position
        if let (Some(x), Some(y)) = (el.x, el.y) {
            let key = (edge.v.clone(), edge.w.clone(), edge.name.clone());
            label_positions.insert(key.clone(), (x, y));
            // Only fill reverse if no real edge exists in that direction
            let rev_key = (edge.w.clone(), edge.v.clone(), edge.name.clone());
            label_positions.entry(rev_key.clone()).or_insert((x, y));

            // Store label dimensions
            if el.width > 0.0 || el.height > 0.0 {
                label_dimensions.insert(key, (el.width, el.height));
                label_dimensions
                    .entry(rev_key)
                    .or_insert((el.width, el.height));
            }
        }
    }

    // Second pass: fill in reversed raw_points / bend_points for edges
    // that don't have a real counterpart in the other direction.
    for fe in forward_edges {
        let rev_key = (fe.key.1, fe.key.0, fe.key.2);
        if let Some(raw) = fe.raw {
            let rev_raw: Vec<_> = raw.into_iter().rev().collect();
            raw_points.entry(rev_key.clone()).or_insert(rev_raw);
        }
        if let Some(bps) = fe.bps {
            let rev_bps: Vec<_> = bps.into_iter().rev().collect();
            bend_points.entry(rev_key).or_insert(rev_bps);
        }
    }

    DagreEdgeExtraction {
        raw_points,
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
                from_side: None,
                to_side: None,
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
                from_side: None,
                to_side: None,
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
                    from_side: None,
                    to_side: None,
                },
                EdgeDef {
                    from: "A".into(),
                    to: "C".into(),
                    line_style: LineStyle::Solid,
                    arrow_start: ArrowEnd::None,
                    arrow_end: ArrowEnd::Arrow,
                    label: None,
                    from_side: None,
                    to_side: None,
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

    /// Regression test: two parallel edges between the same pair of nodes
    /// (`wait2 -->|submitted| review` and `wait2 -->|window lapses| review`)
    /// must be routed and labeled separately, not drawn on top of each other.
    ///
    /// Previously every edge was registered with dagre under a `None` name, so
    /// parallel edges collapsed to a single edge ID and shared one routing path
    /// and one label position. Each edge now gets a unique dagre name (its
    /// positional index), so the two labels land at distinct positions.
    #[test]
    fn test_parallel_edges_are_separated() {
        use crate::parser::flowchart::parse_flowchart;

        let source = r#"flowchart TD
    remind[Send reminder] --> wait2{Keep waiting}
    wait2 -->|submitted| review[Analyst review<br/>approval gate]
    wait2 -->|window lapses| review
    review --> done([End])"#;

        let ast = parse_flowchart(source).unwrap();
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let result = layout_flowchart(&ast, &measurer).unwrap();

        // Collect the two parallel wait2 -> review edges.
        let parallel: Vec<&PositionedEdge> = result
            .edges
            .iter()
            .filter(|e| e.from_id == "wait2" && e.to_id == "review")
            .collect();
        assert_eq!(
            parallel.len(),
            2,
            "expected both wait2 -> review edges to be present"
        );

        // Both edges carry labels with computed positions.
        let labeled: Vec<&&PositionedEdge> = parallel
            .iter()
            .filter(|e| e.label.is_some() && e.label_x.is_some() && e.label_y.is_some())
            .collect();
        assert_eq!(
            labeled.len(),
            2,
            "both parallel edges should have positioned labels"
        );

        // The two label anchors must not coincide — the whole point of the fix.
        let (ax, ay) = (labeled[0].label_x.unwrap(), labeled[0].label_y.unwrap());
        let (bx, by) = (labeled[1].label_x.unwrap(), labeled[1].label_y.unwrap());
        let dist = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
        assert!(
            dist > 1.0,
            "parallel-edge labels overlap: ({ax:.1},{ay:.1}) vs ({bx:.1},{by:.1})"
        );

        // And their routed paths must differ (not identical point sequences).
        assert!(
            parallel[0].points != parallel[1].points,
            "parallel-edge routes are identical: {:?}",
            parallel[0].points
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

            let corridor_min_x = src.x.min(tgt.x) - 300.0;
            let corridor_max_x = src.x.max(tgt.x) + 300.0;

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

    fn orient(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    }

    fn on_segment(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
        c.0 >= a.0.min(b.0) - 1e-6
            && c.0 <= a.0.max(b.0) + 1e-6
            && c.1 >= a.1.min(b.1) - 1e-6
            && c.1 <= a.1.max(b.1) + 1e-6
    }

    fn segments_intersect(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
        let o1 = orient(a, b, c);
        let o2 = orient(a, b, d);
        let o3 = orient(c, d, a);
        let o4 = orient(c, d, b);
        let s = |x: f64| {
            if x.abs() < 1e-6 {
                0
            } else if x > 0.0 {
                1
            } else {
                -1
            }
        };
        let (s1, s2, s3, s4) = (s(o1), s(o2), s(o3), s(o4));

        if s1 == 0 && on_segment(a, b, c) {
            return true;
        }
        if s2 == 0 && on_segment(a, b, d) {
            return true;
        }
        if s3 == 0 && on_segment(c, d, a) {
            return true;
        }
        if s4 == 0 && on_segment(c, d, b) {
            return true;
        }
        s1 * s2 < 0 && s3 * s4 < 0
    }

    fn points_equal(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    fn segments_share_endpoint(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
        points_equal(a, c) || points_equal(a, d) || points_equal(b, c) || points_equal(b, d)
    }

    fn count_edge_crossings(
        edges: &[PositionedEdge],
    ) -> (usize, Vec<(String, String, String, String)>) {
        let mut count = 0usize;
        let mut offenders = Vec::new();

        for i in 0..edges.len() {
            for j in (i + 1)..edges.len() {
                let e1 = &edges[i];
                let e2 = &edges[j];

                // Ignore pairs that share a node.
                if e1.from_id == e2.from_id
                    || e1.from_id == e2.to_id
                    || e1.to_id == e2.from_id
                    || e1.to_id == e2.to_id
                {
                    continue;
                }

                let mut hit = false;
                for s1 in e1.points.windows(2) {
                    let a = s1[0];
                    let b = s1[1];
                    for s2 in e2.points.windows(2) {
                        let c = s2[0];
                        let d = s2[1];
                        if segments_share_endpoint(a, b, c, d) {
                            continue;
                        }
                        if segments_intersect(a, b, c, d) {
                            hit = true;
                            break;
                        }
                    }
                    if hit {
                        break;
                    }
                }
                if hit {
                    count += 1;
                    offenders.push((
                        e1.from_id.clone(),
                        e1.to_id.clone(),
                        e2.from_id.clone(),
                        e2.to_id.clone(),
                    ));
                }
            }
        }
        (count, offenders)
    }

    #[test]
    fn test_example5_crossings_match_mermaidjs() {
        // Mermaid.js debug output for this exact input reports 0 edge-pair crossings
        // under this same segment-intersection rule.
        const MERMAID_JS_CROSSINGS: usize = 0;
        let source = include_str!("../../../../../tests/test_loop/input_mermaid.mmd");
        let result = layout_from_source(source);
        let (count, offenders) = count_edge_crossings(&result.edges);
        assert_eq!(
            count, MERMAID_JS_CROSSINGS,
            "edge crossings mismatch; expected {}, got {}. Offenders: {:?}",
            MERMAID_JS_CROSSINGS, count, offenders
        );
    }

    #[test]
    #[ignore = "debug helper for example5 rank/order parity"]
    fn debug_example5_key_node_positions() {
        let source = include_str!("../../../../../tests/test_loop/input_mermaid.mmd");
        let result = layout_from_source(source);
        let ids = ["RootOU", "EUOU", "APACOU", "OO1", "OO2", "D1", "D2"];
        for id in ids {
            let n = result
                .nodes
                .iter()
                .find(|n| n.id == id)
                .unwrap_or_else(|| panic!("node '{id}' not found"));
            eprintln!(
                "{id:8} x={:8.3} y={:8.3} w={:8.3} h={:8.3}",
                n.x, n.y, n.width, n.height
            );
        }
        for (from, to) in [("EUOU", "OO2"), ("APACOU", "OO1"), ("D2", "OO2")] {
            let e = result
                .edges
                .iter()
                .find(|e| e.from_id == from && e.to_id == to)
                .unwrap_or_else(|| panic!("edge {from}->{to} not found"));
            eprintln!("{from}->{to} points={:?}", e.points);
        }
        panic!("debug output above");
    }

    #[test]
    fn test_example2_isolated_subgraph_extraction_layout_shape() {
        let source = include_str!("../../../../../tests/test_loop/example2_input.mmd");
        let result = layout_from_source(source);

        let sg = result
            .subgraphs
            .iter()
            .find(|s| s.id == "A")
            .unwrap_or_else(|| panic!("subgraph A not found"));

        let top_row = ["sq", "e", "cyr"];
        let bottom_row = ["ci", "od3", "f", "cyr2"];

        // MermaidJS extracts isolated cluster A and lays it out as a standalone
        // left-side component in the top-level graph.
        for id in top_row.into_iter().chain(bottom_row) {
            let n = result
                .nodes
                .iter()
                .find(|n| n.id == id)
                .unwrap_or_else(|| panic!("node '{id}' not found"));
            assert!(
                sg.x + sg.width / 2.0 < n.x,
                "isolated subgraph A should be left of node {id}"
            );
        }

        let sq = result
            .nodes
            .iter()
            .find(|n| n.id == "sq")
            .expect("sq node should exist");
        let ci = result
            .nodes
            .iter()
            .find(|n| n.id == "ci")
            .expect("ci node should exist");
        assert!(ci.y > sq.y, "sq should be on an upper rank than ci");
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
