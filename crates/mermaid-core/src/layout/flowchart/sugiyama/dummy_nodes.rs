use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

use crate::ast::flowchart::NodeShape;
use crate::layout::flowchart::types::*;

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

    // Collect edges that span >1 rank.
    // We store (src, tgt) endpoints — NOT EdgeIndex — because petgraph's
    // `remove_edge` invalidates the last edge index via a swap, making
    // pre-collected indices stale after any removal.
    let long_edges: Vec<(NodeIndex, NodeIndex, usize, usize)> = graph
        .edge_indices()
        .filter_map(|ei| {
            let (src, tgt) = graph.edge_endpoints(ei)?;
            let src_rank = *ranks.get(&src)?;
            let tgt_rank = *ranks.get(&tgt)?;
            if tgt_rank > src_rank + 1 {
                Some((src, tgt, src_rank, tgt_rank))
            } else {
                None
            }
        })
        .collect();

    for (src, tgt, src_rank, tgt_rank) in long_edges {
        // Find and remove the edge by endpoints (safe after prior mutations).
        let edge_idx = graph.find_edge(src, tgt).expect("long edge disappeared");
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
                    line_style: edge_data.line_style,
                    arrow_start: edge_data.arrow_start,
                    arrow_end: edge_data.arrow_end,
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
                line_style: edge_data.line_style,
                arrow_start: edge_data.arrow_start,
                arrow_end: edge_data.arrow_end,
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

/// Remove dummy nodes from the positions map and return bend points for long edges.
#[allow(clippy::type_complexity)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{ArrowEnd, LineStyle};

    fn make_node(id: &str) -> NodeData {
        NodeData {
            id: id.to_string(),
            label: String::new(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        }
    }

    fn make_edge_data(label: Option<&str>) -> EdgeData {
        let (lw, lh) = if label.is_some() { (50.0, 15.0) } else { (0.0, 0.0) };
        EdgeData {
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: label.map(String::from),
            label_width: lw,
            label_height: lh,
        }
    }

    #[test]
    fn test_no_long_edges() {
        // All edges span exactly 1 rank: no dummies needed
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge_data(None));

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 1);

        let chains = insert_dummy_nodes(&mut g, &mut ranks);
        assert!(chains.is_empty(), "no long edges, no dummies");
        assert_eq!(g.node_count(), 2, "no new nodes added");
    }

    #[test]
    fn test_long_edge_spanning_two_ranks() {
        // A at rank 0, B at rank 2 -> edge spans 2 ranks -> 1 dummy at rank 1
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge_data(None));

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 2);

        let chains = insert_dummy_nodes(&mut g, &mut ranks);
        assert_eq!(chains.len(), 1, "one long edge produces one chain");
        assert_eq!(chains[0].dummy_nodes.len(), 1, "spanning 2 ranks needs 1 dummy");
        assert_eq!(chains[0].original_source, a);
        assert_eq!(chains[0].original_target, b);
        assert!(chains[0].label_node.is_none());

        // Verify rank of the dummy
        let dummy = chains[0].dummy_nodes[0];
        assert_eq!(ranks[&dummy], 1);
    }

    #[test]
    fn test_long_edge_spanning_three_ranks() {
        // A at rank 0, B at rank 3 -> 2 dummies at ranks 1 and 2
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge_data(None));

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 3);

        let chains = insert_dummy_nodes(&mut g, &mut ranks);
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].dummy_nodes.len(), 2);
        assert_eq!(ranks[&chains[0].dummy_nodes[0]], 1);
        assert_eq!(ranks[&chains[0].dummy_nodes[1]], 2);
    }

    #[test]
    fn test_long_edge_with_label() {
        // A at rank 0, B at rank 4 -> 3 dummies at ranks 1,2,3
        // Label dummy should be at midpoint rank (0+4)/2 = 2
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge_data(Some("my label")));

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 4);

        let chains = insert_dummy_nodes(&mut g, &mut ranks);
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].dummy_nodes.len(), 3);
        assert!(chains[0].label_node.is_some());

        let label_dummy = chains[0].label_node.unwrap();
        assert_eq!(ranks[&label_dummy], 2, "label dummy should be at midpoint rank");

        // Label dummy should have the label dimensions
        let label_data = &g[label_dummy];
        assert!((label_data.width - 50.0).abs() < 0.1);
        assert!((label_data.height - 15.0).abs() < 0.1);
    }

    #[test]
    fn test_multiple_long_edges() {
        // A->C spans 2, B->D spans 2
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        let c = g.add_node(make_node("C"));
        let d = g.add_node(make_node("D"));
        g.add_edge(a, c, make_edge_data(None));
        g.add_edge(b, d, make_edge_data(None));

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 0);
        ranks.insert(c, 2);
        ranks.insert(d, 2);

        let chains = insert_dummy_nodes(&mut g, &mut ranks);
        assert_eq!(chains.len(), 2, "two long edges, two chains");
    }

    #[test]
    fn test_extract_dummy_positions() {
        // Build a chain manually
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge_data(None));

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 3);

        let chains = insert_dummy_nodes(&mut g, &mut ranks);
        assert_eq!(chains.len(), 1);

        // Create positions for all nodes including dummies
        let mut positions = HashMap::new();
        positions.insert(a, (100.0, 0.0));
        positions.insert(b, (100.0, 300.0));
        for (i, &dummy) in chains[0].dummy_nodes.iter().enumerate() {
            positions.insert(dummy, (100.0, (i + 1) as f64 * 100.0));
        }

        let extracted = extract_dummy_positions(&chains, &positions);
        assert_eq!(extracted.len(), 1);
        let (src, tgt, _data, bps) = &extracted[0];
        assert_eq!(*src, a);
        assert_eq!(*tgt, b);
        assert_eq!(bps.len(), 2); // 2 dummies
        assert!((bps[0].1 - 100.0).abs() < 0.1);
        assert!((bps[1].1 - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_dummy_chain_connectivity() {
        // Verify the chain of edges is properly connected
        let mut g = DiGraph::new();
        let a = g.add_node(make_node("A"));
        let b = g.add_node(make_node("B"));
        g.add_edge(a, b, make_edge_data(None));

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 3);

        let chains = insert_dummy_nodes(&mut g, &mut ranks);
        let chain = &chains[0];

        // Original edge should be removed
        assert!(g.find_edge(a, b).is_none(), "original long edge should be removed");

        // Path should be A -> dummy1 -> dummy2 -> B
        assert!(g.find_edge(a, chain.dummy_nodes[0]).is_some());
        assert!(g.find_edge(chain.dummy_nodes[0], chain.dummy_nodes[1]).is_some());
        assert!(g.find_edge(chain.dummy_nodes[1], b).is_some());
    }
}
