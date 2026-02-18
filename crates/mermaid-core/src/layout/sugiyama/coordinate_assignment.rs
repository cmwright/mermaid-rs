use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::flowchart::Direction;
use crate::layout::graph_builder::SubgraphMembership;
use crate::layout::types::*;

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
) -> HashMap<NodeIndex, (f64, f64)> {
    let is_horizontal = matches!(direction, Direction::LeftToRight | Direction::RightToLeft);
    let empty_path: Vec<String> = Vec::new();

    // ── Main-axis placement ──
    let mut main_pos: HashMap<NodeIndex, f64> = HashMap::new();
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
        rank_offset += max_thick + RANK_SEP;
    }

    // ── Cross-axis: Brandes-Köpf 4-pass ──
    let cross_pos = brandes_kopf(graph, layers, is_horizontal, membership, &empty_path);

    // ── Combine ──
    let mut positions: HashMap<NodeIndex, (f64, f64)> = HashMap::new();
    for &idx in layers.iter().flat_map(|l| l.iter()) {
        let m = main_pos[&idx];
        let c = cross_pos.get(&idx).copied().unwrap_or(0.0);
        positions.insert(
            idx,
            if is_horizontal { (m, c) } else { (c, m) },
        );
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

    for vert in 0..2u8 {
        // vert=0: up (top-to-bottom layers, predecessors)
        // vert=1: down (bottom-to-top layers, successors)
        let adjusted: Vec<Vec<NodeIndex>> = if vert == 0 {
            layers.to_vec()
        } else {
            layers.iter().rev().cloned().collect()
        };

        for horiz in 0..2u8 {
            // horiz=0: left (normal order), horiz=1: right (reversed)
            let final_layers: Vec<Vec<NodeIndex>> = if horiz == 0 {
                adjusted.clone()
            } else {
                adjusted
                    .iter()
                    .map(|l| l.iter().rev().copied().collect())
                    .collect()
            };

            let use_preds = vert == 0;
            let root = vertical_alignment(graph, &final_layers, use_preds);

            let mut xs = horizontal_compaction(
                graph,
                &final_layers,
                &root,
                is_horizontal,
                membership,
                empty_path,
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
        let mut vals: Vec<f64> = xss
            .iter()
            .filter_map(|xs| xs.get(&idx).copied())
            .collect();
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

/// Form vertical blocks by aligning each node with its median neighbor
/// in the adjacent layer. Returns root map (node → block root).
fn vertical_alignment(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    use_predecessors: bool,
) -> HashMap<NodeIndex, NodeIndex> {
    let mut root: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut align: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut pos: HashMap<NodeIndex, usize> = HashMap::new();

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

            for i in lo..=hi {
                let w = ws[i];
                let w_pos = pos.get(&w).copied().unwrap_or(0) as i64;
                if align[&v] == v && prev_idx < w_pos {
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
fn horizontal_compaction(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    root: &HashMap<NodeIndex, NodeIndex>,
    is_horizontal: bool,
    membership: &SubgraphMembership,
    empty_path: &[String],
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
                let sep =
                    node_separation(graph, u, v, is_horizontal, membership, empty_path);
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

    let mut queue: VecDeque<NodeIndex> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(&n, _)| n)
        .collect();
    let mut topo: Vec<NodeIndex> = Vec::with_capacity(block_set.len());

    while let Some(n) = queue.pop_front() {
        topo.push(n);
        if let Some(tos) = out_edges.get(&n) {
            for &to in tos.keys() {
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
    for &b in &block_set {
        if !topo_set.contains(&b) {
            topo.push(b);
        }
    }

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

    // Pass 2: right-to-left compaction
    for &block in topo.iter().rev() {
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

    let u_size = if is_horizontal { un.height } else { un.width };
    let v_size = if is_horizontal { vn.height } else { vn.width };

    let u_path = membership
        .get(&un.id)
        .map(|p| p.as_slice())
        .unwrap_or(empty_path);
    let v_path = membership
        .get(&vn.id)
        .map(|p| p.as_slice())
        .unwrap_or(empty_path);

    let gap = if u_path != v_path {
        let common = u_path
            .iter()
            .zip(v_path.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let divergence = u_path.len().max(v_path.len()) - common;
        SUBGRAPH_GROUP_GAP * divergence as f64
    } else {
        0.0
    };

    u_size / 2.0 + NODE_SEP + gap + v_size / 2.0
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
            let half =
                if is_horizontal { graph[n].height } else { graph[n].width } / 2.0;
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
            let half =
                if is_horizontal { graph[n].height } else { graph[n].width } / 2.0;
            lo = lo.min(x - half);
            hi = hi.max(x + half);
        }
        (lo, hi)
    };

    for i in 0..xss.len() {
        if i == smallest {
            continue;
        }

        let mut xs_lo = f64::INFINITY;
        let mut xs_hi = f64::NEG_INFINITY;
        for (&n, &x) in &xss[i] {
            let half =
                if is_horizontal { graph[n].height } else { graph[n].width } / 2.0;
            xs_lo = xs_lo.min(x - half);
            xs_hi = xs_hi.max(x + half);
        }

        // Even indices (0, 2) are left-biased; odd (1, 3) are right-biased
        let delta = if i % 2 == 0 {
            align_min - xs_lo
        } else {
            align_max - xs_hi
        };

        for v in xss[i].values_mut() {
            *v += delta;
        }
    }
}
