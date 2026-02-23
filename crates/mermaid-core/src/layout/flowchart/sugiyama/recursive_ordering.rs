//! Subgraph-recursive ordering algorithm matching dagre's `order/` module.
//!
//! This module implements the compound-graph-aware crossing minimization
//! algorithm from dagre. The key difference from the flat barycenter heuristic
//! is that it processes subgraphs recursively: innermost subgraphs are sorted
//! first, then their results are used as atomic blocks when sorting the parent.
//!
//! ## Algorithm overview (matching dagre exactly)
//!
//! For each sweep (alternating up/down):
//! 1. Build position maps for the fixed layer
//! 2. For each movable layer, recursively sort the root's children:
//!    a. Compute barycenters from the fixed layer
//!    b. Recurse into child subgraphs, merge recursive barycenters
//!    c. Resolve conflicts with the constraint graph (Forster's algorithm)
//!    d. Expand subgraph entries into their sorted child lists
//!    e. Sort by barycenter with biased tie-breaking, interleaving unsortable entries
//!    f. Pin border nodes at the left/right edges, incorporate border predecessors
//! 3. Record subgraph ordering constraints for subsequent layers

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};

use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::sugiyama::border_segments::BorderSegments;
use crate::layout::flowchart::types::*;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Maximum consecutive non-improving iterations before early stop.
const MAX_NO_IMPROVEMENT: usize = 4;

