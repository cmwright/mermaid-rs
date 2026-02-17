use std::collections::{HashMap, HashSet};

use dagre_rs::{DagreLayout, LayoutOptions, RankDir};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction as PetDirection;

use crate::ast::common::StyleProperties;
use crate::ast::flowchart::{
    ClassAssignment, ClassDef, Direction, EdgeDef, EdgeType, FlowchartAst, NodeDef, NodeShape,
    StyleOverride, SubgraphDef,
};
use crate::error::{MermaidError, Result};
use crate::layout::text_measure::{TextMeasurer, TextMetrics};

// ── Layout constants ────────────────────────────────────────

const NODE_PADDING_H: f64 = 24.0;
const NODE_PADDING_V: f64 = 14.0;
const MIN_NODE_WIDTH: f64 = 70.0;
const MIN_NODE_HEIGHT: f64 = 40.0;
const NODE_SEP: f64 = 60.0;
const RANK_SEP: f64 = 100.0;
const SUBGRAPH_PADDING: f64 = 30.0;
const SUBGRAPH_TITLE_HEIGHT: f64 = 25.0;
const SUBGRAPH_GROUP_GAP: f64 = 30.0;
const ZONE_GAP: f64 = 80.0; // Extra gap between top-level subgraph rank zones

// ── Positioned types ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PositionedNode {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
    pub style: StyleProperties,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct PositionedEdge {
    pub from_id: String,
    pub to_id: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
    pub label_x: Option<f64>,
    pub label_y: Option<f64>,
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct PositionedSubgraph {
    pub id: String,
    pub label: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub style: StyleProperties,
}

#[derive(Debug, Clone)]
pub struct PositionedGraph {
    pub nodes: Vec<PositionedNode>,
    pub edges: Vec<PositionedEdge>,
    pub subgraphs: Vec<PositionedSubgraph>,
    pub width: f64,
    pub height: f64,
    pub direction: Direction,
}

// ── Graph node/edge data ────────────────────────────────────

#[derive(Debug, Clone)]
struct NodeData {
    id: String,
    label: String,
    shape: NodeShape,
    style: StyleProperties,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone)]
struct EdgeData {
    _label: Option<String>,
    _edge_type: EdgeType,
}

// ── Public API ──────────────────────────────────────────────

