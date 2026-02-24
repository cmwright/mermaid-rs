//! Cycle removal via DFS or greedy feedback arc set.
//! Port of dagre's `acyclic.js`.

use crate::graph::{Edge, LayoutGraph};
use crate::greedy_fas;
use crate::types::*;
use crate::util::unique_id;
use std::collections::HashSet;

/// Makes the graph acyclic by reversing edges in feedback arc set.
pub fn run(g: &mut LayoutGraph) {
    let fas = if g.graph().acyclicer == Some(Acyclicer::Greedy) {
        let weight_fn = |e: &Edge, g: &LayoutGraph| -> f64 {
            g.edge_by_obj(e).map(|l| l.weight).unwrap_or(1.0)
        };
        greedy_fas::greedy_fas(g, Some(&weight_fn))
    } else {
        dfs_fas(g)
    };

    for e in fas {
        let label = g.edge_by_obj(&e).cloned();
        g.remove_edge_by_obj(&e);
        if let Some(mut label) = label {
            label.forward_name = e.name.clone();
            label.reversed = true;
            let rev_name = unique_id("rev");
            g.set_edge(&e.w, &e.v, Some(label), Some(&rev_name));
        }
    }
}

/// DFS-based feedback arc set finder.
fn dfs_fas(g: &LayoutGraph) -> Vec<Edge> {
    let mut fas: Vec<Edge> = Vec::new();
    let mut stack: HashSet<String> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();

    fn dfs(
        g: &LayoutGraph,
        v: &str,
        fas: &mut Vec<Edge>,
        stack: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        if visited.contains(v) {
            return;
        }
        visited.insert(v.to_string());
        stack.insert(v.to_string());

        if let Some(out_edges) = g.out_edges(v, None) {
            for e in out_edges {
                if stack.contains(&e.w) {
                    fas.push(e);
                } else {
                    dfs(g, &e.w, fas, stack, visited);
                }
            }
        }

        stack.remove(v);
    }

    for v in g.nodes() {
        dfs(g, &v, &mut fas, &mut stack, &mut visited);
    }

    fas
}

/// Undoes the edge reversals performed by `run`.
pub fn undo(g: &mut LayoutGraph) {
    let edges_to_reverse: Vec<(Edge, EdgeLabel)> = g
        .edges()
        .into_iter()
        .filter_map(|e| {
            let label = g.edge_by_obj(&e)?.clone();
            if label.reversed {
                Some((e, label))
            } else {
                None
            }
        })
        .collect();

    for (e, mut label) in edges_to_reverse {
        g.remove_edge_by_obj(&e);

        let forward_name = label.forward_name.take();
        label.reversed = false;

        g.set_edge(&e.w, &e.v, Some(label), forward_name.as_deref());
    }
}
