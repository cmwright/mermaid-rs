//! Adds subgraph ordering constraints.
//! Port of dagre's `order/add-subgraph-constraints.js`.

use crate::graph::{ConstraintGraph, LayoutGraph};
use std::collections::HashMap;

/// Adds ordering constraints between subgraphs based on the sorted layer.
pub fn add_subgraph_constraints(g: &LayoutGraph, cg: &mut ConstraintGraph, vs: &[String]) {
    let mut prev: HashMap<String, String> = HashMap::new();
    let mut root_prev: Option<String> = None;

    for v in vs {
        let mut child = g.parent(v).map(|s| s.to_string());
        let mut prev_child: Option<String>;

        while let Some(ref c) = child {
            let parent = g.parent(c).map(|s| s.to_string());
            if let Some(ref p) = parent {
                prev_child = prev.get(p).cloned();
                prev.insert(p.clone(), c.clone());
            } else {
                prev_child = root_prev.clone();
                root_prev = Some(c.clone());
            }

            if let Some(ref pc) = prev_child
                && pc != c
            {
                cg.set_edge(pc, c, None, None);
                return; // only add one constraint per node
            }

            child = parent;
        }
    }
}
