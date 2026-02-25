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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    #[test]
    fn empty_graph_returns_empty_fas() {
        let g: Graph<(), (), ()> = Graph::new();
        let result = greedy_fas(&g, None);
        assert!(result.is_empty());
    }

    #[test]
    fn single_node_returns_empty_fas() {
        let mut g: Graph<(), (), ()> = Graph::new();
        g.set_node("a", None);
        let result = greedy_fas(&g, None);
        assert!(result.is_empty());
    }

    #[test]
    fn acyclic_graph_returns_empty_fas() {
        let mut g: Graph<(), (), ()> = Graph::new();
        g.set_node("a", None);
        g.set_node("b", None);
        g.set_node("c", None);
        g.set_edge("a", "b", None, None);
        g.set_edge("b", "c", None, None);
        let result = greedy_fas(&g, None);
        assert!(
            result.is_empty(),
            "acyclic graph should have empty FAS, got {:?}",
            result
        );
    }

    #[test]
    fn simple_cycle_returns_one_edge() {
        let mut g: Graph<(), (), ()> = Graph::new();
        g.set_node("a", None);
        g.set_node("b", None);
        g.set_edge("a", "b", None, None);
        g.set_edge("b", "a", None, None);
        let result = greedy_fas(&g, None);
        assert_eq!(
            result.len(),
            1,
            "simple 2-node cycle should return 1 edge in FAS, got {:?}",
            result
        );
    }

    #[test]
    fn triangle_cycle_returns_one_edge() {
        let mut g: Graph<(), (), ()> = Graph::new();
        g.set_node("a", None);
        g.set_node("b", None);
        g.set_node("c", None);
        g.set_edge("a", "b", None, None);
        g.set_edge("b", "c", None, None);
        g.set_edge("c", "a", None, None);
        let result = greedy_fas(&g, None);
        assert!(
            !result.is_empty(),
            "triangle cycle should return at least 1 edge in FAS"
        );
        // Reversing the FAS edges should make the graph acyclic.
        // Just verify we got a reasonable number.
        assert!(result.len() <= 2);
    }

    #[test]
    fn custom_weight_fn_is_used() {
        let mut g: Graph<(), (), ()> = Graph::new();
        g.set_node("a", None);
        g.set_node("b", None);
        g.set_edge("a", "b", None, None);
        g.set_edge("b", "a", None, None);

        let weight_fn = |_e: &Edge, _g: &Graph<(), (), ()>| -> f64 { 5.0 };
        let result = greedy_fas(&g, Some(&weight_fn));
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn chain_with_back_edge() {
        let mut g: Graph<(), (), ()> = Graph::new();
        for id in ["a", "b", "c", "d"] {
            g.set_node(id, None);
        }
        g.set_edge("a", "b", None, None);
        g.set_edge("b", "c", None, None);
        g.set_edge("c", "d", None, None);
        g.set_edge("d", "a", None, None); // back edge creating cycle
        let result = greedy_fas(&g, None);
        assert!(
            !result.is_empty(),
            "chain with back edge should have non-empty FAS"
        );
    }

    #[test]
    fn disconnected_graph_with_cycle() {
        let mut g: Graph<(), (), ()> = Graph::new();
        // Component 1: acyclic
        g.set_node("a", None);
        g.set_node("b", None);
        g.set_edge("a", "b", None, None);
        // Component 2: cyclic
        g.set_node("x", None);
        g.set_node("y", None);
        g.set_edge("x", "y", None, None);
        g.set_edge("y", "x", None, None);
        let result = greedy_fas(&g, None);
        assert_eq!(
            result.len(),
            1,
            "only the cyclic component should contribute to FAS"
        );
    }

    #[test]
    fn assign_bucket_source_goes_to_last() {
        let mut buckets = vec![
            List::new(),
            List::new(),
            List::new(),
            List::new(),
            List::new(),
        ];
        let zero_idx = 2;
        // in_weight=0 means source -> goes to last bucket
        assign_bucket(&mut buckets, zero_idx, "src", 0.0, 5.0);
        assert_eq!(buckets[4].dequeue(), Some("src".to_string()));
    }

    #[test]
    fn assign_bucket_sink_goes_to_first() {
        let mut buckets = vec![
            List::new(),
            List::new(),
            List::new(),
            List::new(),
            List::new(),
        ];
        let zero_idx = 2;
        // out_weight=0 means sink -> goes to bucket 0
        assign_bucket(&mut buckets, zero_idx, "snk", 3.0, 0.0);
        assert_eq!(buckets[0].dequeue(), Some("snk".to_string()));
    }

    #[test]
    fn assign_bucket_balanced_goes_to_zero_idx() {
        let mut buckets = vec![
            List::new(),
            List::new(),
            List::new(),
            List::new(),
            List::new(),
        ];
        let zero_idx = 2;
        // out_weight == in_weight => index = zero_idx
        assign_bucket(&mut buckets, zero_idx, "bal", 3.0, 3.0);
        assert_eq!(buckets[2].dequeue(), Some("bal".to_string()));
    }
}
