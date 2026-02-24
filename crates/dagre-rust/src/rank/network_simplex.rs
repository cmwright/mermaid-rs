//! Network simplex algorithm for optimal ranking.
//! Port of dagre's `rank/network-simplex.js`.

use crate::graph::{Edge, LayoutGraph};
use crate::rank::feasible_tree::feasible_tree;
use crate::rank::util::{longest_path, slack};
use crate::types::EdgeLabel;
use crate::util::simplify;
use std::collections::HashSet;

/// Runs the network simplex algorithm to assign optimal ranks.
pub fn network_simplex(g: &mut LayoutGraph) {
    let mut sg = simplify(g);
    longest_path(&mut sg);
    let mut t = feasible_tree(&mut sg);
    init_low_lim_values(&mut t, None);
    init_cut_values(&mut t, &sg);

    loop {
        let e = leave_edge(&t);
        if e.is_none() {
            break;
        }
        let e = e.unwrap();
        let f = enter_edge(&t, &sg, &e);
        exchange_edges(&mut t, &mut sg, &e, &f);
    }

    // Copy ranks back to the original graph
    for v in sg.nodes() {
        if let Some(rank) = sg.node(&v).and_then(|n| n.rank) {
            g.node_mut(&v).unwrap().rank = Some(rank);
        }
    }
}

/// Initializes cut values for all edges in the tree.
fn init_cut_values(t: &mut LayoutGraph, g: &LayoutGraph) {
    let all_vs = postorder_traversal(t, &t.nodes());
    let len = all_vs.len();
    for child in all_vs.iter().take(len.saturating_sub(1)) {
        assign_cut_value(t, g, child);
    }
}

fn assign_cut_value(t: &mut LayoutGraph, g: &LayoutGraph, child: &str) {
    let parent = t
        .node(child)
        .and_then(|n| n.parent_node.as_deref())
        .map(|s| s.to_string());

    if let Some(ref parent) = parent {
        let cv = calc_cut_value(t, g, child);
        t.edge_mut(child, parent, None).unwrap().cutvalue = Some(cv);
    }
}

/// Calculates the cut value for the edge between child and its parent.
fn calc_cut_value(t: &LayoutGraph, g: &LayoutGraph, child: &str) -> f64 {
    let parent = t
        .node(child)
        .and_then(|n| n.parent_node.as_deref())
        .unwrap_or("");

    let mut child_is_tail = true;
    let graph_edge = g.edge(child, parent, None);
    let graph_edge = if graph_edge.is_none() {
        child_is_tail = false;
        g.edge(parent, child, None)
    } else {
        graph_edge
    };

    let mut cut_value = graph_edge.map(|e| e.weight).unwrap_or(0.0);

    if let Some(node_edges) = g.node_edges(child, None) {
        for e in &node_edges {
            let is_out_edge = e.v == child;
            let other = if is_out_edge { &e.w } else { &e.v };

            if other != parent {
                let points_to_head = is_out_edge == child_is_tail;
                let other_weight = g.edge_by_obj(e).map(|l| l.weight).unwrap_or(0.0);

                cut_value += if points_to_head {
                    other_weight
                } else {
                    -other_weight
                };

                if is_tree_edge(t, child, other) {
                    let other_cut_value = t
                        .edge(child, other, None)
                        .and_then(|l| l.cutvalue)
                        .unwrap_or(0.0);
                    cut_value += if points_to_head {
                        -other_cut_value
                    } else {
                        other_cut_value
                    };
                }
            }
        }
    }

    cut_value
}

/// Initialize low/lim values for the tree using DFS.
fn init_low_lim_values(tree: &mut LayoutGraph, root: Option<&str>) {
    let root = root
        .map(|s| s.to_string())
        .unwrap_or_else(|| tree.nodes()[0].clone());
    let mut visited = HashSet::new();
    dfs_assign_low_lim(tree, &mut visited, 1, &root, None);
}

fn dfs_assign_low_lim(
    tree: &mut LayoutGraph,
    visited: &mut HashSet<String>,
    mut next_lim: i64,
    v: &str,
    parent: Option<&str>,
) -> i64 {
    let low = next_lim;
    visited.insert(v.to_string());

    let neighbors = tree.neighbors(v).unwrap_or_default();
    for w in &neighbors {
        if !visited.contains(w) {
            next_lim = dfs_assign_low_lim(tree, visited, next_lim, w, Some(v));
        }
    }

    let label = tree.node_mut(v).unwrap();
    label.low = Some(low);
    label.lim = Some(next_lim);
    label.parent_node = parent.map(|s| s.to_string());

    next_lim + 1
}

