//! Utility functions used throughout the layout pipeline.
//!
//! Ports of: addDummyNode, simplify, asNonCompoundGraph, successorWeights,
//! predecessorWeights, intersectRect, buildLayerMatrix, normalizeRanks,
//! removeEmptyRanks, addBorderNode, maxRank, partition, time, notime,
//! uniqueId, range.

use crate::graph::{GraphOptions, LayoutGraph};
use crate::types::*;
use ahash::AHashMap as HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Global unique ID counter, mirroring JS `let idCounter = 0`.
static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Reset the ID counter (useful for tests to get deterministic output).
pub fn reset_id_counter() {
    ID_COUNTER.store(0, Ordering::Relaxed);
}

/// Generates a unique ID with the given prefix. Mirrors JS `uniqueId`.
pub fn unique_id(prefix: &str) -> String {
    let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    format!("{}{}", prefix, id)
}

/// Adds a dummy node to the graph and returns the node ID.
/// If a node with `name` already exists, generates a unique name.
pub fn add_dummy_node(
    g: &mut LayoutGraph,
    typ: DummyType,
    mut attrs: NodeLabel,
    name: &str,
) -> String {
    let mut v = name.to_string();
    while g.has_node(&v) {
        v = unique_id(name);
    }

    attrs.dummy = Some(typ);
    g.set_node(&v, Some(attrs));
    v
}

/// Returns a new graph with only simple edges. Multi-edges are aggregated
/// by summing weights and taking max minlen.
/// Only copies the `rank` field of NodeLabel to avoid expensive cloning.
pub fn simplify(g: &LayoutGraph) -> LayoutGraph {
    let mut simplified = LayoutGraph::new();
    simplified.set_graph(g.graph().clone());

    for v in g.node_ids() {
        // Only copy the rank field — callers (network_simplex) only need rank
        let mut label = NodeLabel::default();
        if let Some(orig) = g.node(v) {
            label.rank = orig.rank;
        }
        simplified.set_node(v, Some(label));
    }

    for eid in g.edge_ids() {
        let eobj = match g.edge_obj_by_id(eid) {
            Some(e) => e,
            None => continue,
        };
        let ev = &eobj.v;
        let ew = &eobj.w;
        let existing = simplified.edge(ev, ew, None);
        let (existing_weight, existing_minlen) = match existing {
            Some(val) => (val.weight, val.minlen),
            None => (0.0, 1.0),
        };

        let label = g.edge_label_by_id(eid).unwrap();
        let weight = label.weight;
        let minlen = label.minlen;

        simplified.set_edge(
            ev,
            ew,
            Some(EdgeLabel {
                weight: existing_weight + weight,
                minlen: if existing_minlen > minlen {
                    existing_minlen
                } else {
                    minlen
                },
                ..Default::default()
            }),
            None,
        );
    }

    simplified
}

/// Converts a compound graph to a non-compound graph, keeping only leaf nodes.
pub fn as_non_compound_graph(g: &LayoutGraph) -> LayoutGraph {
    let mut simplified = LayoutGraph::with_options(&GraphOptions {
        directed: true,
        multigraph: g.is_multigraph(),
        compound: false,
    });
    simplified.set_graph(g.graph().clone());

    for v in g.node_ids() {
        if g.children(Some(v)).is_none_or(|c| c.is_empty()) {
            simplified.set_node(v, g.node(v).cloned());
        }
    }

    for eid in g.edge_ids() {
        let eobj = match g.edge_obj_by_id(eid) {
            Some(e) => e,
            None => continue,
        };
        if simplified.has_node(&eobj.v) && simplified.has_node(&eobj.w) {
            simplified.set_edge_with_obj(eobj, g.edge_label_by_id(eid).cloned());
        }
    }

    simplified
}

/// Lightweight version of as_non_compound_graph for ranking only.
/// Only copies the `rank` field of NodeLabel to avoid expensive cloning.
pub fn as_non_compound_graph_for_rank(g: &LayoutGraph) -> LayoutGraph {
    let mut simplified = LayoutGraph::with_options(&GraphOptions {
        directed: true,
        multigraph: g.is_multigraph(),
        compound: false,
    });
    simplified.set_graph(g.graph().clone());

    for v in g.node_ids() {
        if g.children(Some(v)).is_none_or(|c| c.is_empty()) {
            let mut label = NodeLabel::default();
            if let Some(orig) = g.node(v) {
                label.rank = orig.rank;
            }
            simplified.set_node(v, Some(label));
        }
    }

    for eid in g.edge_ids() {
        let eobj = match g.edge_obj_by_id(eid) {
            Some(e) => e,
            None => continue,
        };
        if simplified.has_node(&eobj.v) && simplified.has_node(&eobj.w) {
            simplified.set_edge_with_obj(eobj, g.edge_label_by_id(eid).cloned());
        }
    }

    simplified
}

