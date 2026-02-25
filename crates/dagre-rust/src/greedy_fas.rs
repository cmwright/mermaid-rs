//! Greedy heuristic for finding a feedback arc set.
//! Port of dagre's `greedy-fas.js`.

use crate::data::list::List;
use crate::graph::{Edge, Graph};

/// Node label for the internal FAS graph.
#[derive(Debug, Clone, Default)]
struct FasNodeLabel {
    in_weight: f64,
    out_weight: f64,
}

/// The internal FAS graph uses typed labels instead of serde_json::Value.
type FasGraph = Graph<FasNodeLabel, f64, ()>;

/// Default weight function returning 1.
fn default_weight_fn<N: Default + Clone, E: Default + Clone, G: Default + Clone>(
    _e: &Edge,
    _g: &Graph<N, E, G>,
) -> f64 {
    1.0
}

/// Weight function type for computing edge weights in FAS.
type WeightFn<'a, N, E, G> = dyn Fn(&Edge, &Graph<N, E, G>) -> f64 + 'a;

/// Finds a feedback arc set using the greedy heuristic.
/// Returns edges that should be reversed to make the graph acyclic.
///
/// Generic over the outer graph's label types so it can be called with
/// both `Graph` (serde_json::Value defaults) and `LayoutGraph`.
pub fn greedy_fas<N: Default + Clone, E: Default + Clone, G: Default + Clone>(
    g: &Graph<N, E, G>,
    weight_fn: Option<&WeightFn<N, E, G>>,
) -> Vec<Edge> {
    if g.node_count() <= 1 {
        return vec![];
    }

    let wfn = weight_fn.unwrap_or(&default_weight_fn);
    let (fas_graph, buckets, zero_idx) = build_state(g, wfn);
    let results = do_greedy_fas(fas_graph, buckets, zero_idx);

    // Expand multi-edges
    results
        .iter()
        .flat_map(|e| g.out_edges(&e.v, Some(&e.w)).unwrap_or_default())
        .collect()
}

fn do_greedy_fas(mut g: FasGraph, mut buckets: Vec<List<String>>, zero_idx: usize) -> Vec<Edge> {
    let mut results: Vec<Edge> = Vec::new();
    let last = buckets.len() - 1;

    while g.node_count() > 0 {
        // Drain sinks (bucket 0)
        while let Some(v) = buckets[0].dequeue() {
            remove_node(&mut g, &mut buckets, zero_idx, &v, false, &mut results);
        }
        // Drain sources (last bucket)
        while let Some(v) = buckets[last].dequeue() {
            remove_node(&mut g, &mut buckets, zero_idx, &v, false, &mut results);
        }
        if g.node_count() > 0 {
            for i in (1..last).rev() {
                if let Some(v) = buckets[i].dequeue() {
                    remove_node(&mut g, &mut buckets, zero_idx, &v, true, &mut results);
                    break;
                }
            }
        }
    }

    results
}

fn remove_node(
    g: &mut FasGraph,
    buckets: &mut [List<String>],
    zero_idx: usize,
    v: &str,
    collect_predecessors: bool,
    results: &mut Vec<Edge>,
) {
    if let Some(in_edges) = g.in_edges(v, None) {
        for edge in &in_edges {
            let weight = g.edge_by_obj(edge).copied().unwrap_or(0.0);

            if collect_predecessors {
                results.push(Edge::new(&edge.v, &edge.w, None));
            }

            // Update u's out_weight
            if let Some(u_label) = g.node_mut(&edge.v) {
                u_label.out_weight -= weight;
                let entry_v = edge.v.clone();
                assign_bucket(
                    buckets,
                    zero_idx,
                    &entry_v,
                    u_label.in_weight,
                    u_label.out_weight,
                );
            }
        }
    }

    if let Some(out_edges) = g.out_edges(v, None) {
        for edge in &out_edges {
            let weight = g.edge_by_obj(edge).copied().unwrap_or(0.0);

            if let Some(w_label) = g.node_mut(&edge.w) {
                w_label.in_weight -= weight;
                let entry_w = edge.w.clone();
                assign_bucket(
                    buckets,
                    zero_idx,
                    &entry_w,
                    w_label.in_weight,
                    w_label.out_weight,
                );
            }
        }
    }

    g.remove_node(v);
}

fn build_state<N: Default + Clone, E: Default + Clone, G: Default + Clone>(
    g: &Graph<N, E, G>,
    weight_fn: &WeightFn<N, E, G>,
) -> (FasGraph, Vec<List<String>>, usize) {
    let mut fas_graph: FasGraph = Graph::new();
    let mut max_in: f64 = 0.0;
    let mut max_out: f64 = 0.0;

    for v in g.node_ids() {
        fas_graph.set_node(
            v,
            Some(FasNodeLabel {
                in_weight: 0.0,
                out_weight: 0.0,
            }),
        );
    }

    for eid in g.edge_ids() {
        let eobj = match g.edge_obj_by_id(eid) {
            Some(e) => e,
            None => continue,
        };
        let ev = eobj.v.clone();
        let ew = eobj.w.clone();
        let prev_weight = fas_graph.edge(&ev, &ew, None).copied().unwrap_or(0.0);
        let weight = weight_fn(eobj, g);
        let edge_weight = prev_weight + weight;
        fas_graph.set_edge(&ev, &ew, Some(edge_weight), None);

        // Update out for e.v
        if let Some(v_label) = fas_graph.node_mut(&ev) {
            v_label.out_weight += weight;
            if v_label.out_weight > max_out {
                max_out = v_label.out_weight;
            }
        }

        // Update in for e.w
        if let Some(w_label) = fas_graph.node_mut(&ew) {
            w_label.in_weight += weight;
            if w_label.in_weight > max_in {
                max_in = w_label.in_weight;
            }
        }
    }

    let bucket_count = (max_out + max_in + 3.0) as usize;
    let mut buckets: Vec<List<String>> = (0..bucket_count).map(|_| List::new()).collect();
    let zero_idx = (max_in + 1.0) as usize;

    for v in fas_graph.node_ids() {
        if let Some(label) = fas_graph.node(v) {
            assign_bucket(&mut buckets, zero_idx, v, label.in_weight, label.out_weight);
        }
    }

    (fas_graph, buckets, zero_idx)
}

fn assign_bucket(
    buckets: &mut [List<String>],
    zero_idx: usize,
    v: &str,
    in_weight: f64,
    out_weight: f64,
) {
    let idx = if out_weight == 0.0 {
        0
    } else if in_weight == 0.0 {
        buckets.len() - 1
    } else {
        ((out_weight - in_weight) as isize + zero_idx as isize) as usize
    };
    if idx < buckets.len() {
        buckets[idx].enqueue(v.to_string());
    }
}
