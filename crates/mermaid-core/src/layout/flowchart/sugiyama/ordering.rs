use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};

use crate::ast::flowchart::{FlowchartAst, SubgraphDef};
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;

use super::dummy_nodes::DummyChain;

// ---------------------------------------------------------------------------
// Crossing-count infrastructure (Fenwick / BIT)
// ---------------------------------------------------------------------------

/// Fenwick tree (Binary Indexed Tree) for O(log n) prefix-sum queries.
struct FenwickTree {
    tree: Vec<usize>,
}

impl FenwickTree {
    fn new(size: usize) -> Self {
        FenwickTree {
            tree: vec![0; size + 1],
        }
    }

    /// Increment position `i` (0-indexed) by 1.
    fn update(&mut self, mut i: usize) {
        i += 1; // 1-indexed internally
        while i < self.tree.len() {
            self.tree[i] += 1;
            i += i & i.wrapping_neg();
        }
    }

    /// Sum of elements in positions [0, i] (0-indexed).
    fn prefix_sum(&self, mut i: usize) -> usize {
        i += 1; // 1-indexed internally
        let mut sum = 0;
        while i > 0 {
            sum += self.tree[i];
            i -= i & i.wrapping_neg();
        }
        sum
    }
}

/// Count edge crossings between two adjacent layers using an inversion-count
/// algorithm with a Fenwick tree.  O(|E| log |V_south|).
fn count_bilayer_crossings(
    graph: &DiGraph<NodeData, EdgeData>,
    north_layer: &[NodeIndex],
    south_layer: &[NodeIndex],
) -> usize {
    let south_pos: HashMap<NodeIndex, usize> = south_layer
        .iter()
        .enumerate()
        .map(|(pos, &node)| (node, pos))
        .collect();

    // Collect south-layer positions for each edge, ordered by north position
    // (ties broken by south position, ascending).
    let mut south_endpoints: Vec<usize> = Vec::new();
    for &north_node in north_layer {
        let mut targets: Vec<usize> = graph
            .neighbors_directed(north_node, petgraph::Direction::Outgoing)
            .filter_map(|n| south_pos.get(&n).copied())
            .collect();
        targets.sort_unstable();
        south_endpoints.extend(targets);
    }

    // Count inversions via Fenwick tree
    let south_size = south_layer.len();
    if south_size == 0 {
        return 0;
    }
    let mut tree = FenwickTree::new(south_size);
    let mut crossings: usize = 0;
    let mut inserted: usize = 0;

    for &pos in &south_endpoints {
        // Elements already inserted with position > pos
        crossings += inserted - tree.prefix_sum(pos);
        tree.update(pos);
        inserted += 1;
    }

    crossings
}

/// Sum of squared position-displacements for all edges between adjacent layers.
/// Used as a tiebreaker when crossing counts are equal: lower displacement
/// means edges are straighter and the layout looks better.
fn edge_displacement_score(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
) -> usize {
    let mut score: usize = 0;
    for i in 0..layers.len().saturating_sub(1) {
        let south_pos: HashMap<NodeIndex, usize> = layers[i + 1]
            .iter()
            .enumerate()
            .map(|(pos, &node)| (node, pos))
            .collect();

        for (north_pos, &north_node) in layers[i].iter().enumerate() {
            for neighbor in graph.neighbors_directed(north_node, petgraph::Direction::Outgoing) {
                if let Some(&sp) = south_pos.get(&neighbor) {
                    let diff = if north_pos > sp {
                        north_pos - sp
                    } else {
                        sp - north_pos
                    };
                    score += diff * diff;
                }
            }
        }
    }
    score
}

