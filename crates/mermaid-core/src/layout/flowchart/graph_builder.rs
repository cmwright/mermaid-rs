use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};

use crate::ast::common::StyleProperties;
use crate::ast::flowchart::{
    ClassAssignment, ClassDef, EdgeDef, FlowchartAst, NodeDef, NodeShape, StyleOverride,
    SubgraphDef,
};
use crate::error::{MermaidError, Result};
use crate::layout::flowchart::types::*;
use crate::layout::text_measure::{TextMeasurer, TextMetrics};

// ── Class map ───────────────────────────────────────────────

pub fn build_class_map(class_defs: &[ClassDef]) -> HashMap<String, StyleProperties> {
    class_defs
        .iter()
        .map(|cd| (cd.name.clone(), cd.properties.clone()))
        .collect()
}

// ── Node collection ─────────────────────────────────────────

pub fn collect_all_nodes(
    ast: &FlowchartAst,
    class_defs: &HashMap<String, StyleProperties>,
) -> HashMap<String, (NodeDef, StyleProperties)> {
    let mut all_nodes: HashMap<String, (NodeDef, StyleProperties)> = HashMap::new();

    for node in &ast.nodes {
        let style = resolve_node_style(
            node,
            class_defs,
            &ast.class_assignments,
            &ast.style_overrides,
        );
        insert_or_merge_node(&mut all_nodes, node.clone(), style);
    }

    collect_subgraph_nodes(
        &ast.subgraphs,
        class_defs,
        &ast.class_assignments,
        &ast.style_overrides,
        &mut all_nodes,
    );

    // Ensure edge-referenced nodes exist
    for edge in &ast.edges {
        for id in [&edge.from, &edge.to] {
            all_nodes.entry(id.clone()).or_insert_with(|| {
                let node = NodeDef {
                    id: id.clone(),
                    label: None,
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                };
                (node, StyleProperties::default())
            });
        }
    }

    all_nodes
}