/// Subgraph-recursive crossing minimization matching dagre's `order()`.
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
    let parent_map = build_parent_map(graph, membership, border_segments);

    // Build subgraph parent map: sg_id -> parent sg_id (for walking hierarchy).
    let sg_parent_map = build_sg_parent_map(membership);

    let mut best_layers: Vec<Vec<NodeIndex>> = layers.to_vec();
    let mut best_cc = count_total_crossings(graph, layers);
    let mut no_improve = 0usize;

    for iteration in 0..num_iterations {
        let bias_right = iteration % 4 >= 2;

        // Constraint graph accumulates ordering constraints during a sweep.
        // dagre creates a fresh one per sweep direction.
        let mut constraint_graph = ConstraintGraph::new();

        // dagre: i % 2 ? downLayerGraphs : upLayerGraphs
        // i=0 → up sweep, i=1 → down sweep, i=2 → up sweep, ...
        if iteration % 2 == 0 {
            // Up sweep: process layers bottom to top
            // dagre: upLayerGraphs = range(maxRank-1, -1, -1), "outEdges"
            for i in (0..layers.len().saturating_sub(1)).rev() {
                let fixed_order = build_order_map(&layers[i + 1]);
                sweep_layer(
                    graph,
                    &mut layers[i],
                    &fixed_order,
                    petgraph::Direction::Outgoing,
                    membership,
                    &parent_map,
                    &sg_parent_map,
                    border_segments,
                    bias_right,
                    &mut constraint_graph,
                );
            }
        } else {
            // Down sweep: process layers top to bottom
            // dagre: downLayerGraphs = range(1, maxRank+1), "inEdges"
            for i in 1..layers.len() {
                let fixed_order = build_order_map(&layers[i - 1]);
                sweep_layer(
                    graph,
                    &mut layers[i],
                    &fixed_order,
                    petgraph::Direction::Incoming,
                    membership,
                    &parent_map,
                    &sg_parent_map,
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
        } else if cc == best_cc {
            // dagre: equal quality still saves (allows drifting through plateaus)
            best_layers = layers.to_vec();
            no_improve += 1;
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
// Constraint graph — matches dagre's cg (a Graph with string node ids)
// ---------------------------------------------------------------------------

/// Tracks ordering constraints between sibling nodes/subgraphs.
/// An edge (a, b) means "a must come before b".
/// Uses string IDs (node IDs or subgraph IDs) to identify entries.
struct ConstraintGraph {
    /// Forward edges: from -> set of to
    edges: HashMap<String, HashSet<String>>,
}

impl ConstraintGraph {
    fn new() -> Self {
        Self {
            edges: HashMap::new(),
        }
    }

    fn add_edge(&mut self, from: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
    }

    /// Get all constraint edges as (from, to) pairs.
    fn all_edges(&self) -> Vec<(String, String)> {
        let mut result = Vec::new();
        for (from, tos) in &self.edges {
            for to in tos {
                result.push((from.clone(), to.clone()));
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Layer sweep
// ---------------------------------------------------------------------------

/// Process a single layer: sort it using the recursive subgraph algorithm.
/// Matches dagre's `sweepLayerGraphs` inner loop body.
fn sweep_layer(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &mut Vec<NodeIndex>,
    fixed_order: &HashMap<NodeIndex, usize>,
    direction: petgraph::Direction,
    membership: &SubgraphMembership,
    parent_map: &HashMap<NodeIndex, Option<String>>,
    sg_parent_map: &HashMap<String, Option<String>>,
    border_segments: &BorderSegments,
    bias_right: bool,
    constraint_graph: &mut ConstraintGraph,
) {
    if layer.len() <= 1 {
        return;
    }

    // Sort the root level (no parent subgraph).
    // This matches dagre's `sortSubgraph(lg, root, cg, biasRight)`.
    let result = sort_subgraph(
        graph,
        layer,
        None, // root level
        fixed_order,
        direction,
        membership,
        parent_map,
        sg_parent_map,
        border_segments,
        bias_right,
        constraint_graph,
    );

    *layer = result.vs;

    // Assign order values to nodes in the layer (so fixed_order works for next layer).
    // dagre does: `sorted.vs.forEach((v, i) => lg.node(v).order = i)`
    // We don't mutate the graph, but the next layer will build its own fixed_order.

    // Record subgraph constraints for subsequent layers.
    // Matches dagre's `addSubgraphConstraints(lg, cg, sorted.vs)`.
    add_subgraph_constraints(parent_map, sg_parent_map, layer, constraint_graph);
}

// ---------------------------------------------------------------------------
// Recursive subgraph sort — matches dagre's `sortSubgraph`
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

/// An entry for the sort algorithm. Can represent a single node or a group
/// of nodes (from conflict resolution or subgraph expansion).
struct SortEntry {
    /// The node(s) in this entry.
    vs: Vec<NodeIndex>,
    /// The representative node ID (for constraint graph lookups).
    v_id: String,
    /// Barycenter from the fixed layer.
    barycenter: Option<f64>,
    /// Weight (sum of edge weights to fixed layer).
    weight: f64,
    /// Original index in the movable set (for stable tie-breaking).
    i: usize,
    /// Whether this entry has been merged into another.
    merged: bool,
}

/// Recursively sort nodes within a subgraph (or at root level if sg_id is None).
/// Matches dagre's `sortSubgraph(g, v, cg, biasRight)`.
fn sort_subgraph(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &[NodeIndex],
    sg_id: Option<&str>,
    fixed_order: &HashMap<NodeIndex, usize>,
    direction: petgraph::Direction,
    membership: &SubgraphMembership,
    parent_map: &HashMap<NodeIndex, Option<String>>,
    sg_parent_map: &HashMap<String, Option<String>>,
    border_segments: &BorderSegments,
    bias_right: bool,
    constraint_graph: &mut ConstraintGraph,
) -> SortResult {
    // Identify border nodes for this subgraph (if any).
    // dagre: `let bl = node ? node.borderLeft : undefined`
    let (border_left, border_right) = get_border_nodes_for_layer(sg_id, layer, border_segments);

    // Find child subgraphs: subgraphs whose parent is sg_id and which have
    // members in this layer.
    let child_subgraphs: HashSet<String> =
        find_child_subgraphs_in_layer(layer, sg_id, parent_map, sg_parent_map, border_segments)
            .into_iter()
            .collect();

    // Build the movable list matching dagre's `g.children(v)`.
    // In dagre's layer graph, children of a compound node v include:
    //   - Base nodes at this rank whose parent is v
    //   - Subgraph nodes that span this rank whose parent is v
    // We reconstruct this by walking the layer in order. For each node:
    //   - If its parent == sg_id → it's a direct leaf child
    //   - If its parent is a child subgraph of sg_id → the first time we see
    //     that child subgraph, we add a subgraph placeholder entry
    //
    // This interleaving ensures the `i` indices match dagre's ordering.
    enum MovableItem {
        Leaf(NodeIndex),
        Subgraph(String),
    }

    let mut movable: Vec<MovableItem> = Vec::new();
    let mut seen_child_sgs: HashSet<String> = HashSet::new();

    for &ni in layer {
        // Skip border nodes
        if Some(ni) == border_left || Some(ni) == border_right {
            continue;
        }

        let node_parent = parent_map.get(&ni).and_then(|p| p.as_deref());

        if node_parent == sg_id {
            // Direct child of this subgraph — add as leaf
            movable.push(MovableItem::Leaf(ni));
        } else if let Some(np) = node_parent {
            // Check if this node belongs to a child subgraph of sg_id.
            // Walk up until we find one of our child_subgraphs or reach sg_id.
            let mut sg = Some(np.to_string());
            while let Some(ref sg_check) = sg {
                if child_subgraphs.contains(sg_check) {
                    if seen_child_sgs.insert(sg_check.clone()) {
                        movable.push(MovableItem::Subgraph(sg_check.clone()));
                    }
                    break;
                }
                let next = sg_parent_map.get(sg_check).cloned().flatten();
                if next.as_deref() == sg_id {
                    break;
                }
                sg = next;
            }
        }
    }

    if movable.is_empty() {
        return SortResult {
            vs: Vec::new(),
            barycenter: None,
            weight: 0.0,
        };
    }

    // Build barycenter entries for each movable item.
    // dagre: `let barycenters = barycenter(g, movable)` computes barycenters
    // for ALL movable items (leaf nodes get barycenters from edges; subgraph
    // nodes from edges in the layer graph — typically none, so no barycenter).
    // Then: for each entry with children, recurse and mergeBarycenters.
    let mut entries: Vec<SortEntry> = Vec::with_capacity(movable.len());
    let mut subgraph_results: HashMap<String, SortResult> = HashMap::new();
    // Map from sentinel NodeIndex → subgraph ID, for expand_subgraphs.
    // We use NodeIndex::new(usize::MAX - n) as unique sentinels.
    let mut sentinel_map: HashMap<NodeIndex, String> = HashMap::new();
    let mut sentinel_counter: usize = 0;

    for (i, item) in movable.into_iter().enumerate() {
        match item {
            MovableItem::Leaf(ni) => {
                let (barycenter, weight) = compute_barycenter(graph, ni, fixed_order, direction);
                entries.push(SortEntry {
                    vs: vec![ni],
                    v_id: graph[ni].id.clone(),
                    barycenter,
                    weight,
                    i,
                    merged: false,
                });
            }
            MovableItem::Subgraph(ref child_sg_id) => {
                // Recurse into child subgraph.
                // dagre: sortSubgraph(g, entry.v, cg, biasRight)
                let child_result = sort_subgraph(
                    graph,
                    layer,
                    Some(child_sg_id),
                    fixed_order,
                    direction,
                    membership,
                    parent_map,
                    sg_parent_map,
                    border_segments,
                    bias_right,
                    constraint_graph,
                );

                // The subgraph "node" in dagre's layer graph typically has no
                // in-edges (it's a compound parent, not a base node), so its
                // initial barycenter is None. After recursion, we merge the
                // recursive result's barycenter with the entry's.
                //
                // dagre: if (subgraphResult.barycenter !== undefined) {
                //           mergeBarycenters(entry, subgraphResult);
                //        }
                //
                // Since the initial barycenter is None, mergeBarycenters just
                // copies the recursive result's barycenter and weight.
                let entry_barycenter = child_result.barycenter;
                let entry_weight = child_result.weight;

                // Use a unique sentinel NodeIndex for this subgraph.
                // expandSubgraphs will replace it with the sorted children.
                let sentinel = NodeIndex::new(usize::MAX - sentinel_counter);
                sentinel_counter += 1;
                sentinel_map.insert(sentinel, child_sg_id.clone());

                entries.push(SortEntry {
                    vs: vec![sentinel],
                    v_id: child_sg_id.clone(),
                    barycenter: entry_barycenter,
                    weight: entry_weight,
                    i,
                    merged: false,
                });
                subgraph_results.insert(child_sg_id.clone(), child_result);
            }
        }
    }

    // Resolve conflicts with the constraint graph.
    // dagre: `let entries = resolveConflicts(barycenters, cg)`
    let mut resolved = resolve_conflicts(&entries, constraint_graph);

    // Expand subgraphs: replace subgraph placeholders with their sorted children.
    // dagre: `expandSubgraphs(entries, subgraphs)`
    expand_subgraphs(&mut resolved, &subgraph_results, &sentinel_map);

    // Sort entries by barycenter.
    // dagre: `let result = sort(entries, biasRight)`
    let sort_result = sort_resolved_entries(resolved, bias_right);

    // Pin border nodes.
    // dagre: if (bl) { result.vs = [bl, result.vs, br].flat(true); ... }
    let mut vs = Vec::new();
    let mut result_barycenter = sort_result.barycenter;
    let mut result_weight = sort_result.weight;

    if let Some(bl) = border_left {
        vs.push(bl);
    }
    vs.extend_from_slice(&sort_result.vs);
    if let Some(br) = border_right {
        vs.push(br);
    }

    // If this subgraph has border nodes with predecessors on the fixed layer,
    // incorporate their positions into the aggregate barycenter.
    // dagre: border predecessor handling
    if border_left.is_some() {
        let bl = border_left.unwrap();
        let br = border_right.unwrap();

        // Get predecessors of border nodes on the fixed layer.
        let bl_pred_order = get_neighbor_order(graph, bl, direction, fixed_order);
        let br_pred_order = get_neighbor_order(graph, br, direction, fixed_order);

        if let (Some(bl_order), Some(br_order)) = (bl_pred_order, br_pred_order) {
            if result_barycenter.is_none() {
                result_barycenter = Some(0.0);
                result_weight = 0.0;
            }
            let bc = result_barycenter.unwrap();
            result_barycenter =
                Some((bc * result_weight + bl_order + br_order) / (result_weight + 2.0));
            result_weight += 2.0;
        }
    }

    SortResult {
        vs,
        barycenter: result_barycenter,
        weight: result_weight,
    }
}

// ---------------------------------------------------------------------------
// Barycenter computation — matches dagre's `barycenter.js`
// ---------------------------------------------------------------------------

/// Compute the barycenter of a node from its neighbors on the fixed layer.
/// Uses edge weights (dagre aggregates them in the layer graph; we sum directly).
fn compute_barycenter(
    graph: &DiGraph<NodeData, EdgeData>,
    node: NodeIndex,
    fixed_order: &HashMap<NodeIndex, usize>,
    direction: petgraph::Direction,
) -> (Option<f64>, f64) {
    let mut sum = 0.0;
    let mut weight = 0.0;

    // dagre uses g.inEdges(v) and looks up edge weight + neighbor order.
    // We iterate over directed neighbors.
    let edges: Vec<_> = graph.edges_directed(node, direction).collect();
    for edge_ref in &edges {
        let neighbor = if direction == petgraph::Direction::Incoming {
            edge_ref.source()
        } else {
            edge_ref.target()
        };
        if let Some(&order) = fixed_order.get(&neighbor) {
            let edge_weight = edge_ref.weight().weight.max(1) as f64;
            sum += edge_weight * order as f64;
            weight += edge_weight;
        }
    }

    if weight > 0.0 {
        (Some(sum / weight), weight)
    } else {
        (None, 0.0)
    }
}

/// Get the order of the first neighbor of a node on the fixed layer.
/// Used for border node predecessor lookups.
fn get_neighbor_order(
    graph: &DiGraph<NodeData, EdgeData>,
    node: NodeIndex,
    direction: petgraph::Direction,
    fixed_order: &HashMap<NodeIndex, usize>,
) -> Option<f64> {
    for neighbor in graph.neighbors_directed(node, direction) {
        if let Some(&order) = fixed_order.get(&neighbor) {
            return Some(order as f64);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Conflict resolution — matches dagre's `resolve-conflicts.js`
// ---------------------------------------------------------------------------

/// Resolve conflicts between entries and the constraint graph using
/// Forster's algorithm. Returns a new list of (possibly merged) entries.
///
/// If the constraint graph says "A must come before B" but B has a
/// barycenter <= A's barycenter, merge them into a single entry.
fn resolve_conflicts(entries: &[SortEntry], constraint_graph: &ConstraintGraph) -> Vec<SortEntry> {
    if entries.is_empty() {
        return Vec::new();
    }

    // Build a map from entry v_id to index.
    let mut id_to_idx: HashMap<&str, usize> = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        id_to_idx.insert(&entry.v_id, i);
    }

    // Build the constraint sub-graph over these entries.
    // indegree, in-list, out-list for each entry.
    let n = entries.len();
    let mut indegree = vec![0usize; n];
    let mut in_edges: Vec<Vec<usize>> = vec![Vec::new(); n]; // predecessors
    let mut out_edges: Vec<Vec<usize>> = vec![Vec::new(); n]; // successors

    for (from, to) in constraint_graph.all_edges() {
        if let (Some(&from_idx), Some(&to_idx)) =
            (id_to_idx.get(from.as_str()), id_to_idx.get(to.as_str()))
        {
            indegree[to_idx] += 1;
            out_edges[from_idx].push(to_idx);
            in_edges[to_idx].push(from_idx);
        }
    }

    // Topological sort using source set (matching dagre's doResolveConflicts).
    let mut source_set: Vec<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();

    // Clone entries into mutable working copies.
    let mut work: Vec<SortEntry> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| SortEntry {
            vs: e.vs.clone(),
            v_id: e.v_id.clone(),
            barycenter: e.barycenter,
            weight: e.weight,
            i,
            merged: false,
        })
        .collect();

    let mut result_order: Vec<usize> = Vec::new();

    while let Some(v_idx) = source_set.pop() {
        result_order.push(v_idx);

        // Handle "in" predecessors (constraint predecessors).
        // dagre: `entry["in"].reverse().forEach(handleIn(entry))`
        let in_list: Vec<usize> = in_edges[v_idx].clone();
        for &u_idx in in_list.iter().rev() {
            if work[u_idx].merged {
                continue;
            }
            // Merge condition: if u has no barycenter, or v has no barycenter,
            // or u's barycenter >= v's barycenter.
            let should_merge = work[u_idx].barycenter.is_none()
                || work[v_idx].barycenter.is_none()
                || work[u_idx].barycenter.unwrap() >= work[v_idx].barycenter.unwrap();

            if should_merge {
                merge_entries(&mut work, v_idx, u_idx);
            }
        }

        // Handle "out" successors: decrement indegree, add to source set if 0.
        // dagre: `entry.out.forEach(handleOut(entry))`
        let out_list: Vec<usize> = out_edges[v_idx].clone();
        for &w_idx in &out_list {
            in_edges[w_idx].push(v_idx);
            indegree[w_idx] -= 1;
            if indegree[w_idx] == 0 {
                source_set.push(w_idx);
            }
        }
    }

    // Filter out merged entries and return in processing order.
    let indices: Vec<usize> = result_order
        .into_iter()
        .filter(|&i| !work[i].merged)
        .collect();

    indices
        .into_iter()
        .map(|i| SortEntry {
            vs: std::mem::take(&mut work[i].vs),
            v_id: work[i].v_id.clone(),
            barycenter: work[i].barycenter,
            weight: work[i].weight,
            i: work[i].i,
            merged: false,
        })
        .collect()
}

/// Merge source entry into target entry.
/// dagre's `mergeEntries(target, source)`.
fn merge_entries(work: &mut [SortEntry], target_idx: usize, source_idx: usize) {
    let mut sum = 0.0f64;
    let mut weight = 0.0f64;

    if work[target_idx].weight > 0.0 {
        sum += work[target_idx].barycenter.unwrap_or(0.0) * work[target_idx].weight;
        weight += work[target_idx].weight;
    }
    if work[source_idx].weight > 0.0 {
        sum += work[source_idx].barycenter.unwrap_or(0.0) * work[source_idx].weight;
        weight += work[source_idx].weight;
    }

    // dagre: `target.vs = source.vs.concat(target.vs)` — prepend source
    let source_vs: Vec<NodeIndex> = std::mem::take(&mut work[source_idx].vs);
    let target_vs = std::mem::take(&mut work[target_idx].vs);
    let mut new_vs = source_vs;
    new_vs.extend(target_vs);
    work[target_idx].vs = new_vs;

    if weight > 0.0 {
        work[target_idx].barycenter = Some(sum / weight);
        work[target_idx].weight = weight;
    }
    work[target_idx].i = work[target_idx].i.min(work[source_idx].i);
    work[source_idx].merged = true;
}

// ---------------------------------------------------------------------------
// Expand subgraphs — matches dagre's expandSubgraphs
// ---------------------------------------------------------------------------

/// Replace subgraph placeholder entries with their recursively sorted children.
///
/// Matches dagre's `expandSubgraphs(entries, subgraphs)`:
/// ```js
/// entry.vs = entry.vs.flatMap(v => subgraphs[v] ? subgraphs[v].vs : v);
/// ```
///
/// In dagre, entry.vs contains string node IDs. Each element is either a base
/// node ID or a subgraph ID. If it's a subgraph ID found in the subgraphs map,
/// it gets replaced with that subgraph's sorted children (NodeIndex values).
///
/// In our implementation, subgraph entries use sentinel NodeIndex values
/// (stored in sentinel_map) that can be resolved to subgraph IDs.
fn expand_subgraphs(
    entries: &mut [SortEntry],
    subgraph_results: &HashMap<String, SortResult>,
    sentinel_map: &HashMap<NodeIndex, String>,
) {
    for entry in entries.iter_mut() {
        let needs_expansion = entry.vs.iter().any(|ni| sentinel_map.contains_key(ni));
        if !needs_expansion {
            continue;
        }
        let mut new_vs = Vec::new();
        for &ni in &entry.vs {
            if let Some(sg_id) = sentinel_map.get(&ni) {
                if let Some(result) = subgraph_results.get(sg_id) {
                    new_vs.extend_from_slice(&result.vs);
                }
                // If no result (empty subgraph), just skip the sentinel
            } else {
                new_vs.push(ni);
            }
        }
        entry.vs = new_vs;
    }
}

// ---------------------------------------------------------------------------
// Sort entries — matches dagre's `sort.js`
// ---------------------------------------------------------------------------

/// Sort resolved entries by barycenter, interleaving unsortable entries.
fn sort_resolved_entries(entries: Vec<SortEntry>, bias_right: bool) -> SortResult {
    // Partition into sortable (has barycenter) and unsortable.
    let mut sortable: Vec<SortEntry> = Vec::new();
    let mut unsortable: Vec<SortEntry> = Vec::new();

    for entry in entries {
        if entry.barycenter.is_some() {
            sortable.push(entry);
        } else {
            unsortable.push(entry);
        }
    }

    // Sort unsortable by descending i (so pop() yields smallest i first).
    // dagre: `unsortable = parts.rhs.sort((a, b) => b.i - a.i)`
    unsortable.sort_by(|a, b| b.i.cmp(&a.i));

    // Sort sortable by barycenter with biased tie-breaking.
    // dagre: `sortable.sort(compareWithBias(!!biasRight))`
    sortable.sort_by(|a, b| {
        let ba = a.barycenter.unwrap();
        let bb = b.barycenter.unwrap();
        match ba.partial_cmp(&bb) {
            Some(std::cmp::Ordering::Equal) | None => {
                if bias_right {
                    b.i.cmp(&a.i)
                } else {
                    a.i.cmp(&b.i)
                }
            }
            Some(ord) => ord,
        }
    });

    // Interleave: consume unsortable entries at their original positions.
    // dagre: consumeUnsortable between each sortable entry.
    let mut vs: Vec<NodeIndex> = Vec::new();
    let mut sum = 0.0f64;
    let mut weight = 0.0f64;
    let mut vs_index = 0usize;

    // consume unsortable entries whose original index <= current position.
    // dagre: index++ (increment by 1 per consumed entry, NOT by vs.len()).
    fn consume_unsortable(
        vs: &mut Vec<NodeIndex>,
        unsortable: &mut Vec<SortEntry>,
        vs_index: &mut usize,
    ) {
        while let Some(last) = unsortable.last() {
            if last.i <= *vs_index {
                let entry = unsortable.pop().unwrap();
                *vs_index += 1; // dagre: index++
                vs.extend(entry.vs);
            } else {
                break;
            }
        }
    }

    consume_unsortable(&mut vs, &mut unsortable, &mut vs_index);

    for entry in sortable {
        vs_index += entry.vs.len();
        sum += entry.barycenter.unwrap() * entry.weight;
        weight += entry.weight;
        vs.extend(entry.vs);
        consume_unsortable(&mut vs, &mut unsortable, &mut vs_index);
    }

    // Any remaining unsortable
    while let Some(entry) = unsortable.pop() {
        vs.extend(entry.vs);
    }

    SortResult {
        vs,
        barycenter: if weight > 0.0 {
            Some(sum / weight)
        } else {
            None
        },
        weight,
    }
}

// ---------------------------------------------------------------------------
// Subgraph constraint recording — matches dagre's `addSubgraphConstraints`
// ---------------------------------------------------------------------------

/// Record the relative order of subgraphs as constraints for subsequent layers.
///
/// Matches dagre's `add-subgraph-constraints.js` exactly: walks each node's
/// ancestry chain, tracking the previous child at each parent level. When two
/// different children are found at the same level, adds a constraint and stops.
fn add_subgraph_constraints(
    parent_map: &HashMap<NodeIndex, Option<String>>,
    sg_parent_map: &HashMap<String, Option<String>>,
    sorted_layer: &[NodeIndex],
    constraint_graph: &mut ConstraintGraph,
) {
    // Track the previous child seen at each parent level.
    // Key: parent (None = root). Value: child ID at that level.
    let mut prev_at_parent: HashMap<Option<String>, String> = HashMap::new();

    for &ni in sorted_layer {
        // Get this node's immediate parent.
        let child_parent = parent_map.get(&ni).cloned().flatten();

        // child = the node's immediate parent subgraph (or None for root-level nodes).
        // We walk up from the node's parent, tracking "child" at each level.
        // dagre walks: let child = g.parent(v), parent, prevChild;
        //              while (child) { parent = g.parent(child); ... child = parent; }
        //
        // But for root-level nodes (parent = None), there's nothing to walk.
        // For subgraph members, child starts as the node's immediate parent.

        let mut child: Option<String> = child_parent.clone();

        loop {
            match &child {
                None => {
                    // We've reached the root level. Nothing to constrain.
                    break;
                }
                Some(child_id) => {
                    // parent = g.parent(child)
                    let parent = sg_parent_map.get(child_id).cloned().flatten();

                    let prev_child = prev_at_parent.get(&parent).cloned();
                    prev_at_parent.insert(parent.clone(), child_id.clone());

                    if let Some(prev) = prev_child {
                        if prev != *child_id {
                            // Different child at this level — record constraint and return.
                            constraint_graph.add_edge(&prev, child_id);
                            break;
                        }
                    }

                    // Walk up.
                    child = parent;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Build a map from NodeIndex to its immediate parent subgraph ID.
/// Matches dagre's `g.parent(v)` for compound graphs.
///
/// In dagre, border nodes are children of their subgraph in the compound graph.
/// Since our border nodes aren't in the membership map, we derive their parent
/// from the BorderSegments data structure.
fn build_parent_map(
    graph: &DiGraph<NodeData, EdgeData>,
    membership: &SubgraphMembership,
    border_segments: &BorderSegments,
) -> HashMap<NodeIndex, Option<String>> {
    // Build reverse map: NodeIndex → subgraph ID for all border nodes.
    let mut border_parent: HashMap<NodeIndex, String> = HashMap::new();
    for (sg_id, borders) in &border_segments.subgraphs {
        for &ni in borders.border_left.values() {
            border_parent.insert(ni, sg_id.clone());
        }
        for &ni in borders.border_right.values() {
            border_parent.insert(ni, sg_id.clone());
        }
    }

    let mut result = HashMap::new();
    for ni in graph.node_indices() {
        let id = &graph[ni].id;
        if let Some(sg_id) = border_parent.get(&ni) {
            result.insert(ni, Some(sg_id.clone()));
        } else {
            let parent = membership.get(id).and_then(|path| path.last().cloned());
            result.insert(ni, parent);
        }
    }
    result
}

/// Build a map from subgraph ID to its parent subgraph ID.
/// E.g., if a node has membership path ["A", "B", "C"], then:
///   "C" -> Some("B"), "B" -> Some("A"), "A" -> None
fn build_sg_parent_map(membership: &SubgraphMembership) -> HashMap<String, Option<String>> {
    let mut result: HashMap<String, Option<String>> = HashMap::new();

    for (_node_id, path) in membership.iter() {
        for (i, sg_id) in path.iter().enumerate() {
            if !result.contains_key(sg_id) {
                let parent = if i > 0 {
                    Some(path[i - 1].clone())
                } else {
                    None
                };
                result.insert(sg_id.clone(), parent);
            }
        }
    }

    result
}

/// Build a map from NodeIndex to its position (order) within the layer.
fn build_order_map(layer: &[NodeIndex]) -> HashMap<NodeIndex, usize> {
    layer
        .iter()
        .enumerate()
        .map(|(pos, &node)| (node, pos))
        .collect()
}

/// Get border nodes for a subgraph at a specific layer (rank).
/// Returns (border_left, border_right) if they exist in the layer.
fn get_border_nodes_for_layer(
    sg_id: Option<&str>,
    layer: &[NodeIndex],
    border_segments: &BorderSegments,
) -> (Option<NodeIndex>, Option<NodeIndex>) {
    let sg = match sg_id {
        Some(sg) => sg,
        None => return (None, None),
    };

    let borders = match border_segments.subgraphs.get(sg) {
        Some(b) => b,
        None => return (None, None),
    };

    let layer_set: HashSet<NodeIndex> = layer.iter().copied().collect();

    let bl = borders
        .border_left
        .values()
        .find(|&&ni| layer_set.contains(&ni))
        .copied();
    let br = borders
        .border_right
        .values()
        .find(|&&ni| layer_set.contains(&ni))
        .copied();

    (bl, br)
}

/// Find child subgraph IDs that have members in this layer and whose
/// parent is the given sg_id.
///
/// A child subgraph is one where:
/// - Its parent in the subgraph hierarchy is sg_id (or root if sg_id is None)
/// - It has at least one member node (or border node) in this layer
fn find_child_subgraphs_in_layer(
    layer: &[NodeIndex],
    parent_sg_id: Option<&str>,
    parent_map: &HashMap<NodeIndex, Option<String>>,
    sg_parent_map: &HashMap<String, Option<String>>,
    border_segments: &BorderSegments,
) -> Vec<String> {
    // Find all subgraph IDs whose parent is parent_sg_id.
    let child_sg_ids: HashSet<&String> = sg_parent_map
        .iter()
        .filter(|(_sg_id, parent)| parent.as_deref() == parent_sg_id)
        .map(|(sg_id, _)| sg_id)
        .collect();

    // Check which of these have members in this layer.
    let mut result: HashSet<String> = HashSet::new();

    for &ni in layer {
        let node_parent = parent_map.get(&ni).and_then(|p| p.as_deref());

        // If the node's immediate parent is one of our child subgraphs,
        // that child subgraph is present in this layer.
        if let Some(np) = node_parent {
            if child_sg_ids.contains(&np.to_string()) {
                result.insert(np.to_string());
                continue;
            }
            // Check if the node's parent is a descendant of one of our child subgraphs.
            // Walk up until we find one of our child_sg_ids or reach parent_sg_id.
            let mut sg = Some(np.to_string());
            while let Some(ref sg_id) = sg {
                if child_sg_ids.contains(sg_id) {
                    result.insert(sg_id.clone());
                    break;
                }
                sg = sg_parent_map.get(sg_id).cloned().flatten();
                if sg.as_deref() == parent_sg_id {
                    break;
                }
            }
        }
    }

    // Also check border segments — subgraphs with border nodes in this layer.
    let layer_set: HashSet<NodeIndex> = layer.iter().copied().collect();
    for sg_id in &child_sg_ids {
        if result.contains(sg_id.as_str()) {
            continue;
        }
        if let Some(borders) = border_segments.subgraphs.get(sg_id.as_str()) {
            let has_border_in_layer = borders
                .border_left
                .values()
                .chain(borders.border_right.values())
                .any(|ni| layer_set.contains(ni));
            if has_border_in_layer {
                result.insert((*sg_id).clone());
            }
        }
    }

    result.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Crossing count
// ---------------------------------------------------------------------------

/// Fenwick tree (Binary Indexed Tree) for O(log n) prefix-sum queries.
/// Stores weighted sums instead of counts.
struct FenwickTree {
    tree: Vec<u64>,
}

impl FenwickTree {
    fn new(size: usize) -> Self {
        FenwickTree {
            tree: vec![0; size + 1],
        }
    }

    fn update(&mut self, mut i: usize, weight: u64) {
        i += 1;
        while i < self.tree.len() {
            self.tree[i] += weight;
            i += i & i.wrapping_neg();
        }
    }

    fn prefix_sum(&self, mut i: usize) -> u64 {
        i += 1;
        let mut sum = 0u64;
        while i > 0 {
            sum += self.tree[i];
            i -= i & i.wrapping_neg();
        }
        sum
    }
}

/// Count weighted edge crossings between two adjacent layers.
/// Matches dagre's `twoLayerCrossCount` which uses `entry.weight * weightSum`.
fn count_bilayer_crossings(
    graph: &DiGraph<NodeData, EdgeData>,
    north_layer: &[NodeIndex],
    south_layer: &[NodeIndex],
) -> u64 {
    let south_pos: HashMap<NodeIndex, usize> = south_layer
        .iter()
        .enumerate()
        .map(|(pos, &node)| (node, pos))
        .collect();

    // Collect (south_position, edge_weight) for each edge, ordered by north position
    // then south position (ascending).
    let mut south_entries: Vec<(usize, u64)> = Vec::new();
    for &north_node in north_layer {
        let mut targets: Vec<(usize, u64)> = graph
            .edges_directed(north_node, petgraph::Direction::Outgoing)
            .filter_map(|edge_ref| {
                south_pos.get(&edge_ref.target()).map(|&pos| {
                    let weight = edge_ref.weight().weight.max(1) as u64;
                    (pos, weight)
                })
            })
            .collect();
        targets.sort_by_key(|&(pos, _)| pos);
        south_entries.extend(targets);
    }

    let south_size = south_layer.len();
    if south_size == 0 {
        return 0;
    }
    let mut tree = FenwickTree::new(south_size);
    let mut crossings: u64 = 0;
    let mut total_weight: u64 = 0;

    for &(pos, weight) in &south_entries {
        // Weighted crossings: weight * (total weight inserted so far - prefix sum up to pos)
        crossings += weight * (total_weight - tree.prefix_sum(pos));
        tree.update(pos, weight);
        total_weight += weight;
    }

    crossings
}

fn count_total_crossings(graph: &DiGraph<NodeData, EdgeData>, layers: &[Vec<NodeIndex>]) -> u64 {
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
