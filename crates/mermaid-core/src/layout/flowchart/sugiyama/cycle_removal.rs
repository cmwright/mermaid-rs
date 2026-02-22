use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashSet;

use crate::layout::flowchart::types::*;

/// DFS-based greedy feedback arc set. Finds back-edges in the graph and
/// reverses them to make the graph acyclic. Returns the set of reversed
/// edge indices so they can be restored later.
///
/// Like dagre, we start DFS from source nodes (in-degree 0) first. This
/// ensures that the "natural" forward edges from sources are traversed
/// first, so back-edges in cycles are correctly identified as the
/// reverse-direction edges rather than the forward ones.
pub fn remove_cycles(graph: &mut DiGraph<NodeData, EdgeData>) -> Vec<EdgeIndex> {
    let mut visited = HashSet::new();
    let mut on_stack = HashSet::new();
    let mut back_edges = Vec::new();

    // Start from source nodes (in-degree 0) first, then remaining nodes.
    // This matches dagre's dfsFAS behavior and produces better cycle breaks.
    let mut sources: Vec<NodeIndex> = Vec::new();
    let mut non_sources: Vec<NodeIndex> = Vec::new();
    for node in graph.node_indices() {
        let in_degree = graph
            .neighbors_directed(node, petgraph::Direction::Incoming)
            .count();
        if in_degree == 0 {
            sources.push(node);
        } else {
            non_sources.push(node);
        }
    }

    for node in sources.iter().chain(non_sources.iter()) {
        if !visited.contains(node) {
            dfs_find_back_edges(graph, *node, &mut visited, &mut on_stack, &mut back_edges);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{ArrowEnd, LineStyle};

    fn make_node(id: &str) -> NodeData {
        NodeData {
            id: id.to_string(),
            label: String::new(),
            shape: Default::default(),
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        }
    }

    fn make_edge() -> EdgeData {
        EdgeData {
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            label_width: 0.0,
            label_height: 0.0,
            weight: 1,
            minlen: 1,
        }
    }

    #[test]
    fn test_remove_cycles_simple_cycle() {
        // A -> B -> C -> A (cycle)
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        g.add_edge(a, b, make_edge());
        g.add_edge(b, c, make_edge());
        g.add_edge(c, a, make_edge());

        let reversed = remove_cycles(&mut g);
        // At least one edge should be reversed
        assert!(
            !reversed.is_empty(),
            "cycle should cause at least one edge reversal"
        );

        // Graph should now be acyclic
        // Verify by checking that a topological sort is possible
        let topo = petgraph::algo::toposort(&g, None);
        assert!(topo.is_ok(), "graph should be acyclic after cycle removal");
    }

    #[test]
    fn test_remove_cycles_no_cycle() {
        // A -> B -> C (no cycle)
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        g.add_edge(a, b, make_edge());
        g.add_edge(b, c, make_edge());

        let reversed = remove_cycles(&mut g);
        assert!(reversed.is_empty(), "no cycle means no reversals");
    }

    #[test]
    fn test_restore_cycles() {
        // A -> B -> C -> A
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        g.add_edge(a, b, make_edge());
        g.add_edge(b, c, make_edge());
        g.add_edge(c, a, make_edge());

        let original_edge_count = g.edge_count();

        let reversed = remove_cycles(&mut g);
        assert!(!reversed.is_empty());

        // Restore should bring back original edge direction
        restore_cycles(&mut g, &reversed);

        // Edge count should be preserved
        assert_eq!(g.edge_count(), original_edge_count);
    }

    #[test]
    fn test_remove_cycles_multiple_cycles() {
        // A -> B -> A, B -> C -> B
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        g.add_edge(a, b, make_edge());
        g.add_edge(b, a, make_edge());
        g.add_edge(b, c, make_edge());
        g.add_edge(c, b, make_edge());

        let reversed = remove_cycles(&mut g);
        // Should handle multiple cycles
        let topo = petgraph::algo::toposort(&g, None);
        assert!(
            topo.is_ok(),
            "graph should be acyclic after removing multiple cycles"
        );

        // Restore
        restore_cycles(&mut g, &reversed);
        assert_eq!(g.edge_count(), 4);
    }

    #[test]
    fn test_remove_cycles_disconnected_with_cycle() {
        // Component 1: A -> B (no cycle)
        // Component 2: C -> D -> C (cycle)
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, b, make_edge());
        g.add_edge(c, d, make_edge());
        g.add_edge(d, c, make_edge());

        let reversed = remove_cycles(&mut g);
        assert!(!reversed.is_empty());

        let topo = petgraph::algo::toposort(&g, None);
        assert!(topo.is_ok());
    }
}
