//! Border segment nodes for compound (subgraph) layout.
//!
//! Implements dagre's `add-border-segments.js` algorithm: after rank assignment,
//! each subgraph spans a range of ranks (minRank to maxRank). This module adds
//! left and right border dummy nodes at every rank in that range, connected
//! vertically by weight-1 edges. These form two "columns" that bracket the
//! subgraph's content.
//!
//! Border nodes serve two purposes:
//! 1. During ordering, they are pinned at the left/right edges of the subgraph's
//!    sorted order, ensuring contiguity.
//! 2. During coordinate assignment, they define the subgraph's horizontal extent.

use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

use crate::ast::flowchart::{FlowchartAst, NodeShape, SubgraphDef};
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;

/// Border node information for all subgraphs.
#[derive(Debug, Clone)]
pub struct BorderSegments {
    /// For each subgraph id: the border info.
    pub subgraphs: HashMap<String, SubgraphBorders>,
}

/// Border node information for a single subgraph.
#[derive(Debug, Clone)]
pub struct SubgraphBorders {
    /// Left border node at each rank (sparse: only ranks in minRank..=maxRank).
    pub border_left: HashMap<usize, NodeIndex>,
    /// Right border node at each rank.
    pub border_right: HashMap<usize, NodeIndex>,
    /// The minimum rank spanned by this subgraph's members.
    pub min_rank: usize,
    /// The maximum rank spanned by this subgraph's members.
    pub max_rank: usize,
}

/// Add border segment nodes to the graph.
///
/// For each subgraph, computes the rank range (minRank..maxRank) from its
/// member nodes, then creates left and right border dummy nodes at each rank,
/// connected vertically by weight-1 edges.
///
/// Returns a `BorderSegments` structure that the ordering algorithm uses
/// to pin border nodes at the subgraph boundaries.
pub fn add_border_segments(
    graph: &mut DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    ast: &FlowchartAst,
    membership: &SubgraphMembership,
) -> BorderSegments {
    add_border_segments_with_ranges(graph, ranks, ast, membership, None)
}

/// Add border segments using pre-computed rank ranges from the nesting state.
///
/// Matches dagre's `addBorderSegments` which reads `minRank/maxRank` from
/// compound node properties (set by `assignRankMinMax` from bt/bb nodes),
/// NOT from scanning member nodes. When `precomputed_ranges` is `Some`,
/// those ranges are used instead of scanning membership.
pub fn add_border_segments_with_ranges(
    graph: &mut DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    ast: &FlowchartAst,
    membership: &SubgraphMembership,
    precomputed_ranges: Option<&BorderSegments>,
) -> BorderSegments {
    let mut segments = BorderSegments {
        subgraphs: HashMap::new(),
    };

    // Build a map from node ID to NodeIndex for lookups.
    let id_to_idx: HashMap<String, NodeIndex> = graph
        .node_indices()
        .map(|ni| (graph[ni].id.clone(), ni))
        .collect();

    // Process subgraphs in post-order (innermost first) so that child
    // subgraph border nodes are already created when we process the parent.
    process_subgraphs_postorder(
        &ast.subgraphs,
        &[],
        graph,
        ranks,
        membership,
        &id_to_idx,
        &mut segments,
        precomputed_ranges,
    );

    segments
}

