//! Assigns parents to dummy nodes in long edge chains.
//! Port of dagre's `parent-dummy-chains.js`.

use crate::graph::LayoutGraph;
use std::collections::HashMap;

/// Assigns compound parents to dummy nodes along long edge chains.
pub fn parent_dummy_chains(g: &mut LayoutGraph) {
    let postorder_nums = postorder(g);

    let dummy_chains = g.graph().dummy_chains.clone();

    for chain_start in dummy_chains {
        let mut v = chain_start;

        let node = g.node(&v).cloned().unwrap_or_default();
        let edge_obj = node
            .edge_obj
            .clone()
            .unwrap_or_else(|| crate::graph::Edge::new("", "", None));
        let edge_obj_v = edge_obj.v.clone();
        let edge_obj_w = edge_obj.w.clone();

        let path_data = find_path(g, &postorder_nums, &edge_obj_v, &edge_obj_w);
        let path = path_data.path;
        let lca = path_data.lca;
        let mut path_idx: usize = 0;
        let mut ascending = true;

        while v != edge_obj_w {
            let node = g.node(&v).cloned().unwrap_or_default();
            let node_rank = node.rank.unwrap_or(0);

            if ascending {
                // JS: while ((pathV = path[pathIdx]) !== lca && g.node(pathV).maxRank < node.rank)
                while path_idx < path.len() && path[path_idx] != lca {
                    if let Some(pv) = &path[path_idx] {
                        let pv_max_rank = g.node(pv).and_then(|n| n.max_rank).unwrap_or(0);
                        if pv_max_rank >= node_rank {
                            break;
                        }
                    } else {
                        break;
                    }
                    path_idx += 1;
                }

                let _path_v = path.get(path_idx).cloned().flatten();
                if path.get(path_idx) == Some(&lca) {
                    ascending = false;
                }
            }

            if !ascending {
                while path_idx < path.len().saturating_sub(1) {
                    let next = &path[path_idx + 1];
                    if let Some(next_pv) = next {
                        let next_min_rank =
                            g.node(next_pv).and_then(|n| n.min_rank).unwrap_or(i64::MAX);
                        if next_min_rank > node_rank {
                            break;
                        }
                    } else {
                        break;
                    }
                    path_idx += 1;
                }
            }

            let path_v = path.get(path_idx).and_then(|o| o.as_deref());
            g.set_parent(&v, path_v);

            v = g
                .successors(&v)
                .and_then(|s| s.first().cloned())
                .unwrap_or_default();
        }
    }
}

#[derive(Debug, Clone)]
struct PostorderNum {
    low: i64,
    lim: i64,
}

struct FindPathResult {
    /// Path from v to w through LCA. Elements are Option<String> because
    /// JS pushes `undefined` when g.parent() returns undefined at root level.
    path: Vec<Option<String>>,
    lca: Option<String>,
}

fn find_path(
    g: &LayoutGraph,
    postorder_nums: &HashMap<String, PostorderNum>,
    v: &str,
    w: &str,
) -> FindPathResult {
    let mut v_path: Vec<Option<String>> = Vec::new();
    let mut w_path: Vec<Option<String>> = Vec::new();

    let v_nums = postorder_nums.get(v);
    let w_nums = postorder_nums.get(w);

    let low = match (v_nums, w_nums) {
        (Some(vn), Some(wn)) => vn.low.min(wn.low),
        _ => 0,
    };
    let lim = match (v_nums, w_nums) {
        (Some(vn), Some(wn)) => vn.lim.max(wn.lim),
        _ => 0,
    };

    // Traverse up from v to find LCA (mirrors JS do-while)
    let mut parent_node = v.to_string();
    let lca = loop {
        let parent = g.parent(&parent_node).map(|s| s.to_string());
        v_path.push(parent.clone()); // Push even if None (matches JS pushing undefined)

        match parent {
            Some(p) => {
                parent_node = p.clone();
                if let Some(pn) = postorder_nums.get(&p) {
                    if pn.low <= low && lim <= pn.lim {
                        break Some(p);
                    }
                } else {
                    break Some(p);
                }
            }
            None => break None,
        }
    };

    // Traverse from w to LCA
    let mut parent_node = w.to_string();
    loop {
        let parent = g.parent(&parent_node).map(|s| s.to_string());
        if parent == lca {
            break;
        }
        match parent {
            Some(p) => {
                w_path.push(Some(p.clone()));
                parent_node = p;
            }
            None => break,
        }
    }

    w_path.reverse();
    v_path.extend(w_path);
    FindPathResult { path: v_path, lca }
}

fn postorder(g: &LayoutGraph) -> HashMap<String, PostorderNum> {
    let mut result = HashMap::new();
    let mut lim: i64 = 0;

    fn dfs(g: &LayoutGraph, v: &str, result: &mut HashMap<String, PostorderNum>, lim: &mut i64) {
        let low = *lim;
        let children = g.children(Some(v)).unwrap_or_default();
        for child in &children {
            dfs(g, child, result, lim);
        }
        result.insert(v.to_string(), PostorderNum { low, lim: *lim });
        *lim += 1;
    }

    let root_children = g.children(None).unwrap_or_default();
    for v in &root_children {
        dfs(g, v, &mut result, &mut lim);
    }

    result
}
