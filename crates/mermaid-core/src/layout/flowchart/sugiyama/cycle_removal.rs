use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashSet;

use crate::layout::flowchart::types::*;

/// DFS-based greedy feedback arc set. Finds back-edges in the graph and
/// reverses them to make the graph acyclic. Returns the set of reversed
/// edge indices so they can be restored later.
pub fn remove_cycles(graph: &mut DiGraph<NodeData, EdgeData>) -> Vec<EdgeIndex> {
    let mut visited = HashSet::new();
    let mut on_stack = HashSet::new();
    let mut back_edges = Vec::new();

    for node in graph.node_indices().collect::<Vec<_>>() {
        if !visited.contains(&node) {
            dfs_find_back_edges(graph, node, &mut visited, &mut on_stack, &mut back_edges);
        }
    }

    // Reverse back-edges
    let mut reversed = Vec::new();
    for edge_idx in back_edges {
        let (src, tgt) = graph.edge_endpoints(edge_idx).unwrap();
        let data = graph.remove_edge(edge_idx).unwrap();
        let new_idx = graph.add_edge(tgt, src, data);
        reversed.push(new_idx);
    }

    reversed
}

fn dfs_find_back_edges(
    graph: &DiGraph<NodeData, EdgeData>,
    node: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
    on_stack: &mut HashSet<NodeIndex>,
    back_edges: &mut Vec<EdgeIndex>,
) {
    visited.insert(node);
    on_stack.insert(node);

    let edges: Vec<_> = graph
        .edges_directed(node, petgraph::Direction::Outgoing)
        .map(|e| (e.id(), e.target()))
        .collect();

    for (edge_id, target) in edges {
        if on_stack.contains(&target) {
            back_edges.push(edge_id);
        } else if !visited.contains(&target) {
            dfs_find_back_edges(graph, target, visited, on_stack, back_edges);
        }
    }

    on_stack.remove(&node);
}

/// Restore reversed edges to their original direction.
pub fn restore_cycles(graph: &mut DiGraph<NodeData, EdgeData>, reversed: &[EdgeIndex]) {
    for &edge_idx in reversed {
        if let Some((src, tgt)) = graph.edge_endpoints(edge_idx) {
            let data = graph.remove_edge(edge_idx).unwrap();
            graph.add_edge(tgt, src, data);
        }
    }
}
