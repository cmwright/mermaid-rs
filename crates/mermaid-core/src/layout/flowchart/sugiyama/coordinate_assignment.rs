use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::flowchart::Direction;
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;

/// Brandes-Köpf coordinate assignment.
/// - Main axis: accumulate max_thickness_in_rank + RANK_SEP
/// - Cross axis: 4-pass block alignment + horizontal compaction + balance
/// - Extra gap at subgraph boundaries
/// - Direction handling (TB/BT/LR/RL)
pub fn assign_coordinates(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    direction: Direction,
    membership: &SubgraphMembership,
    rank_sep: f64,
) -> HashMap<NodeIndex, (f64, f64)> {
    let is_horizontal = matches!(direction, Direction::LeftToRight | Direction::RightToLeft);
    let empty_path: Vec<String> = Vec::new();
    let total_nodes: usize = layers.iter().map(|l| l.len()).sum();

    // ── Main-axis placement ──
    let mut main_pos: HashMap<NodeIndex, f64> = HashMap::with_capacity(total_nodes);
    let mut rank_offset = 0.0;
    for layer in layers {
        let max_thick = layer
            .iter()
            .map(|&i| {
                if is_horizontal {
                    graph[i].width
                } else {
                    graph[i].height
                }
            })
            .fold(0.0f64, f64::max);
        for &idx in layer {
            main_pos.insert(idx, rank_offset + max_thick / 2.0);
        }
        rank_offset += max_thick + rank_sep;
    }

    // ── Cross-axis: Brandes-Köpf 4-pass ──
    let cross_pos = brandes_kopf(graph, layers, is_horizontal, membership, &empty_path);

    // ── Combine ──
    let mut positions: HashMap<NodeIndex, (f64, f64)> = HashMap::with_capacity(total_nodes);
    for &idx in layers.iter().flat_map(|l| l.iter()) {
        let m = main_pos[&idx];
        let c = cross_pos.get(&idx).copied().unwrap_or(0.0);
        positions.insert(idx, if is_horizontal { (m, c) } else { (c, m) });
    }

    // ── BT / RL mirror ──
    if matches!(direction, Direction::BottomToTop | Direction::RightToLeft) {
        let max_coord = positions
            .values()
            .map(|&(x, y)| if is_horizontal { x } else { y })
            .fold(0.0f64, f64::max)
            + graph
                .node_indices()
                .filter_map(|ni| {
                    positions.get(&ni).map(|_| {
                        if is_horizontal {
                            graph[ni].width
                        } else {
                            graph[ni].height
                        }
                    })
                })
                .fold(0.0f64, f64::max);

        for pos in positions.values_mut() {
            if is_horizontal {
                pos.0 = max_coord - pos.0;
            } else {
                pos.1 = max_coord - pos.1;
            }
        }
    }

    positions
}

// ─────────────────────────────────────────────────────────────
// Brandes-Köpf internals
// ─────────────────────────────────────────────────────────────

/// Run 4 alignment passes (up-left, up-right, down-left, down-right)
/// and balance by averaging the middle two values per node.
fn brandes_kopf(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    is_horizontal: bool,
    membership: &SubgraphMembership,
    empty_path: &[String],
) -> HashMap<NodeIndex, f64> {
    let mut xss: Vec<HashMap<NodeIndex, f64>> = Vec::with_capacity(4);

    // Pre-compute reversed layers once (instead of cloning in the loop)
    let reversed_layers: Vec<Vec<NodeIndex>> = layers.iter().rev().cloned().collect();

    // Detect type-1 and type-2 conflicts once (same for all 4 passes).
    // Type-2 conflicts: edges that would cross a border node boundary.
    let conflicts = find_type2_conflicts(graph, layers);

    for vert in 0..2u8 {
        // vert=0: up (top-to-bottom layers, predecessors)
        // vert=1: down (bottom-to-top layers, successors)
        let base_layers = if vert == 0 { layers } else { &reversed_layers };

        for horiz in 0..2u8 {
            // horiz=0: left (normal order), horiz=1: right (reversed)
            let reversed_within: Vec<Vec<NodeIndex>>;
            let final_layers: &[Vec<NodeIndex>] = if horiz == 0 {
                base_layers
            } else {
                reversed_within = base_layers
                    .iter()
                    .map(|l| l.iter().rev().copied().collect())
                    .collect();
                &reversed_within
            };

            let use_preds = vert == 0;
            let reverse_sep = horiz == 1;
            let root = vertical_alignment(graph, final_layers, use_preds, &conflicts);

            let mut xs = horizontal_compaction(
                graph,
                final_layers,
                &root,
                is_horizontal,
                membership,
                empty_path,
                reverse_sep,
            );

            // For right-biased, negate coordinates
            if horiz == 1 {
                for v in xs.values_mut() {
                    *v = -*v;
                }
            }

            xss.push(xs);
        }
    }

    // Align all results to the smallest-width alignment
    let smallest = find_smallest_width(&xss, graph, is_horizontal);
    align_to_smallest(&mut xss, smallest, graph, is_horizontal);

    // Balance: average middle two of 4 values per node
    let mut result: HashMap<NodeIndex, f64> = HashMap::new();
    for &idx in layers.iter().flat_map(|l| l.iter()) {
        let mut vals: Vec<f64> = xss.iter().filter_map(|xs| xs.get(&idx).copied()).collect();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let balanced = match vals.len() {
            4 => (vals[1] + vals[2]) / 2.0,
            3 => vals[1],
            2 => (vals[0] + vals[1]) / 2.0,
            1 => vals[0],
            _ => 0.0,
        };
        result.insert(idx, balanced);
    }

    result
}

