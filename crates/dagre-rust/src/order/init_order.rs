//! Initial order assignment via DFS.
//! Port of dagre's `order/init-order.js`.

use crate::graph::LayoutGraph;
use std::collections::HashSet;

/// Assigns initial order by DFS from nodes sorted by rank.
pub fn init_order(g: &LayoutGraph) -> Vec<Vec<String>> {
    let mut visited = HashSet::new();

    let simple_nodes: Vec<String> = g
        .node_ids()
        .iter()
        .filter(|v| g.children(Some(v)).is_none_or(|c| c.is_empty()))
        .cloned()
        .collect();

    let max_rank = simple_nodes
        .iter()
        .filter_map(|v| g.node(v).and_then(|n| n.rank))
        .max()
        .unwrap_or(0);

    let mut layers: Vec<Vec<String>> = (0..=max_rank).map(|_| Vec::new()).collect();

    fn dfs(g: &LayoutGraph, v: &str, visited: &mut HashSet<String>, layers: &mut Vec<Vec<String>>) {
        if visited.contains(v) {
            return;
        }
        visited.insert(v.to_string());
        if let Some(node) = g.node(v)
            && let Some(rank) = node.rank
            && (rank as usize) < layers.len()
        {
            layers[rank as usize].push(v.to_string());
        }
        for w in g.successors(v).unwrap_or_default() {
            dfs(g, &w, visited, layers);
        }
    }

    let mut ordered_vs = simple_nodes;
    ordered_vs.sort_by_key(|v| g.node(v).and_then(|n| n.rank).unwrap_or(0));

    for v in &ordered_vs {
        dfs(g, v, &mut visited, &mut layers);
    }

    layers
}
