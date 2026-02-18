use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use std::collections::HashMap;

use crate::ast::flowchart::NodeShape;
use crate::layout::types::*;

/// Information about a chain of dummy nodes inserted for a long edge.
#[derive(Debug, Clone)]
pub struct DummyChain {
    /// The original edge endpoints (source, target) in the full graph.
    pub original_source: NodeIndex,
    pub original_target: NodeIndex,
    /// The original edge data.
    pub edge_data: EdgeData,
    /// Dummy node indices in order from source to target.
    pub dummy_nodes: Vec<NodeIndex>,
    /// The dummy node that carries the edge label (if any).
    pub label_node: Option<NodeIndex>,
}

/// For edges spanning >1 rank, remove the original edge and insert a chain
/// of zero-size dummy nodes at intermediate ranks.
/// Returns the list of dummy chains for later edge reconstruction.
pub fn insert_dummy_nodes(
    graph: &mut DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
) -> Vec<DummyChain> {
    let mut chains = Vec::new();

    // Collect edges that span >1 rank
    let long_edges: Vec<(EdgeIndex, NodeIndex, NodeIndex, usize, usize)> = graph
        .edge_indices()
        .filter_map(|ei| {
            let (src, tgt) = graph.edge_endpoints(ei)?;
            let src_rank = *ranks.get(&src)?;
            let tgt_rank = *ranks.get(&tgt)?;
            if tgt_rank > src_rank + 1 {
                Some((ei, src, tgt, src_rank, tgt_rank))
            } else {
                None
            }
        })
        .collect();

    for (edge_idx, src, tgt, src_rank, tgt_rank) in long_edges {
        let edge_data = graph.remove_edge(edge_idx).unwrap();

        // Determine the label rank (midpoint) for labeled edges
        let label_rank = if edge_data.label.is_some() {
            Some((src_rank + tgt_rank) / 2)
        } else {
            None
        };

        let mut dummy_nodes = Vec::new();
        let mut label_node = None;
        let mut prev = src;

        for rank in (src_rank + 1)..tgt_rank {
            // Give the midpoint dummy the label's dimensions
            let (w, h) = if label_rank == Some(rank) {
                (edge_data.label_width, edge_data.label_height)
            } else {
                (0.0, 0.0)
            };

            let dummy = graph.add_node(NodeData {
                id: format!("__dummy_{}_{}", chains.len(), rank),
                label: String::new(),
                shape: NodeShape::Rectangle,
                style: Default::default(),
                width: w,
                height: h,
            });
            ranks.insert(dummy, rank);
            dummy_nodes.push(dummy);

            if label_rank == Some(rank) {
                label_node = Some(dummy);
            }

            // Add edge from previous node to this dummy
            graph.add_edge(
                prev,
                dummy,
                EdgeData {
                    label: None,
                    edge_type: edge_data.edge_type,
                    label_width: 0.0,
                    label_height: 0.0,
                },
            );
            prev = dummy;
        }

        // Add edge from last dummy to original target
        graph.add_edge(
            prev,
            tgt,
            EdgeData {
                label: None,
                edge_type: edge_data.edge_type,
                label_width: 0.0,
                label_height: 0.0,
            },
        );

        chains.push(DummyChain {
            original_source: src,
            original_target: tgt,
            edge_data,
            dummy_nodes,
            label_node,
        });
    }

    chains
}

/// Remove dummy nodes from positions map and return bend points for long edges.
pub fn extract_dummy_positions(
    chains: &[DummyChain],
    positions: &HashMap<NodeIndex, (f64, f64)>,
) -> Vec<(NodeIndex, NodeIndex, EdgeData, Vec<(f64, f64)>)> {
    chains
        .iter()
        .map(|chain| {
            let bend_points: Vec<(f64, f64)> = chain
                .dummy_nodes
                .iter()
                .filter_map(|&dummy| positions.get(&dummy).copied())
                .collect();
            (
                chain.original_source,
                chain.original_target,
                chain.edge_data.clone(),
                bend_points,
            )
        })
        .collect()
}