/// Recursively process subgraphs in post-order, adding border segments.
fn process_subgraphs_postorder(
    subgraphs: &[SubgraphDef],
    parent_path: &[String],
    graph: &mut DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    membership: &SubgraphMembership,
    id_to_idx: &HashMap<String, NodeIndex>,
    segments: &mut BorderSegments,
    precomputed_ranges: Option<&BorderSegments>,
) {
    for sg in subgraphs {
        let mut path = parent_path.to_vec();
        path.push(sg.id.clone());

        // Process children first (post-order).
        process_subgraphs_postorder(
            &sg.subgraphs,
            &path,
            graph,
            ranks,
            membership,
            id_to_idx,
            segments,
            precomputed_ranges,
        );

        // Determine the rank range for this subgraph.
        let (min_rank, max_rank, has_members);

        if let Some(pre) = precomputed_ranges.and_then(|p| p.subgraphs.get(&sg.id)) {
            // Use nesting-derived ranges matching dagre's assignRankMinMax.
            min_rank = pre.min_rank;
            max_rank = pre.max_rank;
            has_members = true;
        } else {
            // Fallback: compute from member nodes and child subgraph borders.
            let mut mn = usize::MAX;
            let mut mx = 0usize;
            let mut found = false;

            for (node_id, node_path) in membership.iter() {
                if node_path.contains(&sg.id) {
                    if let Some(&ni) = id_to_idx.get(node_id.as_str()) {
                        if let Some(&rank) = ranks.get(&ni) {
                            mn = mn.min(rank);
                            mx = mx.max(rank);
                            found = true;
                        }
                    }
                }
            }

            for child_sg in &sg.subgraphs {
                if let Some(child_borders) = segments.subgraphs.get(&child_sg.id) {
                    mn = mn.min(child_borders.min_rank);
                    mx = mx.max(child_borders.max_rank);
                    found = true;
                }
            }

            min_rank = mn;
            max_rank = mx;
            has_members = found;
        }

        if !has_members {
            continue;
        }

        // Create left and right border nodes at each rank in the range.
        let mut border_left = HashMap::new();
        let mut border_right = HashMap::new();

        for rank in min_rank..=max_rank {
            let bl = graph.add_node(NodeData {
                id: format!("__border_bl_{}_{}", sg.id, rank),
                label: String::new(),
                shape: NodeShape::Rectangle,
                style: Default::default(),
                width: 0.0,
                height: 0.0,
            });
            ranks.insert(bl, rank);

            let br = graph.add_node(NodeData {
                id: format!("__border_br_{}_{}", sg.id, rank),
                label: String::new(),
                shape: NodeShape::Rectangle,
                style: Default::default(),
                width: 0.0,
                height: 0.0,
            });
            ranks.insert(br, rank);

            // Connect to previous rank's border nodes (vertical chain).
            if rank > min_rank {
                if let Some(&prev_bl) = border_left.get(&(rank - 1)) {
                    graph.add_edge(
                        prev_bl,
                        bl,
                        EdgeData {
                            label: None,
                            line_style: crate::ast::flowchart::LineStyle::Solid,
                            arrow_start: crate::ast::flowchart::ArrowEnd::None,
                            arrow_end: crate::ast::flowchart::ArrowEnd::None,
                            label_width: 0.0,
                            label_height: 0.0,
                            weight: 1,
                            minlen: 1,
                        },
                    );
                }
                if let Some(&prev_br) = border_right.get(&(rank - 1)) {
                    graph.add_edge(
                        prev_br,
                        br,
                        EdgeData {
                            label: None,
                            line_style: crate::ast::flowchart::LineStyle::Solid,
                            arrow_start: crate::ast::flowchart::ArrowEnd::None,
                            arrow_end: crate::ast::flowchart::ArrowEnd::None,
                            label_width: 0.0,
                            label_height: 0.0,
                            weight: 1,
                            minlen: 1,
                        },
                    );
                }
            }

            border_left.insert(rank, bl);
            border_right.insert(rank, br);
        }

        segments.subgraphs.insert(
            sg.id.clone(),
            SubgraphBorders {
                border_left,
                border_right,
                min_rank,
                max_rank,
            },
        );
    }
}

/// Remove all border segment nodes from the graph and ranks map.
///
/// Called after coordinate assignment is complete and border node positions
/// have been used to compute subgraph bounding boxes.
pub fn remove_border_segments(
    graph: &mut DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    segments: &BorderSegments,
) {
    // Collect all border node indices.
    let mut to_remove: Vec<NodeIndex> = Vec::new();
    for borders in segments.subgraphs.values() {
        to_remove.extend(borders.border_left.values());
        to_remove.extend(borders.border_right.values());
    }

    // Sort descending to avoid index invalidation during swap-removal.
    to_remove.sort_by(|a, b| b.index().cmp(&a.index()));
    to_remove.dedup();

    for ni in to_remove {
        if graph.node_weight(ni).is_none() {
            continue;
        }

        let last_idx = NodeIndex::new(graph.node_count() - 1);
        if ni != last_idx {
            if let Some(rank) = ranks.remove(&last_idx) {
                ranks.insert(ni, rank);
            }
        }
        ranks.remove(&ni);
        graph.remove_node(ni);
    }
}

/// Get the left border node for a subgraph at a specific rank.
pub fn get_border_left(segments: &BorderSegments, sg_id: &str, rank: usize) -> Option<NodeIndex> {
    segments
        .subgraphs
        .get(sg_id)
        .and_then(|b| b.border_left.get(&rank).copied())
}

