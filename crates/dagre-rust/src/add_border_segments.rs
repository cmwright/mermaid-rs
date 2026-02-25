//! Adds border segments to subgraph nodes.
//! Port of dagre's `add-border-segments.js`.

use crate::graph::LayoutGraph;
use crate::types::*;
use crate::util::add_dummy_node;

/// Adds border nodes (left and right) for each rank in each subgraph.
pub fn add_border_segments(g: &mut LayoutGraph) {
    let root_children = g.children(None).unwrap_or_default();
    for v in root_children {
        dfs(g, &v);
    }
}

fn dfs(g: &mut LayoutGraph, v: &str) {
    let children = g.children(Some(v)).unwrap_or_default();
    if !children.is_empty() {
        for child in &children {
            dfs(g, child);
        }
    }

    let (min_rank_opt, min_rank, max_rank) = match g.node(v) {
        Some(n) => (n.min_rank, n.min_rank.unwrap_or(0), n.max_rank.unwrap_or(0)),
        None => return,
    };
    if min_rank_opt.is_some() {
        // Initialize borderLeft and borderRight arrays
        if let Some(node_mut) = g.node_mut(v) {
            node_mut.border_left = Vec::new();
            node_mut.border_right = Vec::new();
        }

        for rank in min_rank..=max_rank {
            add_border_node_for_sg(g, true, "_bl", v, rank);
            add_border_node_for_sg(g, false, "_br", v, rank);
        }
    }
}

fn add_border_node_for_sg(g: &mut LayoutGraph, is_left: bool, prefix: &str, sg: &str, rank: i64) {
    let label = NodeLabel {
        rank: Some(rank),
        border_type: Some(if is_left {
            BorderType::Left
        } else {
            BorderType::Right
        }),
        ..Default::default()
    };

    // Get prev from sgNode's border array at rank - 1
    let prev = {
        let arr = match g.node(sg) {
            Some(n) => {
                if is_left {
                    &n.border_left
                } else {
                    &n.border_right
                }
            }
            None => return,
        };
        let idx = (rank - 1) as usize;
        arr.get(idx).and_then(|o| o.clone())
    };

    let curr = add_dummy_node(g, DummyType::Border, label, prefix);

    // Set sgNode's border array at rank = curr
    if let Some(node_mut) = g.node_mut(sg) {
        let arr = if is_left {
            &mut node_mut.border_left
        } else {
            &mut node_mut.border_right
        };
        let idx = rank as usize;
        while arr.len() <= idx {
            arr.push(None);
        }
        arr[idx] = Some(curr.clone());
    }

    g.set_parent(&curr, Some(sg));

    if let Some(prev) = prev {
        g.set_edge(
            &prev,
            &curr,
            Some(EdgeLabel {
                weight: 1.0,
                ..Default::default()
            }),
            None,
        );
    }
}
