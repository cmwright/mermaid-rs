use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::flowchart::{FlowchartAst, SubgraphDef};
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;

/// Network simplex rank assignment.
/// Produces optimal rank assignments that minimize total edge length.
/// Falls back gracefully for trivial / disconnected graphs.
/// Returns a map from NodeIndex to rank (0-based).
pub fn assign_ranks(graph: &DiGraph<NodeData, EdgeData>) -> HashMap<NodeIndex, usize> {
    if graph.node_count() == 0 {
        return HashMap::new();
    }

    // Find connected components (treating edges as undirected)
    let mut component_id: HashMap<NodeIndex, usize> = HashMap::new();
    let mut num_components = 0usize;

    for start in graph.node_indices() {
        if component_id.contains_key(&start) {
            continue;
        }
        let cid = num_components;
        num_components += 1;
        let mut queue = VecDeque::new();
        queue.push_back(start);
        component_id.insert(start, cid);
        while let Some(node) = queue.pop_front() {
            for neighbor in graph.neighbors_directed(node, petgraph::Direction::Outgoing) {
                if !component_id.contains_key(&neighbor) {
                    component_id.insert(neighbor, cid);
                    queue.push_back(neighbor);
                }
            }
            for neighbor in graph.neighbors_directed(node, petgraph::Direction::Incoming) {
                if !component_id.contains_key(&neighbor) {
                    component_id.insert(neighbor, cid);
                    queue.push_back(neighbor);
                }
            }
        }
    }

    // Group nodes by component
    let mut components: Vec<Vec<NodeIndex>> = vec![Vec::new(); num_components];
    for (&node, &cid) in &component_id {
        components[cid].push(node);
    }
    // Deterministic ordering within each component
    for comp in &mut components {
        comp.sort_unstable();
    }

    let mut ranks: HashMap<NodeIndex, usize> = HashMap::new();

    for comp_nodes in &components {
        if comp_nodes.len() == 1 {
            ranks.insert(comp_nodes[0], 0);
            continue;
        }

        // Check if this component has any edges
        let comp_set: HashSet<NodeIndex> = comp_nodes.iter().copied().collect();
        let has_edges = comp_nodes.iter().any(|&n| {
            graph
                .neighbors_directed(n, petgraph::Direction::Outgoing)
                .any(|nb| comp_set.contains(&nb))
        });

        if !has_edges {
            for &node in comp_nodes {
                ranks.insert(node, 0);
            }
            continue;
        }

        let comp_ranks = network_simplex(graph, comp_nodes);
        for (node, rank) in comp_ranks {
            ranks.insert(node, rank);
        }
    }

    // Safety: any node not yet assigned gets rank 0
    for node in graph.node_indices() {
        ranks.entry(node).or_insert(0);
    }

    ranks
}

/// Network simplex rank assignment on a non-compound graph, matching dagre's
/// `rank(util.asNonCompoundGraph(g))`.
///
/// `asNonCompoundGraph` strips nodes that have children in the compound graph —
/// in dagre's case, this means subgraph container nodes (like "Files", "RBAC")
/// and the nesting root. The nesting border nodes (bt, bb) are KEPT because
/// they are leaf nodes in the compound hierarchy (children of subgraph nodes,
/// but have no children themselves).
///
/// Inside network simplex, parallel edges are merged (sum weights, max minlen)
/// matching dagre's `simplify`.
pub fn assign_ranks_non_compound(
    graph: &DiGraph<NodeData, EdgeData>,
    nesting_state: &super::nesting_graph::NestingState,
) -> HashMap<NodeIndex, usize> {
    if graph.node_count() == 0 {
        return HashMap::new();
    }

    // In dagre, asNonCompoundGraph strips nodes with children.
    // The nesting root has children (everything connected to it) → strip it.
    // Subgraph container nodes don't exist in our graph (we don't add them).
    // The nesting bt/bb nodes are leaf nodes → KEEP them.
    // So we only strip the root node.
    let mut compound_nodes: HashSet<NodeIndex> = HashSet::new();
    compound_nodes.insert(nesting_state.root);

    // Collect leaf nodes (everything except compound nodes).
    let leaf_nodes: Vec<NodeIndex> = graph
        .node_indices()
        .filter(|ni| !compound_nodes.contains(ni))
        .collect();

    if leaf_nodes.is_empty() {
        return HashMap::new();
    }

    let leaf_set: HashSet<NodeIndex> = leaf_nodes.iter().copied().collect();

    // Find connected components among leaf nodes (treating edges as undirected).
    let mut component_id: HashMap<NodeIndex, usize> = HashMap::new();
    let mut num_components = 0usize;

    for &start in &leaf_nodes {
        if component_id.contains_key(&start) {
            continue;
        }
        let cid = num_components;
        num_components += 1;
        let mut queue = VecDeque::new();
        queue.push_back(start);
        component_id.insert(start, cid);
        while let Some(node) = queue.pop_front() {
            for neighbor in graph.neighbors_directed(node, petgraph::Direction::Outgoing) {
                if leaf_set.contains(&neighbor) && !component_id.contains_key(&neighbor) {
                    component_id.insert(neighbor, cid);
                    queue.push_back(neighbor);
                }
            }
            for neighbor in graph.neighbors_directed(node, petgraph::Direction::Incoming) {
                if leaf_set.contains(&neighbor) && !component_id.contains_key(&neighbor) {
                    component_id.insert(neighbor, cid);
                    queue.push_back(neighbor);
                }
            }
        }
    }

    // Group leaf nodes by component
    let mut components: Vec<Vec<NodeIndex>> = vec![Vec::new(); num_components];
    for (&node, &cid) in &component_id {
        components[cid].push(node);
    }
    // Deterministic ordering within each component
    for comp in &mut components {
        comp.sort_unstable();
    }

    let mut ranks: HashMap<NodeIndex, usize> = HashMap::new();

    for comp_nodes in &components {
        if comp_nodes.len() == 1 {
            ranks.insert(comp_nodes[0], 0);
            continue;
        }

        let comp_set: HashSet<NodeIndex> = comp_nodes.iter().copied().collect();
        let has_edges = comp_nodes.iter().any(|&n| {
            graph
                .neighbors_directed(n, petgraph::Direction::Outgoing)
                .any(|nb| comp_set.contains(&nb))
        });

        if !has_edges {
            for &node in comp_nodes {
                ranks.insert(node, 0);
            }
            continue;
        }

        let comp_ranks = network_simplex(graph, comp_nodes);
        for (node, rank) in comp_ranks {
            ranks.insert(node, rank);
        }
    }

    // Safety: any leaf node not yet assigned gets rank 0
    for &node in &leaf_nodes {
        ranks.entry(node).or_insert(0);
    }

    ranks
}