/// Get the right border node for a subgraph at a specific rank.
pub fn get_border_right(segments: &BorderSegments, sg_id: &str, rank: usize) -> Option<NodeIndex> {
    segments
        .subgraphs
        .get(sg_id)
        .and_then(|b| b.border_right.get(&rank).copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{ArrowEnd, LineStyle, NodeDef, SubgraphDef};

    fn make_node_data(id: &str) -> NodeData {
        NodeData {
            id: id.to_string(),
            label: String::new(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 40.0,
            height: 20.0,
        }
    }

    fn make_edge_data() -> EdgeData {
        EdgeData {
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            label_width: 0.0,
            label_height: 0.0,
            weight: 1,
            minlen: 1,
        }
    }

    #[test]
    fn test_single_subgraph_border_segments() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(make_node_data("A"));
        let b = graph.add_node(make_node_data("B"));
        graph.add_edge(a, b, make_edge_data());

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 1);

        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "SG".to_string(),
                label: None,
                direction: None,
                nodes: vec![
                    NodeDef {
                        id: "A".into(),
                        label: Some("A".into()),
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    },
                    NodeDef {
                        id: "B".into(),
                        label: Some("B".into()),
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    },
                ],
                edges: vec![],
                subgraphs: vec![],
            }],
            ..Default::default()
        };

        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec!["SG".to_string()]);

        let segments = add_border_segments(&mut graph, &mut ranks, &ast, &membership);

        // Should have border info for SG
        assert!(segments.subgraphs.contains_key("SG"));
        let sg_borders = &segments.subgraphs["SG"];
        assert_eq!(sg_borders.min_rank, 0);
        assert_eq!(sg_borders.max_rank, 1);

        // Should have left and right border nodes at ranks 0 and 1
        assert!(sg_borders.border_left.contains_key(&0));
        assert!(sg_borders.border_left.contains_key(&1));
        assert!(sg_borders.border_right.contains_key(&0));
        assert!(sg_borders.border_right.contains_key(&1));

        // Border nodes should be in the graph
        let bl0 = sg_borders.border_left[&0];
        let br0 = sg_borders.border_right[&0];
        assert!(graph[bl0].id.starts_with("__border_bl_SG_"));
        assert!(graph[br0].id.starts_with("__border_br_SG_"));

        // Border nodes should have correct ranks
        assert_eq!(ranks[&bl0], 0);
        assert_eq!(ranks[&br0], 0);

        // Vertical chain: bl0 -> bl1 should exist
        let bl1 = sg_borders.border_left[&1];
        assert!(graph.find_edge(bl0, bl1).is_some());
    }

    #[test]
    fn test_nested_subgraph_border_segments() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(make_node_data("A"));
        let b = graph.add_node(make_node_data("B"));
        let c = graph.add_node(make_node_data("C"));
        graph.add_edge(a, b, make_edge_data());
        graph.add_edge(b, c, make_edge_data());

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 1);
        ranks.insert(c, 2);

        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "Outer".to_string(),
                label: None,
                direction: None,
                nodes: vec![NodeDef {
                    id: "C".into(),
                    label: Some("C".into()),
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                }],
                edges: vec![],
                subgraphs: vec![SubgraphDef {
                    id: "Inner".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        NodeDef {
                            id: "A".into(),
                            label: Some("A".into()),
                            shape: NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        NodeDef {
                            id: "B".into(),
                            label: Some("B".into()),
                            shape: NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![],
                    subgraphs: vec![],
                }],
            }],
            ..Default::default()
        };

        let mut membership = SubgraphMembership::new();
        membership.insert(
            "A".to_string(),
            vec!["Outer".to_string(), "Inner".to_string()],
        );
        membership.insert(
            "B".to_string(),
            vec!["Outer".to_string(), "Inner".to_string()],
        );
        membership.insert("C".to_string(), vec!["Outer".to_string()]);

        let segments = add_border_segments(&mut graph, &mut ranks, &ast, &membership);

        // Inner should span ranks 0..1
        let inner = &segments.subgraphs["Inner"];
        assert_eq!(inner.min_rank, 0);
        assert_eq!(inner.max_rank, 1);

        // Outer should span ranks 0..2 (includes Inner's range + C at rank 2)
        let outer = &segments.subgraphs["Outer"];
        assert_eq!(outer.min_rank, 0);
        assert_eq!(outer.max_rank, 2);

        // Outer should have border nodes at ranks 0, 1, 2
        assert_eq!(outer.border_left.len(), 3);
        assert_eq!(outer.border_right.len(), 3);
    }

    #[test]
    fn test_remove_border_segments() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(make_node_data("A"));
        let b = graph.add_node(make_node_data("B"));
        graph.add_edge(a, b, make_edge_data());

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 1);

        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "SG".to_string(),
                label: None,
                direction: None,
                nodes: vec![
                    NodeDef {
                        id: "A".into(),
                        label: Some("A".into()),
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    },
                    NodeDef {
                        id: "B".into(),
                        label: Some("B".into()),
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    },
                ],
                edges: vec![],
                subgraphs: vec![],
            }],
            ..Default::default()
        };

        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec!["SG".to_string()]);

        let orig_node_count = graph.node_count();
        let segments = add_border_segments(&mut graph, &mut ranks, &ast, &membership);

        // Should have added border nodes
        assert!(graph.node_count() > orig_node_count);

        // Remove them
        remove_border_segments(&mut graph, &mut ranks, &segments);

        // Should be back to original count
        assert_eq!(graph.node_count(), orig_node_count);

        // Only real nodes should remain
        let ids: Vec<String> = graph
            .node_indices()
            .map(|ni| graph[ni].id.clone())
            .collect();
        for id in &ids {
            assert!(
                !id.starts_with("__border_"),
                "border node {} should have been removed",
                id
            );
        }
    }

    #[test]
    fn test_empty_subgraph_skipped() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(make_node_data("A"));

        let mut ranks = HashMap::new();
        ranks.insert(a, 0);

        // Subgraph with no members
        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "Empty".to_string(),
                label: None,
                direction: None,
                nodes: vec![],
                edges: vec![],
                subgraphs: vec![],
            }],
            ..Default::default()
        };

        let membership = SubgraphMembership::new();
        let segments = add_border_segments(&mut graph, &mut ranks, &ast, &membership);

        // Empty subgraph should not have border info
        assert!(!segments.subgraphs.contains_key("Empty"));
    }
}
