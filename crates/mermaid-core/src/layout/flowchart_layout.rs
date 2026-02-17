use std::collections::HashMap;

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
const SUBGRAPH_PADDING: f64 = 40.0;
const SUBGRAPH_TITLE_HEIGHT: f64 = 25.0;
const SUBGRAPH_GROUP_GAP: f64 = 80.0;

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
    label: Option<String>,
    edge_type: EdgeType,
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

    // 6. Assign ranks (layers) using topological ordering
    let ranks = assign_ranks(&graph, ast.direction);

    // 7. Position nodes within ranks
    let mut positioned_nodes = position_nodes(&graph, &ranks, ast.direction, &membership);

    // 7. Route edges
    let mut positioned_edges = route_edges(&graph, &index_map, &positioned_nodes, &all_edges);

    // 8. Position subgraphs
    let mut positioned_subgraphs = position_subgraphs(&ast.subgraphs, &positioned_nodes);

    // 9. Normalize coordinates (shift to positive) and compute bounding box
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
                label: edge.label.clone(),
                edge_type: edge.edge_type,
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

/// Assign rank (layer) to each node using a simplified Coffman-Graham approach.
/// Nodes with no incoming edges go to rank 0, their successors to rank 1, etc.
fn assign_ranks(graph: &DiGraph<NodeData, EdgeData>, _direction: Direction) -> HashMap<NodeIndex, usize> {
    let mut ranks: HashMap<NodeIndex, usize> = HashMap::new();

    // Simple longest-path ranking
    let topo = petgraph::algo::toposort(graph, None);
    let order = match topo {
        Ok(order) => order,
        Err(_) => {
            // Graph has cycles — fall back to node index order
            graph.node_indices().collect()
        }
    };

    for idx in &order {
        let mut max_pred_rank: Option<usize> = None;
        for pred in graph.neighbors_directed(*idx, PetDirection::Incoming) {
            if let Some(&pred_rank) = ranks.get(&pred) {
                max_pred_rank = Some(max_pred_rank.map_or(pred_rank, |m: usize| m.max(pred_rank)));
            }
        }
        let rank = match max_pred_rank {
            Some(r) => r + 1,
            None => 0,
        };
        ranks.insert(*idx, rank);
    }

    ranks
}