/// Returns a map from node -> { successor -> weight sum }.
pub fn successor_weights(g: &LayoutGraph) -> HashMap<String, HashMap<String, f64>> {
    let mut result = HashMap::new();
    for v in g.node_ids() {
        let mut sucs: HashMap<String, f64> = HashMap::new();
        if let Some(edge_ids) = g.out_edge_ids(v) {
            for eid in edge_ids {
                let w = g.edge_label_by_id(eid).map(|l| l.weight).unwrap_or(0.0);
                if let Some(eobj) = g.edge_obj_by_id(eid) {
                    *sucs.entry(eobj.w.clone()).or_insert(0.0) += w;
                }
            }
        }
        result.insert(v.clone(), sucs);
    }
    result
}

/// Returns a map from node -> { predecessor -> weight sum }.
pub fn predecessor_weights(g: &LayoutGraph) -> HashMap<String, HashMap<String, f64>> {
    let mut result = HashMap::new();
    for v in g.node_ids() {
        let mut preds: HashMap<String, f64> = HashMap::new();
        if let Some(edge_ids) = g.in_edge_ids(v) {
            for eid in edge_ids {
                let w = g.edge_label_by_id(eid).map(|l| l.weight).unwrap_or(0.0);
                if let Some(eobj) = g.edge_obj_by_id(eid) {
                    *preds.entry(eobj.v.clone()).or_insert(0.0) += w;
                }
            }
        }
        result.insert(v.clone(), preds);
    }
    result
}

/// Finds where a line from `point` toward `rect`'s center intersects the rectangle border.
pub fn intersect_rect(rect: &NodeLabel, point: &Point) -> Point {
    let rx = rect.x.unwrap_or(0.0);
    let ry = rect.y.unwrap_or(0.0);
    let px = point.x;
    let py = point.y;

    let dx = px - rx;
    let dy = py - ry;
    let w = rect.width / 2.0;
    let h = rect.height / 2.0;

    if dx == 0.0 && dy == 0.0 {
        panic!("Not possible to find intersection inside of the rectangle");
    }

    let (sx, sy);
    if dy.abs() * w > dx.abs() * h {
        // Intersection is top or bottom
        let h = if dy < 0.0 { -h } else { h };
        sx = h * dx / dy;
        sy = h;
    } else {
        // Intersection is left or right
        let w = if dx < 0.0 { -w } else { w };
        sx = w;
        sy = w * dy / dx;
    }

    Point {
        x: rx + sx,
        y: ry + sy,
    }
}

/// Builds a 2D matrix of node IDs indexed by [rank][order].
pub fn build_layer_matrix(g: &LayoutGraph) -> Vec<Vec<String>> {
    let max_r = max_rank(g);
    let mut layering: Vec<Vec<String>> = vec![Vec::new(); (max_r + 1) as usize];

    for v in g.node_ids() {
        if let Some(node) = g.node(v)
            && let Some(rank) = node.rank
        {
            let order = node.order.unwrap_or(0) as usize;
            let rank_idx = rank as usize;
            if rank_idx < layering.len() {
                // Ensure the inner vec is large enough
                if order >= layering[rank_idx].len() {
                    layering[rank_idx].resize(order + 1, String::new());
                }
                layering[rank_idx][order] = v.clone();
            }
        }
    }

    layering
}

/// Adjusts ranks so the minimum rank is 0.
pub fn normalize_ranks(g: &mut LayoutGraph) {
    let min_rank = g
        .node_ids()
        .iter()
        .filter_map(|v| g.node(v).and_then(|n| n.rank))
        .min()
        .unwrap_or(0);

    let node_ids: Vec<String> = g.node_ids().to_vec();
    for v in &node_ids {
        if let Some(node) = g.node_mut(v)
            && let Some(rank) = node.rank
        {
            node.rank = Some(rank - min_rank);
        }
    }
}

/// Removes empty ranks, compacting the graph. Respects nodeRankFactor.
pub fn remove_empty_ranks(g: &mut LayoutGraph) {
    let node_ranks: Vec<i64> = g
        .node_ids()
        .iter()
        .filter_map(|v| g.node(v).and_then(|n| n.rank))
        .collect();

    if node_ranks.is_empty() {
        return;
    }

    let offset = *node_ranks.iter().min().unwrap();

    // Build layers
    let mut layers: Vec<Option<Vec<String>>> = Vec::new();
    for v in g.node_ids() {
        if let Some(node) = g.node(v)
            && let Some(rank) = node.rank
        {
            let idx = (rank - offset) as usize;
            if idx >= layers.len() {
                layers.resize(idx + 1, None);
            }
            layers[idx].get_or_insert_with(Vec::new).push(v.clone());
        }
    }

    let node_rank_factor = g.graph().node_rank_factor.unwrap_or(1) as usize;

    let mut delta: i64 = 0;
    for (i, layer) in layers.iter().enumerate() {
        match layer {
            None => {
                if node_rank_factor == 0 || i % node_rank_factor != 0 {
                    delta -= 1;
                }
            }
            Some(vs) => {
                if delta != 0 {
                    for v in vs {
                        if let Some(node) = g.node_mut(v)
                            && let Some(rank) = node.rank
                        {
                            node.rank = Some(rank + delta);
                        }
                    }
                }
            }
        }
    }
}