/// Compute layout positions for a flowchart AST.
pub fn layout_flowchart(ast: &FlowchartAst, measurer: &TextMeasurer<'_>) -> Result<PositionedGraph> {
    // 1. Build class definitions map
    let class_defs = build_class_map(&ast.class_defs);

    // 2. Collect all nodes, merging style information
    let all_nodes = collect_all_nodes(ast, &class_defs);

    // 3. Collect all edges (including from subgraphs)
    let all_edges = collect_all_edges(ast);

    // 4. Build petgraph
    let (graph, index_map) = build_petgraph(&all_nodes, &all_edges, measurer)?;

    // 5. Build subgraph membership map
    let membership = build_subgraph_membership(ast);

    // 6. Compound layout: run dagre independently per top-level subgraph.
    // This ensures nodes from different top-level subgraphs (e.g. Platform,
    // OryNetwork, ExtIdPs) occupy separate rank zones.
    let layers = build_compound_layers(
        &graph, &index_map, &membership, &all_edges, &ast.subgraphs, ast.direction,
    );

    // 6b. Within each layer, group nodes by second-level subgraph membership
    // while preserving dagre's crossing-reduced ordering.
    let layers = constrain_layers_by_subgraph(&graph, &layers, &membership);

    // 7. Position nodes using constrained layers with our own sizing
    let mut positioned_nodes = position_nodes_from_layers(
        &graph,
        &layers,
        ast.direction,
        &membership,
    );

    // 8. Route edges
    let mut positioned_edges = route_edges(&graph, &index_map, &positioned_nodes, &all_edges);

    // 9. Position subgraphs (with style overrides)
    let mut positioned_subgraphs = position_subgraphs(
        &ast.subgraphs, &positioned_nodes, &ast.style_overrides,
    );

    // 10. Normalize coordinates (shift to positive) and compute bounding box
    let (width, height) = normalize_and_compute_bounds(
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

// ── Internal implementation ─────────────────────────────────

fn build_class_map(class_defs: &[ClassDef]) -> HashMap<String, StyleProperties> {
    class_defs
        .iter()
        .map(|cd| (cd.name.clone(), cd.properties.clone()))
        .collect()
}

fn collect_all_nodes(
    ast: &FlowchartAst,
    class_defs: &HashMap<String, StyleProperties>,
) -> HashMap<String, (NodeDef, StyleProperties)> {
    let mut all_nodes: HashMap<String, (NodeDef, StyleProperties)> = HashMap::new();

    // Collect from top-level nodes
    for node in &ast.nodes {
        let style = resolve_node_style(node, class_defs, &ast.class_assignments, &ast.style_overrides);
        all_nodes.insert(node.id.clone(), (node.clone(), style));
    }

    // Collect from subgraphs recursively
    collect_subgraph_nodes(&ast.subgraphs, class_defs, &ast.class_assignments, &ast.style_overrides, &mut all_nodes);

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
        // Ensure edge-referenced nodes within subgraphs exist
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
        collect_subgraph_nodes(&sg.subgraphs, class_defs, class_assignments, style_overrides, all_nodes);
    }
}

/// Maps node ID to its subgraph containment path.
/// E.g., node "X" in subgraph "Inner" inside "Outer" gets path ["Outer", "Inner"].
type SubgraphMembership = HashMap<String, Vec<String>>;

fn build_subgraph_membership(ast: &FlowchartAst) -> SubgraphMembership {
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

fn collect_all_edges(ast: &FlowchartAst) -> Vec<EdgeDef> {
    let mut all_edges = ast.edges.clone();
    collect_subgraph_edges(&ast.subgraphs, &mut all_edges);
    // Deduplicate by (from, to) pair
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

/// Build layers using recursive compound layout: at each nesting level,
/// group nodes by their immediate subgraph, run dagre per group, then
/// enforce rank separation between groups connected by cross-group edges.
///
/// After computing compound ranks (zone-separated), we run a GLOBAL dagre
/// pass to get optimal horizontal ordering (crossing reduction across all
/// edges including cross-zone ones), and re-sort each compound layer to
/// match the global ordering. This gives us zone-separated Y positions
/// with globally-optimized X ordering.
fn build_compound_layers(
    full_graph: &DiGraph<NodeData, EdgeData>,
    index_map: &HashMap<String, NodeIndex>,
    membership: &SubgraphMembership,
    all_edges: &[EdgeDef],
    subgraphs: &[SubgraphDef],
    direction: Direction,
) -> Vec<Vec<NodeIndex>> {
    let all_node_indices: Vec<NodeIndex> = full_graph.node_indices().collect();

    // Step 1: Get compound rank assignment (zone-separated)
    let compound_layers = compound_layout_recursive(
        full_graph, index_map, membership, all_edges,
        &all_node_indices, subgraphs, direction, 0,
    );

    // Step 2: Run global dagre for horizontal ordering hints.
    // This considers ALL edges (including cross-zone) for crossing reduction.
    let global_layers = run_dagre_on_subset(
        full_graph, index_map, all_edges, &all_node_indices, direction,
    );

    // Build global order map: node → (global_rank, position_in_rank)
    let mut global_pos: HashMap<NodeIndex, (usize, usize)> = HashMap::new();
    for (rank, layer) in global_layers.iter().enumerate() {
        for (pos, &ni) in layer.iter().enumerate() {
            global_pos.insert(ni, (rank, pos));
        }
    }

    // Step 3: Re-sort each compound layer using global ordering.
    // Nodes that dagre placed at an earlier global rank come first,
    // and within the same global rank, dagre's position order is preserved.
    let mut layers: Vec<Vec<NodeIndex>> = compound_layers.into_iter().map(|mut layer| {
        layer.sort_by(|a, b| {
            let ga = global_pos.get(a).copied().unwrap_or((usize::MAX, usize::MAX));
            let gb = global_pos.get(b).copied().unwrap_or((usize::MAX, usize::MAX));
            ga.cmp(&gb)
        });
        layer
    }).collect();

    // Safety: ensure all nodes are assigned to a layer
    let assigned: HashSet<NodeIndex> = layers.iter()
        .flat_map(|layer| layer.iter().copied())
        .collect();
    let unassigned: Vec<NodeIndex> = full_graph.node_indices()
        .filter(|ni| !assigned.contains(ni))
        .collect();
    if !unassigned.is_empty() {
        layers.push(unassigned);
    }

    layers
}

/// Recursive compound layout: group nodes by child subgraphs, determine
/// ordering via cross-group edges, run dagre within each group, then
/// enforce rank zone separation between groups.
fn compound_layout_recursive(
    full_graph: &DiGraph<NodeData, EdgeData>,
    index_map: &HashMap<String, NodeIndex>,
    membership: &SubgraphMembership,
    all_edges: &[EdgeDef],
    node_indices: &[NodeIndex],
    child_subgraphs: &[SubgraphDef],
    direction: Direction,
    depth: usize,
) -> Vec<Vec<NodeIndex>> {
    // If no child subgraphs, just run dagre on these nodes
    if child_subgraphs.is_empty() {
        return run_dagre_on_subset(full_graph, index_map, all_edges, node_indices, direction);
    }

    // Group nodes by which child subgraph they belong to (at this level)
    let node_id_set: HashSet<&str> = node_indices.iter()
        .map(|&ni| full_graph[ni].id.as_str())
        .collect();

    // Map: child subgraph ID → list of node indices in it (direct + nested)
    let mut sg_nodes: HashMap<String, Vec<NodeIndex>> = HashMap::new();
    let mut root_nodes: Vec<NodeIndex> = Vec::new(); // nodes not in any child subgraph

    for &ni in node_indices {
        let id = &full_graph[ni].id;
        let path = membership.get(id).cloned().unwrap_or_default();

        // Find which child subgraph this node belongs to at this depth
        let child_sg_id = path.get(depth).cloned();

        if let Some(sg_id) = child_sg_id {
            // Verify this is actually one of the child subgraphs
            if child_subgraphs.iter().any(|sg| sg.id == sg_id) {
                sg_nodes.entry(sg_id).or_default().push(ni);
            } else {
                root_nodes.push(ni);
            }
        } else {
            root_nodes.push(ni);
        }
    }

    // Determine ordering of child subgraph groups via cross-group edges
    let internal_edges: Vec<&EdgeDef> = all_edges.iter()
        .filter(|e| node_id_set.contains(e.from.as_str()) && node_id_set.contains(e.to.as_str()))
        .collect();

    // Build meta-graph of child subgraphs
    let mut meta_deps: HashMap<String, HashSet<String>> = HashMap::new(); // target → set of sources
    for edge in &internal_edges {
        let from_path = membership.get(&edge.from).cloned().unwrap_or_default();
        let to_path = membership.get(&edge.to).cloned().unwrap_or_default();
        let from_sg = from_path.get(depth).cloned();
        let to_sg = to_path.get(depth).cloned();

        if let (Some(from_id), Some(to_id)) = (from_sg, to_sg) {
            if from_id != to_id
                && child_subgraphs.iter().any(|sg| sg.id == from_id)
                && child_subgraphs.iter().any(|sg| sg.id == to_id)
            {
                meta_deps.entry(to_id).or_default().insert(from_id);
            }
        }
    }

    // Topological sort of child subgraphs (using Kahn's algorithm)
    let sg_ids: Vec<String> = child_subgraphs.iter().map(|sg| sg.id.clone()).collect();
    let sg_order = topological_sort_subgraphs(&sg_ids, &meta_deps);

    // Group subgraphs into tiers: subgraphs with no cross-group edges between
    // them share a tier (run dagre together), while dependent groups get separate zones.
    let mut tiers: Vec<Vec<String>> = Vec::new();
    let mut sg_tier: HashMap<String, usize> = HashMap::new();

    for sg_id in &sg_order {
        // Find the max tier of any predecessor
        let pred_tier = meta_deps.get(sg_id)
            .map(|preds| {
                preds.iter()
                    .filter_map(|p| sg_tier.get(p))
                    .max()
                    .copied()
            })
            .flatten();

        let tier_idx = match pred_tier {
            Some(t) => t + 1, // Must be in a later tier than all predecessors
            None => 0,
        };

        // Extend tiers if needed
        while tiers.len() <= tier_idx {
            tiers.push(Vec::new());
        }
        tiers[tier_idx].push(sg_id.clone());
        sg_tier.insert(sg_id.clone(), tier_idx);
    }

    // For each tier, collect all nodes and run recursive layout
    let mut all_layers: Vec<Vec<NodeIndex>> = Vec::new();

    // Root nodes (not in any child subgraph) go into the first tier
    if !root_nodes.is_empty() {
        let root_layers = run_dagre_on_subset(
            full_graph, index_map, all_edges, &root_nodes, direction,
        );
        all_layers.extend(root_layers);
    }

    for tier in &tiers {
        // Collect all nodes in this tier's subgraphs
        let mut tier_nodes: Vec<NodeIndex> = Vec::new();
        let mut tier_child_subgraphs: Vec<&SubgraphDef> = Vec::new();

        for sg_id in tier {
            if let Some(nodes) = sg_nodes.get(sg_id) {
                tier_nodes.extend(nodes);
            }
            if let Some(sg_def) = child_subgraphs.iter().find(|sg| sg.id == *sg_id) {
                tier_child_subgraphs.push(sg_def);
            }
        }

        if tier_nodes.is_empty() {
            continue;
        }

        // If this tier has subgraphs with their own children, recurse
        // Otherwise just run dagre on the tier's nodes
        let has_nested = tier_child_subgraphs.iter().any(|sg| !sg.subgraphs.is_empty());

        if has_nested && tier_child_subgraphs.len() == 1 {
            // Single subgraph with nested children: recurse into it
            let sg = tier_child_subgraphs[0];
            let tier_layers = compound_layout_recursive(
                full_graph, index_map, membership, all_edges,
                &tier_nodes, &sg.subgraphs, direction, depth + 1,
            );
            all_layers.extend(tier_layers);
        } else {
            // Multiple subgraphs in same tier or no nested children:
            // run dagre on all tier nodes together
            let tier_layers = run_dagre_on_subset(
                full_graph, index_map, all_edges, &tier_nodes, direction,
            );
            all_layers.extend(tier_layers);
        }
    }

    all_layers
}

/// Run dagre on a subset of nodes, using only edges where both endpoints
/// are in the subset. Returns layers with full-graph NodeIndex values.
fn run_dagre_on_subset(
    full_graph: &DiGraph<NodeData, EdgeData>,
    index_map: &HashMap<String, NodeIndex>,
    all_edges: &[EdgeDef],
    node_indices: &[NodeIndex],
    direction: Direction,
) -> Vec<Vec<NodeIndex>> {
    if node_indices.is_empty() {
        return Vec::new();
    }

    let node_id_set: HashSet<&str> = node_indices.iter()
        .map(|&ni| full_graph[ni].id.as_str())
        .collect();

    // Build sub-graph
    let mut sub_graph = DiGraph::<NodeData, EdgeData>::new();
    let mut full_to_sub: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut sub_to_full: HashMap<NodeIndex, NodeIndex> = HashMap::new();

    for &ni in node_indices {
        let sub_ni = sub_graph.add_node(full_graph[ni].clone());
        full_to_sub.insert(ni, sub_ni);
        sub_to_full.insert(sub_ni, ni);
    }

    // Add only internal edges
    for edge in all_edges {
        if node_id_set.contains(edge.from.as_str()) && node_id_set.contains(edge.to.as_str()) {
            let from_full = index_map.get(&edge.from);
            let to_full = index_map.get(&edge.to);
            if let (Some(&ff), Some(&tf)) = (from_full, to_full) {
                if let (Some(&from_sub), Some(&to_sub)) = (
                    full_to_sub.get(&ff),
                    full_to_sub.get(&tf),
                ) {
                    sub_graph.add_edge(from_sub, to_sub, EdgeData {
                        _label: edge.label.clone(),
                        _edge_type: edge.edge_type,
                    });
                }
            }
        }
    }

    // Run dagre
    let rank_dir = match direction {
        Direction::LeftToRight | Direction::RightToLeft => RankDir::LeftToRight,
        _ => RankDir::TopToBottom,
    };

    let dagre = DagreLayout::with_options(LayoutOptions {
        rank_dir,
        node_sep: NODE_SEP as f32,
        rank_sep: RANK_SEP as f32,
        max_iterations: 24,
    });
    let result = dagre.compute(&sub_graph);

    // Map layers back to full-graph NodeIndex
    result.layers.iter()
        .filter_map(|layer| {
            let full_layer: Vec<NodeIndex> = layer.iter()
                .filter_map(|&sub_ni| sub_to_full.get(&sub_ni).copied())
                .collect();
            if full_layer.is_empty() { None } else { Some(full_layer) }
        })
        .collect()
}

/// Topological sort of subgraph IDs using Kahn's algorithm.
fn topological_sort_subgraphs(
    ids: &[String],
    deps: &HashMap<String, HashSet<String>>, // target → set of predecessors
) -> Vec<String> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    for id in ids {
        in_degree.insert(id.as_str(), 0);
    }
    for (target, sources) in deps {
        if let Some(deg) = in_degree.get_mut(target.as_str()) {
            *deg = sources.iter().filter(|s| ids.iter().any(|id| id == *s)).count();
        }
    }

    let mut queue: Vec<&str> = ids.iter()
        .filter(|id| *in_degree.get(id.as_str()).unwrap_or(&0) == 0)
        .map(|s| s.as_str())
        .collect();
    let mut result: Vec<String> = Vec::new();

    while let Some(id) = queue.pop() {
        result.push(id.to_string());
        // Decrement in-degree of dependents
        for (target, sources) in deps {
            if sources.contains(id) {
                if let Some(deg) = in_degree.get_mut(target.as_str()) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push(target.as_str());
                    }
                }
            }
        }
    }

    // Add any remaining (cycles or unvisited)
    for id in ids {
        if !result.contains(id) {
            result.push(id.clone());
        }
    }

    result
}

