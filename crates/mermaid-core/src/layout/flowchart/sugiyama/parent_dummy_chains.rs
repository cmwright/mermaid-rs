//! Parent dummy chains — assigns dummy nodes to the correct subgraph.
//!
//! Implements dagre's `parent-dummy-chains.js`: when long edges are split into
//! chains of dummy nodes (during normalization), those dummy nodes initially
//! have no subgraph membership. This module assigns each dummy node in a chain
//! to the most deeply nested subgraph that contains that rank, following the
//! path from source's ancestor chain to target's ancestor chain via their LCA.

use std::collections::HashMap;

use crate::ast::flowchart::{FlowchartAst, SubgraphDef};
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::sugiyama::border_segments::BorderSegments;
use crate::layout::flowchart::sugiyama::dummy_nodes::DummyChain;

/// Assign dummy chain nodes to the correct subgraph membership.
///
/// For each dummy chain, finds the path through the subgraph hierarchy from
/// the source endpoint's subgraph to the target endpoint's subgraph via their
/// LCA. Then walks the chain and assigns each dummy node to the deepest
/// subgraph on that path whose rank range contains the dummy's rank.
///
/// Updates `membership` in place with entries for each dummy node.
pub fn parent_dummy_chains(
    dummy_chains: &[DummyChain],
    ranks: &HashMap<petgraph::graph::NodeIndex, usize>,
    membership: &mut SubgraphMembership,
    ast: &FlowchartAst,
    border_segments: &BorderSegments,
    node_ids: &HashMap<petgraph::graph::NodeIndex, String>,
) {
    if dummy_chains.is_empty() || ast.subgraphs.is_empty() {
        return;
    }

    // Build subgraph hierarchy: parent_id -> children ids, and depth info.
    let mut sg_parent: HashMap<String, Option<String>> = HashMap::new();
    build_sg_parent_map(&ast.subgraphs, None, &mut sg_parent);

    for chain in dummy_chains {
        let src_id = node_ids
            .get(&chain.original_source)
            .cloned()
            .unwrap_or_default();
        let tgt_id = node_ids
            .get(&chain.original_target)
            .cloned()
            .unwrap_or_default();

        let src_path = membership.get(&src_id).cloned().unwrap_or_default();
        let tgt_path = membership.get(&tgt_id).cloned().unwrap_or_default();

        // Find LCA path: walk up from source to LCA, then down to target.
        let (ascending, descending) = find_lca_path(&src_path, &tgt_path);

        // Build the full path of subgraph ids from source side to target side.
        // ascending: subgraphs from source's deepest to LCA (exclusive)
        // descending: subgraphs from LCA (exclusive) to target's deepest
        let path: Vec<&str> = ascending
            .iter()
            .chain(descending.iter())
            .map(|s| s.as_str())
            .collect();

        if path.is_empty() {
            // Both endpoints are at the same level (or root level).
            // Assign dummies to the LCP (same as current behavior).
            let lcp: Vec<String> = src_path
                .iter()
                .zip(tgt_path.iter())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a.clone())
                .collect();
            for &dummy in &chain.dummy_nodes {
                let dummy_id = node_ids.get(&dummy).cloned().unwrap_or_default();
                membership.insert(dummy_id, lcp.clone());
            }
            continue;
        }

        // Walk the dummy chain and assign each dummy to the deepest subgraph
        // on the path whose rank range contains the dummy's rank.
        let ascending_len = ascending.len();

        for &dummy in &chain.dummy_nodes {
            let dummy_id = node_ids.get(&dummy).cloned().unwrap_or_default();
            let dummy_rank = ranks.get(&dummy).copied().unwrap_or(0);

            // Find the deepest subgraph on the path that contains this rank.
            let mut best_path: Vec<String> = src_path
                .iter()
                .zip(tgt_path.iter())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a.clone())
                .collect();

            // Check ascending (source-side) subgraphs.
            for (i, sg_id) in ascending.iter().enumerate() {
                if let Some(borders) = border_segments.subgraphs.get(sg_id.as_str()) {
                    if dummy_rank >= borders.min_rank && dummy_rank <= borders.max_rank {
                        // Build the membership path for this subgraph.
                        // It's the LCA prefix + the ascending path up to this point (reversed).
                        let lcp_len = src_path.len() - ascending_len;
                        let path_for_sg: Vec<String> =
                            src_path[..lcp_len + ascending_len - i].to_vec();
                        // Verify the last element matches
                        if path_for_sg.last().map(|s| s.as_str()) == Some(sg_id.as_str()) {
                            best_path = path_for_sg;
                        }
                    }
                }
            }

            // Check descending (target-side) subgraphs.
            for (i, sg_id) in descending.iter().enumerate() {
                if let Some(borders) = border_segments.subgraphs.get(sg_id.as_str()) {
                    if dummy_rank >= borders.min_rank && dummy_rank <= borders.max_rank {
                        let lcp: Vec<String> = src_path
                            .iter()
                            .zip(tgt_path.iter())
                            .take_while(|(a, b)| a == b)
                            .map(|(a, _)| a.clone())
                            .collect();
                        let tgt_offset = tgt_path.len() - descending.len();
                        let mut path_for_sg: Vec<String> = lcp.clone();
                        path_for_sg.extend_from_slice(&tgt_path[lcp.len()..tgt_offset + i + 1]);
                        if path_for_sg.last().map(|s| s.as_str()) == Some(sg_id.as_str()) {
                            best_path = path_for_sg;
                        }
                    }
                }
            }

            membership.insert(dummy_id, best_path);
        }
    }
}

