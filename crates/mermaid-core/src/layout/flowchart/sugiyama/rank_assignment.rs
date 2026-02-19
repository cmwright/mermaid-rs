use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};

use crate::ast::flowchart::{FlowchartAst, SubgraphDef};
use crate::layout::flowchart::types::*;

/// Longest-path rank assignment.
/// Process nodes in topological order: rank[n] = max(rank[predecessor] + 1).
/// Returns a map from NodeIndex to rank (0-based).
pub fn assign_ranks(graph: &DiGraph<NodeData, EdgeData>) -> HashMap<NodeIndex, usize> {
    let mut ranks: HashMap<NodeIndex, usize> = HashMap::new();

    // Topological sort using Kahn's algorithm
    let topo_order = topological_sort(graph);

    for node in &topo_order {
        let max_pred_rank = graph
            .neighbors_directed(*node, petgraph::Direction::Incoming)
            .filter_map(|pred| ranks.get(&pred))
            .max()
            .copied();

        let rank = match max_pred_rank {
            Some(r) => r + 1,
            None => 0,
        };
        ranks.insert(*node, rank);
    }

    // Handle any nodes not reached by topo sort (disconnected components)
    for node in graph.node_indices() {
        ranks.entry(node).or_insert(0);
    }

    ranks
}

/// Convert rank map to layers: Vec<Vec<NodeIndex>> indexed by rank.
/// Returns two candidate orderings — one from DFS (better for complex
/// dependency graphs) and one from Kahn's topological sort (better for
/// simpler graphs).  The crossing-minimisation pass picks whichever
/// produces fewer crossings.
pub fn ranks_to_layers(
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &HashMap<NodeIndex, usize>,
) -> Vec<Vec<NodeIndex>> {
    if ranks.is_empty() {
        return Vec::new();
    }

    let max_rank = *ranks.values().max().unwrap();
    let mut layers = vec![Vec::new(); max_rank + 1];

    let dfs_order = dfs_ordering(graph);
    for node in dfs_order {
        if let Some(&rank) = ranks.get(&node) {
            layers[rank].push(node);
        }
    }

    layers
}

/// Like `ranks_to_layers` but returns a second candidate using Kahn's
/// topological sort instead of DFS.
pub fn ranks_to_layers_alt(
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &HashMap<NodeIndex, usize>,
) -> Vec<Vec<NodeIndex>> {
    if ranks.is_empty() {
        return Vec::new();
    }

    let max_rank = *ranks.values().max().unwrap();
    let mut layers = vec![Vec::new(); max_rank + 1];

    let topo_order = topological_sort(graph);
    for node in topo_order {
        if let Some(&rank) = ranks.get(&node) {
            layers[rank].push(node);
        }
    }

    layers
}

/// DFS-based ordering that keeps connected nodes closer together in the
/// initial layer assignment.  Starting from roots (no incoming edges), we
/// recursively visit children before moving to the next root.  This tends
/// to place siblings adjacent in their shared layer, giving the barycenter
/// heuristic a better starting point.
fn dfs_ordering(graph: &DiGraph<NodeData, EdgeData>) -> Vec<NodeIndex> {
    let mut visited = HashSet::new();
    let mut result = Vec::with_capacity(graph.node_count());

    // Start from roots (nodes with no incoming edges), sorted for determinism
    let mut roots: Vec<NodeIndex> = graph
        .node_indices()
        .filter(|&n| {
            graph
                .neighbors_directed(n, petgraph::Direction::Incoming)
                .next()
                .is_none()
        })
        .collect();
    roots.sort_by_key(|n| n.index());

    for root in roots {
        dfs_visit(graph, root, &mut visited, &mut result);
    }

    // Handle any remaining nodes (disconnected or in cycles that escaped removal)
    let mut remaining: Vec<NodeIndex> = graph
        .node_indices()
        .filter(|n| !visited.contains(n))
        .collect();
    remaining.sort_by_key(|n| n.index());
    for node in remaining {
        dfs_visit(graph, node, &mut visited, &mut result);
    }

    result
}