// ── Network Simplex Implementation ─────────────────────────────────────────

/// Check if node `w` is in the subtree rooted at `v` using low/lim numbering.
/// w is in subtree of v iff low[v] <= lim[w] && lim[w] <= lim[v]
#[inline]
fn ns_in_subtree(w: usize, v: usize, low: &[usize], lim: &[usize]) -> bool {
    low[v] <= lim[w] && lim[w] <= lim[v]
}

/// Compute post-order numbering and parent pointers for the spanning tree.
/// Sets lim[v] = post-order number of v, low[v] = min post-order in subtree of v.
fn ns_compute_tree_order(
    root: usize,
    tree_adj: &[Vec<(usize, usize)>],
    lim: &mut [usize],
    low: &mut [usize],
    par: &mut [Option<usize>],
) {
    let n = lim.len();
    let mut visited = vec![false; n];
    // Iterative DFS: stack of (node, adj_index)
    let mut stack: Vec<(usize, usize)> = Vec::new();

    visited[root] = true;
    par[root] = None;
    stack.push((root, 0));

    let mut post_num: usize = 0;

    while let Some(frame) = stack.last_mut() {
        let node = frame.0;
        let adj_idx = &mut frame.1;

        let mut pushed_child = false;
        while *adj_idx < tree_adj[node].len() {
            let (nb, _ei) = tree_adj[node][*adj_idx];
            *adj_idx += 1;
            if !visited[nb] {
                visited[nb] = true;
                par[nb] = Some(node);
                stack.push((nb, 0));
                pushed_child = true;
                break;
            }
        }

        if !pushed_child {
            // Post-order visit: all children have been processed
            let node = stack.pop().unwrap().0;
            lim[node] = post_num;
            low[node] = post_num;
            post_num += 1;

            // Extend low/lim to cover children's subtrees
            for &(nb, _) in &tree_adj[node] {
                if par[nb] == Some(node) {
                    if low[nb] < low[node] {
                        low[node] = low[nb];
                    }
                    if lim[nb] > lim[node] {
                        lim[node] = lim[nb];
                    }
                }
            }
        }
    }
}

/// Compute cut values for all tree edges.
///
/// For each tree edge, removing it partitions the tree into two components.
/// The "tail" component is the subtree of the child node.
/// cut_value = (sum of weights of edges from tail to head)
///           - (sum of weights of edges from head to tail)
///
/// A negative cut value for a tree edge directed s->t (where the tree edge
/// direction matches the original graph direction) means we can improve the
/// objective by pivoting.
fn ns_compute_cut_values(
    m: usize,
    local_edges: &[(usize, usize)],
    tree_edge: &[bool],
    edge_weights: &[i64],
    cut_value: &mut [i64],
    lim: &[usize],
    low: &[usize],
    par: &[Option<usize>],
) {
    for ei in 0..m {
        if !tree_edge[ei] {
            cut_value[ei] = 0;
            continue;
        }
        let (s, t) = local_edges[ei];

        // Determine which end is the child in the spanning tree
        let (tail_root, _head_side) = if par[t] == Some(s) {
            (t, s)
        } else if par[s] == Some(t) {
            (s, t)
        } else {
            cut_value[ei] = 0;
            continue;
        };

        // Does the original graph edge go from tail to head?
        let tree_edge_from_tail = tail_root == s;

        let mut cv: i64 = 0;
        for ej in 0..m {
            let (es, et) = local_edges[ej];
            let s_in_tail = ns_in_subtree(es, tail_root, low, lim);
            let t_in_tail = ns_in_subtree(et, tail_root, low, lim);

            if s_in_tail && !t_in_tail {
                // Edge from tail to head
                cv += edge_weights[ej];
            } else if !s_in_tail && t_in_tail {
                // Edge from head to tail
                cv -= edge_weights[ej];
            }
        }

        // Negate if tree edge goes from head to tail (so that negative means improvable)
        if !tree_edge_from_tail {
            cv = -cv;
        }

        cut_value[ei] = cv;
    }
}

