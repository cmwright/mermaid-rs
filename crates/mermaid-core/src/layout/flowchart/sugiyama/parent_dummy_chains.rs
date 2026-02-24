//! Parent dummy chains — assigns dummy nodes to the correct subgraph.
//!
//! Implements dagre's `parent-dummy-chains.js`: when long edges are split into
//! chains of dummy nodes (during normalization), those dummy nodes initially
//! have no subgraph membership. This module assigns each dummy node in a chain
//! to the correct subgraph by walking the path from source's subgraph to
//! target's subgraph through the LCA, matching dagre's stateful walk.

use std::collections::HashMap;

use crate::ast::flowchart::FlowchartAst;
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::sugiyama::border_segments::BorderSegments;
use crate::layout::flowchart::sugiyama::dummy_nodes::DummyChain;

/// An entry in the LCA path from source's subgraph to target's subgraph.
///
/// Matches dagre's `path` array from `findPath()`, which contains subgraph IDs
/// from the source's parent chain up through the LCA to the target's parent chain.
enum PathEntry {
    /// A subgraph on the ascending (source-side) or descending (target-side) path.
    Subgraph {
        /// Full membership path for dummies assigned to this subgraph.
        membership: Vec<String>,
        min_rank: usize,
        max_rank: usize,
    },
    /// The LCA position (root or deepest common ancestor subgraph).
    Lca {
        /// Full membership path for dummies assigned at the LCA level.
        membership: Vec<String>,
    },
}