fn dfs_visit(
    graph: &DiGraph<NodeData, EdgeData>,
    node: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
    result: &mut Vec<NodeIndex>,
) {
    if !visited.insert(node) {
        return;
    }
    result.push(node);

    // Visit children sorted by index for determinism
    let mut children: Vec<NodeIndex> = graph
        .neighbors_directed(node, petgraph::Direction::Outgoing)
        .collect();
    children.sort_by_key(|n| n.index());

    for child in children {
        dfs_visit(graph, child, visited, result);
    }
}

/// Align ranks of nodes in sibling subgraphs so that subgraphs at the same
/// dependency tier start at the same rank. Processes depth levels top-down,
/// re-propagating ranks after each level to cascade alignment effects.
pub fn align_sibling_subgraph_ranks(
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    ast: &FlowchartAst,
) {
    let id_to_idx: HashMap<String, NodeIndex> = graph
        .node_indices()
        .map(|idx| (graph[idx].id.clone(), idx))
        .collect();

    let groups_by_depth = collect_sibling_groups_by_depth(ast);

    for depth_groups in &groups_by_depth {
        let mut changed = false;
        for siblings in depth_groups {
            if siblings.len() <= 1 {
                continue;
            }
            changed |= align_one_sibling_group(siblings, graph, ranks, &id_to_idx);
        }
        if changed {
            propagate_ranks_forward(graph, ranks);
        }
    }
}

/// Collect sibling subgraph groups organized by depth level.
fn collect_sibling_groups_by_depth(ast: &FlowchartAst) -> Vec<Vec<&[SubgraphDef]>> {
    let mut result: Vec<Vec<&[SubgraphDef]>> = Vec::new();
    collect_groups_recursive(&ast.subgraphs, 0, &mut result);
    result
}

fn collect_groups_recursive<'a>(
    subgraphs: &'a [SubgraphDef],
    depth: usize,
    result: &mut Vec<Vec<&'a [SubgraphDef]>>,
) {
    if subgraphs.is_empty() {
        return;
    }
    while result.len() <= depth {
        result.push(Vec::new());
    }
    result[depth].push(subgraphs);
    for sg in subgraphs {
        collect_groups_recursive(&sg.subgraphs, depth + 1, result);
    }
}

/// Align one group of sibling subgraphs within each dependency tier.
fn align_one_sibling_group(
    siblings: &[SubgraphDef],
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    id_to_idx: &HashMap<String, NodeIndex>,
) -> bool {
    let sg_nodes: Vec<HashSet<NodeIndex>> = siblings
        .iter()
        .map(|sg| {
            let mut nodes = HashSet::new();
            collect_descendant_node_indices(sg, id_to_idx, &mut nodes);
            nodes
        })
        .collect();

    let tiers = compute_dependency_tiers(&sg_nodes, graph);

    let mut changed = false;
    for tier in &tiers {
        if tier.len() <= 1 {
            continue;
        }

        // Align by max rank (the "output" level of each subgraph) so that
        // downstream subgraphs begin at a consistent rank.
        let target_max_rank = tier
            .iter()
            .filter_map(|&sg_idx| {
                sg_nodes[sg_idx]
                    .iter()
                    .filter_map(|n| ranks.get(n))
                    .max()
                    .copied()
            })
            .max()
            .unwrap_or(0);

        for &sg_idx in tier {
            let sg_max_rank = sg_nodes[sg_idx]
                .iter()
                .filter_map(|n| ranks.get(n))
                .max()
                .copied()
                .unwrap_or(0);

            let delta = target_max_rank - sg_max_rank;
            if delta > 0 {
                for &node_idx in &sg_nodes[sg_idx] {
                    if let Some(r) = ranks.get_mut(&node_idx) {
                        *r += delta;
                    }
                }
                changed = true;
            }
        }
    }

    changed
}

/// Recursively collect all node indices that are descendants of a subgraph.
fn collect_descendant_node_indices(
    sg: &SubgraphDef,
    id_to_idx: &HashMap<String, NodeIndex>,
    result: &mut HashSet<NodeIndex>,
) {
    for node in &sg.nodes {
        if let Some(&idx) = id_to_idx.get(&node.id) {
            result.insert(idx);
        }
    }
    for edge in &sg.edges {
        for id in [&edge.from, &edge.to] {
            if let Some(&idx) = id_to_idx.get(id) {
                result.insert(idx);
            }
        }
    }
    for child_sg in &sg.subgraphs {
        collect_descendant_node_indices(child_sg, id_to_idx, result);
    }
}