// ─────────────────────────────────────────────────────────────
// Type-2 conflict detection — matches dagre's findType2Conflicts
// ─────────────────────────────────────────────────────────────

/// A conflict between two nodes: alignment between them should be avoided.
/// Key: (min_index, max_index) ordered pair.
type ConflictSet = HashSet<(NodeIndex, NodeIndex)>;

fn conflict_key(a: NodeIndex, b: NodeIndex) -> (NodeIndex, NodeIndex) {
    if a.index() < b.index() {
        (a, b)
    } else {
        (b, a)
    }
}

fn has_conflict(conflicts: &ConflictSet, a: NodeIndex, b: NodeIndex) -> bool {
    conflicts.contains(&conflict_key(a, b))
}

/// Detect type-2 conflicts: edges that cross border node boundaries.
///
/// Border nodes partition each layer into segments. An inner-segment edge
/// (dummy→dummy) that crosses a border node's predecessor position creates
/// a type-2 conflict. These edges should not be aligned vertically.
///
/// Matches dagre's `findType2Conflicts` exactly: border nodes without
/// predecessors at the north layer are skipped (their boundaries are not
/// used for conflict detection). This prevents false conflicts at subgraph
/// boundaries where border chains start/end.
fn find_type2_conflicts(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
) -> ConflictSet {
    let mut conflicts = ConflictSet::new();

    // Build position map for each layer
    let pos_maps: Vec<HashMap<NodeIndex, usize>> = layers
        .iter()
        .map(|layer| layer.iter().enumerate().map(|(i, &n)| (n, i)).collect())
        .collect();

    // For each pair of adjacent layers (north, south), find type-2 conflicts.
    // Structure matches dagre's visitLayer: scan border-bounded segments,
    // skipping borders that have no predecessor at the north layer.
    for li in 1..layers.len() {
        let north = &layers[li - 1];
        let south = &layers[li];
        let north_pos = &pos_maps[li - 1];

        let mut prev_north_pos: i64 = -1;
        // next_north_pos tracks the last border predecessor position found.
        // None until we encounter a border with a predecessor (matches dagre's
        // `nextNorthPos` starting as `undefined`).
        let mut next_north_pos: Option<i64> = None;
        let mut south_pos: usize = 0;

        for (south_lookahead, &south_node) in south.iter().enumerate() {
            if graph[south_node].id.starts_with("__border_") {
                // Border node: find its predecessor at the north layer.
                let pred_order = graph
                    .neighbors_directed(south_node, petgraph::Direction::Incoming)
                    .filter_map(|n| north_pos.get(&n).copied())
                    .next();

                if let Some(npp) = pred_order {
                    // Border has a predecessor — scan the segment up to this border.
                    let npp = npp as i64;
                    scan_type2_segment(
                        south,
                        south_pos,
                        south_lookahead,
                        prev_north_pos,
                        npp,
                        graph,
                        north_pos,
                        &mut conflicts,
                    );
                    south_pos = south_lookahead;
                    prev_north_pos = npp;
                    next_north_pos = Some(npp);
                }
                // If no predecessor, skip this border entirely (dagre behavior:
                // the `if (predecessors.length)` block is not entered).
            }

            // Unconditional scan for remaining segment after the last processed
            // border. dagre: scan(south, southPos, south.length, nextNorthPos,
            // north.length). When next_north_pos is None (dagre: undefined),
            // the scan is a no-op since `x < undefined` is false in JS.
            if let Some(nnp) = next_north_pos {
                scan_type2_segment(
                    south,
                    south_pos,
                    south.len(),
                    nnp,
                    north.len() as i64,
                    graph,
                    north_pos,
                    &mut conflicts,
                );
            }
        }
    }

    conflicts
}