/// Run the network simplex algorithm on a single connected component.
/// Includes `simplify` (dagre's util.simplify): merges parallel edges between
/// the same node pair by summing weights and taking max minlen.
/// Returns a map from NodeIndex to rank (0-based, normalized).
fn network_simplex(
    graph: &DiGraph<NodeData, EdgeData>,
    component: &[NodeIndex],
) -> HashMap<NodeIndex, usize> {
    let comp_set: HashSet<NodeIndex> = component.iter().copied().collect();

    // Collect directed edges within this component
    let raw_edges: Vec<(NodeIndex, NodeIndex, EdgeIndex)> = graph
        .edge_indices()
        .filter_map(|ei| {
            let (src, tgt) = graph.edge_endpoints(ei).unwrap();
            if comp_set.contains(&src) && comp_set.contains(&tgt) {
                Some((src, tgt, ei))
            } else {
                None
            }
        })
        .collect();

    if raw_edges.is_empty() {
        return component.iter().map(|&n| (n, 0)).collect();
    }

    // Map NodeIndex to dense local indices 0..n
    let node_to_local: HashMap<NodeIndex, usize> =
        component.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    let n = component.len();

    // ── Simplify: merge parallel edges (dagre's util.simplify) ─────────
    // For each (src, tgt) pair, sum weights and take max minlen.
    let mut merged: HashMap<(usize, usize), (i64, i64)> = HashMap::new();
    for &(src, tgt, ei) in &raw_edges {
        let ls = node_to_local[&src];
        let lt = node_to_local[&tgt];
        let w = graph[ei].weight;
        let ml = graph[ei].minlen as i64;
        let entry = merged.entry((ls, lt)).or_insert((0, 1));
        entry.0 += w; // sum weights
        entry.1 = entry.1.max(ml); // max minlen
    }

    let mut local_edges: Vec<(usize, usize)> = merged.keys().copied().collect();
    local_edges.sort_unstable(); // Deterministic edge ordering
    let edge_weights: Vec<i64> = local_edges.iter().map(|k| merged[k].0).collect();
    let edge_minlens: Vec<i64> = local_edges.iter().map(|k| merged[k].1).collect();
    let m = local_edges.len();

    // ── Step 1: Initial feasible rank assignment via longest-path ───────
    // Matches dagre's longestPath (rank/util.js) exactly:
    //   - DFS from source nodes (no in-edges)
    //   - Sinks (no out-edges) get rank 0
    //   - For each node: rank = min(dfs(successor) - minlen)
    //   - Produces negative ranks that get normalized later
    let mut rank: Vec<i64> = vec![0; n];
    {
        let mut out_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_count: Vec<usize> = vec![0; n];
        for (ei, &(s, t)) in local_edges.iter().enumerate() {
            out_adj[s].push(ei);
            in_count[t] += 1;
        }

        // dagre: g.sources().forEach(dfs)
        // Sources are nodes with no incoming edges.
        let sources: Vec<usize> = (0..n).filter(|&i| in_count[i] == 0).collect();

        // Iterative DFS matching dagre's recursive dfs(v):
        //   if visited[v] { return rank[v]; }
        //   visited[v] = true;
        //   rank[v] = min(dfs(w) - minlen for (v,w) in outEdges(v));
        //   if no out-edges: rank[v] = 0;
        let mut visited = vec![false; n];

        // Stack frames: (node, adj_index, current_min)
        // When adj_index == out_adj[node].len(), we've visited all successors.
        let mut stack: Vec<(usize, usize, i64)> = Vec::new();

        for &source in &sources {
            if visited[source] {
                continue;
            }
            stack.push((source, 0, i64::MAX));

            while let Some(frame) = stack.last_mut() {
                let node = frame.0;
                let adj_idx = frame.1;

                if !visited[node] && adj_idx == 0 {
                    // First visit: mark as in-progress
                    // (We don't mark visited yet — dagre checks visited at
                    // entry and returns early. We mark after processing.)
                }

                if adj_idx < out_adj[node].len() {
                    let ei = out_adj[node][adj_idx];
                    let (_, t) = local_edges[ei];
                    frame.1 += 1; // advance to next adjacency

                    if visited[t] {
                        // Successor already computed — use its rank
                        let candidate = rank[t] - edge_minlens[ei];
                        if candidate < frame.2 {
                            frame.2 = candidate;
                        }
                    } else {
                        // Need to recurse into successor first
                        stack.push((t, 0, i64::MAX));
                    }
                } else {
                    // All successors processed — compute rank for this node
                    let (node, _, current_min) = stack.pop().unwrap();
                    if visited[node] {
                        // Already computed (can happen with diamond patterns)
                        // Update parent frame if needed
                        if let Some(parent_frame) = stack.last_mut() {
                            let parent_node = parent_frame.0;
                            let parent_adj_idx = parent_frame.1 - 1;
                            let ei = out_adj[parent_node][parent_adj_idx];
                            let candidate = rank[node] - edge_minlens[ei];
                            if candidate < parent_frame.2 {
                                parent_frame.2 = candidate;
                            }
                        }
                        continue;
                    }

                    // dagre: rank = POSITIVE_INFINITY means no out-edges → rank = 0
                    rank[node] = if current_min == i64::MAX {
                        0
                    } else {
                        current_min
                    };
                    visited[node] = true;

                    // Propagate result back to parent frame
                    if let Some(parent_frame) = stack.last_mut() {
                        let parent_node = parent_frame.0;
                        let parent_adj_idx = parent_frame.1 - 1;
                        let ei = out_adj[parent_node][parent_adj_idx];
                        let candidate = rank[node] - edge_minlens[ei];
                        if candidate < parent_frame.2 {
                            parent_frame.2 = candidate;
                        }
                    }
                }
            }
        }

        // Safety: any unvisited nodes (disconnected) get rank 0
        // (dagre only calls dfs from sources, but the graph should be connected
        // within a component after simplification)
        for i in 0..n {
            if !visited[i] {
                rank[i] = 0;
            }
        }
    }

    // ── Step 2: Build initial feasible spanning tree ────────────────────
    // Matches dagre's feasibleTree (rank/feasible-tree.js) exactly:
    //   1. Start with an arbitrary node (node 0)
    //   2. Grow maximal tight tree via DFS (tightTree)
    //   3. If tree doesn't span all nodes:
    //      a. Find minimum slack edge crossing tree boundary (findMinSlackEdge)
    //      b. Shift ALL tree node ranks by delta (shiftRanks)
    //      c. Repeat from step 2

    let mut tree_edge: Vec<bool> = vec![false; m];
    let mut in_tree: Vec<bool> = vec![false; n];
    let mut tree_edge_count: usize = 0;

    // Start with node 0 in the tree
    in_tree[0] = true;

    // Build undirected adjacency for all edges (not just tight ones)
    let mut all_adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (ei, &(s, t)) in local_edges.iter().enumerate() {
        all_adj[s].push((t, ei));
        all_adj[t].push((s, ei));
    }

    // Repeat: grow tight tree, then shift ranks if needed
    loop {
        // tightTree: DFS from all current tree nodes, adding tight edges.
        // dagre: `function tightTree(t, g) { t.nodes().forEach(dfs); }`
        // The DFS adds any neighbor connected by a tight edge (slack == 0).
        let tree_nodes: Vec<usize> = (0..n).filter(|&i| in_tree[i]).collect();
        let mut stack: Vec<usize> = tree_nodes;
        while let Some(node) = stack.pop() {
            for &(nb, ei) in &all_adj[node] {
                if in_tree[nb] {
                    continue;
                }
                // Check if this edge is tight (slack == 0)
                let (s, t) = local_edges[ei];
                let slack = rank[t] - rank[s] - edge_minlens[ei];
                if slack == 0 {
                    in_tree[nb] = true;
                    tree_edge[ei] = true;
                    tree_edge_count += 1;
                    stack.push(nb);
                }
            }
        }

        if tree_edge_count >= n - 1 {
            break; // All nodes in tree
        }

        // findMinSlackEdge: find edge with smallest slack crossing tree boundary.
        // dagre: edge where exactly one endpoint is in tree.
        let mut min_slack = i64::MAX;
        let mut min_edge: Option<usize> = None;
        for ei in 0..m {
            let (s, t) = local_edges[ei];
            if in_tree[s] != in_tree[t] {
                let slack = rank[t] - rank[s] - edge_minlens[ei];
                if slack < min_slack {
                    min_slack = slack;
                    min_edge = Some(ei);
                }
            }
        }

        // shiftRanks: shift all tree node ranks by delta.
        // dagre: delta = t.hasNode(edge.v) ? slack : -slack
        // If the source of the min-slack edge is in the tree, shift tree up by +slack.
        // If the target is in the tree, shift tree down by -slack.
        if let Some(ei) = min_edge {
            let (s, _t) = local_edges[ei];
            let delta = if in_tree[s] { min_slack } else { -min_slack };
            for i in 0..n {
                if in_tree[i] {
                    rank[i] += delta;
                }
            }
        } else {
            break; // No crossing edge found (disconnected graph)
        }
    }

    // ── Step 3: Build tree adjacency and compute tree metadata ──────────
    let mut tree_adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for ei in 0..m {
        if tree_edge[ei] {
            let (s, t) = local_edges[ei];
            tree_adj[s].push((t, ei));
            tree_adj[t].push((s, ei));
        }
    }

    let tree_root: usize = 0;
    let mut lim: Vec<usize> = vec![0; n];
    let mut low: Vec<usize> = vec![0; n];
    let mut par: Vec<Option<usize>> = vec![None; n];

    ns_compute_tree_order(tree_root, &tree_adj, &mut lim, &mut low, &mut par);

    // ── Step 4: Compute initial cut values ──────────────────────────────
    let mut cut_value: Vec<i64> = vec![0; m];
    ns_compute_cut_values(
        m,
        &local_edges,
        &tree_edge,
        &edge_weights,
        &mut cut_value,
        &lim,
        &low,
        &par,
    );

    // ── Step 5: Pivot loop ──────────────────────────────────────────────
    let max_iterations = (n * m).max(n * n) + 1;
    for _iter in 0..max_iterations {
        // leaveEdge: find FIRST tree edge with negative cut value.
        // dagre: tree.edges().find(e => tree.edge(e).cutvalue < 0)
        // Note: dagre finds the FIRST (not most negative), so order matters.
        let leaving = {
            let mut found: Option<usize> = None;
            for ei in 0..m {
                if tree_edge[ei] && cut_value[ei] < 0 {
                    found = Some(ei);
                    break;
                }
            }
            match found {
                Some(e) => e,
                None => break, // Optimal - no negative cut values
            }
        };

        // enterEdge: matching dagre's enterEdge(t, g, edge) exactly.
        //
        // 1. Find the graph direction: v is tail (source), w is head (target).
        //    In our local_edges, (s, t) = (source, target) in the directed graph.
        let (ls, lt) = local_edges[leaving];

        // 2. Determine which subtree is the "tail" component.
        //    dagre: if (vLabel.lim > wLabel.lim) → root is in v's subtree,
        //    so tailLabel = wLabel, flip = true.
        //
        //    In our tree: lim[v] > lim[w] means v's post-order number is higher,
        //    so v is closer to the root (or IS the root). The tail component
        //    (the one that would be disconnected) is w's subtree.
        let (tail_root, flip) = if lim[ls] > lim[lt] {
            (lt, true) // root is in ls's side, tail = lt's subtree
        } else {
            (ls, false) // root is in lt's side, tail = ls's subtree
        };

        // 3. Filter candidates: non-tree edges where
        //    flip === isDescendant(edge.v, tailLabel) &&
        //    flip !== isDescendant(edge.w, tailLabel)
        //
        // When flip=false: edge.v NOT in tail, edge.w in tail
        //   → edge goes from head component into tail component
        // When flip=true: edge.v in tail, edge.w NOT in tail
        //   → edge goes from tail component into head component
        let mut entering: Option<usize> = None;
        let mut min_slack: i64 = i64::MAX;

        for ei in 0..m {
            if tree_edge[ei] {
                continue;
            }
            let (es, et) = local_edges[ei];
            let v_in_tail = ns_in_subtree(es, tail_root, &low, &lim);
            let w_in_tail = ns_in_subtree(et, tail_root, &low, &lim);

            // dagre: flip === isDescendant(edge.v, tailLabel) &&
            //        flip !== isDescendant(edge.w, tailLabel)
            if (flip == v_in_tail) && (flip != w_in_tail) {
                let slack = rank[et] - rank[es] - edge_minlens[ei];
                if slack < min_slack {
                    min_slack = slack;
                    entering = Some(ei);
                }
            }
        }

        let entering = match entering {
            Some(e) => e,
            None => break,
        };

        // exchangeEdges + updateRanks:
        // Compute rank shift delta to make the entering edge tight.
        let (es, et) = local_edges[entering];
        let s_in_tail = ns_in_subtree(es, tail_root, &low, &lim);
        let slack = rank[et] - rank[es] - edge_minlens[entering];

        let delta = if s_in_tail { slack } else { -slack };

        if delta != 0 {
            for node in 0..n {
                if ns_in_subtree(node, tail_root, &low, &lim) {
                    rank[node] += delta;
                }
            }
        }

        // Swap edges in the tree
        tree_edge[leaving] = false;
        tree_edge[entering] = true;

        // Rebuild tree adjacency
        for adj in tree_adj.iter_mut() {
            adj.clear();
        }
        for ei in 0..m {
            if tree_edge[ei] {
                let (s, t) = local_edges[ei];
                tree_adj[s].push((t, ei));
                tree_adj[t].push((s, ei));
            }
        }

        // Recompute tree order and cut values
        for p in par.iter_mut() {
            *p = None;
        }
        ns_compute_tree_order(tree_root, &tree_adj, &mut lim, &mut low, &mut par);
        ns_compute_cut_values(
            m,
            &local_edges,
            &tree_edge,
            &edge_weights,
            &mut cut_value,
            &lim,
            &low,
            &par,
        );
    }

    // ── Step 6: Normalize ranks so minimum is 0 ────────────────────────
    let min_rank = rank.iter().copied().min().unwrap_or(0);
    component
        .iter()
        .enumerate()
        .map(|(i, &node)| (node, (rank[i] - min_rank) as usize))
        .collect()
}

