//! Feasible tree construction for the network simplex algorithm.
//! Port of dagre's `rank/feasible-tree.js`.

use crate::graph::{GraphOptions, LayoutGraph};
use crate::rank::util::slack;
use crate::types::{EdgeLabel, NodeLabel};

/// Constructs a spanning tree with tight edges and adjusts ranks.
pub fn feasible_tree(g: &mut LayoutGraph) -> LayoutGraph {
    let mut t = LayoutGraph::with_options(&GraphOptions {
        directed: false,
        multigraph: false,
        compound: false,
    });

    let start = g.nodes()[0].clone();
    let size = g.node_count();
    t.set_node(&start, Some(NodeLabel::default()));

    while tight_tree(&mut t, g) < size {
        let edge = find_min_slack_edge(&t, g);
        let delta = if t.has_node(&edge.v) {
            slack(g, &edge)
        } else {
            -slack(g, &edge)
        };
        shift_ranks(&t, g, delta);
    }

    t
}

/// Finds a maximal tree of tight edges using DFS, returns node count.
fn tight_tree(t: &mut LayoutGraph, g: &LayoutGraph) -> usize {
    fn dfs(t: &mut LayoutGraph, g: &LayoutGraph, v: &str) {
        if let Some(out_edge_ids) = g.out_edge_ids(v) {
            for edge_id in out_edge_ids {
                let Some(e) = g.edge_obj_by_id(edge_id) else {
                    continue;
                };
                if !t.has_node(&e.w) && slack(g, e) == 0 {
                    t.set_node(&e.w, Some(NodeLabel::default()));
                    t.set_edge(v, &e.w, Some(EdgeLabel::default()), None);
                    dfs(t, g, &e.w);
                }
            }
        }
        if let Some(in_edge_ids) = g.in_edge_ids(v) {
            for edge_id in in_edge_ids {
                let Some(e) = g.edge_obj_by_id(edge_id) else {
                    continue;
                };
                if !t.has_node(&e.v) && slack(g, e) == 0 {
                    t.set_node(&e.v, Some(NodeLabel::default()));
                    t.set_edge(v, &e.v, Some(EdgeLabel::default()), None);
                    dfs(t, g, &e.v);
                }
            }
        }
    }

    let nodes = t.nodes();
    for v in &nodes {
        dfs(t, g, v);
    }
    t.node_count()
}

/// Finds the edge with the smallest slack incident on the tree.
fn find_min_slack_edge(t: &LayoutGraph, g: &LayoutGraph) -> crate::graph::Edge {
    let mut best_slack = i64::MAX;
    let mut best_edge = None;

    for edge_id in g.edge_ids() {
        let Some(edge) = g.edge_obj_by_id(edge_id) else {
            continue;
        };
        if t.has_node(&edge.v) != t.has_node(&edge.w) {
            let s = slack(g, edge).abs();
            if s < best_slack {
                best_slack = s;
                best_edge = Some(edge.clone());
            }
        }
    }

    best_edge.expect("No min slack edge found")
}

fn shift_ranks(t: &LayoutGraph, g: &mut LayoutGraph, delta: i64) {
    for v in t.nodes() {
        let rank = g.node(&v).and_then(|n| n.rank).unwrap_or(0);
        if let Some(node) = g.node_mut(&v) {
            node.rank = Some(rank + delta);
        }
    }
}