/// Adds a border dummy node with zero width/height.
pub fn add_border_node(
    g: &mut LayoutGraph,
    prefix: &str,
    rank: Option<i64>,
    order: Option<i64>,
) -> String {
    let mut node = NodeLabel {
        width: 0.0,
        height: 0.0,
        ..Default::default()
    };
    if let (Some(r), Some(o)) = (rank, order) {
        node.rank = Some(r);
        node.order = Some(o);
    }
    add_dummy_node(g, DummyType::Border, node, prefix)
}

/// Returns the maximum rank in the graph.
pub fn max_rank(g: &LayoutGraph) -> i64 {
    g.node_ids()
        .iter()
        .filter_map(|v| g.node(v).and_then(|n| n.rank))
        .max()
        .unwrap_or(i64::MIN)
}

/// Partition a collection into lhs (predicate true) and rhs (predicate false).
pub fn partition<T, F>(collection: Vec<T>, pred: F) -> (Vec<T>, Vec<T>)
where
    F: Fn(&T) -> bool,
{
    let mut lhs = Vec::new();
    let mut rhs = Vec::new();
    for item in collection {
        if pred(&item) {
            lhs.push(item);
        } else {
            rhs.push(item);
        }
    }
    (lhs, rhs)
}

/// No-op timer wrapper; simply calls fn. Mirrors JS `notime`.
pub fn notime<F, R>(_name: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    f()
}

/// Timer wrapper that logs execution time. Mirrors JS `time`.
pub fn time<F, R>(name: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let start = std::time::Instant::now();
    let result = f();
    eprintln!("{} time: {}ms", name, start.elapsed().as_millis());
    result
}

/// Generates a range of integers [start, limit) with given step.
pub fn range(start: i64, limit: Option<i64>, step: i64) -> Vec<i64> {
    let (start, limit) = match limit {
        None => (0, start),
        Some(l) => (start, l),
    };

    let mut result = Vec::new();
    if step > 0 {
        let mut i = start;
        while i < limit {
            result.push(i);
            i += step;
        }
    } else if step < 0 {
        let mut i = start;
        while limit < i {
            result.push(i);
            i += step;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphOptions;

    #[test]
    fn test_unique_id() {
        reset_id_counter();
        assert_eq!(unique_id("_"), "_1");
        assert_eq!(unique_id("_"), "_2");
        assert_eq!(unique_id("foo"), "foo3");
    }

    #[test]
    fn test_range() {
        assert_eq!(range(5, None, 1), vec![0, 1, 2, 3, 4]);
        assert_eq!(range(2, Some(5), 1), vec![2, 3, 4]);
        assert_eq!(range(5, Some(1), -1), vec![5, 4, 3, 2]);
    }

    #[test]
    fn test_intersect_rect() {
        let rect = NodeLabel {
            x: Some(0.0),
            y: Some(0.0),
            width: 10.0,
            height: 10.0,
            ..Default::default()
        };
        let point = Point { x: 20.0, y: 0.0 };
        let result = intersect_rect(&rect, &point);
        assert_eq!(result.x, 5.0);
        assert_eq!(result.y, 0.0);
    }

    #[test]
    fn test_partition() {
        let (lhs, rhs) = partition(vec![1, 2, 3, 4, 5], |x| *x > 3);
        assert_eq!(lhs, vec![4, 5]);
        assert_eq!(rhs, vec![1, 2, 3]);
    }

    #[test]
    fn test_simplify() {
        let mut g = LayoutGraph::with_options(&GraphOptions {
            directed: true,
            multigraph: true,
            compound: false,
        });
        g.set_edge(
            "a",
            "b",
            Some(EdgeLabel {
                weight: 1.0,
                minlen: 1.0,
                ..Default::default()
            }),
            Some("e1"),
        );
        g.set_edge(
            "a",
            "b",
            Some(EdgeLabel {
                weight: 2.0,
                minlen: 3.0,
                ..Default::default()
            }),
            Some("e2"),
        );

        let s = simplify(&g);
        assert_eq!(s.edge_count(), 1);
        let label = s.edge("a", "b", None).unwrap();
        assert_eq!(label.weight, 3.0);
        assert_eq!(label.minlen, 3.0);
    }
}
