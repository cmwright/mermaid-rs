use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};

use crate::ast::common::StyleProperties;
use crate::ast::flowchart::{
    ClassAssignment, ClassDef, EdgeDef, FlowchartAst, NodeDef, NodeShape, StyleOverride,
    SubgraphDef,
};
use crate::error::{MermaidError, Result};
use crate::layout::text_measure::{TextMeasurer, TextMetrics};
use crate::layout::types::*;

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
        all_nodes.insert(node.id.clone(), (node.clone(), style));
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
            all_nodes.insert(node.id.clone(), (node.clone(), style));
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
    for node in &ast.nodes {
        membership.entry(node.id.clone()).or_default();
    }
    collect_membership(&ast.subgraphs, &[], &mut membership);
    membership
}

fn collect_membership(
    subgraphs: &[SubgraphDef],
    parent_path: &[String],
    membership: &mut SubgraphMembership,
) {
    for sg in subgraphs {
        let mut path = parent_path.to_vec();
        path.push(sg.id.clone());
        for node in &sg.nodes {
            membership.insert(node.id.clone(), path.clone());
        }
        for edge in &sg.edges {
            for id in [&edge.from, &edge.to] {
                membership.entry(id.clone()).or_insert_with(|| path.clone());
            }
        }
        collect_membership(&sg.subgraphs, &path, membership);
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

        let measure_text = crate::render::html_util::strip_html_tags(
            &crate::render::html_util::normalize_br(&label),
        );
        let text_metrics = if measure_text.contains('\n') {
            measurer.measure_multiline(&measure_text, 4.0)
        } else {
            measurer.measure(&measure_text)
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
            let metrics = measurer.measure(label_text);
            (metrics.width + 10.0, metrics.height + 6.0)
        } else {
            (0.0, 0.0)
        };

        graph.add_edge(
            *from_idx,
            *to_idx,
            EdgeData {
                label: edge.label.clone(),
                edge_type: edge.edge_type,
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

    match shape {
        NodeShape::Diamond => (base_w * 1.42, base_h * 1.42),
        NodeShape::Circle | NodeShape::DoubleCircle => {
            let diameter = base_w.max(base_h);
            (diameter, diameter)
        }
        NodeShape::Hexagon => (base_w + base_h * 0.5, base_h),
        _ => (base_w, base_h),
    }
}