/// Find the ascending and descending parts of the LCA path.
///
/// Given two membership paths (e.g., `["A", "B", "C"]` and `["A", "D"]`),
/// finds the LCA (longest common prefix, e.g., `["A"]`), then returns:
/// - ascending: elements of `src_path` after the LCA, reversed (deepest first)
/// - descending: elements of `tgt_path` after the LCA (shallowest first)
fn find_lca_path(src_path: &[String], tgt_path: &[String]) -> (Vec<String>, Vec<String>) {
    // Find LCA length (longest common prefix).
    let lca_len = src_path
        .iter()
        .zip(tgt_path.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Ascending: from source's deepest to LCA (exclusive).
    // In dagre, this is the path from v's parent up to LCA.
    let ascending: Vec<String> = src_path[lca_len..].iter().rev().cloned().collect();

    // Descending: from LCA (exclusive) to target's deepest.
    let descending: Vec<String> = tgt_path[lca_len..].to_vec();

    (ascending, descending)
}

/// Build a map from subgraph ID to its parent subgraph ID.
fn build_sg_parent_map(
    subgraphs: &[SubgraphDef],
    parent: Option<String>,
    out: &mut HashMap<String, Option<String>>,
) {
    for sg in subgraphs {
        out.insert(sg.id.clone(), parent.clone());
        build_sg_parent_map(&sg.subgraphs, Some(sg.id.clone()), out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{ArrowEnd, LineStyle, NodeDef, NodeShape, SubgraphDef};
    use crate::layout::flowchart::sugiyama::border_segments::SubgraphBorders;
    use crate::layout::flowchart::sugiyama::dummy_nodes::DummyChain;
    use crate::layout::flowchart::types::EdgeData;
    use petgraph::graph::NodeIndex;

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
    fn test_find_lca_path_same_parent() {
        let src = vec!["A".to_string(), "B".to_string()];
        let tgt = vec!["A".to_string(), "C".to_string()];
        let (asc, desc) = find_lca_path(&src, &tgt);
        assert_eq!(asc, vec!["B"]);
        assert_eq!(desc, vec!["C"]);
    }

    #[test]
    fn test_find_lca_path_same_subgraph() {
        let src = vec!["A".to_string()];
        let tgt = vec!["A".to_string()];
        let (asc, desc) = find_lca_path(&src, &tgt);
        assert!(asc.is_empty());
        assert!(desc.is_empty());
    }

    #[test]
    fn test_find_lca_path_root_level() {
        let src = vec!["A".to_string()];
        let tgt = vec!["B".to_string()];
        let (asc, desc) = find_lca_path(&src, &tgt);
        assert_eq!(asc, vec!["A"]);
        assert_eq!(desc, vec!["B"]);
    }

    #[test]
    fn test_parent_dummy_chains_basic() {
        // Source in SG_A (rank 0), Target in SG_B (rank 4)
        // Dummy at rank 2 should be assigned to root (LCA = root)
        let src = NodeIndex::new(0);
        let tgt = NodeIndex::new(1);
        let dummy = NodeIndex::new(2);

        let chains = vec![DummyChain {
            original_source: src,
            original_target: tgt,
            edge_data: make_edge_data(),
            dummy_nodes: vec![dummy],
            label_node: None,
            is_reversed: false,
        }];

        let mut ranks = HashMap::new();
        ranks.insert(src, 0);
        ranks.insert(tgt, 4);
        ranks.insert(dummy, 2);

        let mut membership = SubgraphMembership::new();
        membership.insert("S".to_string(), vec!["SG_A".to_string()]);
        membership.insert("T".to_string(), vec!["SG_B".to_string()]);

        let node_ids: HashMap<NodeIndex, String> = [
            (src, "S".to_string()),
            (tgt, "T".to_string()),
            (dummy, "__dummy_0_2".to_string()),
        ]
        .into_iter()
        .collect();

        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "SG_A".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![NodeDef {
                        id: "S".into(),
                        label: Some("S".into()),
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    }],
                    edges: vec![],
                    subgraphs: vec![],
                },
                SubgraphDef {
                    id: "SG_B".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![NodeDef {
                        id: "T".into(),
                        label: Some("T".into()),
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    }],
                    edges: vec![],
                    subgraphs: vec![],
                },
            ],
            ..Default::default()
        };

        let mut segments = BorderSegments {
            subgraphs: HashMap::new(),
        };
        segments.subgraphs.insert(
            "SG_A".to_string(),
            SubgraphBorders {
                border_left: HashMap::new(),
                border_right: HashMap::new(),
                min_rank: 0,
                max_rank: 1,
            },
        );
        segments.subgraphs.insert(
            "SG_B".to_string(),
            SubgraphBorders {
                border_left: HashMap::new(),
                border_right: HashMap::new(),
                min_rank: 3,
                max_rank: 4,
            },
        );

        parent_dummy_chains(&chains, &ranks, &mut membership, &ast, &segments, &node_ids);

        // Dummy at rank 2 is between SG_A (0..1) and SG_B (3..4),
        // so it should be at root level (empty path or LCA path).
        let dummy_path = membership.get("__dummy_0_2").unwrap();
        assert!(
            dummy_path.is_empty(),
            "dummy between sibling subgraphs should be at root level, got {:?}",
            dummy_path
        );
    }

    #[test]
    fn test_parent_dummy_chains_within_subgraph() {
        // Both endpoints in same subgraph — dummies should also be in that subgraph.
        let src = NodeIndex::new(0);
        let tgt = NodeIndex::new(1);
        let dummy = NodeIndex::new(2);

        let chains = vec![DummyChain {
            original_source: src,
            original_target: tgt,
            edge_data: make_edge_data(),
            dummy_nodes: vec![dummy],
            label_node: None,
            is_reversed: false,
        }];

        let mut ranks = HashMap::new();
        ranks.insert(src, 0);
        ranks.insert(tgt, 2);
        ranks.insert(dummy, 1);

        let mut membership = SubgraphMembership::new();
        membership.insert("S".to_string(), vec!["SG".to_string()]);
        membership.insert("T".to_string(), vec!["SG".to_string()]);

        let node_ids: HashMap<NodeIndex, String> = [
            (src, "S".to_string()),
            (tgt, "T".to_string()),
            (dummy, "__dummy_0_1".to_string()),
        ]
        .into_iter()
        .collect();

        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "SG".to_string(),
                label: None,
                direction: None,
                nodes: vec![
                    NodeDef {
                        id: "S".into(),
                        label: Some("S".into()),
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    },
                    NodeDef {
                        id: "T".into(),
                        label: Some("T".into()),
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    },
                ],
                edges: vec![],
                subgraphs: vec![],
            }],
            ..Default::default()
        };

        let segments = BorderSegments {
            subgraphs: HashMap::new(),
        };

        parent_dummy_chains(&chains, &ranks, &mut membership, &ast, &segments, &node_ids);

        // Same subgraph — dummy should be in SG
        let dummy_path = membership.get("__dummy_0_1").unwrap();
        assert_eq!(dummy_path, &vec!["SG".to_string()]);
    }
}
