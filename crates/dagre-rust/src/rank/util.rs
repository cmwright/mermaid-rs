//! Ranking utilities.
//! Port of dagre's `rank/util.js`.

use crate::graph::{Edge, LayoutGraph};

/// Assigns initial ranks using the longest path algorithm.
/// Nodes are pushed to the lowest position possible.
pub fn longest_path(g: &mut LayoutGraph) {
    let mut visited = std::collections::HashSet::new();
    let sources = g.sources();

    fn dfs(g: &mut LayoutGraph, v: &str, visited: &mut std::collections::HashSet<String>) -> i64 {
        if visited.contains(v) {
            return g.node(v).and_then(|n| n.rank).unwrap_or(0);
        }
        visited.insert(v.to_string());

        let out_edges = g.out_edges(v, None).unwrap_or_default();
        let mut min_rank = i64::MAX;

        for e in &out_edges {
            let minlen = g.edge_by_obj(e).map(|l| l.minlen as i64).unwrap_or(1);
            let w_rank = dfs(g, &e.w, visited);
            let candidate = w_rank - minlen;
            if candidate < min_rank {
                min_rank = candidate;
            }
        }

        if min_rank == i64::MAX {
            min_rank = 0;
        }

        if let Some(node) = g.node_mut(v) {
            node.rank = Some(min_rank);
        }
        min_rank
    }

    for v in sources {
        dfs(g, &v, &mut visited);
    }
}

/// Returns the slack for the given edge: rank(w) - rank(v) - minlen.
pub fn slack(g: &LayoutGraph, e: &Edge) -> i64 {
    let w_rank = g.node(&e.w).and_then(|n| n.rank).unwrap_or(0);
    let v_rank = g.node(&e.v).and_then(|n| n.rank).unwrap_or(0);
    let minlen = g.edge_by_obj(e).map(|l| l.minlen as i64).unwrap_or(1);
    w_rank - v_rank - minlen
}