fn collect_subgraph_nodes(
    subgraphs: &[SubgraphDef],
    class_defs: &HashMap<String, StyleProperties>,
    class_assignments: &[ClassAssignment],
    style_overrides: &[StyleOverride],
    all_nodes: &mut HashMap<String, (NodeDef, StyleProperties)>,
) {
    for sg in subgraphs {
        for node in &sg.nodes {
            let style = resolve_node_style(node, class_defs, class_assignments, style_overrides);
            insert_or_merge_node(all_nodes, node.clone(), style);
        }
        for edge in &sg.edges {
            for id in [&edge.from, &edge.to] {
                all_nodes.entry(id.clone()).or_insert_with(|| {
                    let node = NodeDef {
                        id: id.clone(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    };
                    (node, StyleProperties::default())
                });
            }
        }
        collect_subgraph_nodes(
            &sg.subgraphs,
            class_defs,
            class_assignments,
            style_overrides,
            all_nodes,
        );
    }
}

/// Merge node entries by preferring explicit/labeled definitions over bare references.
/// This prevents cross-subgraph references from clobbering previously defined labels.
fn insert_or_merge_node(
    all_nodes: &mut HashMap<String, (NodeDef, StyleProperties)>,
    new_node: NodeDef,
    new_style: StyleProperties,
) {
    use std::collections::hash_map::Entry;
    match all_nodes.entry(new_node.id.clone()) {
        Entry::Vacant(v) => {
            v.insert((new_node, new_style));
        }
        Entry::Occupied(mut o) => {
            let (existing_node, existing_style) = o.get_mut();
            let existing_has_label = existing_node.label.is_some();
            let new_has_label = new_node.label.is_some();

            if !existing_has_label && new_has_label {
                *existing_node = new_node;
                *existing_style = new_style;
            } else if existing_has_label && new_has_label {
                // Keep the first explicit declaration's label/shape, but merge style.
                *existing_style = existing_style.merge(&new_style);
            } else {
                // Keep existing explicit node if present; otherwise keep first bare node.
                *existing_style = existing_style.merge(&new_style);
            }
        }
    }
}

fn resolve_node_style(
    node: &NodeDef,
    class_defs: &HashMap<String, StyleProperties>,
    class_assignments: &[ClassAssignment],
    style_overrides: &[StyleOverride],
) -> StyleProperties {
    let mut style = StyleProperties::default();

    if let Some(class_name) = &node.class_shorthand {
        if let Some(class_style) = class_defs.get(class_name) {
            style = style.merge(class_style);
        }
    }

    for ca in class_assignments {
        if ca.node_ids.contains(&node.id) {
            if let Some(class_style) = class_defs.get(&ca.class_name) {
                style = style.merge(class_style);
            }
        }
    }

    for so in style_overrides {
        if so.node_id == node.id {
            style = style.merge(&so.properties);
        }
    }

    style
}

// ── Edge collection ─────────────────────────────────────────

pub fn collect_all_edges(ast: &FlowchartAst) -> Vec<EdgeDef> {
    let mut all_edges = ast.edges.clone();
    collect_subgraph_edges(&ast.subgraphs, &mut all_edges);
    let mut seen = std::collections::HashSet::new();
    all_edges.retain(|e| seen.insert((e.from.clone(), e.to.clone())));
    all_edges
}

fn collect_subgraph_edges(subgraphs: &[SubgraphDef], all_edges: &mut Vec<EdgeDef>) {
    for sg in subgraphs {
        all_edges.extend(sg.edges.iter().cloned());
        collect_subgraph_edges(&sg.subgraphs, all_edges);
    }
}

// ── Subgraph membership ─────────────────────────────────────

/// Maps node ID to its subgraph containment path.
/// E.g., node "X" in subgraph "Inner" inside "Outer" gets path ["Outer", "Inner"].
pub type SubgraphMembership = HashMap<String, Vec<String>>;

pub fn build_subgraph_membership(ast: &FlowchartAst) -> SubgraphMembership {
    let mut membership = SubgraphMembership::new();
    let mut explicit_in_subgraph = std::collections::HashSet::new();
    for node in &ast.nodes {
        membership.entry(node.id.clone()).or_default();
    }
    collect_membership(
        &ast.subgraphs,
        &[],
        &mut membership,
        &mut explicit_in_subgraph,
    );
    membership
}

fn collect_membership(
    subgraphs: &[SubgraphDef],
    parent_path: &[String],
    membership: &mut SubgraphMembership,
    explicit_in_subgraph: &mut std::collections::HashSet<String>,
) {
    for sg in subgraphs {
        let mut path = parent_path.to_vec();
        path.push(sg.id.clone());
        for node in &sg.nodes {
            if node.label.is_some() {
                if !explicit_in_subgraph.contains(&node.id) {
                    membership.insert(node.id.clone(), path.clone());
                    explicit_in_subgraph.insert(node.id.clone());
                }
            } else {
                membership.entry(node.id.clone()).or_insert_with(|| path.clone());
            }
        }
        for edge in &sg.edges {
            for id in [&edge.from, &edge.to] {
                membership.entry(id.clone()).or_insert_with(|| path.clone());
            }
        }
        collect_membership(&sg.subgraphs, &path, membership, explicit_in_subgraph);
    }
}

// ── Petgraph construction ───────────────────────────────────

pub fn build_petgraph(
    all_nodes: &HashMap<String, (NodeDef, StyleProperties)>,
    edges: &[EdgeDef],
    measurer: &TextMeasurer<'_>,
) -> Result<(DiGraph<NodeData, EdgeData>, HashMap<String, NodeIndex>)> {
    let mut graph = DiGraph::new();
    let mut index_map: HashMap<String, NodeIndex> = HashMap::new();

    // Sort nodes by ID to ensure deterministic NodeIndex assignment.
    let mut sorted_nodes: Vec<_> = all_nodes.iter().collect();
    sorted_nodes.sort_by_key(|(id, _)| id.as_str());

    for (id, (node_def, style)) in sorted_nodes {
        let label = node_def.label.clone().unwrap_or_else(|| id.clone());

        let clean_text = crate::render::html_util::strip_html_tags(
            &crate::render::html_util::normalize_br(&label),
        );

        // Word-wrap long text and update the label if wrapping occurred
        let wrapped_text = measurer.wrap_text(&clean_text, MAX_NODE_TEXT_WIDTH);
        let label = if wrapped_text != clean_text {
            // Wrapping occurred — use the wrapped plain text as the label
            wrapped_text.clone()
        } else {
            // No wrapping needed — preserve original label (may contain HTML)
            label
        };

        let measure_text = &wrapped_text;
        let text_metrics = if measure_text.contains('\n') {
            measurer.measure_multiline(measure_text, 4.0)
        } else {
            measurer.measure(measure_text)
        };
        let (width, height) = compute_node_size(&node_def.shape, &text_metrics);

        let data = NodeData {
            id: id.clone(),
            label,
            shape: node_def.shape,
            style: style.clone(),
            width,
            height,
        };

        let idx = graph.add_node(data);
        index_map.insert(id.clone(), idx);
    }

    for edge in edges {
        let from_idx = index_map
            .get(&edge.from)
            .ok_or_else(|| MermaidError::Layout(format!("Unknown source node: {}", edge.from)))?;
        let to_idx = index_map
            .get(&edge.to)
            .ok_or_else(|| MermaidError::Layout(format!("Unknown target node: {}", edge.to)))?;

        let (label_width, label_height) = if let Some(ref label_text) = edge.label {
            let clean = crate::render::html_util::normalize_br(label_text);
            let clean = crate::render::html_util::strip_html_tags(&clean);
            let metrics = if clean.contains('\n') {
                measurer.measure_multiline(&clean, 4.0)
            } else {
                measurer.measure(&clean)
            };
            (metrics.width + 10.0, metrics.height + 6.0)
        } else {
            (0.0, 0.0)
        };

        graph.add_edge(
            *from_idx,
            *to_idx,
            EdgeData {
                label: edge.label.clone(),
                line_style: edge.line_style,
                arrow_start: edge.arrow_start,
                arrow_end: edge.arrow_end,
                label_width,
                label_height,
            },
        );
    }

    Ok((graph, index_map))
}

fn compute_node_size(shape: &NodeShape, text: &TextMetrics) -> (f64, f64) {
    let base_w = (text.width + 2.0 * NODE_PADDING_H).max(MIN_NODE_WIDTH);
    let base_h = (text.height + 2.0 * NODE_PADDING_V).max(MIN_NODE_HEIGHT);
    const RECT_LABEL_EXTRA_WIDTH: f64 = 12.0;

    match shape {
        NodeShape::Rectangle
        | NodeShape::RoundedRectangle
        | NodeShape::Stadium
        | NodeShape::Subroutine
        | NodeShape::Cylinder => (base_w + RECT_LABEL_EXTRA_WIDTH, base_h),
        NodeShape::Diamond => {
            // Rotated square: to inscribe a rectangle W×H inside a 45°-rotated square,
            // the half-diagonal must be (W + H) / 2. Both diagonals are equal.
            let d = base_w + base_h;
            (d, d)
        }
        NodeShape::Circle | NodeShape::DoubleCircle => {
            // Circle must fully contain the text rectangle: diameter = diagonal of the rect
            let diameter = (base_w * base_w + base_h * base_h).sqrt();
            (diameter, diameter)
        }
        NodeShape::Hexagon => (base_w + base_h * 0.5, base_h),
        NodeShape::Asymmetric => {
            // The left V-notch eats into text area: notch = hh * 0.6 = base_h * 0.3
            // Add full notch width (text is shifted right by notch/2, so both sides need room)
            let notch = base_h * 0.3;
            (base_w + 2.0 * notch + RECT_LABEL_EXTRA_WIDTH, base_h)
        }
        _ => (base_w, base_h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::common::{parse_style_string, StyleProperties};
    use crate::font::FontProvider;
    use crate::parser::flowchart::parse_flowchart;

    fn make_measurer(provider: &FontProvider) -> TextMeasurer<'_> {
        let font = provider.font_ref().unwrap();
        TextMeasurer::new(font, 14.0)
    }

    #[test]
    fn test_collect_subgraph_nodes_nested() {
        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "Outer".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![NodeDef {
                        id: "A".into(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    }],
                    edges: vec![],
                    subgraphs: vec![SubgraphDef {
                        id: "Inner".to_string(),
                        label: None,
                        direction: None,
                        nodes: vec![NodeDef {
                            id: "B".into(),
                            label: None,
                            shape: NodeShape::Rectangle,
                            class_shorthand: None,
                        }],
                        edges: vec![],
                        subgraphs: vec![],
                    }],
                },
            ],
            ..Default::default()
        };
        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        assert!(all_nodes.contains_key("A"));
        assert!(all_nodes.contains_key("B"));
    }

    #[test]
    fn test_resolve_node_style_class_defs_and_overrides() {
        let ast = FlowchartAst {
            nodes: vec![NodeDef {
                id: "A".into(),
                label: None,
                shape: NodeShape::Rectangle,
                class_shorthand: Some("green".into()),
            }],
            class_defs: vec![
                ClassDef {
                    name: "green".to_string(),
                    properties: parse_style_string("fill:#9f6"),
                },
                ClassDef {
                    name: "orange".to_string(),
                    properties: parse_style_string("fill:#f96"),
                },
            ],
            class_assignments: vec![ClassAssignment {
                node_ids: vec!["B".into()],
                class_name: "orange".to_string(),
            }],
            style_overrides: vec![StyleOverride {
                node_id: "A".into(),
                properties: parse_style_string("stroke:#333"),
            }],
            edges: vec![],
            subgraphs: vec![SubgraphDef {
                id: "SG".to_string(),
                label: None,
                direction: None,
                nodes: vec![NodeDef {
                    id: "B".into(),
                    label: None,
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                }],
                edges: vec![],
                subgraphs: vec![],
            }],
            ..Default::default()
        };
        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        let (_, style_a) = all_nodes.get("A").unwrap();
        assert!(style_a.fill.is_some() || style_a.stroke.is_some());
    }

    #[test]
    fn test_collect_subgraph_edges_recursive() {
        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "Outer".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![],
                    edges: vec![EdgeDef {
                        from: "A".into(),
                        to: "B".into(),
                        line_style: crate::ast::flowchart::LineStyle::Solid,
                        arrow_start: crate::ast::flowchart::ArrowEnd::None,
                        arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                        label: None,
                    }],
                    subgraphs: vec![SubgraphDef {
                        id: "Inner".to_string(),
                        label: None,
                        direction: None,
                        nodes: vec![],
                        edges: vec![EdgeDef {
                            from: "C".into(),
                            to: "D".into(),
                            line_style: crate::ast::flowchart::LineStyle::Solid,
                            arrow_start: crate::ast::flowchart::ArrowEnd::None,
                            arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                            label: None,
                        }],
                        subgraphs: vec![],
                    }],
                },
            ],
            ..Default::default()
        };
        let all_edges = collect_all_edges(&ast);
        assert!(all_edges.iter().any(|e| e.from == "A" && e.to == "B"));
        assert!(all_edges.iter().any(|e| e.from == "C" && e.to == "D"));
    }

    #[test]
    fn test_collect_membership_nested() {
        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "Outer".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![NodeDef {
                        id: "A".into(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    }],
                    edges: vec![],
                    subgraphs: vec![SubgraphDef {
                        id: "Inner".to_string(),
                        label: None,
                        direction: None,
                        nodes: vec![NodeDef {
                            id: "B".into(),
                            label: None,
                            shape: NodeShape::Rectangle,
                            class_shorthand: None,
                        }],
                        edges: vec![],
                        subgraphs: vec![],
                    }],
                },
            ],
            ..Default::default()
        };
        let membership = build_subgraph_membership(&ast);
        assert_eq!(membership.get("A").unwrap(), &vec!["Outer".to_string()]);
        assert_eq!(
            membership.get("B").unwrap(),
            &vec!["Outer".to_string(), "Inner".to_string()]
        );
    }

    #[test]
    fn test_collect_all_nodes_edge_referenced() {
        // Edge references nodes not in ast.nodes -> or_insert_with (lines 53-60)
        let ast = FlowchartAst {
            nodes: vec![],
            edges: vec![
                EdgeDef {
                    from: "A".into(),
                    to: "B".into(),
                    line_style: crate::ast::flowchart::LineStyle::Solid,
                    arrow_start: crate::ast::flowchart::ArrowEnd::None,
                    arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                    label: None,
                },
            ],
            ..Default::default()
        };
        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        assert!(all_nodes.contains_key("A"));
        assert!(all_nodes.contains_key("B"));
        let (node_a, _) = all_nodes.get("A").unwrap();
        assert_eq!(node_a.shape, NodeShape::Rectangle);
    }

    #[test]
    fn test_collect_subgraph_nodes_edge_referenced() {
        // Subgraph edge references nodes not in sg.nodes -> or_insert_with (lines 82-89)
        let ast = FlowchartAst {
            nodes: vec![],
            edges: vec![],
            subgraphs: vec![SubgraphDef {
                id: "SG".to_string(),
                label: None,
                direction: None,
                nodes: vec![],
                edges: vec![
                    EdgeDef {
                        from: "X".into(),
                        to: "Y".into(),
                        line_style: crate::ast::flowchart::LineStyle::Solid,
                        arrow_start: crate::ast::flowchart::ArrowEnd::None,
                        arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                        label: None,
                    },
                ],
                subgraphs: vec![],
            }],
            ..Default::default()
        };
        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        assert!(all_nodes.contains_key("X"));
        assert!(all_nodes.contains_key("Y"));
    }

    #[test]
    fn test_compute_node_size_shapes() {
        let mut all_nodes = HashMap::new();
        for (id, shape) in [
            ("D", NodeShape::Diamond),
            ("C", NodeShape::Circle),
            ("DC", NodeShape::DoubleCircle),
            ("H", NodeShape::Hexagon),
            ("A", NodeShape::Asymmetric),
        ] {
            all_nodes.insert(
                id.to_string(),
                (
                    NodeDef {
                        id: id.to_string(),
                        label: Some("X".into()),
                        shape,
                        class_shorthand: None,
                    },
                    StyleProperties::default(),
                ),
            );
        }
        let edges = vec![
            EdgeDef {
                from: "D".into(),
                to: "C".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
            },
            EdgeDef {
                from: "C".into(),
                to: "DC".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
            },
            EdgeDef {
                from: "DC".into(),
                to: "H".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
            },
            EdgeDef {
                from: "H".into(),
                to: "A".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
            },
        ];
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let (graph, _) = build_petgraph(&all_nodes, &edges, &measurer).unwrap();
        for (id, expected_shape) in [
            ("D", NodeShape::Diamond),
            ("C", NodeShape::Circle),
            ("DC", NodeShape::DoubleCircle),
            ("H", NodeShape::Hexagon),
            ("A", NodeShape::Asymmetric),
        ] {
            let node = graph.node_indices().find(|i| graph[*i].id == id).unwrap();
            assert_eq!(graph[node].shape, expected_shape);
            assert!(graph[node].width > 0.0);
            assert!(graph[node].height > 0.0);
        }
    }

    #[test]
    fn test_cross_subgraph_refs_do_not_clobber_labeled_nodes_or_membership() {
        let source = include_str!("../../../../../tests/test_loop/test_graphs.mmd");
        let ast = parse_flowchart(source).unwrap();

        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);

        assert_eq!(
            all_nodes.get("Eng").unwrap().0.label.as_deref(),
            Some("Folder: engineering")
        );
        assert_eq!(
            all_nodes.get("Backend").unwrap().0.label.as_deref(),
            Some("Folder: backend")
        );
        assert_eq!(
            all_nodes.get("F3").unwrap().0.label.as_deref(),
            Some("secret-report.pdf")
        );

        let membership = build_subgraph_membership(&ast);
        assert_eq!(membership.get("Eng").unwrap(), &vec!["Folders".to_string()]);
        assert_eq!(membership.get("Backend").unwrap(), &vec!["Folders".to_string()]);
        assert_eq!(membership.get("F3").unwrap(), &vec!["Files".to_string()]);
    }
}