fn resolve_node_style(
    node: &NodeDef,
    class_defs: &HashMap<String, StyleProperties>,
    class_assignments: &[ClassAssignment],
    style_overrides: &[StyleOverride],
) -> StyleProperties {
    let mut style = StyleProperties::default();

    // Apply class from shorthand (:::className)
    if let Some(class_name) = &node.class_shorthand {
        if let Some(class_style) = class_defs.get(class_name) {
            style = style.merge(class_style);
        }
    }

    // Apply class from class assignments
    for ca in class_assignments {
        if ca.node_ids.contains(&node.id) {
            if let Some(class_style) = class_defs.get(&ca.class_name) {
                style = style.merge(class_style);
            }
        }
    }

    // Apply inline style overrides
    for so in style_overrides {
        if so.node_id == node.id {
            style = style.merge(&so.properties);
        }
    }

    style
}

fn build_petgraph(
    all_nodes: &HashMap<String, (NodeDef, StyleProperties)>,
    edges: &[EdgeDef],
    measurer: &TextMeasurer<'_>,
) -> Result<(DiGraph<NodeData, EdgeData>, HashMap<String, NodeIndex>)> {
    let mut graph = DiGraph::new();
    let mut index_map: HashMap<String, NodeIndex> = HashMap::new();

    // Add all nodes
    for (id, (node_def, style)) in all_nodes {
        let label = node_def.label.clone().unwrap_or_else(|| id.clone());

        // Strip HTML tags and normalize <br> for measurement
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

    // Add all edges
    for edge in edges {
        let from_idx = index_map.get(&edge.from).ok_or_else(|| {
            MermaidError::Layout(format!("Unknown source node: {}", edge.from))
        })?;
        let to_idx = index_map.get(&edge.to).ok_or_else(|| {
            MermaidError::Layout(format!("Unknown target node: {}", edge.to))
        })?;

        graph.add_edge(
            *from_idx,
            *to_idx,
            EdgeData {
                _label: edge.label.clone(),
                _edge_type: edge.edge_type,
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

/// Re-sort layers so that nodes belonging to the same subgraph are contiguous
/// within each rank. Groups are ordered by their first appearance in dagre's
/// ordering (preserving crossing reduction), not alphabetically.
fn constrain_layers_by_subgraph(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    membership: &SubgraphMembership,
) -> Vec<Vec<NodeIndex>> {
    let empty_path: Vec<String> = Vec::new();

    layers
        .iter()
        .map(|layer| {
            // Group nodes by their membership path, preserving dagre's order
            let mut groups: Vec<(Vec<String>, Vec<NodeIndex>)> = Vec::new();

            for &ni in layer {
                let path = membership.get(&graph[ni].id)
                    .unwrap_or(&empty_path)
                    .clone();

                if let Some(group) = groups.iter_mut().find(|(p, _)| *p == path) {
                    group.1.push(ni);
                } else {
                    groups.push((path, vec![ni]));
                }
            }

            // Flatten: groups keep dagre's order, nodes within groups keep dagre's order
            groups.into_iter()
                .flat_map(|(_, members)| members)
                .collect()
        })
        .collect()
}

/// Position nodes using dagre-rs layers (which provide rank assignment + crossing-reduced ordering).
/// We apply our own coordinate assignment that accounts for actual node dimensions,
/// subgraph membership, and spacing.
fn position_nodes_from_layers(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    direction: Direction,
    membership: &SubgraphMembership,
) -> Vec<PositionedNode> {
    let is_horizontal = matches!(direction, Direction::LeftToRight | Direction::RightToLeft);
    let empty_path: Vec<String> = Vec::new();

    // Phase 1: Initial size-aware coordinate assignment
    // Place nodes accounting for actual widths/heights and subgraph gaps
    let mut node_positions: HashMap<NodeIndex, (f64, f64)> = HashMap::new();
    let mut rank_offset = 0.0;
    let mut prev_paths: Option<Vec<Vec<String>>> = None;

    for layer in layers {
        // Add extra spacing when transitioning between subgraph zones at any depth
        if !layer.is_empty() {
            let curr_paths: Vec<Vec<String>> = layer.iter()
                .map(|&ni| membership.get(&graph[ni].id).cloned().unwrap_or_default())
                .collect();

            if let Some(ref prev) = prev_paths {
                // Check if the first element of any path changed (top-level subgraph transition)
                let prev_tops: HashSet<Option<&String>> = prev.iter()
                    .map(|p| p.first())
                    .collect();
                let curr_tops: HashSet<Option<&String>> = curr_paths.iter()
                    .map(|p| p.first())
                    .collect();
                if prev_tops != curr_tops {
                    rank_offset += ZONE_GAP;
                }
            }
            prev_paths = Some(curr_paths);
        }

        let max_thickness = layer
            .iter()
            .map(|&idx| {
                let node = &graph[idx];
                if is_horizontal { node.width } else { node.height }
            })
            .fold(0.0f64, f64::max);

        let mut cross_offset = 0.0;
        let mut prev_path: Option<&Vec<String>> = None;

        for &idx in layer {
            let node = &graph[idx];
            let node_path = membership.get(&node.id).unwrap_or(&empty_path);

            // Add spacing at subgraph boundaries
            if let Some(prev) = prev_path {
                if prev != node_path {
                    let common = prev.iter().zip(node_path.iter())
                        .take_while(|(a, b)| a == b).count();
                    let divergence = prev.len().max(node_path.len()) - common;
                    cross_offset += SUBGRAPH_GROUP_GAP * divergence as f64;
                }
            }

            let (x, y) = if is_horizontal {
                (rank_offset + max_thickness / 2.0, cross_offset + node.height / 2.0)
            } else {
                (cross_offset + node.width / 2.0, rank_offset + max_thickness / 2.0)
            };

            node_positions.insert(idx, (x, y));
            cross_offset += if is_horizontal { node.height } else { node.width };
            cross_offset += NODE_SEP;
            prev_path = Some(node_path);
        }

        rank_offset += max_thickness + RANK_SEP;
    }

    // Phase 2: Undirected barycenter centering
    // Unlike directional forward/backward passes (which oscillate), this considers
    // ALL connections simultaneously with a blend factor for smooth convergence.
    // Critical for spreading nodes across zones: cross-zone connections pull
    // Platform nodes to align with their OryNetwork counterparts.
    for _pass in 0..30 {
        for layer in layers.iter() {
            let updates: Vec<(NodeIndex, f64)> = layer.iter().filter_map(|&idx| {
                let mut neighbors: Vec<NodeIndex> = Vec::new();
                neighbors.extend(graph.neighbors_directed(idx, PetDirection::Incoming));
                neighbors.extend(graph.neighbors_directed(idx, PetDirection::Outgoing));
                neighbors.sort();
                neighbors.dedup();

                if neighbors.is_empty() { return None; }

                let neighbor_positions: Vec<f64> = neighbors.iter()
                    .filter_map(|&n| node_positions.get(&n))
                    .map(|&(x, y)| if is_horizontal { y } else { x })
                    .collect();

                if neighbor_positions.is_empty() { return None; }

                let avg_cross = neighbor_positions.iter().sum::<f64>()
                    / neighbor_positions.len() as f64;

                let current_cross = {
                    let &(x, y) = node_positions.get(&idx)?;
                    if is_horizontal { y } else { x }
                };

                // Blend: 30% current + 70% target → fast convergence, smooth
                let new_cross = current_cross * 0.3 + avg_cross * 0.7;
                Some((idx, new_cross))
            }).collect();

            for (idx, new_cross) in updates {
                if let Some(pos) = node_positions.get_mut(&idx) {
                    if is_horizontal { pos.1 = new_cross; } else { pos.0 = new_cross; }
                }
            }
        }

        // Re-run overlap removal after each pass to maintain minimum spacing
        for layer in layers {
            remove_overlaps_in_layer(graph, layer, &mut node_positions, is_horizontal, membership, &empty_path);
        }
    }

    // Phase 4: Shift everything so minimum coordinate is 0
    let min_cross = node_positions.values()
        .map(|&(x, y)| if is_horizontal { y } else { x })
        .fold(f64::MAX, f64::min);
    if min_cross < 0.0 {
        for (x, y) in node_positions.values_mut() {
            if is_horizontal { *y -= min_cross; } else { *x -= min_cross; }
        }
    }

    // Build positioned nodes
    let mut positioned: Vec<PositionedNode> = graph
        .node_indices()
        .filter_map(|idx| {
            let (x, y) = node_positions.get(&idx)?;
            let node = &graph[idx];
            Some(PositionedNode {
                id: node.id.clone(),
                label: node.label.clone(),
                shape: node.shape,
                style: node.style.clone(),
                x: *x,
                y: *y,
                width: node.width,
                height: node.height,
            })
        })
        .collect();

    // For BT or RL directions, mirror the positions
    if matches!(direction, Direction::BottomToTop | Direction::RightToLeft) {
        let max_coord = if is_horizontal {
            positioned.iter().map(|n| n.x + n.width / 2.0).fold(0.0f64, f64::max)
        } else {
            positioned.iter().map(|n| n.y + n.height / 2.0).fold(0.0f64, f64::max)
        };

        for node in &mut positioned {
            if is_horizontal {
                node.x = max_coord - node.x;
            } else {
                node.y = max_coord - node.y;
            }
        }
    }

    positioned
}

/// Barycenter centering: move each node toward the average position of its neighbors
/// in the given direction. Returns updated positions preserving the ordering from dagre.
fn apply_barycenter_sized(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &[NodeIndex],
    positions: &HashMap<NodeIndex, (f64, f64)>,
    is_horizontal: bool,
    neighbor_dir: PetDirection,
) -> Vec<(NodeIndex, (f64, f64))> {
    let mut updates = Vec::new();

    for &idx in layer {
        let neighbors: Vec<NodeIndex> = graph.neighbors_directed(idx, neighbor_dir).collect();
        if neighbors.is_empty() {
            continue;
        }

        let avg_cross: f64 = neighbors
            .iter()
            .filter_map(|&n| positions.get(&n))
            .map(|&(x, y)| if is_horizontal { y } else { x })
            .sum::<f64>()
            / neighbors.len() as f64;

        if let Some(&(x, y)) = positions.get(&idx) {
            let new_pos = if is_horizontal {
                (x, avg_cross)
            } else {
                (avg_cross, y)
            };
            updates.push((idx, new_pos));
        }
    }

    // Sort by desired cross position to maintain relative order
    updates.sort_by(|a, b| {
        let ca = if is_horizontal { (a.1).1 } else { (a.1).0 };
        let cb = if is_horizontal { (b.1).1 } else { (b.1).0 };
        ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
    });

    updates
}

/// Ensure no nodes overlap within a layer after barycenter centering.
fn remove_overlaps_in_layer(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &[NodeIndex],
    positions: &mut HashMap<NodeIndex, (f64, f64)>,
    is_horizontal: bool,
    membership: &SubgraphMembership,
    empty_path: &Vec<String>,
) {
    if layer.len() < 2 {
        return;
    }

    // Sort by current cross position
    let mut sorted: Vec<NodeIndex> = layer.to_vec();
    sorted.sort_by(|a, b| {
        let ca = positions.get(a).map(|&(x, y)| if is_horizontal { y } else { x }).unwrap_or(0.0);
        let cb = positions.get(b).map(|&(x, y)| if is_horizontal { y } else { x }).unwrap_or(0.0);
        ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Push nodes apart if they overlap
    for i in 1..sorted.len() {
        let prev_idx = sorted[i - 1];
        let curr_idx = sorted[i];

        let prev_node = &graph[prev_idx];
        let curr_node = &graph[curr_idx];

        let prev_cross = positions.get(&prev_idx)
            .map(|&(x, y)| if is_horizontal { y } else { x }).unwrap_or(0.0);
        let prev_size = if is_horizontal { prev_node.height } else { prev_node.width };
        let curr_size = if is_horizontal { curr_node.height } else { curr_node.width };

        // Extra gap at subgraph boundaries
        let prev_path = membership.get(&prev_node.id).unwrap_or(empty_path);
        let curr_path = membership.get(&curr_node.id).unwrap_or(empty_path);
        let extra_gap = if prev_path != curr_path {
            let common = prev_path.iter().zip(curr_path.iter())
                .take_while(|(a, b)| a == b).count();
            let divergence = prev_path.len().max(curr_path.len()) - common;
            SUBGRAPH_GROUP_GAP * divergence as f64
        } else {
            0.0
        };

        let min_center_dist = prev_size / 2.0 + curr_size / 2.0 + NODE_SEP + extra_gap;
        let curr_cross = positions.get(&curr_idx)
            .map(|&(x, y)| if is_horizontal { y } else { x }).unwrap_or(0.0);

        if curr_cross - prev_cross < min_center_dist {
            let new_cross = prev_cross + min_center_dist;
            if let Some(pos) = positions.get_mut(&curr_idx) {
                if is_horizontal { pos.1 = new_cross; } else { pos.0 = new_cross; }
            }
        }
    }
}

/// Route edges as straight lines between node centers.
fn route_edges(
    _graph: &DiGraph<NodeData, EdgeData>,
    _index_map: &HashMap<String, NodeIndex>,
    positioned_nodes: &[PositionedNode],
    edges: &[EdgeDef],
) -> Vec<PositionedEdge> {
    let node_pos: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    edges
        .iter()
        .filter_map(|edge| {
            let from = node_pos.get(edge.from.as_str())?;
            let to = node_pos.get(edge.to.as_str())?;

            // Compute connection points at node boundaries
            let (from_x, from_y) = edge_connection_point(from, to.x, to.y);
            let (to_x, to_y) = edge_connection_point(to, from.x, from.y);

            let mid_x = (from_x + to_x) / 2.0;
            let mid_y = (from_y + to_y) / 2.0;

            Some(PositionedEdge {
                from_id: edge.from.clone(),
                to_id: edge.to.clone(),
                edge_type: edge.edge_type,
                label: edge.label.clone(),
                label_x: edge.label.as_ref().map(|_| mid_x),
                label_y: edge.label.as_ref().map(|_| mid_y),
                points: vec![(from_x, from_y), (to_x, to_y)],
            })
        })
        .collect()
}

/// Compute the point on a node's boundary closest to a target point.
fn edge_connection_point(node: &PositionedNode, target_x: f64, target_y: f64) -> (f64, f64) {
    let dx = target_x - node.x;
    let dy = target_y - node.y;
    let hw = node.width / 2.0;
    let hh = node.height / 2.0;

    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return (node.x, node.y + hh);
    }

    // For rectangles: find intersection with the bounding box
    let scale_x = if dx.abs() > 1e-6 { hw / dx.abs() } else { f64::MAX };
    let scale_y = if dy.abs() > 1e-6 { hh / dy.abs() } else { f64::MAX };
    let scale = scale_x.min(scale_y);

    (node.x + dx * scale, node.y + dy * scale)
}

/// Position subgraphs as bounding boxes around their contained nodes.
/// Recursively processes nested subgraphs from innermost to outermost.
fn position_subgraphs(
    subgraphs: &[SubgraphDef],
    positioned_nodes: &[PositionedNode],
    style_overrides: &[StyleOverride],
) -> Vec<PositionedSubgraph> {
    let node_pos: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let mut result = Vec::new();
    position_subgraphs_recursive(subgraphs, &node_pos, style_overrides, &mut result);
    result
}

fn position_subgraphs_recursive(
    subgraphs: &[SubgraphDef],
    node_pos: &HashMap<&str, &PositionedNode>,
    style_overrides: &[StyleOverride],
    result: &mut Vec<PositionedSubgraph>,
) {
    for sg in subgraphs {
        // Recurse into children first so their bounds are available
        position_subgraphs_recursive(&sg.subgraphs, node_pos, style_overrides, result);

        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut has_content = false;

        // Include direct member nodes
        for node in &sg.nodes {
            if let Some(pn) = node_pos.get(node.id.as_str()) {
                min_x = min_x.min(pn.x - pn.width / 2.0);
                min_y = min_y.min(pn.y - pn.height / 2.0);
                max_x = max_x.max(pn.x + pn.width / 2.0);
                max_y = max_y.max(pn.y + pn.height / 2.0);
                has_content = true;
            }
        }

        // Include child subgraph bounds
        for child_sg in &sg.subgraphs {
            if let Some(child_pos) = result.iter().find(|ps| ps.id == child_sg.id) {
                min_x = min_x.min(child_pos.x);
                min_y = min_y.min(child_pos.y);
                max_x = max_x.max(child_pos.x + child_pos.width);
                max_y = max_y.max(child_pos.y + child_pos.height);
                has_content = true;
            }
        }

        // Include edge-referenced nodes within this subgraph
        for edge in &sg.edges {
            for id in [&edge.from, &edge.to] {
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
            // Account for multi-line titles (e.g. labels with <br/>)
            let title_height = if let Some(ref label) = sg.label {
                let normalized = crate::render::html_util::normalize_br(label);
                let line_count = normalized.split('\n').count();
                SUBGRAPH_TITLE_HEIGHT + (line_count.saturating_sub(1) as f64) * 16.0
            } else {
                SUBGRAPH_TITLE_HEIGHT
            };

            // Resolve style overrides for this subgraph
            let mut style = StyleProperties::default();
            for so in style_overrides {
                if so.node_id == sg.id {
                    style = style.merge(&so.properties);
                }
            }

            // Use subgraph ID as label when no explicit label is given
            // (matches mermaid.js behavior for `subgraph ID` without ["label"])
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

/// Shift all positioned elements so everything has positive coordinates,
/// then compute the total bounding box.
fn normalize_and_compute_bounds(
    nodes: &mut [PositionedNode],
    edges: &mut [PositionedEdge],
    subgraphs: &mut [PositionedSubgraph],
) -> (f64, f64) {
    // Find minimum coordinates across all elements
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;

    for node in nodes.iter() {
        min_x = min_x.min(node.x - node.width / 2.0);
        min_y = min_y.min(node.y - node.height / 2.0);
    }
    for sg in subgraphs.iter() {
        min_x = min_x.min(sg.x);
        min_y = min_y.min(sg.y);
    }

    // Shift everything so min coords are at 0
    if min_x < 0.0 || min_y < 0.0 {
        let shift_x = if min_x < 0.0 { -min_x } else { 0.0 };
        let shift_y = if min_y < 0.0 { -min_y } else { 0.0 };

        for node in nodes.iter_mut() {
            node.x += shift_x;
            node.y += shift_y;
        }
        for edge in edges.iter_mut() {
            for point in &mut edge.points {
                point.0 += shift_x;
                point.1 += shift_y;
            }
            if let Some(ref mut lx) = edge.label_x { *lx += shift_x; }
            if let Some(ref mut ly) = edge.label_y { *ly += shift_y; }
        }
        for sg in subgraphs.iter_mut() {
            sg.x += shift_x;
            sg.y += shift_y;
        }
    }

    // Now compute max bounds
    let mut max_x = 0.0f64;
    let mut max_y = 0.0f64;

    for node in nodes.iter() {
        max_x = max_x.max(node.x + node.width / 2.0);
        max_y = max_y.max(node.y + node.height / 2.0);
    }
    for sg in subgraphs.iter() {
        max_x = max_x.max(sg.x + sg.width);
        max_y = max_y.max(sg.y + sg.height);
    }

    // Add margin
    (max_x + 40.0, max_y + 40.0)
}

#[cfg(test)]
mod tests {
    use super::*;
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
                NodeDef { id: "A".into(), label: Some("Node A".into()), shape: NodeShape::Rectangle, class_shorthand: None },
                NodeDef { id: "B".into(), label: Some("Node B".into()), shape: NodeShape::Rectangle, class_shorthand: None },
                NodeDef { id: "C".into(), label: Some("Node C".into()), shape: NodeShape::Rectangle, class_shorthand: None },
            ],
            edges: vec![
                EdgeDef { from: "A".into(), to: "B".into(), edge_type: EdgeType::SolidArrow, label: None },
                EdgeDef { from: "A".into(), to: "C".into(), edge_type: EdgeType::SolidArrow, label: None },
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
                    a.id, b.id, a.x, a.y, b.x, b.y
                );
            }
        }
    }
}