/// Scan a segment of the south layer for type-2 conflicts.
///
/// Checks each dummy node in `south[south_pos..south_end]` for dummy
/// predecessors whose position falls outside `[prev_north_border,
/// next_north_border]`.
fn scan_type2_segment(
    south: &[NodeIndex],
    south_pos: usize,
    south_end: usize,
    prev_north_border: i64,
    next_north_border: i64,
    graph: &DiGraph<NodeData, EdgeData>,
    north_pos: &HashMap<NodeIndex, usize>,
    conflicts: &mut ConflictSet,
) {
    for i in south_pos..south_end {
        let v = south[i];
        if !graph[v].id.starts_with("__dummy_") {
            continue;
        }
        for u in graph.neighbors_directed(v, petgraph::Direction::Incoming) {
            if let Some(&u_pos) = north_pos.get(&u) {
                if graph[u].id.starts_with("__dummy_") {
                    let up = u_pos as i64;
                    if up < prev_north_border || up > next_north_border {
                        conflicts.insert(conflict_key(v, u));
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────
// Vertical alignment — with conflict awareness
// ─────────────────────────────────────────────────────────────

/// Form vertical blocks by aligning each node with its median neighbor
/// in the adjacent layer. Skips edges that have type-2 conflicts.
/// Returns root map (node → block root).
fn vertical_alignment(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    use_predecessors: bool,
    conflicts: &ConflictSet,
) -> HashMap<NodeIndex, NodeIndex> {
    let total: usize = layers.iter().map(|l| l.len()).sum();
    let mut root: HashMap<NodeIndex, NodeIndex> = HashMap::with_capacity(total);
    let mut align: HashMap<NodeIndex, NodeIndex> = HashMap::with_capacity(total);
    let mut pos: HashMap<NodeIndex, usize> = HashMap::with_capacity(total);

    for layer in layers {
        for (order, &v) in layer.iter().enumerate() {
            root.insert(v, v);
            align.insert(v, v);
            pos.insert(v, order);
        }
    }

    let dir = if use_predecessors {
        petgraph::Direction::Incoming
    } else {
        petgraph::Direction::Outgoing
    };

    for li in 1..layers.len() {
        let prev_set: HashSet<NodeIndex> = layers[li - 1].iter().copied().collect();
        let mut prev_idx: i64 = -1;

        for &v in &layers[li] {
            // Neighbors in the immediately adjacent layer only
            let mut ws: Vec<NodeIndex> = graph
                .neighbors_directed(v, dir)
                .filter(|n| prev_set.contains(n))
                .collect();

            if ws.is_empty() {
                continue;
            }

            ws.sort_by_key(|n| pos.get(n).copied().unwrap_or(0));

            // Median neighbor(s)
            let mp = (ws.len() as f64 - 1.0) / 2.0;
            let lo = mp.floor() as usize;
            let hi = mp.ceil() as usize;

            for &w in ws.iter().take(hi + 1).skip(lo) {
                let w_pos = pos.get(&w).copied().unwrap_or(0) as i64;
                // Skip alignment if there's a type-2 conflict between v and w
                if align[&v] == v && prev_idx < w_pos && !has_conflict(conflicts, v, w) {
                    // Form block: w → v, and v joins w's block
                    align.insert(w, v);
                    let rw = root[&w];
                    root.insert(v, rw);
                    align.insert(v, rw);
                    prev_idx = w_pos;
                }
            }
        }
    }

    root
}

/// Assign cross-axis coordinates via block-graph compaction.
/// Two passes: left-to-right placement, then right-to-left compaction.
///
/// `reverse_sep`: when true (right-biased pass), border-left nodes are pinned;
/// when false (left-biased pass), border-right nodes are pinned.
/// This matches dagre's pass2 border-type-aware compaction.
fn horizontal_compaction(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    root: &HashMap<NodeIndex, NodeIndex>,
    is_horizontal: bool,
    membership: &SubgraphMembership,
    empty_path: &[String],
    reverse_sep: bool,
) -> HashMap<NodeIndex, f64> {
    // Build block graph: out_edges[from_root][to_root] = min_separation
    let mut out_edges: HashMap<NodeIndex, HashMap<NodeIndex, f64>> = HashMap::new();
    let mut block_set: HashSet<NodeIndex> = HashSet::new();

    for layer in layers {
        for &v in layer {
            block_set.insert(root[&v]);
        }
        for pair in layer.windows(2) {
            let u = pair[0];
            let v = pair[1];
            let ur = root[&u];
            let vr = root[&v];
            if ur != vr {
                let sep = node_separation(graph, u, v, is_horizontal, membership, empty_path);
                let w = out_edges.entry(ur).or_default().entry(vr).or_insert(0.0f64);
                *w = w.max(sep);
            }
        }
    }

    // In-edges (reverse of out_edges)
    let mut in_edges: HashMap<NodeIndex, HashMap<NodeIndex, f64>> = HashMap::new();
    for (&from, tos) in &out_edges {
        for (&to, &weight) in tos {
            in_edges.entry(to).or_default().insert(from, weight);
        }
    }

    // Topological sort (Kahn's algorithm)
    let mut in_degree: HashMap<NodeIndex, usize> = HashMap::new();
    for &b in &block_set {
        in_degree.insert(b, 0);
    }
    for tos in out_edges.values() {
        for &to in tos.keys() {
            *in_degree.entry(to).or_insert(0) += 1;
        }
    }

    let mut initial_queue: Vec<NodeIndex> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(&n, _)| n)
        .collect();
    initial_queue.sort_by_key(|n| n.index()); // deterministic order
    let mut queue: VecDeque<NodeIndex> = initial_queue.into_iter().collect();
    let mut topo: Vec<NodeIndex> = Vec::with_capacity(block_set.len());

    while let Some(n) = queue.pop_front() {
        topo.push(n);
        if let Some(tos) = out_edges.get(&n) {
            let mut sorted_tos: Vec<NodeIndex> = tos.keys().copied().collect();
            sorted_tos.sort_by_key(|n| n.index()); // deterministic order
            for to in sorted_tos {
                let d = in_degree.get_mut(&to).unwrap();
                *d -= 1;
                if *d == 0 {
                    queue.push_back(to);
                }
            }
        }
    }

    // Handle any blocks not reached by Kahn's (cycles)
    let topo_set: HashSet<NodeIndex> = topo.iter().copied().collect();
    let mut remaining: Vec<NodeIndex> = block_set
        .iter()
        .filter(|b| !topo_set.contains(b))
        .copied()
        .collect();
    remaining.sort_by_key(|n| n.index()); // deterministic order
    topo.extend(remaining);

    // Pass 1: left-to-right (smallest coordinates)
    let mut xs: HashMap<NodeIndex, f64> = HashMap::new();
    for &block in &topo {
        let x = in_edges
            .get(&block)
            .map(|preds| {
                preds
                    .iter()
                    .map(|(&p, &sep)| xs.get(&p).copied().unwrap_or(0.0) + sep)
                    .fold(0.0f64, f64::max)
            })
            .unwrap_or(0.0);
        xs.insert(block, x);
    }

    // Pass 2: right-to-left compaction with border-type awareness.
    // dagre's pass2 skips compaction for border nodes whose borderType matches
    // the current direction. In a left-biased pass (reverse_sep=false),
    // borderRight nodes are pinned (they resist leftward pull). In a right-biased
    // pass (reverse_sep=true), borderLeft nodes are pinned.
    //
    // Note: we only check for border nodes that are block roots to avoid
    // scanning all layers for each block. This is a pragmatic simplification.
    let pin_border_type = if reverse_sep {
        "__border_bl_"
    } else {
        "__border_br_"
    };

    // Pre-compute which blocks contain pinned border nodes.
    let pinned_blocks: HashSet<NodeIndex> = layers
        .iter()
        .flat_map(|layer| layer.iter())
        .filter(|&&v| graph[v].id.starts_with(pin_border_type))
        .map(|&v| root[&v])
        .collect();

    for &block in topo.iter().rev() {
        if pinned_blocks.contains(&block) {
            continue;
        }

        if let Some(succs) = out_edges.get(&block) {
            let min_succ = succs
                .iter()
                .map(|(&s, &sep)| xs.get(&s).copied().unwrap_or(f64::INFINITY) - sep)
                .fold(f64::INFINITY, f64::min);
            if min_succ.is_finite() {
                let cur = xs[&block];
                xs.insert(block, cur.max(min_succ));
            }
        }
    }

    // Map all nodes to their block root's coordinate
    let mut result: HashMap<NodeIndex, f64> = HashMap::new();
    for layer in layers {
        for &v in layer {
            result.insert(v, xs.get(&root[&v]).copied().unwrap_or(0.0));
        }
    }

    result
}

/// Minimum separation between adjacent nodes in the cross-axis.
/// Uses EDGE_SEP for dummy nodes (like dagre's edgesep) and NODE_SEP for real nodes.
fn node_separation(
    graph: &DiGraph<NodeData, EdgeData>,
    u: NodeIndex,
    v: NodeIndex,
    is_horizontal: bool,
    membership: &SubgraphMembership,
    empty_path: &[String],
) -> f64 {
    let un = &graph[u];
    let vn = &graph[v];

    let u_is_dummy = un.id.starts_with("__dummy_")
        || un.id.starts_with("__border_")
        || un.id.starts_with("__nesting_");
    let v_is_dummy = vn.id.starts_with("__dummy_")
        || vn.id.starts_with("__border_")
        || vn.id.starts_with("__nesting_");

    let u_size = if is_horizontal { un.height } else { un.width };
    let v_size = if is_horizontal { vn.height } else { vn.width };

    // Dagre uses edgesep for dummy nodes, nodesep for real nodes
    let u_sep = if u_is_dummy { EDGE_SEP } else { NODE_SEP };
    let v_sep = if v_is_dummy { EDGE_SEP } else { NODE_SEP };
    let base_sep = (u_sep + v_sep) / 2.0;

    // Add gap for subgraph borders between nodes in different subgraphs.
    // Count the actual number of subgraph borders crossed (each border adds
    // SUBGRAPH_PADDING for the space between the node and the border line).
    let gap = if u_is_dummy || v_is_dummy {
        0.0
    } else {
        let u_path = membership
            .get(&un.id)
            .map(|p| p.as_slice())
            .unwrap_or(empty_path);
        let v_path = membership
            .get(&vn.id)
            .map(|p| p.as_slice())
            .unwrap_or(empty_path);

        if u_path != v_path {
            let common = u_path
                .iter()
                .zip(v_path.iter())
                .take_while(|(a, b)| a == b)
                .count();
            // Total borders crossed = borders leaving u's subgraphs + borders
            // entering v's subgraphs, each relative to their common ancestor.
            let borders_crossed = (u_path.len() - common) + (v_path.len() - common);
            borders_crossed as f64 * SUBGRAPH_PADDING
        } else {
            0.0
        }
    };

    u_size / 2.0 + base_sep + gap + v_size / 2.0
}

/// Find the alignment with the smallest total width.
fn find_smallest_width(
    xss: &[HashMap<NodeIndex, f64>],
    graph: &DiGraph<NodeData, EdgeData>,
    is_horizontal: bool,
) -> usize {
    let mut best_idx = 0;
    let mut best_width = f64::INFINITY;

    for (i, xs) in xss.iter().enumerate() {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for (&n, &x) in xs {
            let half = if is_horizontal {
                graph[n].height
            } else {
                graph[n].width
            } / 2.0;
            lo = lo.min(x - half);
            hi = hi.max(x + half);
        }
        let w = hi - lo;
        if w < best_width {
            best_width = w;
            best_idx = i;
        }
    }

    best_idx
}

/// Shift each alignment so left-biased ones match the smallest's min
/// and right-biased ones match the smallest's max.
fn align_to_smallest(
    xss: &mut [HashMap<NodeIndex, f64>],
    smallest: usize,
    graph: &DiGraph<NodeData, EdgeData>,
    is_horizontal: bool,
) {
    let (align_min, align_max) = {
        let xs = &xss[smallest];
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for (&n, &x) in xs {
            let half = if is_horizontal {
                graph[n].height
            } else {
                graph[n].width
            } / 2.0;
            lo = lo.min(x - half);
            hi = hi.max(x + half);
        }
        (lo, hi)
    };

    for (i, xs) in xss.iter_mut().enumerate() {
        if i == smallest {
            continue;
        }

        let mut xs_lo = f64::INFINITY;
        let mut xs_hi = f64::NEG_INFINITY;
        for (&n, &x) in xs.iter() {
            let half = if is_horizontal {
                graph[n].height
            } else {
                graph[n].width
            } / 2.0;
            xs_lo = xs_lo.min(x - half);
            xs_hi = xs_hi.max(x + half);
        }

        // Even indices (0, 2) are left-biased; odd (1, 3) are right-biased
        let delta = if i % 2 == 0 {
            align_min - xs_lo
        } else {
            align_max - xs_hi
        };

        for v in xs.values_mut() {
            *v += delta;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{ArrowEnd, Direction, LineStyle, NodeShape};

    fn make_node(id: &str) -> NodeData {
        NodeData {
            id: id.to_string(),
            label: String::new(),
            shape: NodeShape::Rectangle,
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
    fn test_assign_coordinates_tb() {
        // A -> B, top-to-bottom
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge());

        let layers = vec![vec![a], vec![b]];
        let membership = SubgraphMembership::new();
        let positions = assign_coordinates(&g, &layers, Direction::TopToBottom, &membership, 50.0);

        let (ax, ay) = positions[&a];
        let (bx, by) = positions[&b];
        assert!(by > ay, "B should be below A in TB direction");
        assert!(
            (ax - bx).abs() < 1.0,
            "A and B should be vertically aligned"
        );
    }

    #[test]
    fn test_assign_coordinates_bt() {
        // A -> B, bottom-to-top: positions should be mirrored on y-axis
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge());

        let layers = vec![vec![a], vec![b]];
        let membership = SubgraphMembership::new();
        let positions = assign_coordinates(&g, &layers, Direction::BottomToTop, &membership, 50.0);

        let (ax, ay) = positions[&a];
        let (bx, by) = positions[&b];
        // In BT, the y-axis is mirrored: rank 0 (A) should have HIGHER y than rank 1 (B)
        assert!(
            ay > by,
            "In BT, A (rank 0) should have higher y than B (rank 1) (A.y={ay:.1}, B.y={by:.1})"
        );
        assert!(
            (ax - bx).abs() < 1.0,
            "A and B should be vertically aligned"
        );
    }

    #[test]
    fn test_assign_coordinates_lr() {
        // A -> B, left-to-right
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge());

        let layers = vec![vec![a], vec![b]];
        let membership = SubgraphMembership::new();
        let positions = assign_coordinates(&g, &layers, Direction::LeftToRight, &membership, 50.0);

        let (ax, ay) = positions[&a];
        let (bx, by) = positions[&b];
        assert!(bx > ax, "B should be to the right of A in LR direction");
        assert!(
            (ay - by).abs() < 1.0,
            "A and B should be horizontally aligned"
        );
    }

    #[test]
    fn test_assign_coordinates_rl() {
        // A -> B, right-to-left: x-axis mirrored
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge());

        let layers = vec![vec![a], vec![b]];
        let membership = SubgraphMembership::new();
        let positions = assign_coordinates(&g, &layers, Direction::RightToLeft, &membership, 50.0);

        let (ax, _ay) = positions[&a];
        let (bx, _by) = positions[&b];
        // In RL, rank 0 (A) should have HIGHER x than rank 1 (B)
        assert!(
            ax > bx,
            "In RL, A (rank 0) should have higher x than B (rank 1) (A.x={ax:.1}, B.x={bx:.1})"
        );
    }

    #[test]
    fn test_assign_coordinates_multi_node_layer() {
        // A -> C, B -> D: two nodes per layer
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, c, make_edge());
        g.add_edge(b, d, make_edge());

        let layers = vec![vec![a, b], vec![c, d]];
        let membership = SubgraphMembership::new();
        let positions = assign_coordinates(&g, &layers, Direction::TopToBottom, &membership, 50.0);

        // A and B should be on the same y (same rank), different x
        let (ax, ay) = positions[&a];
        let (bx, by) = positions[&b];
        assert!((ay - by).abs() < 0.1, "same rank nodes should have same y");
        assert!(
            (ax - bx).abs() > 10.0,
            "same rank nodes should have different x"
        );
    }

    #[test]
    fn test_assign_coordinates_with_subgraph_gap() {
        // Two nodes in different subgraphs should have extra separation
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        // No edges — just two nodes in same layer

        let layers = vec![vec![a, b]];

        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG1".to_string()]);
        membership.insert("B".to_string(), vec!["SG2".to_string()]);

        let positions_with_sg =
            assign_coordinates(&g, &layers, Direction::TopToBottom, &membership, 50.0);

        let empty_membership = SubgraphMembership::new();
        let positions_no_sg =
            assign_coordinates(&g, &layers, Direction::TopToBottom, &empty_membership, 50.0);

        let sep_with = (positions_with_sg[&a].0 - positions_with_sg[&b].0).abs();
        let sep_without = (positions_no_sg[&a].0 - positions_no_sg[&b].0).abs();

        assert!(
            sep_with > sep_without,
            "subgraph gap should increase separation (with={sep_with:.1}, without={sep_without:.1})"
        );
    }

    #[test]
    fn test_assign_coordinates_rl_multi_layer_compaction() {
        // RL with 3 layers - exercises right-to-left compaction (min_succ.is_finite())
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        g.add_edge(a, b, make_edge());
        g.add_edge(b, c, make_edge());

        let layers = vec![vec![a], vec![b], vec![c]];
        let membership = SubgraphMembership::new();
        let positions = assign_coordinates(&g, &layers, Direction::RightToLeft, &membership, 50.0);
        let (ax, _) = positions[&a];
        let (cx, _) = positions[&c];
        assert!(ax > cx, "In RL, A should have higher x than C");
    }

    #[test]
    fn test_node_separation_different_subgraphs() {
        // node_separation when u_path != v_path adds border-aware gap
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let layers = vec![vec![a, b]];
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG1".to_string()]);
        membership.insert("B".to_string(), vec!["SG2".to_string()]);

        let positions_sg =
            assign_coordinates(&g, &layers, Direction::TopToBottom, &membership, 50.0);
        let empty_membership = SubgraphMembership::new();
        let positions_no_sg =
            assign_coordinates(&g, &layers, Direction::TopToBottom, &empty_membership, 50.0);

        let sep_sg = (positions_sg[&a].0 - positions_sg[&b].0).abs();
        let sep_no = (positions_no_sg[&a].0 - positions_no_sg[&b].0).abs();
        assert!(
            sep_sg > sep_no,
            "subgraph gap should increase separation (with={sep_sg}, without={sep_no})"
        );
    }

    // ── Type-2 conflict detection tests ──────────────────────────────────

    fn make_dummy(id: &str) -> NodeData {
        NodeData {
            id: id.to_string(),
            label: String::new(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 0.0,
            height: 0.0,
        }
    }

    fn make_border(id: &str) -> NodeData {
        NodeData {
            id: id.to_string(),
            label: String::new(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 0.0,
            height: 0.0,
        }
    }

    #[test]
    fn test_type2_no_conflicts_simple_chain() {
        // Two layers, each with a border pair and one dummy.
        // Dummy chain doesn't cross borders → no conflicts.
        //
        // North: [bl_0, d_north, br_0]
        // South: [bl_1, d_south, br_1]
        // Edges: bl_0→bl_1, d_north→d_south, br_0→br_1
        let mut g = DiGraph::new();
        let bl0 = g.add_node(make_border("__border_bl_SG_0"));
        let dn = g.add_node(make_dummy("__dummy_0_0"));
        let br0 = g.add_node(make_border("__border_br_SG_0"));
        let bl1 = g.add_node(make_border("__border_bl_SG_1"));
        let ds = g.add_node(make_dummy("__dummy_0_1"));
        let br1 = g.add_node(make_border("__border_br_SG_1"));

        g.add_edge(bl0, bl1, make_edge());
        g.add_edge(dn, ds, make_edge());
        g.add_edge(br0, br1, make_edge());

        let layers = vec![vec![bl0, dn, br0], vec![bl1, ds, br1]];
        let conflicts = find_type2_conflicts(&g, &layers);
        assert!(conflicts.is_empty(), "no crossings → no conflicts");
    }

    #[test]
    fn test_type2_conflict_crossing_border() {
        // Dummy edge crosses a border boundary → type-2 conflict.
        //
        // North: [bl_0, d_north, br_0]
        // South: [bl_1, br_1, d_south]
        // Edges: bl_0→bl_1, d_north→d_south (crosses br boundary), br_0→br_1
        let mut g = DiGraph::new();
        let bl0 = g.add_node(make_border("__border_bl_SG_0"));
        let dn = g.add_node(make_dummy("__dummy_0_0"));
        let br0 = g.add_node(make_border("__border_br_SG_0"));
        let bl1 = g.add_node(make_border("__border_bl_SG_1"));
        let br1 = g.add_node(make_border("__border_br_SG_1"));
        let ds = g.add_node(make_dummy("__dummy_0_1"));

        g.add_edge(bl0, bl1, make_edge());
        g.add_edge(dn, ds, make_edge());
        g.add_edge(br0, br1, make_edge());

        // South order: [bl_1, br_1, d_south] — d_south is AFTER br_1
        let layers = vec![vec![bl0, dn, br0], vec![bl1, br1, ds]];
        let conflicts = find_type2_conflicts(&g, &layers);
        // d_north (pos 1) → d_south crosses br boundary (br_0 at pos 2 maps to br_1 at pos 1)
        assert!(
            !conflicts.is_empty(),
            "dummy edge crossing border should create a conflict"
        );
    }

    #[test]
    fn test_type2_no_conflict_border_without_predecessor() {
        // Border node at south layer has no predecessor at north layer.
        // This happens at subgraph boundaries where border chains start.
        // dagre skips these borders → no false conflicts.
        //
        // North (CI layer): [ci_bl, d_0, d_1, ci_br]
        // South (Staging layer): [stg_bl, d_0s, stg_br]
        // Edges: d_0→d_0s (dummy chain crosses CI→Staging boundary)
        // stg_bl and stg_br have NO predecessors at north layer.
        let mut g = DiGraph::new();
        let ci_bl = g.add_node(make_border("__border_bl_CI_10"));
        let d0 = g.add_node(make_dummy("__dummy_0_10"));
        let d1 = g.add_node(make_dummy("__dummy_1_10"));
        let ci_br = g.add_node(make_border("__border_br_CI_10"));
        let stg_bl = g.add_node(make_border("__border_bl_STG_11"));
        let d0s = g.add_node(make_dummy("__dummy_0_11"));
        let stg_br = g.add_node(make_border("__border_br_STG_11"));

        // Dummy chain: d0→d0s (crosses subgraph boundary)
        g.add_edge(d0, d0s, make_edge());
        // No edges from north to stg_bl or stg_br (they're new borders)

        let layers = vec![
            vec![ci_bl, d0, d1, ci_br],
            vec![stg_bl, d0s, stg_br],
        ];
        let conflicts = find_type2_conflicts(&g, &layers);

        // The d0→d0s edge should NOT be flagged as a conflict.
        // Bug fix: borders without predecessors are skipped (matching dagre).
        assert!(
            !has_conflict(&conflicts, d0, d0s),
            "dummy chain crossing subgraph boundary should not be a type-2 conflict \
             when border nodes have no predecessors"
        );
    }

    #[test]
    fn test_type2_no_conflicts_empty_layers() {
        // Edge case: single-node layers, no borders.
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge());

        let layers = vec![vec![a], vec![b]];
        let conflicts = find_type2_conflicts(&g, &layers);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_type2_mixed_borders_with_and_without_predecessors() {
        // Mix of border nodes: some with predecessors, some without.
        // Only borders WITH predecessors should define segment boundaries.
        //
        // North: [bl_0, d_north, br_0]
        // South: [new_bl, bl_1, d_south, br_1, new_br]
        // new_bl/new_br have no predecessors. bl_1/br_1 chain from bl_0/br_0.
        let mut g = DiGraph::new();
        let bl0 = g.add_node(make_border("__border_bl_SG_0"));
        let dn = g.add_node(make_dummy("__dummy_0_0"));
        let br0 = g.add_node(make_border("__border_br_SG_0"));

        let new_bl = g.add_node(make_border("__border_bl_NEW_1"));
        let bl1 = g.add_node(make_border("__border_bl_SG_1"));
        let ds = g.add_node(make_dummy("__dummy_0_1"));
        let br1 = g.add_node(make_border("__border_br_SG_1"));
        let new_br = g.add_node(make_border("__border_br_NEW_1"));

        g.add_edge(bl0, bl1, make_edge());
        g.add_edge(dn, ds, make_edge());
        g.add_edge(br0, br1, make_edge());
        // new_bl and new_br have NO predecessors

        let layers = vec![
            vec![bl0, dn, br0],
            vec![new_bl, bl1, ds, br1, new_br],
        ];
        let conflicts = find_type2_conflicts(&g, &layers);

        // d_north→d_south doesn't cross the bl_0→bl_1 / br_0→br_1 boundaries.
        // new_bl and new_br (no predecessors) should be skipped.
        assert!(
            !has_conflict(&conflicts, dn, ds),
            "non-crossing dummy edge should not be flagged as conflict"
        );
    }
}