/// Total crossings across all adjacent layer pairs.
fn count_total_crossings(graph: &DiGraph<NodeData, EdgeData>, layers: &[Vec<NodeIndex>]) -> usize {
    (0..layers.len().saturating_sub(1))
        .map(|i| count_bilayer_crossings(graph, &layers[i], &layers[i + 1]))
        .sum()
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Maximum consecutive non-improving iterations before early stop.
const MAX_NO_IMPROVEMENT: usize = 4;

/// Run crossing minimisation from multiple initial orderings and return
/// whichever converges to the fewest crossings.
pub fn minimize_crossings_best_of(
    graph: &DiGraph<NodeData, EdgeData>,
    candidates: &[Vec<Vec<NodeIndex>>],
    membership: &SubgraphMembership,
    num_iterations: usize,
) -> Vec<Vec<NodeIndex>> {
    let mut best: Option<Vec<Vec<NodeIndex>>> = None;
    let mut best_cc = usize::MAX;
    let mut best_disp = usize::MAX;

    for candidate in candidates {
        let mut layers = candidate.clone();
        minimize_crossings(graph, &mut layers, membership, num_iterations);
        let cc = count_total_crossings(graph, &layers);
        let disp = edge_displacement_score(graph, &layers);
        // Prefer fewer crossings; break ties with better edge straightness.
        if cc < best_cc || (cc == best_cc && disp < best_disp) {
            best_cc = cc;
            best_disp = disp;
            best = Some(layers);
        }
    }

    best.unwrap_or_default()
}

/// Barycenter heuristic with alternating up/down sweeps.
/// Enforces subgraph contiguity: nodes belonging to the same subgraph
/// remain contiguous within each rank.
///
/// Tracks the best crossing count seen and reverts to the best ordering
/// if later iterations fail to improve.  Stops early after
/// `MAX_NO_IMPROVEMENT` consecutive non-improving sweeps.
fn minimize_crossings(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &mut [Vec<NodeIndex>],
    membership: &SubgraphMembership,
    num_iterations: usize,
) {
    let empty_path: Vec<String> = Vec::new();

    // Snapshot the best ordering found so far.
    let mut best_layers: Vec<Vec<NodeIndex>> = layers.to_vec();
    let mut best_cc = count_total_crossings(graph, layers);
    let mut no_improve = 0usize;

    for iteration in 0..num_iterations {
        // Dagre-style bias: alternates every 2 iterations.
        let bias_right = iteration % 4 >= 2;

        if iteration % 2 == 0 {
            // Down sweep: process layers top to bottom
            for i in 1..layers.len() {
                let prev_positions = build_position_map(&layers[i - 1]);
                sort_layer_by_barycenter(
                    graph,
                    &mut layers[i],
                    &prev_positions,
                    petgraph::Direction::Incoming,
                    membership,
                    &empty_path,
                    bias_right,
                );
            }
        } else {
            // Up sweep: process layers bottom to top
            for i in (0..layers.len().saturating_sub(1)).rev() {
                let next_positions = build_position_map(&layers[i + 1]);
                sort_layer_by_barycenter(
                    graph,
                    &mut layers[i],
                    &next_positions,
                    petgraph::Direction::Outgoing,
                    membership,
                    &empty_path,
                    bias_right,
                );
            }
        }

        // Evaluate quality and track best.
        let cc = count_total_crossings(graph, layers);
        if cc < best_cc {
            best_cc = cc;
            best_layers = layers.to_vec();
            no_improve = 0;
        } else {
            no_improve += 1;
        }

        if best_cc == 0 || no_improve >= MAX_NO_IMPROVEMENT {
            break;
        }
    }

    // Restore the best ordering found.
    for (dst, src) in layers.iter_mut().zip(best_layers.into_iter()) {
        *dst = src;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a map from NodeIndex to its position within the layer.
fn build_position_map(layer: &[NodeIndex]) -> HashMap<NodeIndex, usize> {
    layer
        .iter()
        .enumerate()
        .map(|(pos, &node)| (node, pos))
        .collect()
}

/// Sort a layer using barycenter heuristic while maintaining subgraph contiguity.
///
/// `bias_right` controls tie-breaking direction to prevent symmetric oscillation:
/// when `true`, ties are broken in favour of higher original positions (rightward);
/// when `false`, in favour of lower original positions (leftward).
fn sort_layer_by_barycenter(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &mut Vec<NodeIndex>,
    adjacent_positions: &HashMap<NodeIndex, usize>,
    direction: petgraph::Direction,
    membership: &SubgraphMembership,
    empty_path: &Vec<String>,
    bias_right: bool,
) {
    // Capture original positions before sorting (for stable tie-breaking).
    let original_positions: HashMap<NodeIndex, usize> =
        layer.iter().enumerate().map(|(i, &n)| (n, i)).collect();

    // Compute barycenter for each node
    let barycenters: HashMap<NodeIndex, f64> = layer
        .iter()
        .enumerate()
        .map(|(idx, &node)| {
            let neighbors: Vec<usize> = graph
                .neighbors_directed(node, direction)
                .filter_map(|n| adjacent_positions.get(&n).copied())
                .collect();

            let bc = if neighbors.is_empty() {
                // Keep current relative position as fallback
                idx as f64
            } else {
                neighbors.iter().sum::<usize>() as f64 / neighbors.len() as f64
            };
            (node, bc)
        })
        .collect();

    // Group nodes by subgraph membership path, preserving order.
    // Only nodes that share a NON-EMPTY subgraph path are grouped together
    // (contiguity constraint).  Dummy nodes and nodes with no subgraph
    // membership are each their own singleton group so they can move freely
    // to their optimal barycenter position — matching dagre's behaviour.
    let mut groups: Vec<(Vec<String>, Vec<NodeIndex>)> = Vec::new();
    for &node in layer.iter() {
        let is_dummy = graph[node].id.starts_with("__dummy_");
        let path = if is_dummy {
            vec![graph[node].id.clone()]
        } else {
            membership
                .get(&graph[node].id)
                .unwrap_or(empty_path)
                .clone()
        };

        let is_singleton = is_dummy || path.is_empty();

        if !is_singleton {
            // Subgraph member: merge into existing group for contiguity
            if let Some(group) = groups.iter_mut().find(|(p, _)| *p == path) {
                group.1.push(node);
            } else {
                groups.push((path, vec![node]));
            }
        } else {
            // Free node (dummy or no subgraph): independent singleton
            groups.push((vec![graph[node].id.clone()], vec![node]));
        }
    }

    // Sort nodes within each group by barycenter, with biased tie-breaking.
    for (_, members) in &mut groups {
        members.sort_by(|a, b| {
            let ba = barycenters.get(a).copied().unwrap_or(0.0);
            let bb = barycenters.get(b).copied().unwrap_or(0.0);
            let cmp = ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal);
            if cmp != std::cmp::Ordering::Equal {
                return cmp;
            }
            // Tie-break using original positions with directional bias
            let pa = original_positions.get(a).copied().unwrap_or(0);
            let pb = original_positions.get(b).copied().unwrap_or(0);
            if bias_right {
                pb.cmp(&pa)
            } else {
                pa.cmp(&pb)
            }
        });
    }

    // Sort groups by average barycenter of their members, with biased tie-breaking.
    groups.sort_by(|(_, a_members), (_, b_members)| {
        let avg_a = a_members
            .iter()
            .filter_map(|n| barycenters.get(n))
            .sum::<f64>()
            / a_members.len().max(1) as f64;
        let avg_b = b_members
            .iter()
            .filter_map(|n| barycenters.get(n))
            .sum::<f64>()
            / b_members.len().max(1) as f64;
        let cmp = avg_a
            .partial_cmp(&avg_b)
            .unwrap_or(std::cmp::Ordering::Equal);
        if cmp != std::cmp::Ordering::Equal {
            return cmp;
        }
        // Tie-break: average original position
        let orig_a = a_members
            .iter()
            .filter_map(|n| original_positions.get(n))
            .sum::<usize>() as f64
            / a_members.len().max(1) as f64;
        let orig_b = b_members
            .iter()
            .filter_map(|n| original_positions.get(n))
            .sum::<usize>() as f64
            / b_members.len().max(1) as f64;
        if bias_right {
            orig_b
                .partial_cmp(&orig_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            orig_a
                .partial_cmp(&orig_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // Flatten back into layer
    *layer = groups
        .into_iter()
        .flat_map(|(_, members)| members)
        .collect();
}

// ---------------------------------------------------------------------------
// Subgraph-local ordering refinement
// ---------------------------------------------------------------------------

/// Information about a single subgraph for local refinement.
#[allow(dead_code)]
struct SubgraphInfo {
    /// The subgraph's id.
    id: String,
    /// The full membership path (e.g., ["Outer", "Inner"]).
    path: Vec<String>,
    /// All nodes in this subgraph including nested descendants.
    all_descendants: HashSet<NodeIndex>,
    /// IDs of immediate child subgraphs.
    child_ids: Vec<String>,
}

/// A contiguous block of subgraph members within a global layer.
struct LayerBlock {
    layer_idx: usize,
    start: usize,
    end: usize, // exclusive
}

/// Post-pass local refinement of within-subgraph node ordering.
///
/// After global crossing minimization, internal node arrangement within
/// subgraphs may be suboptimal because external edge pulls distort local
/// ordering. This function processes each subgraph (innermost first),
/// runs localized barycenter sweeps on its contiguous block in each layer,
/// and accepts the result only if it reduces crossings.
pub fn refine_subgraph_ordering(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &mut [Vec<NodeIndex>],
    membership: &SubgraphMembership,
    ast: &FlowchartAst,
    dummy_chains: &[DummyChain],
) {
    if ast.subgraphs.is_empty() {
        return;
    }

    // Build effective membership for all nodes (NodeIndex -> path).
    let effective_mem = build_node_membership(graph, membership, dummy_chains);

    // Collect subgraphs in post-order (innermost first).
    let subgraph_infos = collect_subgraphs_postorder(&ast.subgraphs, &[], &effective_mem);

    for sg_info in &subgraph_infos {
        if sg_info.all_descendants.len() < 2 {
            continue;
        }

        // Build child membership map: descendant node -> immediate child subgraph ID.
        let child_members: HashMap<NodeIndex, String> =
            build_child_member_map(sg_info, &subgraph_infos);

        // Find contiguous blocks in each layer.
        let blocks = match find_contiguous_blocks(layers, &sg_info.all_descendants) {
            Some(b) => b,
            None => continue, // non-contiguous or <2 occupied layers
        };

        // Collect affected layer indices for crossing count.
        let affected_layers: Vec<usize> = blocks.iter().map(|b| b.layer_idx).collect();

        // Count initial crossings for affected layer pairs.
        let initial_cc = count_affected_crossings(graph, layers, &affected_layers);

        // Save current block orderings for rollback.
        let saved: Vec<Vec<NodeIndex>> = blocks
            .iter()
            .map(|b| layers[b.layer_idx][b.start..b.end].to_vec())
            .collect();

        // Run barycenter sweeps.
        let num_sweeps = 6;
        let mut best_cc = initial_cc;
        let mut best_blocks: Vec<Vec<NodeIndex>> = saved.clone();

        for sweep in 0..num_sweeps {
            let bias_right = sweep % 4 >= 2;

            if sweep % 2 == 0 {
                // Down sweep.
                for b in &blocks {
                    if b.layer_idx == 0 {
                        continue;
                    }
                    let adj_positions = build_position_map(&layers[b.layer_idx - 1]);
                    sort_sublayer_by_barycenter(
                        graph,
                        &mut layers[b.layer_idx],
                        b.start,
                        b.end,
                        &adj_positions,
                        petgraph::Direction::Incoming,
                        &child_members,
                        bias_right,
                    );
                }
            } else {
                // Up sweep.
                for b in blocks.iter().rev() {
                    if b.layer_idx + 1 >= layers.len() {
                        continue;
                    }
                    let adj_positions = build_position_map(&layers[b.layer_idx + 1]);
                    sort_sublayer_by_barycenter(
                        graph,
                        &mut layers[b.layer_idx],
                        b.start,
                        b.end,
                        &adj_positions,
                        petgraph::Direction::Outgoing,
                        &child_members,
                        bias_right,
                    );
                }
            }

            let cc = count_affected_crossings(graph, layers, &affected_layers);
            if cc < best_cc {
                best_cc = cc;
                best_blocks = blocks
                    .iter()
                    .map(|b| layers[b.layer_idx][b.start..b.end].to_vec())
                    .collect();
            }
        }

        // Apply best result (rollback if no improvement).
        let to_apply = if best_cc < initial_cc {
            best_blocks
        } else {
            saved
        };
        for (bi, block_nodes) in to_apply.into_iter().enumerate() {
            let b = &blocks[bi];
            layers[b.layer_idx][b.start..b.end].copy_from_slice(&block_nodes);
        }
    }
}

/// Build effective membership (NodeIndex -> path) for all nodes including dummies.
///
/// Dummy nodes get the longest common prefix of their chain's source and target
/// membership paths, so dummies on intra-subgraph edges are treated as members.
fn build_node_membership(
    graph: &DiGraph<NodeData, EdgeData>,
    membership: &SubgraphMembership,
    dummy_chains: &[DummyChain],
) -> HashMap<NodeIndex, Vec<String>> {
    let empty = Vec::new();

    // Compute dummy membership via longest common prefix of chain endpoints.
    let mut dummy_mem: HashMap<NodeIndex, Vec<String>> = HashMap::new();
    for chain in dummy_chains {
        let src_path = membership
            .get(&graph[chain.original_source].id)
            .unwrap_or(&empty);
        let tgt_path = membership
            .get(&graph[chain.original_target].id)
            .unwrap_or(&empty);
        let lcp: Vec<String> = src_path
            .iter()
            .zip(tgt_path.iter())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a.clone())
            .collect();
        for &dummy in &chain.dummy_nodes {
            dummy_mem.insert(dummy, lcp.clone());
        }
    }

    // Build unified map.
    let mut result = HashMap::new();
    for node_idx in graph.node_indices() {
        if let Some(path) = dummy_mem.get(&node_idx) {
            result.insert(node_idx, path.clone());
        } else {
            let id = &graph[node_idx].id;
            result.insert(node_idx, membership.get(id).unwrap_or(&empty).clone());
        }
    }
    result
}

/// Traverse subgraph tree depth-first, returning SubgraphInfos in post-order
/// (innermost subgraphs first).
fn collect_subgraphs_postorder(
    subgraphs: &[SubgraphDef],
    parent_path: &[String],
    effective_mem: &HashMap<NodeIndex, Vec<String>>,
) -> Vec<SubgraphInfo> {
    let mut result = Vec::new();
    for sg in subgraphs {
        let mut path = parent_path.to_vec();
        path.push(sg.id.clone());

        // Recurse children first (post-order).
        let children = collect_subgraphs_postorder(&sg.subgraphs, &path, effective_mem);
        let child_ids: Vec<String> = sg.subgraphs.iter().map(|c| c.id.clone()).collect();

        // Collect all descendants from children.
        let mut all_descendants: HashSet<NodeIndex> = HashSet::new();
        for child in &children {
            all_descendants.extend(&child.all_descendants);
        }

        // Add direct members: nodes whose effective path is exactly this path.
        for (&node_idx, node_path) in effective_mem {
            if *node_path == path {
                all_descendants.insert(node_idx);
            }
        }

        result.extend(children);
        result.push(SubgraphInfo {
            id: sg.id.clone(),
            path,
            all_descendants,
            child_ids,
        });
    }
    result
}

/// Build a map from node to its immediate child subgraph ID (for atomic block grouping).
fn build_child_member_map(
    sg_info: &SubgraphInfo,
    all_infos: &[SubgraphInfo],
) -> HashMap<NodeIndex, String> {
    let mut map = HashMap::new();
    for child_id in &sg_info.child_ids {
        let mut child_path = sg_info.path.clone();
        child_path.push(child_id.clone());
        if let Some(child_info) = all_infos.iter().find(|si| si.path == child_path) {
            for &node in &child_info.all_descendants {
                map.insert(node, child_id.clone());
            }
        }
    }
    map
}

/// Find contiguous blocks of subgraph members in each layer.
/// Returns None if members are non-contiguous in any layer or occupy <2 layers.
fn find_contiguous_blocks(
    layers: &[Vec<NodeIndex>],
    members: &HashSet<NodeIndex>,
) -> Option<Vec<LayerBlock>> {
    let mut blocks = Vec::new();
    for (layer_idx, layer) in layers.iter().enumerate() {
        let member_positions: Vec<usize> = layer
            .iter()
            .enumerate()
            .filter(|(_, n)| members.contains(n))
            .map(|(pos, _)| pos)
            .collect();

        if member_positions.is_empty() {
            continue;
        }

        let start = *member_positions.first().unwrap();
        let end = *member_positions.last().unwrap() + 1;

        // Check contiguity.
        if member_positions.len() != end - start {
            return None;
        }

        blocks.push(LayerBlock {
            layer_idx,
            start,
            end,
        });
    }

    if blocks.len() < 2 {
        return None;
    }

    Some(blocks)
}

/// Count crossings for layer pairs adjacent to any affected layer.
fn count_affected_crossings(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    affected_layer_indices: &[usize],
) -> usize {
    let mut affected_pairs: HashSet<usize> = HashSet::new();
    for &li in affected_layer_indices {
        if li > 0 {
            affected_pairs.insert(li - 1);
        }
        if li + 1 < layers.len() {
            affected_pairs.insert(li);
        }
    }
    affected_pairs
        .iter()
        .map(|&i| count_bilayer_crossings(graph, &layers[i], &layers[i + 1]))
        .sum()
}

/// Sort a sub-layer block by barycenter, keeping child subgraph members as
/// atomic blocks (their internal order is frozen from earlier refinement).
fn sort_sublayer_by_barycenter(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &mut [NodeIndex],
    block_start: usize,
    block_end: usize,
    adjacent_positions: &HashMap<NodeIndex, usize>,
    direction: petgraph::Direction,
    child_members: &HashMap<NodeIndex, String>,
    bias_right: bool,
) {
    let block_len = block_end - block_start;
    if block_len <= 1 {
        return;
    }

    let block: Vec<NodeIndex> = layer[block_start..block_end].to_vec();

    // Capture original positions for tie-breaking (relative to block start).
    let original_positions: HashMap<NodeIndex, usize> =
        block.iter().enumerate().map(|(i, &n)| (n, i)).collect();

    // Compute barycenters using full global adjacent layer positions.
    let barycenters: HashMap<NodeIndex, f64> = block
        .iter()
        .enumerate()
        .map(|(idx, &node)| {
            let neighbors: Vec<usize> = graph
                .neighbors_directed(node, direction)
                .filter_map(|n| adjacent_positions.get(&n).copied())
                .collect();
            let bc = if neighbors.is_empty() {
                // Use global position as fallback to maintain relative order.
                (block_start + idx) as f64
            } else {
                neighbors.iter().sum::<usize>() as f64 / neighbors.len() as f64
            };
            (node, bc)
        })
        .collect();

    // Group by child subgraph (atomic blocks) or singleton.
    let mut groups: Vec<(Option<String>, Vec<NodeIndex>)> = Vec::new();
    for &node in &block {
        if let Some(child_id) = child_members.get(&node) {
            if let Some(group) = groups
                .iter_mut()
                .find(|(id, _)| id.as_deref() == Some(child_id.as_str()))
            {
                group.1.push(node);
            } else {
                groups.push((Some(child_id.clone()), vec![node]));
            }
        } else {
            // Direct member or dummy — singleton, can move freely.
            groups.push((None, vec![node]));
        }
    }

    // Do NOT sort within child groups — their internal order is frozen.
    // Sort groups by average barycenter, with biased tie-breaking.
    groups.sort_by(|(_, a_members), (_, b_members)| {
        let avg_a = a_members
            .iter()
            .filter_map(|n| barycenters.get(n))
            .sum::<f64>()
            / a_members.len().max(1) as f64;
        let avg_b = b_members
            .iter()
            .filter_map(|n| barycenters.get(n))
            .sum::<f64>()
            / b_members.len().max(1) as f64;
        let cmp = avg_a
            .partial_cmp(&avg_b)
            .unwrap_or(std::cmp::Ordering::Equal);
        if cmp != std::cmp::Ordering::Equal {
            return cmp;
        }
        let orig_a = a_members
            .iter()
            .filter_map(|n| original_positions.get(n))
            .sum::<usize>() as f64
            / a_members.len().max(1) as f64;
        let orig_b = b_members
            .iter()
            .filter_map(|n| original_positions.get(n))
            .sum::<usize>() as f64
            / b_members.len().max(1) as f64;
        if bias_right {
            orig_b
                .partial_cmp(&orig_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            orig_a
                .partial_cmp(&orig_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    // Flatten and write back.
    let sorted: Vec<NodeIndex> = groups
        .into_iter()
        .flat_map(|(_, members)| members)
        .collect();
    layer[block_start..block_end].copy_from_slice(&sorted);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use petgraph::graph::DiGraph;

    fn make_node(id: &str) -> NodeData {
        NodeData {
            id: id.to_string(),
            label: String::new(),
            shape: Default::default(),
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        }
    }

    fn make_edge() -> EdgeData {
        EdgeData {
            edge_type: Default::default(),
            label: None,
            label_width: 0.0,
            label_height: 0.0,
        }
    }

    #[test]
    fn test_count_bilayer_crossings_none() {
        // A -> C, B -> D  (no crossing)
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, c, make_edge());
        g.add_edge(b, d, make_edge());

        let north = vec![a, b];
        let south = vec![c, d];
        assert_eq!(count_bilayer_crossings(&g, &north, &south), 0);
    }

    #[test]
    fn test_count_bilayer_crossings_one() {
        // A -> D, B -> C  (one crossing)
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, d, make_edge());
        g.add_edge(b, c, make_edge());

        let north = vec![a, b];
        let south = vec![c, d];
        assert_eq!(count_bilayer_crossings(&g, &north, &south), 1);
    }

    #[test]
    fn test_count_bilayer_crossings_three() {
        // A -> D, B -> C, A -> C  (three edges, but only some cross)
        // North: [A, B, C_n], South: [C, D]
        // Actually let's do the classic: A->C, B->A_s, C_n->B_s
        // Simpler: 3 north, 3 south, fully reversed = 3 crossings
        // A->Z, B->Y, C->X  with south=[X,Y,Z]
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let x = g.add_node(make_node("X"));
        let y = g.add_node(make_node("Y"));
        let z = g.add_node(make_node("Z"));
        g.add_edge(a, z, make_edge());
        g.add_edge(b, y, make_edge());
        g.add_edge(c, x, make_edge());

        let north = vec![a, b, c];
        let south = vec![x, y, z];
        assert_eq!(count_bilayer_crossings(&g, &north, &south), 3);
    }

    #[test]
    fn test_best_result_tracking() {
        // Build a graph where the initial ordering already has 0 crossings.
        // Verify minimize_crossings preserves it (doesn't degrade).
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, c, make_edge());
        g.add_edge(b, d, make_edge());

        let membership = SubgraphMembership::new();
        let mut layers = vec![vec![a, b], vec![c, d]];
        minimize_crossings(&g, &mut layers, &membership, 24);

        // Should stay at 0 crossings
        assert_eq!(count_total_crossings(&g, &layers), 0);
    }

    #[test]
    fn test_reduces_crossings() {
        // Start with a bad ordering and verify crossings are reduced.
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, c, make_edge());
        g.add_edge(b, d, make_edge());

        let membership = SubgraphMembership::new();
        // Start with reversed south layer (1 crossing)
        let mut layers = vec![vec![a, b], vec![d, c]];
        assert_eq!(count_total_crossings(&g, &layers), 1);

        minimize_crossings(&g, &mut layers, &membership, 24);

        assert_eq!(count_total_crossings(&g, &layers), 0);
    }

    // -----------------------------------------------------------------------
    // Subgraph refinement tests
    // -----------------------------------------------------------------------

    fn make_ast_with_subgraphs(subgraphs: Vec<SubgraphDef>) -> FlowchartAst {
        FlowchartAst {
            subgraphs,
            ..Default::default()
        }
    }

    fn make_subgraph(id: &str, children: Vec<SubgraphDef>) -> SubgraphDef {
        SubgraphDef {
            id: id.to_string(),
            label: None,
            direction: None,
            nodes: vec![],
            edges: vec![],
            subgraphs: children,
        }
    }

    #[test]
    fn test_refinement_improves_subgraph_ordering() {
        // Subgraph "SG" has an internal crossing that refinement should fix.
        //   Layer 0: [X]              (not in SG)
        //   Layer 1: [A, B]           (both in SG)
        //   Layer 2: [C, D]           (both in SG)
        // Edges: X->A, X->B, A->D, B->C
        // Initial: A(0)->D(1), B(1)->C(0) = 1 crossing
        // After:   layer 2 reordered to [D, C] -> 0 crossings
        let mut g = DiGraph::new();
        let x = g.add_node(make_node("X"));
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(x, a, make_edge());
        g.add_edge(x, b, make_edge());
        g.add_edge(a, d, make_edge());
        g.add_edge(b, c, make_edge());

        let mut membership = SubgraphMembership::new();
        membership.insert("X".to_string(), vec![]);
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec!["SG".to_string()]);
        membership.insert("C".to_string(), vec!["SG".to_string()]);
        membership.insert("D".to_string(), vec!["SG".to_string()]);

        let ast = make_ast_with_subgraphs(vec![make_subgraph("SG", vec![])]);

        let mut layers = vec![vec![x], vec![a, b], vec![c, d]];
        let initial_cc = count_total_crossings(&g, &layers);
        assert_eq!(initial_cc, 1);

        refine_subgraph_ordering(&g, &mut layers, &membership, &ast, &[]);

        let final_cc = count_total_crossings(&g, &layers);
        assert_eq!(
            final_cc, 0,
            "refinement should eliminate the intra-subgraph crossing"
        );
    }

    #[test]
    fn test_refinement_never_worsens_crossings() {
        // Already optimal ordering — refinement must not degrade it.
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, c, make_edge());
        g.add_edge(b, d, make_edge());

        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec!["SG".to_string()]);
        membership.insert("C".to_string(), vec!["SG".to_string()]);
        membership.insert("D".to_string(), vec!["SG".to_string()]);

        let ast = make_ast_with_subgraphs(vec![make_subgraph("SG", vec![])]);

        let mut layers = vec![vec![a, b], vec![c, d]];
        let initial_cc = count_total_crossings(&g, &layers);
        assert_eq!(initial_cc, 0);

        refine_subgraph_ordering(&g, &mut layers, &membership, &ast, &[]);

        let final_cc = count_total_crossings(&g, &layers);
        assert!(
            final_cc <= initial_cc,
            "refinement must never worsen crossings (was {initial_cc}, now {final_cc})"
        );
    }

    #[test]
    fn test_nested_inner_ordering_preserved() {
        // Inner subgraph is refined first (post-order).  When the outer
        // subgraph is subsequently refined, inner members must move as an
        // atomic block, preserving the improved inner order.
        //
        //   Layer 0: [X]                     (root)
        //   Layer 1: [A, B, C]               (A,B in Inner⊂Outer; C in Outer)
        //   Layer 2: [D, E, F]               (D,E in Inner⊂Outer; F in Outer)
        // Edges: X->A, X->C, A->E, B->D, C->F
        // Initial inner crossing: A(0)->E(1), B(1)->D(0) = 1 crossing
        // Inner refinement should reorder layer 2 to [E, D, F].
        // Outer refinement must keep E before D (atomic inner block).
        let mut g = DiGraph::new();
        let x = g.add_node(make_node("X"));
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        let e = g.add_node(make_node("E"));
        let f = g.add_node(make_node("F"));
        g.add_edge(x, a, make_edge());
        g.add_edge(x, c, make_edge());
        g.add_edge(a, e, make_edge());
        g.add_edge(b, d, make_edge());
        g.add_edge(c, f, make_edge());

        let mut membership = SubgraphMembership::new();
        membership.insert("X".to_string(), vec![]);
        membership.insert(
            "A".to_string(),
            vec!["Outer".to_string(), "Inner".to_string()],
        );
        membership.insert(
            "B".to_string(),
            vec!["Outer".to_string(), "Inner".to_string()],
        );
        membership.insert("C".to_string(), vec!["Outer".to_string()]);
        membership.insert(
            "D".to_string(),
            vec!["Outer".to_string(), "Inner".to_string()],
        );
        membership.insert(
            "E".to_string(),
            vec!["Outer".to_string(), "Inner".to_string()],
        );
        membership.insert("F".to_string(), vec!["Outer".to_string()]);

        let ast = make_ast_with_subgraphs(vec![make_subgraph(
            "Outer",
            vec![make_subgraph("Inner", vec![])],
        )]);

        let mut layers = vec![vec![x], vec![a, b, c], vec![d, e, f]];
        let initial_cc = count_total_crossings(&g, &layers);

        refine_subgraph_ordering(&g, &mut layers, &membership, &ast, &[]);

        let final_cc = count_total_crossings(&g, &layers);
        assert!(final_cc <= initial_cc, "crossings should not worsen");

        // Inner refinement should have placed E before D (fixing the crossing).
        let d_pos = layers[2].iter().position(|&n| n == d).unwrap();
        let e_pos = layers[2].iter().position(|&n| n == e).unwrap();
        assert!(
            e_pos < d_pos,
            "inner order E < D should be established and preserved (E@{e_pos}, D@{d_pos})"
        );
    }

    #[test]
    fn test_non_contiguous_members_skip() {
        // Subgraph members are not contiguous in a layer — refinement must
        // skip this subgraph entirely and leave layers unchanged.
        //   Layer 0: [A, B, C]   (A,C in SG; B NOT in SG — breaks contiguity)
        //   Layer 1: [D, E]      (both in SG)
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        let e = g.add_node(make_node("E"));
        g.add_edge(a, d, make_edge());
        g.add_edge(c, e, make_edge());

        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec![]);
        membership.insert("C".to_string(), vec!["SG".to_string()]);
        membership.insert("D".to_string(), vec!["SG".to_string()]);
        membership.insert("E".to_string(), vec!["SG".to_string()]);

        let ast = make_ast_with_subgraphs(vec![make_subgraph("SG", vec![])]);

        let mut layers = vec![vec![a, b, c], vec![d, e]];
        let original = layers.clone();

        refine_subgraph_ordering(&g, &mut layers, &membership, &ast, &[]);

        assert_eq!(
            layers, original,
            "non-contiguous subgraph members should cause skip — layers unchanged"
        );
    }
}
