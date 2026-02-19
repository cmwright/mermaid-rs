use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;

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
fn count_total_crossings(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
) -> usize {
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
    let original_positions: HashMap<NodeIndex, usize> = layer
        .iter()
        .enumerate()
        .map(|(i, &n)| (n, i))
        .collect();

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
}
