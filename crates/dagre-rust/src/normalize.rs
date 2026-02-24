//! Edge normalization: splitting long edges into chains of unit-length edges.
//! Port of dagre's `normalize.js`.

use crate::graph::{Edge, LayoutGraph};
use crate::types::*;
use crate::util::add_dummy_node;

/// Splits long edges into chains of dummy nodes, each spanning one rank.
pub fn run(g: &mut LayoutGraph) {
    // Initialize dummyChains on graph label
    g.graph_mut().dummy_chains = Vec::new();

    let edges = g.edges();
    for e in edges {
        normalize_edge(g, &e);
    }
}

fn normalize_edge(g: &mut LayoutGraph, e: &Edge) {
    let v = e.v.clone();
    let v_rank = g.node(&v).and_then(|n| n.rank).unwrap_or(0);
    let w = e.w.clone();
    let w_rank = g.node(&w).and_then(|n| n.rank).unwrap_or(0);
    let name = e.name.clone();
    let edge_label = g.edge_by_obj(e).cloned().unwrap_or_default();
    let label_rank = edge_label.label_rank;
    if w_rank == v_rank + 1 {
        return;
    }

    g.remove_edge_by_obj(e);

    let weight = edge_label.weight;
    let mut current_v = v.clone();
    let mut v_rank = v_rank + 1;
    let mut i = 0;

    while v_rank < w_rank {
        let mut edge_label_clone = edge_label.clone();
        edge_label_clone.points = Vec::new();

        let dummy_type = if Some(v_rank) == label_rank {
            DummyType::EdgeLabel
        } else {
            DummyType::Edge
        };

        let mut attrs = NodeLabel {
            edge_label: Some(Box::new(edge_label_clone)),
            edge_obj: Some(Edge::new(&e.v, &e.w, e.name.as_deref())),
            rank: Some(v_rank),
            ..Default::default()
        };

        if Some(v_rank) == label_rank {
            attrs.width = edge_label.width;
            attrs.height = edge_label.height;
            attrs.label_pos = Some(edge_label.labelpos);
        }

        let dummy = add_dummy_node(g, dummy_type, attrs, "_d");

        g.set_edge(
            &current_v,
            &dummy,
            Some(EdgeLabel {
                weight,
                ..Default::default()
            }),
            name.as_deref(),
        );

        if i == 0 {
            g.graph_mut().dummy_chains.push(dummy.clone());
        }

        current_v = dummy;
        i += 1;
        v_rank += 1;
    }

    g.set_edge(
        &current_v,
        &w,
        Some(EdgeLabel {
            weight,
            ..Default::default()
        }),
        name.as_deref(),
    );
}

/// Undoes normalization by restoring original long edges from dummy chains.
pub fn undo(g: &mut LayoutGraph) {
    let dummy_chains = g.graph().dummy_chains.clone();

    for chain_start in dummy_chains {
        let mut v = chain_start;
        let node = g.node(&v).cloned().unwrap_or_default();
        let orig_label = node.edge_label.map(|b| *b).unwrap_or_default();

        // Reconstruct edge obj
        let edge_obj = node
            .edge_obj
            .clone()
            .unwrap_or_else(|| Edge::new("", "", None));
        let edge_obj_v = edge_obj.v.clone();
        let edge_obj_w = edge_obj.w.clone();
        let edge_obj_name = edge_obj.name.clone();

        g.set_edge(
            &edge_obj_v,
            &edge_obj_w,
            Some(orig_label.clone()),
            edge_obj_name.as_deref(),
        );

        let mut orig_label = orig_label;

        loop {
            let node = g.node(&v).cloned().unwrap_or_default();
            let is_dummy = node.dummy.is_some();

            if !is_dummy {
                break;
            }

            let w = g
                .successors(&v)
                .and_then(|s| s.first().cloned())
                .unwrap_or_default();

            g.remove_node(&v);

            // Push point
            let x = node.x.unwrap_or(0.0);
            let y = node.y.unwrap_or(0.0);
            orig_label.points.push(Point { x, y });

            // If edge-label dummy, copy position info
            if node.dummy == Some(DummyType::EdgeLabel) {
                orig_label.x = Some(x);
                orig_label.y = Some(y);
                orig_label.width = node.width;
                orig_label.height = node.height;
            }

            v = w;
        }

        // Update the edge label we set earlier
        g.set_edge(
            &edge_obj_v,
            &edge_obj_w,
            Some(orig_label),
            edge_obj_name.as_deref(),
        );
    }
}
