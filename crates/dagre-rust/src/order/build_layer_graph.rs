//! Builds a layer graph for sorting a single rank.
//! Port of dagre's `order/build-layer-graph.js`.

use crate::graph::{GraphOptions, LayoutGraph};
use crate::types::*;
use crate::util::unique_id;

/// Constructs a graph for sorting nodes at a given rank.
pub fn build_layer_graph(
    g: &LayoutGraph,
    rank: i64,
    use_in_edges: bool,
    nodes_with_rank: &[String],
) -> LayoutGraph {
    let root = create_root_node(g);
    let mut result = LayoutGraph::with_options(&GraphOptions {
        directed: true,
        multigraph: false,
        compound: true,
    });
    result.set_graph(GraphLabel {
        root: Some(root.clone()),
        ..Default::default()
    });

    for v in nodes_with_rank {
        let node = match g.node(v) {
            Some(n) => n,
            None => continue,
        };

        let in_range = node.rank == Some(rank)
            || matches!((node.min_rank, node.max_rank), (Some(min_r), Some(max_r)) if min_r <= rank && rank <= max_r);

        if !in_range {
            continue;
        }

        // Layer graph sorting only needs order (and border refs for subgraph nodes),
        // so avoid cloning full node labels into every temporary layer graph.
        result.set_node(
            v,
            Some(NodeLabel {
                order: node.order,
                ..Default::default()
            }),
        );
        let parent = g.parent(v).unwrap_or(&root);
        result.set_parent(v, Some(parent));

        // Get edges based on relationship (inEdges or outEdges)
        let edge_ids = if use_in_edges {
            g.in_edge_ids(v).unwrap_or(&[])
        } else {
            g.out_edge_ids(v).unwrap_or(&[])
        };

        for edge_id in edge_ids {
            let Some(e) = g.edge_obj_by_id(edge_id) else {
                continue;
            };
            let u = if e.v == *v {
                e.w.as_str()
            } else {
                e.v.as_str()
            };
            // Ensure the adjacent-layer node has a label so its order field
            // is accessible.  In JS dagre this happens automatically via
            // setDefaultNodeLabel which returns g.node(v) by reference.
            if !result.has_node(u)
                && let Some(u_node) = g.node(u)
            {
                result.set_node(
                    u,
                    Some(NodeLabel {
                        order: u_node.order,
                        ..Default::default()
                    }),
                );
            }
            let edge_weight = g.edge_label_by_id(edge_id).map(|l| l.weight).unwrap_or(0.0);
            let existing_weight = result.edge(u, v, None).map(|l| l.weight).unwrap_or(0.0);
            result.set_edge(
                u,
                v,
                Some(EdgeLabel {
                    weight: edge_weight + existing_weight,
                    ..Default::default()
                }),
                None,
            );
        }

        // If node has minRank, set border info
        if node.min_rank.is_some() {
            let bl = node
                .border_left
                .get(rank as usize)
                .and_then(|o| o.as_deref());
            let br = node
                .border_right
                .get(rank as usize)
                .and_then(|o| o.as_deref());
            let mut override_label = NodeLabel {
                order: node.order,
                ..Default::default()
            };
            if let Some(bl_str) = bl {
                override_label.border_left = vec![Some(bl_str.to_string())];
            }
            if let Some(br_str) = br {
                override_label.border_right = vec![Some(br_str.to_string())];
            }
            result.set_node(v, Some(override_label));
        }
    }

    result
}

fn create_root_node(g: &LayoutGraph) -> String {
    let mut v;
    loop {
        v = unique_id("_root");
        if !g.has_node(&v) {
            break;
        }
    }
    v
}