/// Convert rank map to layers: Vec<Vec<NodeIndex>> indexed by rank.
/// Nodes are inserted in stable topological order to keep layout quality while
/// remaining deterministic across runs.
/// Convert rank assignments to layer arrays.
///
/// Uses DFS from lowest-rank nodes following outgoing edges, matching dagre's
/// `initOrder`. To ensure good subgraph structure, nodes at the same rank are
/// grouped by subgraph: we first process each subgraph's border-left chain,
/// then the subgraph's content nodes, then its border-right chain. This ensures
/// nodes from the same subgraph are contiguous in each layer.
pub fn ranks_to_layers(
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &HashMap<NodeIndex, usize>,
) -> Vec<Vec<NodeIndex>> {
    if ranks.is_empty() {
        return Vec::new();
    }

    let max_rank = *ranks.values().max().unwrap();
    let mut layers = vec![Vec::new(); max_rank + 1];

    // Sort by rank ascending, then by NodeIndex (insertion order) as tiebreaker.
    // Matches dagre's initOrder: `simpleNodes.sort((a, b) => g.node(a).rank - g.node(b).rank)`
    // where within the same rank, JavaScript's stable sort preserves insertion order.
    let mut nodes_by_rank: Vec<NodeIndex> = ranks.keys().copied().collect();
    nodes_by_rank.sort_by(|&a, &b| {
        ranks[&a]
            .cmp(&ranks[&b])
            .then_with(|| a.index().cmp(&b.index()))
    });

    let mut visited: HashSet<NodeIndex> = HashSet::new();

    fn dfs(
        node: NodeIndex,
        graph: &DiGraph<NodeData, EdgeData>,
        ranks: &HashMap<NodeIndex, usize>,
        layers: &mut [Vec<NodeIndex>],
        visited: &mut HashSet<NodeIndex>,
    ) {
        if !visited.insert(node) {
            return;
        }
        if let Some(&rank) = ranks.get(&node) {
            layers[rank].push(node);
        }
        // Follow outgoing edges (successors) in insertion order.
        // petgraph uses head-insertion (LIFO) for adjacency lists, so
        // neighbors_directed returns in REVERSE insertion order. Dagre's
        // g.successors(v) returns in insertion order (FIFO). Collect and
        // reverse to match dagre's initOrder DFS traversal.
        let mut successors: Vec<NodeIndex> =
            graph.neighbors_directed(node, petgraph::Direction::Outgoing).collect();
        successors.reverse();
        for neighbor in successors {
            dfs(neighbor, graph, ranks, layers, visited);
        }
    }

    for &node in &nodes_by_rank {
        dfs(node, graph, ranks, &mut layers, &mut visited);
    }

    layers
}