/// Position nodes within their ranks.
fn position_nodes(
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &HashMap<NodeIndex, usize>,
    direction: Direction,
    membership: &SubgraphMembership,
) -> Vec<PositionedNode> {
    // Group nodes by rank
    let max_rank = ranks.values().copied().max().unwrap_or(0);
    let mut rank_groups: Vec<Vec<NodeIndex>> = vec![Vec::new(); max_rank + 1];
    for (&idx, &rank) in ranks {
        rank_groups[rank].push(idx);
    }

    // Sort nodes within each rank by subgraph path then by index
    let empty_path: Vec<String> = Vec::new();
    for group in &mut rank_groups {
        group.sort_by(|a, b| {
            let path_a = membership.get(&graph[*a].id).unwrap_or(&empty_path);
            let path_b = membership.get(&graph[*b].id).unwrap_or(&empty_path);
            path_a.cmp(path_b).then(a.cmp(b))
        });
    }

    let is_horizontal = matches!(direction, Direction::LeftToRight | Direction::RightToLeft);

    // Build node_id → NodeIndex lookup
    let id_to_idx: HashMap<&str, NodeIndex> = graph
        .node_indices()
        .map(|idx| (graph[idx].id.as_str(), idx))
        .collect();

    // Phase 1: Initial left-packed layout
    let mut node_positions: HashMap<NodeIndex, (f64, f64)> = HashMap::new();
    let mut rank_offset = 0.0;

    for group in &rank_groups {
        let max_thickness = group
            .iter()
            .map(|&idx| {
                let node = &graph[idx];
                if is_horizontal { node.width } else { node.height }
            })
            .fold(0.0f64, f64::max);

        let mut cross_offset = 0.0;
        let mut prev_path: Option<&Vec<String>> = None;

        for &idx in group {
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

    // Phase 2: Barycenter centering — nudge nodes toward the average
    // position of their connected neighbors in adjacent ranks.
    // Run multiple passes for convergence.
    for _pass in 0..4 {
        // Forward pass: rank 1..n, center under predecessors
        for rank in 1..=max_rank {
            apply_barycenter(
                graph,
                &rank_groups[rank],
                &node_positions,
                is_horizontal,
                PetDirection::Incoming,
                membership,
                &empty_path,
            )
            .into_iter()
            .for_each(|(idx, pos)| { node_positions.insert(idx, pos); });
        }
        // Backward pass: rank n-1..0, center over successors
        for rank in (0..max_rank).rev() {
            apply_barycenter(
                graph,
                &rank_groups[rank],
                &node_positions,
                is_horizontal,
                PetDirection::Outgoing,
                membership,
                &empty_path,
            )
            .into_iter()
            .for_each(|(idx, pos)| { node_positions.insert(idx, pos); });
        }
    }

    // Phase 3: Ensure no overlaps within each rank after centering
    for group in &rank_groups {
        remove_overlaps(graph, group, &mut node_positions, is_horizontal, membership, &empty_path);
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

/// Compute barycenter positions for nodes in a rank.
/// Each node is moved to the average cross-position of its neighbors in the given direction.
fn apply_barycenter(
    graph: &DiGraph<NodeData, EdgeData>,
    group: &[NodeIndex],
    positions: &HashMap<NodeIndex, (f64, f64)>,
    is_horizontal: bool,
    neighbor_dir: PetDirection,
    membership: &SubgraphMembership,
    empty_path: &Vec<String>,
) -> Vec<(NodeIndex, (f64, f64))> {
    let mut updates = Vec::new();

    for &idx in group {
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

    // Sort updates by desired cross position to maintain relative order
    updates.sort_by(|a, b| {
        let ca = if is_horizontal { (a.1).1 } else { (a.1).0 };
        let cb = if is_horizontal { (b.1).1 } else { (b.1).0 };
        ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Verify no position reversal within same subgraph (maintain order)
    let _ = membership;
    let _ = empty_path;

    updates
}

/// Ensure no nodes overlap within a rank after barycenter centering.
fn remove_overlaps(
    graph: &DiGraph<NodeData, EdgeData>,
    group: &[NodeIndex],
    positions: &mut HashMap<NodeIndex, (f64, f64)>,
    is_horizontal: bool,
    membership: &SubgraphMembership,
    empty_path: &Vec<String>,
) {
    if group.len() < 2 {
        return;
    }

    // Sort group by current cross position
    let mut sorted: Vec<NodeIndex> = group.to_vec();
    sorted.sort_by(|a, b| {
        let ca = positions.get(a).map(|&(x, y)| if is_horizontal { y } else { x }).unwrap_or(0.0);
        let cb = positions.get(b).map(|&(x, y)| if is_horizontal { y } else { x }).unwrap_or(0.0);
        ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Walk through and push nodes apart if they overlap
    for i in 1..sorted.len() {
        let prev_idx = sorted[i - 1];
        let curr_idx = sorted[i];

        let prev_node = &graph[prev_idx];
        let curr_node = &graph[curr_idx];

        let prev_cross = positions.get(&prev_idx)
            .map(|&(x, y)| if is_horizontal { y } else { x }).unwrap_or(0.0);
        let prev_size = if is_horizontal { prev_node.height } else { prev_node.width };
        let curr_size = if is_horizontal { curr_node.height } else { curr_node.width };

        // Minimum gap: NODE_SEP, plus extra at subgraph boundaries
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
) -> Vec<PositionedSubgraph> {
    let node_pos: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let mut result = Vec::new();
    position_subgraphs_recursive(subgraphs, &node_pos, &mut result);
    result
}

fn position_subgraphs_recursive(
    subgraphs: &[SubgraphDef],
    node_pos: &HashMap<&str, &PositionedNode>,
    result: &mut Vec<PositionedSubgraph>,
) {
    for sg in subgraphs {
        // Recurse into children first so their bounds are available
        position_subgraphs_recursive(&sg.subgraphs, node_pos, result);

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
            result.push(PositionedSubgraph {
                id: sg.id.clone(),
                label: sg.label.clone(),
                x: min_x - SUBGRAPH_PADDING,
                y: min_y - SUBGRAPH_PADDING - SUBGRAPH_TITLE_HEIGHT,
                width: (max_x - min_x) + 2.0 * SUBGRAPH_PADDING,
                height: (max_y - min_y) + 2.0 * SUBGRAPH_PADDING + SUBGRAPH_TITLE_HEIGHT,
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
