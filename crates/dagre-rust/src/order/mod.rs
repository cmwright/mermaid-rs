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

    let mut down_layer_graphs = build_layer_graphs(g, &down_ranks, true);
    let mut up_layer_graphs = build_layer_graphs(g, &up_ranks, false);

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
        let graphs: &mut Vec<(i64, LayoutGraph)> = if i % 2 != 0 {
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
    use_in_edges: bool,
) -> Vec<(i64, LayoutGraph)> {
    // Build index of nodes by rank
    let mut nodes_by_rank: ahash::AHashMap<i64, Vec<String>> = ahash::AHashMap::new();
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

    let empty: Vec<String> = Vec::new();
    ranks
        .iter()
        .map(|&rank| {
            let nodes = nodes_by_rank.get(&rank).unwrap_or(&empty);
            let lg = build_layer_graph::build_layer_graph(g, rank, use_in_edges, nodes);
            (rank, lg)
        })
        .collect()
}

fn sweep_layer_graphs(
    g: &mut LayoutGraph,
    layer_graphs: &mut [(i64, LayoutGraph)],
    bias_right: bool,
) {
    let mut cg: ConstraintGraph = ConstraintGraph::new();

    for (_, lg) in layer_graphs.iter_mut() {
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

#[cfg(test)]
mod tests {
    use crate::{EdgeLabel, Graph, GraphLabel, GraphOptions, NodeLabel};

    /// Regression test: when two nodes at the same rank have tied barycenters,
    /// the ordering should preserve insertion order (matching dagre.js v0.8.5).
    ///
    /// Graph: A-->B, A-->C, B-->D, C-->D (diamond shape)
    /// Expected rank 1 order: B before C (insertion order), not C before B.
    #[test]
    fn test_order_preserves_insertion_order_on_tie() {
        let mut g = Graph::with_options(&GraphOptions {
            directed: true,
            multigraph: false,
            compound: false,
        });
        g.set_graph(GraphLabel {
            rankdir: crate::RankDir::LR,
            ..GraphLabel::default()
        });

        // Insert B before C — B should be ordered first at rank 1
        for id in &["A", "B", "C", "D"] {
            let mut nl = NodeLabel::default();
            nl.width = 50.0;
            nl.height = 30.0;
            g.set_node(id, Some(nl));
        }
        g.set_edge("A", "B", Some(EdgeLabel::default()), None);
        g.set_edge("A", "C", Some(EdgeLabel::default()), None);
        g.set_edge("B", "D", Some(EdgeLabel::default()), None);
        g.set_edge("C", "D", Some(EdgeLabel::default()), None);

        crate::layout(&mut g);

        let b = g.node("B").unwrap();
        let c = g.node("C").unwrap();

        // B and C should be at the same rank
        assert_eq!(b.rank, c.rank, "B and C should share a rank");

        // B should be ordered before C (lower order = higher/left position)
        assert!(
            b.order.unwrap() < c.order.unwrap(),
            "B (order={}) should be ordered before C (order={}); insertion order should be preserved on tie",
            b.order.unwrap(),
            c.order.unwrap(),
        );
    }
}