/// Align ranks of nodes in sibling subgraphs so that subgraphs at the same
/// dependency tier start at the same rank. Processes depth levels top-down,
/// re-propagating ranks after each level to cascade alignment effects.
pub fn align_sibling_subgraph_ranks(
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    ast: &FlowchartAst,
    membership: &SubgraphMembership,
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
            changed |= align_one_sibling_group(siblings, graph, ranks, &id_to_idx, membership);
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
///
/// Pass 1 only: within each tier, align sibling subgraphs to the same max rank.
/// This provides vertical compaction without enforcing strict tier separation,
/// allowing dagre-like side-by-side layout where subgraphs can share rank ranges.
fn align_one_sibling_group(
    siblings: &[SubgraphDef],
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    id_to_idx: &HashMap<String, NodeIndex>,
    membership: &SubgraphMembership,
) -> bool {
    // Build sg_nodes from membership so that each node is attributed to its
    // true owning subgraph, not to a subgraph that merely references it via a
    // cross-subgraph edge.  A node belongs to sibling S if S.id appears
    // anywhere in its membership path (catching nested child subgraphs).
    let sg_nodes: Vec<HashSet<NodeIndex>> = siblings
        .iter()
        .map(|sg| {
            membership
                .iter()
                .filter(|(_, path)| path.iter().any(|p| p == &sg.id))
                .filter_map(|(id, _)| id_to_idx.get(id).copied())
                .collect()
        })
        .collect();

    let tiers = compute_dependency_tiers(&sg_nodes, graph);

    let mut changed = false;

    // Pass 1: Within each tier, align sibling subgraphs to the same max rank.
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

    // Late scheduling: sink each subgraph to the latest tier that still keeps
    // it above all its direct successors.  This minimises cross-subgraph edge
    // lengths (e.g. RBAC sinks from tier 0 to tier 1 when its only targets
    // are in tier 2).
    if tiers.len() > 1 {
        let mut sg_tier: Vec<usize> = vec![0; n];
        for (t, tier) in tiers.iter().enumerate() {
            for &sg in tier {
                sg_tier[sg] = t;
            }
        }

        // Process from highest tier to lowest so that when we sink a
        // subgraph, its successors have already been (potentially) sunk.
        for t in (0..tiers.len()).rev() {
            for &sg in &tiers[t] {
                if adj[sg].is_empty() {
                    continue;
                }
                let latest = adj[sg]
                    .iter()
                    .map(|&succ| sg_tier[succ])
                    .min()
                    .unwrap()
                    .saturating_sub(1);
                if latest > sg_tier[sg] {
                    sg_tier[sg] = latest;
                }
            }
        }

        // Rebuild tiers from the updated assignment, compacting away any
        // empty tiers.
        let max_tier = sg_tier.iter().max().copied().unwrap_or(0);
        let mut new_tiers: Vec<Vec<usize>> = vec![Vec::new(); max_tier + 1];
        for (sg, &t) in sg_tier.iter().enumerate() {
            new_tiers[t].push(sg);
        }
        new_tiers.retain(|t| !t.is_empty());
        tiers = new_tiers;
    }

    tiers
}

/// Align nodes within each subgraph that are at the same internal topological
/// depth.  Cross-subgraph edges can push some nodes to higher ranks even
/// though they are "peers" inside their subgraph.  This pass groups nodes by
/// their depth in the subgraph's *internal* edge graph and aligns each group
/// to the max rank, then re-propagates.
pub fn align_within_subgraph_peers(
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    membership: &SubgraphMembership,
    ast: &FlowchartAst,
) {
    let id_to_idx: HashMap<String, NodeIndex> = graph
        .node_indices()
        .map(|idx| (graph[idx].id.clone(), idx))
        .collect();

    align_subgraph_peers_recursive(&ast.subgraphs, graph, ranks, membership, &id_to_idx);
    propagate_ranks_forward(graph, ranks);
}

fn align_subgraph_peers_recursive(
    subgraphs: &[SubgraphDef],
    graph: &DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    membership: &SubgraphMembership,
    id_to_idx: &HashMap<String, NodeIndex>,
) {
    for sg in subgraphs {
        // Recurse into nested subgraphs first
        align_subgraph_peers_recursive(&sg.subgraphs, graph, ranks, membership, id_to_idx);

        // Collect node indices that truly belong to this subgraph (not bare refs)
        let member_indices: HashSet<NodeIndex> = membership
            .iter()
            .filter(|(_, path)| path.last().map(|s| s.as_str()) == Some(&sg.id))
            .filter_map(|(id, _)| id_to_idx.get(id).copied())
            .collect();

        if member_indices.len() <= 1 {
            continue;
        }

        // Build internal edge set: edges where BOTH endpoints belong to this subgraph
        let mut internal_preds: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
        for &n in &member_indices {
            internal_preds.insert(n, Vec::new());
        }
        for edge_idx in graph.edge_indices() {
            if let Some((src, tgt)) = graph.edge_endpoints(edge_idx) {
                if member_indices.contains(&src) && member_indices.contains(&tgt) {
                    internal_preds.get_mut(&tgt).unwrap().push(src);
                }
            }
        }

        // Compute internal depth via longest-path within the subgraph
        let mut internal_depth: HashMap<NodeIndex, usize> = HashMap::new();
        // Simple iterative: keep processing until stable
        let mut changed = true;
        for &n in &member_indices {
            internal_depth.insert(n, 0);
        }
        while changed {
            changed = false;
            for &n in &member_indices {
                let max_pred_depth = internal_preds[&n]
                    .iter()
                    .filter_map(|p| internal_depth.get(p))
                    .max()
                    .copied();
                if let Some(d) = max_pred_depth {
                    let new_depth = d + 1;
                    if new_depth > internal_depth[&n] {
                        internal_depth.insert(n, new_depth);
                        changed = true;
                    }
                }
            }
        }

        // Group by internal depth, then align each group to max rank
        let mut depth_groups: HashMap<usize, Vec<NodeIndex>> = HashMap::new();
        for (&n, &depth) in &internal_depth {
            depth_groups.entry(depth).or_default().push(n);
        }

        for group in depth_groups.values() {
            if group.len() <= 1 {
                continue;
            }
            let max_rank = group
                .iter()
                .filter_map(|n| ranks.get(n).copied())
                .max()
                .unwrap_or(0);
            for &n in group {
                if let Some(r) = ranks.get_mut(&n) {
                    if *r < max_rank {
                        *r = max_rank;
                    }
                }
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{ArrowEnd, EdgeDef, LineStyle, NodeDef, NodeShape, SubgraphDef};

    fn make_node_data(id: &str) -> NodeData {
        NodeData {
            id: id.to_string(),
            label: String::new(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        }
    }

    fn make_edge_data() -> EdgeData {
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
    fn test_assign_ranks_linear_chain() {
        // A -> B -> C
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, b, make_edge_data());
        g.add_edge(b, c, make_edge_data());

        let ranks = assign_ranks(&g);
        assert_eq!(ranks[&a], 0);
        assert_eq!(ranks[&b], 1);
        assert_eq!(ranks[&c], 2);
    }

    #[test]
    fn test_assign_ranks_fork() {
        // A -> B, A -> C
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, b, make_edge_data());
        g.add_edge(a, c, make_edge_data());

        let ranks = assign_ranks(&g);
        assert_eq!(ranks[&a], 0);
        assert_eq!(ranks[&b], 1);
        assert_eq!(ranks[&c], 1);
    }

    #[test]
    fn test_assign_ranks_merge() {
        // A -> C, B -> C
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, c, make_edge_data());
        g.add_edge(b, c, make_edge_data());

        let ranks = assign_ranks(&g);
        assert_eq!(ranks[&a], 0);
        assert_eq!(ranks[&b], 0);
        assert_eq!(ranks[&c], 1);
    }

    #[test]
    fn test_assign_ranks_disconnected() {
        // A -> B, C (disconnected)
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, b, make_edge_data());

        let ranks = assign_ranks(&g);
        assert_eq!(ranks[&a], 0);
        assert_eq!(ranks[&b], 1);
        assert_eq!(ranks[&c], 0);
    }

    #[test]
    fn test_ranks_to_layers() {
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, b, make_edge_data());
        g.add_edge(a, c, make_edge_data());

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 1);
        ranks.insert(c, 1);

        let layers = ranks_to_layers(&g, &ranks);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].len(), 1);
        assert_eq!(layers[1].len(), 2);
    }

    #[test]
    fn test_ranks_to_layers_empty() {
        let g: DiGraph<NodeData, EdgeData> = DiGraph::new();
        let ranks: HashMap<NodeIndex, usize> = HashMap::new();
        let layers = ranks_to_layers(&g, &ranks);
        assert!(layers.is_empty());
    }

    #[test]
    fn test_align_sibling_subgraph_ranks() {
        // Two sibling subgraphs: Left (A->B, depth 2) and Right (C, depth 1)
        // After alignment, both should end at the same max rank
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, b, make_edge_data());

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 1);
        ranks.insert(c, 0);

        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "Left".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        NodeDef {
                            id: "A".into(),
                            label: None,
                            shape: NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        NodeDef {
                            id: "B".into(),
                            label: None,
                            shape: NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![EdgeDef {
                        from: "A".into(),
                        to: "B".into(),
                        line_style: LineStyle::Solid,
                        arrow_start: ArrowEnd::None,
                        arrow_end: ArrowEnd::Arrow,
                        label: None,
                    }],
                    subgraphs: vec![],
                },
                SubgraphDef {
                    id: "Right".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![NodeDef {
                        id: "C".into(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    }],
                    edges: vec![],
                    subgraphs: vec![],
                },
            ],
            ..Default::default()
        };

        let mut membership: SubgraphMembership = HashMap::new();
        membership.insert("A".to_string(), vec!["Left".to_string()]);
        membership.insert("B".to_string(), vec!["Left".to_string()]);
        membership.insert("C".to_string(), vec!["Right".to_string()]);

        align_sibling_subgraph_ranks(&g, &mut ranks, &ast, &membership);

        // After alignment, the max rank among Left and Right siblings should be equal
        let left_max = [ranks[&a], ranks[&b]].iter().max().copied().unwrap();
        let right_max = ranks[&c];
        assert_eq!(
            left_max, right_max,
            "sibling subgraphs should be aligned to the same max rank (left_max={left_max}, right_max={right_max})"
        );
    }

    #[test]
    fn test_align_sibling_subgraph_ranks_single_subgraph() {
        // Single subgraph -> no alignment needed
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        g.add_edge(a, b, make_edge_data());

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 1);

        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "SG".to_string(),
                label: None,
                direction: None,
                nodes: vec![
                    NodeDef {
                        id: "A".into(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    },
                    NodeDef {
                        id: "B".into(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    },
                ],
                edges: vec![],
                subgraphs: vec![],
            }],
            ..Default::default()
        };

        let mut membership: SubgraphMembership = HashMap::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec!["SG".to_string()]);

        let orig_ranks = ranks.clone();
        align_sibling_subgraph_ranks(&g, &mut ranks, &ast, &membership);

        // Ranks should be unchanged since there's only one subgraph
        assert_eq!(ranks[&a], orig_ranks[&a]);
        assert_eq!(ranks[&b], orig_ranks[&b]);
    }

    #[test]
    fn test_propagate_ranks_forward() {
        // A -> B -> C, but B is at a lower rank than A+1
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, b, make_edge_data());
        g.add_edge(b, c, make_edge_data());

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 0); // Intentionally wrong
        ranks.insert(c, 0); // Intentionally wrong

        propagate_ranks_forward(&g, &mut ranks);

        assert_eq!(ranks[&a], 0);
        assert!(ranks[&b] >= 1, "B should be at least rank 1");
        assert!(ranks[&c] >= 2, "C should be at least rank 2");
    }

    #[test]
    fn test_collect_groups_recursive_empty_subgraphs() {
        // FlowchartAst with empty subgraphs list - collect_groups_recursive early return
        let ast = FlowchartAst {
            subgraphs: vec![],
            ..Default::default()
        };
        let groups = collect_sibling_groups_by_depth(&ast);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_compute_dependency_tiers_empty() {
        // n == 0 -> return Vec::new() (line 206-208)
        let g: DiGraph<NodeData, EdgeData> = DiGraph::new();
        let sg_nodes: Vec<std::collections::HashSet<NodeIndex>> = vec![];
        let tiers = compute_dependency_tiers(&sg_nodes, &g);
        assert!(tiers.is_empty());
    }

    #[test]
    fn test_compute_dependency_tiers_cycle() {
        // Sibling subgraphs with cycle: A->B, B->C, C->A
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, b, make_edge_data());
        g.add_edge(b, c, make_edge_data());
        g.add_edge(c, a, make_edge_data());

        let sg_nodes = vec![
            std::collections::HashSet::from([a]),
            std::collections::HashSet::from([b]),
            std::collections::HashSet::from([c]),
        ];
        let tiers = compute_dependency_tiers(&sg_nodes, &g);
        // With cycle, processed.len() < n, remaining nodes go to last tier
        assert!(!tiers.is_empty());
        assert_eq!(tiers.iter().map(|t| t.len()).sum::<usize>(), 3);
    }

    #[test]
    fn test_propagate_ranks_forward_rank_promotion() {
        // B has rank 0 but predecessor A has rank 0 -> B must be promoted to 1
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        g.add_edge(a, b, make_edge_data());

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 0); // Wrong: should be at least 1

        propagate_ranks_forward(&g, &mut ranks);
        assert_eq!(ranks[&b], 1);
    }

    #[test]
    fn test_topological_sort_cycle() {
        // Graph with cycle: A -> B -> C -> A
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, b, make_edge_data());
        g.add_edge(b, c, make_edge_data());
        g.add_edge(c, a, make_edge_data());

        let result = topological_sort(&g);
        // Cycle: result.len() < node_count, remaining nodes appended
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_align_one_sibling_group_delta_positive() {
        // Two sibling subgraphs where one has lower max rank - delta > 0
        let mut g = DiGraph::new();
        let a = g.add_node(make_node_data("A"));
        let b = g.add_node(make_node_data("B"));
        let c = g.add_node(make_node_data("C"));
        g.add_edge(a, b, make_edge_data());

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 1);
        ranks.insert(c, 0);

        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "Left".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        NodeDef {
                            id: "A".into(),
                            label: None,
                            shape: NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        NodeDef {
                            id: "B".into(),
                            label: None,
                            shape: NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![EdgeDef {
                        from: "A".into(),
                        to: "B".into(),
                        line_style: LineStyle::Solid,
                        arrow_start: ArrowEnd::None,
                        arrow_end: ArrowEnd::Arrow,
                        label: None,
                    }],
                    subgraphs: vec![],
                },
                SubgraphDef {
                    id: "Right".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![NodeDef {
                        id: "C".into(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    }],
                    edges: vec![],
                    subgraphs: vec![],
                },
            ],
            ..Default::default()
        };

        let mut id_to_idx = HashMap::new();
        id_to_idx.insert("A".to_string(), a);
        id_to_idx.insert("B".to_string(), b);
        id_to_idx.insert("C".to_string(), c);

        let mut membership: SubgraphMembership = HashMap::new();
        membership.insert("A".to_string(), vec!["Left".to_string()]);
        membership.insert("B".to_string(), vec!["Left".to_string()]);
        membership.insert("C".to_string(), vec!["Right".to_string()]);

        let siblings = &ast.subgraphs;
        let changed = align_one_sibling_group(siblings, &g, &mut ranks, &id_to_idx, &membership);
        assert!(changed);
        assert_eq!(
            ranks[&c], 1,
            "C should be promoted to match Left's max rank"
        );
    }
}
