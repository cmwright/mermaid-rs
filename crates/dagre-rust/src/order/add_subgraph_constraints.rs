//! Adds subgraph ordering constraints.
//! Port of dagre's `order/add-subgraph-constraints.js`.

use crate::graph::{ConstraintGraph, LayoutGraph};
use ahash::AHashMap as HashMap;

/// Adds ordering constraints between subgraphs based on the sorted layer.
pub fn add_subgraph_constraints(g: &LayoutGraph, cg: &mut ConstraintGraph, vs: &[String]) {
    let mut prev: HashMap<&str, &str> = HashMap::new();
    let mut root_prev: Option<&str> = None;

    for v in vs {
        let mut child = g.parent(v.as_str());
        let mut prev_child: Option<&str>;

        while let Some(c) = child {
            let parent = g.parent(c);
            if let Some(p) = parent {
                prev_child = prev.get(p).copied();
                prev.insert(p, c);
            } else {
                prev_child = root_prev;
                root_prev = Some(c);
            }

            if let Some(pc) = prev_child
                && pc != c
            {
                cg.set_edge(pc, c, None, None);
                return; // only add one constraint per node
            }

            child = parent;
        }
    }
}
