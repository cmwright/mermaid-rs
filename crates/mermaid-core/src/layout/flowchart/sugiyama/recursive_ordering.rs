//! Subgraph-recursive ordering algorithm matching dagre's `order/` module.
//!
//! This module implements the compound-graph-aware crossing minimization
//! algorithm from dagre. The key difference from the flat barycenter heuristic
//! is that it processes subgraphs recursively: innermost subgraphs are sorted
//! first, then their results are used as atomic blocks when sorting the parent.
//!
//! ## Algorithm overview
//!
//! For each sweep (alternating up/down):
//! 1. Build a layer graph for each layer (edges to the adjacent fixed layer)
//! 2. For each layer graph, recursively sort the root's children:
//!    a. Compute barycenters from the fixed layer
//!    b. Recurse into child subgraphs
//!    c. Resolve conflicts with the constraint graph
//!    d. Sort by barycenter
//!    e. Pin border nodes at the left/right edges
//! 3. Record subgraph ordering constraints for subsequent layers

use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};

use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::sugiyama::border_segments::BorderSegments;
use crate::layout::flowchart::types::*;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Maximum consecutive non-improving iterations before early stop.
const MAX_NO_IMPROVEMENT: usize = 4;

/// Subgraph-recursive crossing minimization.
///
/// This replaces the flat `minimize_crossings` + `refine_subgraph_ordering`
/// with a single recursive algorithm that properly handles compound graphs.
pub fn minimize_crossings_recursive(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &mut [Vec<NodeIndex>],
    membership: &SubgraphMembership,
    border_segments: &BorderSegments,
    num_iterations: usize,
) {
    if layers.is_empty() {
        return;
    }

    // Build parent map: node -> immediate parent subgraph id.
    let parent_map = build_parent_map(graph, membership);

    // Build children map: subgraph id -> direct children node indices in each layer.
    // We need this to know which nodes to sort within each subgraph.

    let mut best_layers: Vec<Vec<NodeIndex>> = layers.to_vec();
    let mut best_cc = count_total_crossings(graph, layers);
    let mut no_improve = 0usize;

    for iteration in 0..num_iterations {
        let bias_right = iteration % 4 >= 2;

        if iteration % 2 == 0 {
            // Down sweep: process layers top to bottom
            let mut constraint_graph = ConstraintGraph::new();
            for i in 1..layers.len() {
                let fixed_positions = build_position_map(&layers[i - 1]);
                sweep_layer(
                    graph,
                    &mut layers[i],
                    &fixed_positions,
                    petgraph::Direction::Incoming,
                    membership,
                    &parent_map,
                    border_segments,
                    bias_right,
                    &mut constraint_graph,
                );
            }
        } else {
            // Up sweep: process layers bottom to top
            let mut constraint_graph = ConstraintGraph::new();
            for i in (0..layers.len().saturating_sub(1)).rev() {
                let fixed_positions = build_position_map(&layers[i + 1]);
                sweep_layer(
                    graph,
                    &mut layers[i],
                    &fixed_positions,
                    petgraph::Direction::Outgoing,
                    membership,
                    &parent_map,
                    border_segments,
                    bias_right,
                    &mut constraint_graph,
                );
            }
        }

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

    // Restore best ordering.
    for (dst, src) in layers.iter_mut().zip(best_layers.into_iter()) {
        *dst = src;
    }
}

// ---------------------------------------------------------------------------
// Constraint graph
// ---------------------------------------------------------------------------

/// Tracks ordering constraints between sibling nodes/subgraphs.
/// An edge (a, b) means "a must come before b".
struct ConstraintGraph {
    edges: HashSet<(String, String)>,
}

impl ConstraintGraph {
    fn new() -> Self {
        Self {
            edges: HashSet::new(),
        }
    }

    fn add_edge(&mut self, from: &str, to: &str) {
        self.edges.insert((from.to_string(), to.to_string()));
    }

    fn has_edge(&self, from: &str, to: &str) -> bool {
        self.edges.contains(&(from.to_string(), to.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Layer sweep
// ---------------------------------------------------------------------------

/// Process a single layer: sort it using the recursive subgraph algorithm.
fn sweep_layer(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &mut Vec<NodeIndex>,
    fixed_positions: &HashMap<NodeIndex, usize>,
    direction: petgraph::Direction,
    membership: &SubgraphMembership,
    parent_map: &HashMap<NodeIndex, Option<String>>,
    border_segments: &BorderSegments,
    bias_right: bool,
    constraint_graph: &mut ConstraintGraph,
) {
    if layer.len() <= 1 {
        return;
    }

    // Sort the root level (nodes that have no parent subgraph in this layer).
    let result = sort_subgraph(
        graph,
        layer,
        None, // root level
        fixed_positions,
        direction,
        membership,
        parent_map,
        border_segments,
        bias_right,
        constraint_graph,
    );

    *layer = result.vs;

    // Record subgraph constraints for subsequent layers.
    add_subgraph_constraints(graph, parent_map, &layer, constraint_graph);
}

// ---------------------------------------------------------------------------
// Recursive subgraph sort
// ---------------------------------------------------------------------------

/// Result of sorting a subgraph.
struct SortResult {
    /// Ordered node indices.
    vs: Vec<NodeIndex>,
    /// Aggregate barycenter (if computed).
    barycenter: Option<f64>,
    /// Aggregate weight.
    weight: f64,
}

/// An entry in the sort: either a single node or a sorted subgraph.
struct SortEntry {
    /// The node(s) in this entry. For a subgraph, these are the recursively
    /// sorted children. For a leaf, this is a single node.
    vs: Vec<NodeIndex>,
    /// The representative node (for looking up the entry).
    v: NodeIndex,
    /// Barycenter from the fixed layer.
    barycenter: Option<f64>,
    /// Weight (number of edges to fixed layer).
    weight: f64,
    /// Original index in the movable set (for stable tie-breaking).
    i: usize,
}

/// Recursively sort nodes within a subgraph (or at root level if sg_id is None).
fn sort_subgraph(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &[NodeIndex],
    sg_id: Option<&str>,
    fixed_positions: &HashMap<NodeIndex, usize>,
    direction: petgraph::Direction,
    membership: &SubgraphMembership,
    parent_map: &HashMap<NodeIndex, Option<String>>,
    border_segments: &BorderSegments,
    bias_right: bool,
    constraint_graph: &mut ConstraintGraph,
) -> SortResult {
    // Find children of this subgraph in the current layer.
    let movable: Vec<NodeIndex> = layer
        .iter()
        .copied()
        .filter(|&ni| {
            let parent = parent_map.get(&ni).and_then(|p| p.as_deref());
            parent == sg_id
        })
        .collect();

    if movable.is_empty() {
        return SortResult {
            vs: Vec::new(),
            barycenter: None,
            weight: 0.0,
        };
    }

    // Identify border nodes (if this is a subgraph with borders).
    let (border_left, border_right) = if let Some(sg) = sg_id {
        // Find border nodes by checking if they're in the layer.
        let bl = border_segments
            .subgraphs
            .get(sg)
            .and_then(|borders| {
                borders
                    .border_left
                    .values()
                    .find(|&&ni| layer.contains(&ni))
            })
            .copied();
        let br = border_segments
            .subgraphs
            .get(sg)
            .and_then(|borders| {
                borders
                    .border_right
                    .values()
                    .find(|&&ni| layer.contains(&ni))
            })
            .copied();
        (bl, br)
    } else {
        (None, None)
    };

    // Remove border nodes from movable set (they'll be pinned at the edges).
    let movable: Vec<NodeIndex> = movable
        .into_iter()
        .filter(|ni| Some(*ni) != border_left && Some(*ni) != border_right)
        .collect();

    // Compute barycenters for each movable node.
    let mut entries: Vec<SortEntry> = movable
        .iter()
        .enumerate()
        .map(|(i, &ni)| {
            let (barycenter, weight) = compute_barycenter(graph, ni, fixed_positions, direction);
            SortEntry {
                vs: vec![ni],
                v: ni,
                barycenter,
                weight,
                i,
            }
        })
        .collect();

    // Recurse into child subgraphs.
    // A child subgraph is identified by: it's a node in the movable set that
    // is a border-left node of some subgraph (indicating a subgraph is present
    // at this layer). Actually, we should check which nodes are "subgraph
    // representatives" — nodes that have children in this layer.
    //
    // Simpler approach: find all subgraph IDs that have children in this layer
    // and whose parent is the current sg_id.
    let child_subgraphs: Vec<String> =
        find_child_subgraphs_in_layer(layer, sg_id, membership, parent_map, graph, border_segments);

    for child_sg in &child_subgraphs {
        // Recursively sort the child subgraph.
        let child_result = sort_subgraph(
            graph,
            layer,
            Some(child_sg),
            fixed_positions,
            direction,
            membership,
            parent_map,
            border_segments,
            bias_right,
            constraint_graph,
        );

        // Find the entry for the first node of this child subgraph and expand it.
        // Actually, the child subgraph members aren't directly in our entries
        // (they're children of the child subgraph, not direct children of us).
        // We need to find which of our movable nodes belongs to this child subgraph.
        //
        // In dagre, the layer graph has a node for the subgraph itself.
        // In our approach, we need to find the entry whose node is a member
        // of the child subgraph and replace it with the recursive result.

        // Find entries that belong to this child subgraph.
        // These are entries whose node's parent is the child subgraph.
        // But wait — we already filtered movable to only include direct children
        // of sg_id. Nodes belonging to child_sg would have parent = child_sg,
        // not sg_id. So they wouldn't be in our entries.
        //
        // This means the child subgraph sort result needs to be treated as a
        // single atomic entry in our sort. We add it as a new entry.
        if !child_result.vs.is_empty() {
            entries.push(SortEntry {
                vs: child_result.vs,
                v: entries.last().map(|e| e.v).unwrap_or(NodeIndex::new(0)),
                barycenter: child_result.barycenter,
                weight: child_result.weight,
                i: entries.len(),
            });
        }
    }

    // Resolve conflicts with the constraint graph.
    resolve_conflicts(&mut entries, constraint_graph, graph);

    // Sort entries by barycenter.
    sort_entries(&mut entries, bias_right);

    // Flatten entries into final order.
    let mut vs: Vec<NodeIndex> = Vec::new();

    // Pin border_left at the start.
    if let Some(bl) = border_left {
        vs.push(bl);
    }

    for entry in &entries {
        vs.extend_from_slice(&entry.vs);
    }

    // Pin border_right at the end.
    if let Some(br) = border_right {
        vs.push(br);
    }

    // Compute aggregate barycenter.
    let total_weight: f64 = entries.iter().map(|e| e.weight).sum();
    let aggregate_bc = if total_weight > 0.0 {
        let weighted_sum: f64 = entries
            .iter()
            .filter_map(|e| e.barycenter.map(|bc| bc * e.weight))
            .sum();
        Some(weighted_sum / total_weight)
    } else {
        None
    };

    SortResult {
        vs,
        barycenter: aggregate_bc,
        weight: total_weight,
    }
}

// ---------------------------------------------------------------------------
// Barycenter computation
// ---------------------------------------------------------------------------

/// Compute the barycenter of a node from its neighbors in the fixed layer.
fn compute_barycenter(
    graph: &DiGraph<NodeData, EdgeData>,
    node: NodeIndex,
    fixed_positions: &HashMap<NodeIndex, usize>,
    direction: petgraph::Direction,
) -> (Option<f64>, f64) {
    let mut sum = 0.0;
    let mut weight = 0.0;

    for neighbor in graph.neighbors_directed(node, direction) {
        if let Some(&pos) = fixed_positions.get(&neighbor) {
            sum += pos as f64;
            weight += 1.0;
        }
    }

    if weight > 0.0 {
        (Some(sum / weight), weight)
    } else {
        (None, 0.0)
    }
}

// ---------------------------------------------------------------------------
// Conflict resolution
// ---------------------------------------------------------------------------

/// Resolve conflicts between entries and the constraint graph.
///
/// If the constraint graph says "A must come before B" but barycenters put
/// B before A, merge them into a single entry.
fn resolve_conflicts(
    entries: &mut Vec<SortEntry>,
    constraint_graph: &ConstraintGraph,
    graph: &DiGraph<NodeData, EdgeData>,
) {
    if entries.len() <= 1 {
        return;
    }

    // Check each constraint edge and merge if needed.
    // For now, use a simplified approach: check pairwise constraints
    // and merge entries that violate them.
    let mut merged = vec![false; entries.len()];

    for i in 0..entries.len() {
        if merged[i] {
            continue;
        }
        for j in (i + 1)..entries.len() {
            if merged[j] {
                continue;
            }

            // Check if there's a constraint j->i (j must come before i).
            // If so and j is after i in barycenter order, merge.
            let id_i = entries[i]
                .vs
                .first()
                .map(|&ni| graph[ni].id.clone())
                .unwrap_or_default();
            let id_j = entries[j]
                .vs
                .first()
                .map(|&ni| graph[ni].id.clone())
                .unwrap_or_default();

            if constraint_graph.has_edge(&id_j, &id_i) {
                // j should come before i, but j has index > i.
                // Check if barycenters agree.
                if let (Some(bc_i), Some(bc_j)) = (entries[i].barycenter, entries[j].barycenter) {
                    if bc_j > bc_i {
                        // Conflict: merge j into i.
                        let j_vs = std::mem::take(&mut entries[j].vs);
                        let j_weight = entries[j].weight;
                        let j_bc = entries[j].barycenter;

                        // Prepend j's nodes (it should come before i).
                        let mut new_vs = j_vs;
                        new_vs.extend_from_slice(&entries[i].vs);
                        entries[i].vs = new_vs;

                        // Merge barycenters.
                        let i_weight = entries[i].weight;
                        let total_w = i_weight + j_weight;
                        if total_w > 0.0 {
                            let i_bc = entries[i].barycenter.unwrap_or(0.0);
                            entries[i].barycenter =
                                Some((i_bc * i_weight + j_bc.unwrap_or(0.0) * j_weight) / total_w);
                            entries[i].weight = total_w;
                        }

                        merged[j] = true;
                    }
                }
            }
        }
    }

    // Remove merged entries.
    let mut result = Vec::new();
    for (i, entry) in entries.drain(..).enumerate() {
        if !merged[i] {
            result.push(entry);
        }
    }
    *entries = result;
}

// ---------------------------------------------------------------------------
// Sorting
// ---------------------------------------------------------------------------

/// Sort entries by barycenter, interleaving unsortable entries.
fn sort_entries(entries: &mut Vec<SortEntry>, bias_right: bool) {
    // Separate into sortable (has barycenter) and unsortable.
    let old_entries: Vec<SortEntry> = entries.drain(..).collect();

    let mut sortable_owned: Vec<SortEntry> = Vec::new();
    let mut unsortable_owned: Vec<(usize, SortEntry)> = Vec::new();

    for (i, entry) in old_entries.into_iter().enumerate() {
        if entry.barycenter.is_some() {
            sortable_owned.push(entry);
        } else {
            unsortable_owned.push((i, entry));
        }
    }

    // Sort sortable by barycenter with biased tie-breaking.
    sortable_owned.sort_by(|a, b| {
        let ba = a.barycenter.unwrap_or(0.0);
        let bb = b.barycenter.unwrap_or(0.0);
        let cmp = ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal);
        if cmp != std::cmp::Ordering::Equal {
            return cmp;
        }
        if bias_right {
            b.i.cmp(&a.i)
        } else {
            a.i.cmp(&b.i)
        }
    });

    // Interleave unsortable entries at their original positions.
    unsortable_owned.sort_by(|a, b| b.0.cmp(&a.0)); // reverse for pop
    let mut vs_idx = 0;
    for s in sortable_owned {
        while let Some(&(orig_i, _)) = unsortable_owned.last() {
            if orig_i <= vs_idx {
                entries.push(unsortable_owned.pop().unwrap().1);
            } else {
                break;
            }
        }
        vs_idx += s.vs.len();
        entries.push(s);
    }
    // Remaining unsortable.
    while let Some((_, entry)) = unsortable_owned.pop() {
        entries.push(entry);
    }
}

// ---------------------------------------------------------------------------
// Subgraph constraint recording
// ---------------------------------------------------------------------------

/// Record the relative order of subgraphs as constraints for subsequent layers.
fn add_subgraph_constraints(
    graph: &DiGraph<NodeData, EdgeData>,
    parent_map: &HashMap<NodeIndex, Option<String>>,
    sorted_layer: &[NodeIndex],
    constraint_graph: &mut ConstraintGraph,
) {
    // Track the previous child seen at each parent level.
    let mut prev_at_parent: HashMap<Option<String>, String> = HashMap::new();

    for &ni in sorted_layer {
        let node_id = &graph[ni].id;

        // Walk up the parent chain.
        let mut child_id = node_id.clone();
        let mut parent = parent_map.get(&ni).and_then(|p| p.clone());

        loop {
            let key = parent.clone();
            if let Some(prev_child) = prev_at_parent.get(&key) {
                if *prev_child != child_id {
                    // Different child at this level — record constraint.
                    constraint_graph.add_edge(prev_child, &child_id);
                    // Only record the first difference in the hierarchy.
                    break;
                }
            }
            prev_at_parent.insert(key.clone(), child_id.clone());

            // Walk up.
            match &parent {
                Some(p) => {
                    child_id = p.clone();
                    // Find parent of this subgraph.
                    // We need a subgraph parent map. For now, use a simplified
                    // approach — stop at root.
                    parent = None; // TODO: walk subgraph hierarchy properly
                }
                None => break,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Build a map from NodeIndex to its immediate parent subgraph ID.
fn build_parent_map(
    graph: &DiGraph<NodeData, EdgeData>,
    membership: &SubgraphMembership,
) -> HashMap<NodeIndex, Option<String>> {
    let mut result = HashMap::new();
    for ni in graph.node_indices() {
        let id = &graph[ni].id;
        let parent = membership.get(id).and_then(|path| path.last().cloned());
        result.insert(ni, parent);
    }
    result
}

/// Build a map from NodeIndex to its position within the layer.
fn build_position_map(layer: &[NodeIndex]) -> HashMap<NodeIndex, usize> {
    layer
        .iter()
        .enumerate()
        .map(|(pos, &node)| (node, pos))
        .collect()
}

/// Find child subgraph IDs that have members in this layer and whose
/// parent is the given sg_id.
fn find_child_subgraphs_in_layer(
    layer: &[NodeIndex],
    parent_sg_id: Option<&str>,
    membership: &SubgraphMembership,
    parent_map: &HashMap<NodeIndex, Option<String>>,
    graph: &DiGraph<NodeData, EdgeData>,
    border_segments: &BorderSegments,
) -> Vec<String> {
    // A child subgraph of `parent_sg_id` is a subgraph whose immediate parent
    // in the hierarchy is `parent_sg_id`, and which has at least one member
    // in this layer (either a direct member or a border node).
    let mut child_sgs: HashSet<String> = HashSet::new();

    for &ni in layer {
        let id = &graph[ni].id;
        if let Some(path) = membership.get(id) {
            // Find the subgraph at the level just below parent_sg_id.
            let parent_idx = match parent_sg_id {
                Some(pid) => path.iter().position(|p| p == pid).map(|i| i + 1),
                None => Some(0),
            };

            if let Some(idx) = parent_idx {
                if idx < path.len() {
                    let child_sg = &path[idx];
                    // Only include if there are actual child nodes in this layer
                    // that belong to this child subgraph (not the node itself).
                    if parent_map.get(&ni).and_then(|p| p.as_deref()) != parent_sg_id {
                        // This node's parent is NOT the current sg_id, meaning
                        // it's deeper in the hierarchy. Its immediate parent's
                        // immediate parent might be sg_id.
                        // For simplicity, just collect unique child subgraph IDs.
                    }
                    child_sgs.insert(child_sg.clone());
                }
            }
        }
    }

    // Filter to only subgraphs that have border segments (i.e., they actually
    // span this layer's rank range).
    child_sgs
        .into_iter()
        .filter(|sg| border_segments.subgraphs.contains_key(sg))
        .collect()
}

// ---------------------------------------------------------------------------
// Crossing count (shared with ordering.rs)
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

    fn update(&mut self, mut i: usize) {
        i += 1;
        while i < self.tree.len() {
            self.tree[i] += 1;
            i += i & i.wrapping_neg();
        }
    }

    fn prefix_sum(&self, mut i: usize) -> usize {
        i += 1;
        let mut sum = 0;
        while i > 0 {
            sum += self.tree[i];
            i -= i & i.wrapping_neg();
        }
        sum
    }
}

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

    let mut south_endpoints: Vec<usize> = Vec::new();
    for &north_node in north_layer {
        let mut targets: Vec<usize> = graph
            .neighbors_directed(north_node, petgraph::Direction::Outgoing)
            .filter_map(|n| south_pos.get(&n).copied())
            .collect();
        targets.sort_unstable();
        south_endpoints.extend(targets);
    }

    let south_size = south_layer.len();
    if south_size == 0 {
        return 0;
    }
    let mut tree = FenwickTree::new(south_size);
    let mut crossings: usize = 0;
    let mut inserted: usize = 0;

    for &pos in &south_endpoints {
        crossings += inserted - tree.prefix_sum(pos);
        tree.update(pos);
        inserted += 1;
    }

    crossings
}

fn count_total_crossings(graph: &DiGraph<NodeData, EdgeData>, layers: &[Vec<NodeIndex>]) -> usize {
    (0..layers.len().saturating_sub(1))
        .map(|i| count_bilayer_crossings(graph, &layers[i], &layers[i + 1]))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{ArrowEnd, LineStyle};

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
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            label_width: 0.0,
            label_height: 0.0,
            weight: 1,
            minlen: 1,
        }
    }

    #[test]
    fn test_recursive_ordering_basic() {
        // Simple 2-layer graph with crossing.
        // A->D, B->C: initial order [C, D] has 1 crossing.
        // Should reorder to [D, C] or [C, D] -> 0 crossings.
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, c, make_edge());
        g.add_edge(b, d, make_edge());

        let membership = SubgraphMembership::new();
        let border_segments = BorderSegments {
            subgraphs: HashMap::new(),
        };

        let mut layers = vec![vec![a, b], vec![d, c]]; // 1 crossing
        assert_eq!(count_total_crossings(&g, &layers), 1);

        minimize_crossings_recursive(&g, &mut layers, &membership, &border_segments, 24);

        assert_eq!(
            count_total_crossings(&g, &layers),
            0,
            "should eliminate crossing"
        );
    }

    #[test]
    fn test_recursive_ordering_preserves_zero() {
        // Already 0 crossings — should not degrade.
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, c, make_edge());
        g.add_edge(b, d, make_edge());

        let membership = SubgraphMembership::new();
        let border_segments = BorderSegments {
            subgraphs: HashMap::new(),
        };

        let mut layers = vec![vec![a, b], vec![c, d]];
        assert_eq!(count_total_crossings(&g, &layers), 0);

        minimize_crossings_recursive(&g, &mut layers, &membership, &border_segments, 24);

        assert_eq!(count_total_crossings(&g, &layers), 0);
    }
}