/// Compute dependency tiers among sibling subgraphs using topological ordering.
/// Siblings in the same tier have no dependency edges between them.
fn compute_dependency_tiers(
    sg_nodes: &[HashSet<NodeIndex>],
    graph: &DiGraph<NodeData, EdgeData>,
) -> Vec<Vec<usize>> {
    let n = sg_nodes.len();
    if n == 0 {
        return Vec::new();
    }

    let mut node_to_sg: HashMap<NodeIndex, usize> = HashMap::new();
    for (i, nodes) in sg_nodes.iter().enumerate() {
        for &node in nodes {
            node_to_sg.insert(node, i);
        }
    }

    let mut has_edge: HashSet<(usize, usize)> = HashSet::new();
    let mut in_degree: Vec<usize> = vec![0; n];

    for edge_idx in graph.edge_indices() {
        if let Some((src, tgt)) = graph.edge_endpoints(edge_idx) {
            if let (Some(&src_sg), Some(&tgt_sg)) = (node_to_sg.get(&src), node_to_sg.get(&tgt)) {
                if src_sg != tgt_sg && has_edge.insert((src_sg, tgt_sg)) {
                    in_degree[tgt_sg] += 1;
                }
            }
        }
    }

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(src, tgt) in &has_edge {
        adj[src].push(tgt);
    }
    for deps in &mut adj {
        deps.sort_unstable();
        deps.dedup();
    }

    let mut tiers: Vec<Vec<usize>> = Vec::new();
    let mut queue: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
    let mut processed = HashSet::new();

    while !queue.is_empty() {
        tiers.push(queue.clone());
        for &sg in &queue {
            processed.insert(sg);
        }
        let mut next_queue = Vec::new();
        for &sg in &queue {
            for &dep in &adj[sg] {
                in_degree[dep] -= 1;
                if in_degree[dep] == 0 {
                    next_queue.push(dep);
                }
            }
        }
        queue = next_queue;
    }

    if processed.len() < n {
        let remaining: Vec<usize> = (0..n).filter(|i| !processed.contains(i)).collect();
        if !remaining.is_empty() {
            tiers.push(remaining);
        }
    }

    tiers
}

/// Re-propagate ranks forward to ensure all edges go from lower to higher rank.
fn propagate_ranks_forward(
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
) {
    let topo = topological_sort(graph);
    for node in &topo {
        let max_pred = graph
            .neighbors_directed(*node, petgraph::Direction::Incoming)
            .filter_map(|pred| ranks.get(&pred))
            .max()
            .copied();
        if let Some(max_pred_rank) = max_pred {
            let min_rank = max_pred_rank + 1;
            if let Some(r) = ranks.get_mut(node) {
                if *r < min_rank {
                    *r = min_rank;
                }
            }
        }
    }
}

fn topological_sort(graph: &DiGraph<NodeData, EdgeData>) -> Vec<NodeIndex> {
    let node_count = graph.node_count();
    let mut in_degree: HashMap<NodeIndex, usize> = HashMap::with_capacity(node_count);
    for node in graph.node_indices() {
        in_degree.insert(node, 0);
    }
    for edge in graph.edge_indices() {
        let (_, tgt) = graph.edge_endpoints(edge).unwrap();
        *in_degree.entry(tgt).or_insert(0) += 1;
    }

    let mut queue: Vec<NodeIndex> = graph
        .node_indices()
        .filter(|n| *in_degree.get(n).unwrap_or(&0) == 0)
        .collect();
    let mut result = Vec::with_capacity(node_count);

    while let Some(node) = queue.pop() {
        result.push(node);
        for neighbor in graph.neighbors_directed(node, petgraph::Direction::Outgoing) {
            if let Some(deg) = in_degree.get_mut(&neighbor) {
                *deg = deg.saturating_sub(1);
                if *deg == 0 {
                    queue.push(neighbor);
                }
            }
        }
    }

    // Add any remaining nodes (in cycles — shouldn't happen after cycle removal)
    if result.len() < node_count {
        let visited: HashSet<NodeIndex> = result.iter().copied().collect();
        for node in graph.node_indices() {
            if !visited.contains(&node) {
                result.push(node);
            }
        }
    }

    result
}
