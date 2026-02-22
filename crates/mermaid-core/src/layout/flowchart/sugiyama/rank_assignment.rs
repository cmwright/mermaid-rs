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
/// Returns a map from NodeIndex to rank (0-based, normalized).
fn network_simplex(
    graph: &DiGraph<NodeData, EdgeData>,
    component: &[NodeIndex],
) -> HashMap<NodeIndex, usize> {
    let comp_set: HashSet<NodeIndex> = component.iter().copied().collect();

    // Collect directed edges within this component
    let edges: Vec<(NodeIndex, NodeIndex, EdgeIndex)> = graph
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

    if edges.is_empty() {
        return component.iter().map(|&n| (n, 0)).collect();
    }

    // Map NodeIndex to dense local indices 0..n
    let node_to_local: HashMap<NodeIndex, usize> =
        component.iter().enumerate().map(|(i, &n)| (n, i)).collect();
    let n = component.len();
    let m = edges.len();

    let local_edges: Vec<(usize, usize)> = edges
        .iter()
        .map(|&(s, t, _)| (node_to_local[&s], node_to_local[&t]))
        .collect();

    // Read edge weights and minlens from EdgeData
    let edge_weights: Vec<i64> = edges.iter().map(|&(_, _, ei)| graph[ei].weight).collect();
    let edge_minlens: Vec<i64> = edges
        .iter()
        .map(|&(_, _, ei)| graph[ei].minlen as i64)
        .collect();

    // ── Step 1: Initial feasible rank assignment via longest-path ───────
    let mut rank: Vec<i64> = vec![0; n];
    {
        let mut in_deg: Vec<usize> = vec![0; n];
        let mut out_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut in_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (ei, &(s, t)) in local_edges.iter().enumerate() {
            out_adj[s].push(ei);
            in_adj[t].push(ei);
            in_deg[t] += 1;
        }

        // Kahn's algorithm for topological sort
        let mut queue: VecDeque<usize> = VecDeque::new();
        for i in 0..n {
            if in_deg[i] == 0 {
                queue.push_back(i);
            }
        }

        let mut topo_order: Vec<usize> = Vec::with_capacity(n);
        while let Some(node) = queue.pop_front() {
            topo_order.push(node);
            for &ei in &out_adj[node] {
                let (_, t) = local_edges[ei];
                in_deg[t] -= 1;
                if in_deg[t] == 0 {
                    queue.push_back(t);
                }
            }
        }

        // Handle any nodes not reached (cycles)
        if topo_order.len() < n {
            let in_topo: HashSet<usize> = topo_order.iter().copied().collect();
            for i in 0..n {
                if !in_topo.contains(&i) {
                    topo_order.push(i);
                }
            }
        }

        // Longest-path: rank[t] = max(rank[s] + minlen) over all predecessors
        for &node in &topo_order {
            for &ei in &in_adj[node] {
                let (src, _) = local_edges[ei];
                let candidate = rank[src] + edge_minlens[ei];
                if candidate > rank[node] {
                    rank[node] = candidate;
                }
            }
        }
    }

    // ── Step 2: Build initial feasible spanning tree ────────────────────
    // Start from tight edges (slack == 0), then add non-tight edges
    // adjusting ranks to make them tight.

    let mut tree_edge: Vec<bool> = vec![false; m];
    let mut in_tree: Vec<bool> = vec![false; n];
    let mut tree_edge_count: usize = 0;

    // Gather tight edges (undirected adjacency)
    let mut tight_adj: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    for (ei, &(s, t)) in local_edges.iter().enumerate() {
        let slack = rank[t] - rank[s] - edge_minlens[ei];
        if slack == 0 {
            tight_adj[s].push((t, ei));
            tight_adj[t].push((s, ei));
        }
    }

    // DFS to build spanning tree from tight edges, starting at node 0
    in_tree[0] = true;
    {
        let mut stack: Vec<usize> = vec![0];
        while let Some(node) = stack.pop() {
            for &(nb, ei) in &tight_adj[node] {
                if !in_tree[nb] {
                    in_tree[nb] = true;
                    tree_edge[ei] = true;
                    tree_edge_count += 1;
                    stack.push(nb);
                }
            }
        }
    }

    // If the tight-edge tree doesn't span all nodes, greedily add non-tight
    // edges and adjust ranks to make them tight.
    if tree_edge_count < n - 1 {
        let mut progress = true;
        while tree_edge_count < n - 1 && progress {
            progress = false;
            for ei in 0..m {
                if tree_edge[ei] {
                    continue;
                }
                let (s, t) = local_edges[ei];
                if in_tree[s] == in_tree[t] {
                    continue;
                }

                // One endpoint in tree, other not. Add edge and make it tight.
                let added = if in_tree[s] && !in_tree[t] {
                    rank[t] = rank[s] + edge_minlens[ei];
                    in_tree[t] = true;
                    t
                } else {
                    rank[s] = rank[t] - edge_minlens[ei];
                    in_tree[s] = true;
                    s
                };
                tree_edge[ei] = true;
                tree_edge_count += 1;
                progress = true;

                // BFS to find more tight edges from the added node
                let mut bfs = VecDeque::new();
                bfs.push_back(added);
                while let Some(node) = bfs.pop_front() {
                    for ej in 0..m {
                        if tree_edge[ej] {
                            continue;
                        }
                        let (es, et) = local_edges[ej];
                        if es == node && !in_tree[et] {
                            if rank[et] - rank[es] == edge_minlens[ej] {
                                in_tree[et] = true;
                                tree_edge[ej] = true;
                                tree_edge_count += 1;
                                bfs.push_back(et);
                            }
                        } else if et == node && !in_tree[es] {
                            if rank[et] - rank[es] == edge_minlens[ej] {
                                in_tree[es] = true;
                                tree_edge[ej] = true;
                                tree_edge_count += 1;
                                bfs.push_back(es);
                            }
                        }
                    }
                }
            }
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
        // Find a tree edge with the most negative cut value (leaving edge)
        let leaving = {
            let mut best: Option<usize> = None;
            let mut best_val: i64 = 0;
            for ei in 0..m {
                if tree_edge[ei] && cut_value[ei] < best_val {
                    best_val = cut_value[ei];
                    best = Some(ei);
                }
            }
            match best {
                Some(e) => e,
                None => break, // Optimal - no negative cut values
            }
        };

        // Determine the tail component (the child's subtree)
        let (ls, lt) = local_edges[leaving];
        let tail_root = if par[lt] == Some(ls) {
            lt
        } else if par[ls] == Some(lt) {
            ls
        } else {
            lt // fallback
        };

        // Find the entering edge: non-tree edge with minimum slack crossing
        // the partition. The entering edge replaces the leaving edge.
        //
        // For a tree edge with negative cut value, we need a non-tree edge
        // that connects across the cut and would improve the objective.
        // Specifically, we look for edges that "go against" the surplus:
        // - If the tree edge is s->t with tail_root=t (child=t),
        //   and cut_value < 0 means too much flow head->tail,
        //   we want non-tree edges going from head to tail (to tighten).
        //   Actually, for the standard NS algorithm:
        //   We want non-tree edges (u,v) where exactly one of u,v is in the
        //   tail component AND the edge direction is from the component with
        //   "too many edges" leaving it.
        //
        // The correct rule (matching dagre/Graphviz):
        // For a leaving edge with negative cut value:
        //   - If tail_root is the target of the original directed edge (tail_root == lt
        //     when par[lt] == Some(ls)), then we look for non-tree edges where
        //     the target is NOT in tail (edge goes tail -> head) with minimum slack.
        //   Wait - let's just use the standard approach: scan all non-tree edges
        //   crossing the cut and pick the one with minimum slack.

        let mut entering: Option<usize> = None;
        let mut min_slack: i64 = i64::MAX;

        for ei in 0..m {
            if tree_edge[ei] {
                continue;
            }
            let (es, et) = local_edges[ei];
            let s_in_tail = ns_in_subtree(es, tail_root, &low, &lim);
            let t_in_tail = ns_in_subtree(et, tail_root, &low, &lim);

            // Must cross the partition
            if s_in_tail == t_in_tail {
                continue;
            }

            let slack = rank[et] - rank[es] - edge_minlens[ei];
            if slack < 0 {
                continue;
            }

            if slack < min_slack {
                min_slack = slack;
                entering = Some(ei);
            }
        }

        let entering = match entering {
            Some(e) => e,
            None => break,
        };

        // Compute the rank shift delta to make the entering edge tight
        let (es, et) = local_edges[entering];
        let s_in_tail = ns_in_subtree(es, tail_root, &low, &lim);
        let slack = rank[et] - rank[es] - edge_minlens[entering];

        // If the entering edge source is in the tail, we shift tail ranks UP by +slack
        // (increasing tail ranks moves source closer to target).
        // If the entering edge target is in the tail, we shift tail ranks DOWN by -slack.
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
pub fn ranks_to_layers(
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

    // Pass 2: Ensure separation between tiers — each tier's min rank must be
    // strictly greater than the previous tier's max rank.  This prevents
    // upstream subgraphs from sharing ranks with downstream subgraphs.
    if tiers.len() > 1 {
        let mut prev_tier_max: usize = 0;
        for (tier_idx, tier) in tiers.iter().enumerate() {
            // Collect all node indices in this tier
            let tier_nodes: Vec<NodeIndex> = tier
                .iter()
                .flat_map(|&sg_idx| sg_nodes[sg_idx].iter().copied())
                .collect();

            if tier_idx > 0 {
                let tier_min = tier_nodes
                    .iter()
                    .filter_map(|n| ranks.get(n).copied())
                    .min()
                    .unwrap_or(0);

                if tier_min <= prev_tier_max {
                    let delta = prev_tier_max + 1 - tier_min;
                    for &node_idx in &tier_nodes {
                        if let Some(r) = ranks.get_mut(&node_idx) {
                            *r += delta;
                        }
                    }
                    changed = true;
                }
            }

            // Update prev_tier_max (re-read in case we shifted this tier)
            prev_tier_max = tier_nodes
                .iter()
                .filter_map(|n| ranks.get(n).copied())
                .max()
                .unwrap_or(prev_tier_max);
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