/// Assign dummy chain nodes to the correct subgraph membership.
///
/// Matches dagre's `parentDummyChains(g)` exactly:
/// 1. For each dummy chain, build a path from source's subgraph ancestors
///    through the LCA to target's subgraph ancestors (matching `findPath`).
/// 2. Walk the dummy chain with a stateful path index (matching dagre's
///    ascending/descending walk with `pathIdx`).
///
/// In the ascending phase, the path index advances past source-side subgraphs
/// whose `maxRank < dummy_rank`. When reaching the LCA, switches to descending.
/// In the descending phase, advances INTO target-side subgraphs whose
/// `minRank <= dummy_rank`.
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

        // Find LCA length (longest common prefix).
        let lca_len = src_path
            .iter()
            .zip(tgt_path.iter())
            .take_while(|(a, b)| a == b)
            .count();

        // Build the path matching dagre's findPath():
        //   vPath (source ancestors from deepest to LCA, inclusive of LCA)
        //   + wPath reversed (target ancestors from LCA+1 to deepest)
        //
        // Our path = [ascending subgraphs] + [LCA] + [descending subgraphs]
        let mut path: Vec<PathEntry> = Vec::new();

        // Ascending: src_path[lca_len..] reversed (deepest first, moving toward LCA).
        for i in (lca_len..src_path.len()).rev() {
            let sg_id = &src_path[i];
            let (min_rank, max_rank) = border_segments
                .subgraphs
                .get(sg_id.as_str())
                .map(|b| (b.min_rank, b.max_rank))
                .unwrap_or((0, usize::MAX));
            path.push(PathEntry::Subgraph {
                membership: src_path[..=i].to_vec(),
                min_rank,
                max_rank,
            });
        }

        // LCA position.
        let lca_membership = src_path[..lca_len].to_vec();
        path.push(PathEntry::Lca {
            membership: lca_membership,
        });
        let lca_idx = path.len() - 1;

        // Descending: tgt_path[lca_len..] (shallowest first, moving toward target).
        for i in lca_len..tgt_path.len() {
            let sg_id = &tgt_path[i];
            let (min_rank, max_rank) = border_segments
                .subgraphs
                .get(sg_id.as_str())
                .map(|b| (b.min_rank, b.max_rank))
                .unwrap_or((0, usize::MAX));
            path.push(PathEntry::Subgraph {
                membership: tgt_path[..=i].to_vec(),
                min_rank,
                max_rank,
            });
        }

        // Walk the dummy chain matching dagre's stateful algorithm:
        //
        //   let pathIdx = 0, pathV = path[0], ascending = true;
        //   while (v !== edgeObj.w) {
        //     if (ascending) {
        //       while (pathV !== lca && pathV.maxRank < node.rank) pathIdx++;
        //       if (pathV === lca) ascending = false;
        //     }
        //     if (!ascending) {
        //       while (pathIdx < path.length-1 && path[pathIdx+1].minRank <= node.rank)
        //         pathIdx++;
        //       pathV = path[pathIdx];
        //     }
        //     g.setParent(v, pathV);
        //     v = g.successors(v)[0];
        //   }
        let mut path_idx = 0usize;
        let mut ascending = true;

        for &dummy in &chain.dummy_nodes {
            let dummy_id = node_ids.get(&dummy).cloned().unwrap_or_default();
            let dummy_rank = ranks.get(&dummy).copied().unwrap_or(0);

            // Ascending phase: advance past subgraphs whose maxRank < dummy_rank.
            if ascending {
                while path_idx != lca_idx {
                    match &path[path_idx] {
                        PathEntry::Subgraph { max_rank, .. } => {
                            if *max_rank < dummy_rank {
                                path_idx += 1;
                            } else {
                                break;
                            }
                        }
                        PathEntry::Lca { .. } => break,
                    }
                }
                if path_idx == lca_idx {
                    ascending = false;
                }
            }

            // Descending phase: advance INTO subgraphs whose minRank <= dummy_rank.
            if !ascending {
                while path_idx + 1 < path.len() {
                    match &path[path_idx + 1] {
                        PathEntry::Subgraph { min_rank, .. } => {
                            if *min_rank <= dummy_rank {
                                path_idx += 1;
                            } else {
                                break;
                            }
                        }
                        PathEntry::Lca { .. } => break,
                    }
                }
            }

            // Assign dummy to the membership of the current path entry.
            let entry_membership = match &path[path_idx] {
                PathEntry::Subgraph { membership: m, .. } => m.clone(),
                PathEntry::Lca { membership: m } => m.clone(),
            };
            membership.insert(dummy_id, entry_membership);
        }
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

    #[test]
    fn test_parent_dummy_chains_cross_sibling_with_overlap() {
        // Matches the Gate→PAPI scenario: source in CI, target in Prod/ProdBE.
        // CI and Prod have overlapping rank ranges. Dagre's stateful walk
        // should keep dummies within CI's range assigned to CI, not Prod.
        let src = NodeIndex::new(0);
        let tgt = NodeIndex::new(1);
        let d1 = NodeIndex::new(2); // rank 11 — inside CI [0,12]
        let d2 = NodeIndex::new(3); // rank 12 — inside CI [0,12]
        let d3 = NodeIndex::new(4); // rank 13 — past CI, inside Prod [6,42]
        let d4 = NodeIndex::new(5); // rank 16 — inside ProdBE [16,30]

        let chains = vec![DummyChain {
            original_source: src,
            original_target: tgt,
            edge_data: make_edge_data(),
            dummy_nodes: vec![d1, d2, d3, d4],
            label_node: None,
            is_reversed: false,
        }];

        let mut ranks = HashMap::new();
        ranks.insert(src, 10);
        ranks.insert(tgt, 20);
        ranks.insert(d1, 11);
        ranks.insert(d2, 12);
        ranks.insert(d3, 13);
        ranks.insert(d4, 16);

        let mut membership = SubgraphMembership::new();
        membership.insert("Gate".to_string(), vec!["CI".to_string()]);
        membership.insert(
            "PAPI".to_string(),
            vec!["Prod".to_string(), "ProdBE".to_string()],
        );

        let node_ids: HashMap<NodeIndex, String> = [
            (src, "Gate".to_string()),
            (tgt, "PAPI".to_string()),
            (d1, "d1".to_string()),
            (d2, "d2".to_string()),
            (d3, "d3".to_string()),
            (d4, "d4".to_string()),
        ]
        .into_iter()
        .collect();

        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "CI".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![],
                    edges: vec![],
                    subgraphs: vec![],
                },
                SubgraphDef {
                    id: "Prod".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![],
                    edges: vec![],
                    subgraphs: vec![SubgraphDef {
                        id: "ProdBE".to_string(),
                        label: None,
                        direction: None,
                        nodes: vec![],
                        edges: vec![],
                        subgraphs: vec![],
                    }],
                },
            ],
            ..Default::default()
        };

        let mut segments = BorderSegments {
            subgraphs: HashMap::new(),
        };
        segments.subgraphs.insert(
            "CI".to_string(),
            SubgraphBorders {
                border_left: HashMap::new(),
                border_right: HashMap::new(),
                min_rank: 0,
                max_rank: 12,
            },
        );
        segments.subgraphs.insert(
            "Prod".to_string(),
            SubgraphBorders {
                border_left: HashMap::new(),
                border_right: HashMap::new(),
                min_rank: 6,
                max_rank: 42,
            },
        );
        segments.subgraphs.insert(
            "ProdBE".to_string(),
            SubgraphBorders {
                border_left: HashMap::new(),
                border_right: HashMap::new(),
                min_rank: 16,
                max_rank: 30,
            },
        );

        parent_dummy_chains(&chains, &ranks, &mut membership, &ast, &segments, &node_ids);

        // rank 11: still inside CI (maxRank=12 >= 11) → ["CI"]
        assert_eq!(
            membership.get("d1").unwrap(),
            &vec!["CI".to_string()],
            "rank 11 should stay in CI"
        );
        // rank 12: still inside CI (maxRank=12 >= 12) → ["CI"]
        assert_eq!(
            membership.get("d2").unwrap(),
            &vec!["CI".to_string()],
            "rank 12 should stay in CI"
        );
        // rank 13: past CI (maxRank=12 < 13), descend through LCA to Prod → ["Prod"]
        assert_eq!(
            membership.get("d3").unwrap(),
            &vec!["Prod".to_string()],
            "rank 13 should be in Prod"
        );
        // rank 16: descend further into ProdBE (minRank=16 <= 16) → ["Prod", "ProdBE"]
        assert_eq!(
            membership.get("d4").unwrap(),
            &vec!["Prod".to_string(), "ProdBE".to_string()],
            "rank 16 should be in ProdBE"
        );
    }

    #[test]
    fn test_parent_dummy_chains_child_to_parent() {
        // Edge from a node in a child subgraph to a node in the parent.
        // src in ["A", "B"], tgt in ["A"]
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
        membership.insert(
            "S".to_string(),
            vec!["A".to_string(), "B".to_string()],
        );
        membership.insert("T".to_string(), vec!["A".to_string()]);

        let node_ids: HashMap<NodeIndex, String> = [
            (src, "S".to_string()),
            (tgt, "T".to_string()),
            (dummy, "d".to_string()),
        ]
        .into_iter()
        .collect();

        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "A".to_string(),
                label: None,
                direction: None,
                nodes: vec![],
                edges: vec![],
                subgraphs: vec![SubgraphDef {
                    id: "B".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![],
                    edges: vec![],
                    subgraphs: vec![],
                }],
            }],
            ..Default::default()
        };

        let mut segments = BorderSegments {
            subgraphs: HashMap::new(),
        };
        segments.subgraphs.insert(
            "B".to_string(),
            SubgraphBorders {
                border_left: HashMap::new(),
                border_right: HashMap::new(),
                min_rank: 0,
                max_rank: 1,
            },
        );
        segments.subgraphs.insert(
            "A".to_string(),
            SubgraphBorders {
                border_left: HashMap::new(),
                border_right: HashMap::new(),
                min_rank: 0,
                max_rank: 4,
            },
        );

        parent_dummy_chains(&chains, &ranks, &mut membership, &ast, &segments, &node_ids);

        // Dummy at rank 2 is past B (maxRank=1) but inside A.
        // Ascending path: [B], LCA = A (lca_len=1, lca_membership=["A"]).
        // B.maxRank=1 < 2 → advance past B → hit LCA → descending.
        // No descending subgraphs. pathV = LCA → membership ["A"].
        let dummy_path = membership.get("d").unwrap();
        assert_eq!(
            dummy_path,
            &vec!["A".to_string()],
            "dummy should be in parent subgraph A"
        );
    }
}