/// Find an edge with a negative cut value to leave the tree.
fn leave_edge(tree: &LayoutGraph) -> Option<Edge> {
    tree.edges().into_iter().find(|e| {
        tree.edge_by_obj(e)
            .and_then(|l| l.cutvalue)
            .map(|c| c < 0.0)
            .unwrap_or(false)
    })
}

/// Find the edge to enter the tree that replaces the leaving edge.
fn enter_edge(t: &LayoutGraph, g: &LayoutGraph, edge: &Edge) -> Edge {
    let mut v = edge.v.clone();
    let mut w = edge.w.clone();

    // Ensure v is tail, w is head in the graph
    if !g.has_edge(&v, &w, None) {
        std::mem::swap(&mut v, &mut w);
    }

    let v_lim = t.node(&v).and_then(|n| n.lim).unwrap_or(0);
    let w_lim = t.node(&w).and_then(|n| n.lim).unwrap_or(0);

    // Clone the tail_label fields we need, since they are Copy types
    let (tail_low, tail_lim, flip) = if v_lim > w_lim {
        let wn = t.node(&w).unwrap();
        (wn.low.unwrap_or(0), wn.lim.unwrap_or(0), true)
    } else {
        let vn = t.node(&v).unwrap();
        (vn.low.unwrap_or(0), vn.lim.unwrap_or(0), false)
    };

    let candidates: Vec<Edge> = g
        .edges()
        .into_iter()
        .filter(|e| {
            let v_lim_val = t.node(&e.v).and_then(|n| n.lim).unwrap_or(0);
            let w_lim_val = t.node(&e.w).and_then(|n| n.lim).unwrap_or(0);
            let v_desc = tail_low <= v_lim_val && v_lim_val <= tail_lim;
            let w_desc = tail_low <= w_lim_val && w_lim_val <= tail_lim;
            flip == v_desc && flip != w_desc
        })
        .collect();

    candidates
        .into_iter()
        .min_by(|a, b| {
            let sa = slack(g, a);
            let sb = slack(g, b);
            sa.cmp(&sb)
        })
        .expect("No enter edge found")
}

fn exchange_edges(t: &mut LayoutGraph, g: &mut LayoutGraph, e: &Edge, f: &Edge) {
    t.remove_edge(&e.v, &e.w, None);
    t.set_edge(&f.v, &f.w, Some(EdgeLabel::default()), None);
    init_low_lim_values(t, None);
    init_cut_values(t, g);
    update_ranks(t, g);
}

fn update_ranks(t: &LayoutGraph, g: &mut LayoutGraph) {
    // Find root: node without a parent in t's labels
    let root = t
        .nodes()
        .into_iter()
        .find(|v| t.node(v).and_then(|n| n.parent_node.as_ref()).is_none())
        .unwrap_or_default();

    let vs = preorder_traversal(t, &root);
    // Skip root (first element)
    for v in vs.iter().skip(1) {
        let parent = t
            .node(v)
            .and_then(|n| n.parent_node.as_deref())
            .unwrap_or("")
            .to_string();

        let mut flipped = false;
        let edge = g.edge(v, &parent, None);
        let edge = if edge.is_none() {
            flipped = true;
            g.edge(&parent, v, None)
        } else {
            edge
        };

        let minlen = edge.map(|e| e.minlen as i64).unwrap_or(1);

        let parent_rank = g.node(&parent).and_then(|n| n.rank).unwrap_or(0);

        let new_rank = parent_rank + if flipped { minlen } else { -minlen };

        g.node_mut(v).unwrap().rank = Some(new_rank);
    }
}

fn is_tree_edge(tree: &LayoutGraph, u: &str, v: &str) -> bool {
    tree.has_edge(u, v, None)
}

// === Graph traversal algorithms (mirrors graphlib's alg.preorder/postorder) ===

/// Pre-order DFS traversal.
fn preorder_traversal(g: &LayoutGraph, root: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();

    fn dfs(g: &LayoutGraph, v: &str, visited: &mut HashSet<String>, result: &mut Vec<String>) {
        if visited.contains(v) {
            return;
        }
        visited.insert(v.to_string());
        result.push(v.to_string());
        for w in g.neighbors(v).unwrap_or_default() {
            dfs(g, &w, visited, result);
        }
    }

    dfs(g, root, &mut visited, &mut result);
    result
}

/// Post-order DFS traversal from multiple roots.
fn postorder_traversal(g: &LayoutGraph, roots: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();

    fn dfs(g: &LayoutGraph, v: &str, visited: &mut HashSet<String>, result: &mut Vec<String>) {
        if visited.contains(v) {
            return;
        }
        visited.insert(v.to_string());
        for w in g.neighbors(v).unwrap_or_default() {
            dfs(g, &w, visited, result);
        }
        result.push(v.to_string());
    }

    for root in roots {
        dfs(g, root, &mut visited, &mut result);
    }
    result
}
