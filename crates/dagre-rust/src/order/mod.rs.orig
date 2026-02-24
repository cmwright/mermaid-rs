//! Ordering module - minimizes edge crossings.
//! Port of dagre's `order/index.js`.

pub mod add_subgraph_constraints;
pub mod barycenter;
pub mod build_layer_graph;
pub mod cross_count;
pub mod init_order;
pub mod resolve_conflicts;
pub mod sort;
pub mod sort_subgraph;

use crate::graph::{ConstraintGraph, LayoutGraph};
use crate::util;

/// Applies heuristics to minimize edge crossings and sets order attributes.
pub fn order(g: &mut LayoutGraph, disable_optimal_order_heuristic: bool) {
    let max_rank = util::max_rank(g);
    let down_ranks: Vec<i64> = util::range(1, Some(max_rank + 1), 1);
    let up_ranks: Vec<i64> = util::range(max_rank - 1, Some(-1), -1);

    let mut down_layer_graphs = build_layer_graphs(g, &down_ranks, "inEdges");
    let mut up_layer_graphs = build_layer_graphs(g, &up_ranks, "outEdges");

    let mut layering = init_order::init_order(g);
    assign_order(g, &layering);

    if disable_optimal_order_heuristic {
        return;
    }

    let mut best_cc = f64::INFINITY;
    let mut best: Option<Vec<Vec<String>>> = None;

    let mut last_best = 0;
    let mut i = 0;
    while last_best < 4 {
        let graphs: &mut Vec<(i64, LayoutGraph, String)> = if i % 2 != 0 {
            &mut down_layer_graphs
        } else {
            &mut up_layer_graphs
        };
        sweep_layer_graphs(g, graphs, i % 4 >= 2);

        layering = util::build_layer_matrix(g);
        let cc = cross_count::cross_count(g, &layering) as f64;
        if cc < best_cc {
            last_best = 0;
            best = Some(layering.clone());
            best_cc = cc;
        } else if cc == best_cc {
            best = Some(layering.clone());
        }
        i += 1;
        last_best += 1;
    }

    if let Some(best) = best {
        assign_order(g, &best);
    }
}

fn build_layer_graphs(
    g: &LayoutGraph,
    ranks: &[i64],
    relationship: &str,
) -> Vec<(i64, LayoutGraph, String)> {
    // Build index of nodes by rank
    let mut nodes_by_rank: std::collections::HashMap<i64, Vec<String>> =
        std::collections::HashMap::new();
    for v in g.node_ids() {
        if let Some(node) = g.node(v) {
            if let Some(rank) = node.rank {
                nodes_by_rank.entry(rank).or_default().push(v.clone());
            }
            if let (Some(min_r), Some(max_r)) = (node.min_rank, node.max_rank) {
                let node_rank = node.rank;
                for r in min_r..=max_r {
                    if Some(r) != node_rank {
                        nodes_by_rank.entry(r).or_default().push(v.clone());
                    }
                }
            }
        }
    }

    ranks
        .iter()
        .map(|&rank| {
            let nodes = nodes_by_rank.get(&rank).cloned().unwrap_or_default();
            let lg = build_layer_graph::build_layer_graph(g, rank, relationship, &nodes);
            (rank, lg, relationship.to_string())
        })
        .collect()
}

fn sweep_layer_graphs(
    g: &mut LayoutGraph,
    layer_graphs: &mut [(i64, LayoutGraph, String)],
    bias_right: bool,
) {
    let mut cg: ConstraintGraph = ConstraintGraph::new();

    for (_, lg, _) in layer_graphs.iter_mut() {
        // Sync order attributes from g into the layer graph so barycenter reads current orders.
        // In JS, layer graph nodes share references with the original graph, so order updates
        // are automatically visible. In Rust, we must explicitly copy them.
        let node_ids: Vec<String> = lg.node_ids().to_vec();
        for v in &node_ids {
            if let Some(order_val) = g.node(v).and_then(|n| n.order)
                && let Some(lg_node) = lg.node_mut(v)
            {
                lg_node.order = Some(order_val);
            }
        }

        let root = lg.graph().root.as_deref().unwrap_or("");

        let sorted = sort_subgraph::sort_subgraph(lg, root, &cg, bias_right);

        // Assign order to nodes in g
        for (i, v) in sorted.vs.iter().enumerate() {
            if let Some(node) = g.node_mut(v) {
                node.order = Some(i as i64);
            }
        }

        add_subgraph_constraints::add_subgraph_constraints(lg, &mut cg, &sorted.vs);
    }
}

fn assign_order(g: &mut LayoutGraph, layering: &[Vec<String>]) {
    for layer in layering {
        for (i, v) in layer.iter().enumerate() {
            if !v.is_empty()
                && let Some(node) = g.node_mut(v)
            {
                node.order = Some(i as i64);
            }
        }
    }
}
