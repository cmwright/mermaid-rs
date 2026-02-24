//! Network simplex algorithm for optimal ranking.
//! Port of dagre's `rank/network-simplex.js`.

use crate::graph::{Edge, LayoutGraph};
use crate::rank::feasible_tree::feasible_tree;
use crate::rank::util::{longest_path, slack};
use crate::types::EdgeLabel;
use crate::util::simplify;

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
    let root = t
        .node_ids()
        .iter()
        .find(|v| t.node(v).and_then(|n| n.parent_node.as_ref()).is_none())
        .cloned()
        .unwrap_or_default();
    let all_vs = postorder_traversal(t, &root);
    let len = all_vs.len();
    for child in all_vs.iter().take(len.saturating_sub(1)) {
        assign_cut_value(t, g, child);
    }
}

fn assign_cut_value(t: &mut LayoutGraph, g: &LayoutGraph, child: &str) {
    let parent = t
        .node(child)
        .and_then(|n| n.parent_node.as_deref());

    if let Some(parent) = parent {
        let parent = parent.to_string();
        let cv = calc_cut_value(t, g, child);
        t.edge_mut(child, &parent, None).unwrap().cutvalue = Some(cv);
    }
}

/// Calculates the cut value for the edge between child and its parent.
fn calc_cut_value(t: &LayoutGraph, g: &LayoutGraph, child: &str) -> f64 {
    let parent = t
        .node(child)
        .and_then(|n| n.parent_node.as_deref())
        .unwrap_or("");

    let mut child_is_tail = true;
    let mut cut_value = 0.0;
    let mut found_parent_edge = false;
    if let Some(out_edge_ids) = g.out_edge_ids(child) {
        for edge_id in out_edge_ids {
            let Some(e) = g.edge_obj_by_id(edge_id) else {
                continue;
            };
            if e.w == parent {
                cut_value = g.edge_label_by_id(edge_id).map(|l| l.weight).unwrap_or(0.0);
                child_is_tail = true;
                found_parent_edge = true;
                break;
            }
        }
    }
    if !found_parent_edge
        && let Some(in_edge_ids) = g.in_edge_ids(child)
    {
        for edge_id in in_edge_ids {
            let Some(e) = g.edge_obj_by_id(edge_id) else {
                continue;
            };
            if e.v == parent {
                cut_value = g.edge_label_by_id(edge_id).map(|l| l.weight).unwrap_or(0.0);
                child_is_tail = false;
                break;
            }
        }
    }

    if let Some(out_edge_ids) = g.out_edge_ids(child) {
        for edge_id in out_edge_ids {
            let Some(e) = g.edge_obj_by_id(edge_id) else {
                continue;
            };
            let other = &e.w;
            if other == parent {
                continue;
            }
            let points_to_head = child_is_tail;
            let other_weight = g.edge_label_by_id(edge_id).map(|l| l.weight).unwrap_or(0.0);

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
    if let Some(in_edge_ids) = g.in_edge_ids(child) {
        for edge_id in in_edge_ids {
            let Some(e) = g.edge_obj_by_id(edge_id) else {
                continue;
            };
            let other = &e.v;
            if other == parent {
                continue;
            }
            let points_to_head = !child_is_tail;
            let other_weight = g.edge_label_by_id(edge_id).map(|l| l.weight).unwrap_or(0.0);

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

    cut_value
}

/// Initialize low/lim values for the tree using DFS.
fn init_low_lim_values(tree: &mut LayoutGraph, root: Option<&str>) {
    let root = root
        .map(|s| s.to_string())
        .unwrap_or_else(|| tree.node_ids()[0].clone());
    dfs_assign_low_lim(tree, 1, &root, None);
}

fn dfs_assign_low_lim(
    tree: &mut LayoutGraph,
    mut next_lim: i64,
    v: &str,
    parent: Option<&str>,
) -> i64 {
    let low = next_lim;

    let neighbors = tree.neighbors(v).unwrap_or_default();
    for w in &neighbors {
        if Some(w.as_str()) != parent {
            next_lim = dfs_assign_low_lim(tree, next_lim, w, Some(v));
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
    for edge_id in tree.edge_ids() {
        let e = tree
            .edge_obj_by_id(edge_id)
            .expect("edge ID should resolve to edge object");
        if tree
            .edge_by_obj(e)
            .and_then(|l| l.cutvalue)
            .map(|c| c < 0.0)
            .unwrap_or(false)
        {
            return Some(e.clone());
        }
    }
    None
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

    let mut best_edge: Option<Edge> = None;
    let mut best_slack: Option<i64> = None;
    for edge_id in g.edge_ids() {
        let e = g
            .edge_obj_by_id(edge_id)
            .expect("edge ID should resolve to edge object");
        let v_lim_val = t.node(&e.v).and_then(|n| n.lim).unwrap_or(0);
        let w_lim_val = t.node(&e.w).and_then(|n| n.lim).unwrap_or(0);
        let v_desc = tail_low <= v_lim_val && v_lim_val <= tail_lim;
        let w_desc = tail_low <= w_lim_val && w_lim_val <= tail_lim;
        if !(flip == v_desc && flip != w_desc) {
            continue;
        }

        let s = slack(g, e);
        if best_slack.is_none_or(|cur| s < cur) {
            best_slack = Some(s);
            best_edge = Some(e.clone());
        }
    }

    best_edge.expect("No enter edge found")
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
        .node_ids()
        .iter()
        .into_iter()
        .find(|v| t.node(v).and_then(|n| n.parent_node.as_ref()).is_none())
        .cloned()
        .unwrap_or_default();

    let vs = preorder_traversal(t, &root);
    // Skip root (first element)
    for v in vs.iter().skip(1) {
        let parent = t
            .node(v)
            .and_then(|n| n.parent_node.as_deref())
            .unwrap_or("");

        let mut flipped = false;
        let edge = g.edge(v, parent, None);
        let edge = if edge.is_none() {
            flipped = true;
            g.edge(parent, v, None)
        } else {
            edge
        };

        let minlen = edge.map(|e| e.minlen as i64).unwrap_or(1);

        let parent_rank = g.node(parent).and_then(|n| n.rank).unwrap_or(0);

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
    fn dfs(g: &LayoutGraph, v: &str, parent: Option<&str>, result: &mut Vec<String>) {
        result.push(v.to_string());
        for w in g.neighbors(v).unwrap_or_default() {
            if Some(w.as_str()) != parent {
                dfs(g, &w, Some(v), result);
            }
        }
    }

    dfs(g, root, None, &mut result);
    result
}

/// Post-order DFS traversal from a single root.
fn postorder_traversal(g: &LayoutGraph, root: &str) -> Vec<String> {
    let mut result = Vec::new();
    fn dfs(g: &LayoutGraph, v: &str, parent: Option<&str>, result: &mut Vec<String>) {
        for w in g.neighbors(v).unwrap_or_default() {
            if Some(w.as_str()) != parent {
                dfs(g, &w, Some(v), result);
            }
        }
        result.push(v.to_string());
    }

    dfs(g, root, None, &mut result);
    result
}
