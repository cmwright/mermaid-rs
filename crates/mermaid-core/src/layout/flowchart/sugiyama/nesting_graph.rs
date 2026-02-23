//! Nesting graph construction for compound (subgraph) layout.
//!
//! Implements dagre's `nesting-graph.js` algorithm: encodes the parent-child
//! subgraph hierarchy as weighted edges so that network simplex rank assignment
//! naturally keeps children within their parent's rank range.
//!
//! ## Algorithm overview
//!
//! 1. Add a synthetic root node connected to all top-level subgraph border-top
//!    nodes and all unparented nodes.
//! 2. For each subgraph, create `borderTop` and `borderBottom` dummy nodes.
//! 3. Connect borderTop → child (and child → borderBottom) with high-weight
//!    nesting edges so rank assignment keeps children between the borders.
//! 4. Scale existing edge minlens by `nodeSep` to leave room for border ranks.
//! 5. After rank assignment, clean up: remove the root node, all nesting edges,
//!    and restore original minlens.

use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use std::collections::HashMap;

use crate::ast::flowchart::{FlowchartAst, NodeShape, SubgraphDef};
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;

/// Data produced by `run()` that `cleanup()` needs to undo.
pub struct NestingState {
    /// The synthetic root node.
    pub root: NodeIndex,
    /// All nesting edges added (to be removed during cleanup).
    pub nesting_edges: Vec<EdgeIndex>,
    /// Original (pre-scaled) minlen for each real edge, keyed by edge index.
    /// These are restored during cleanup.
    pub original_minlens: HashMap<EdgeIndex, usize>,
    /// Border-top and border-bottom nodes for each subgraph.
    /// Key: subgraph id. Value: (borderTop, borderBottom).
    pub subgraph_borders: HashMap<String, (NodeIndex, NodeIndex)>,
    /// The node-sep factor that was used to scale minlens.
    pub node_rank_factor: usize,
}

/// Compute the depth of each subgraph in the hierarchy.
/// Top-level subgraphs have depth 1, their children depth 2, etc.
/// Returns (depths map, max_depth).
fn compute_tree_depths(ast: &FlowchartAst) -> (HashMap<String, usize>, usize) {
    let mut depths = HashMap::new();
    let mut max_depth = 0usize;

    fn recurse(
        subgraphs: &[SubgraphDef],
        depth: usize,
        depths: &mut HashMap<String, usize>,
        max_depth: &mut usize,
    ) {
        for sg in subgraphs {
            depths.insert(sg.id.clone(), depth);
            if depth > *max_depth {
                *max_depth = depth;
            }
            recurse(&sg.subgraphs, depth + 1, depths, max_depth);
        }
    }

    recurse(&ast.subgraphs, 1, &mut depths, &mut max_depth);
    (depths, max_depth)
}

/// Encode the subgraph hierarchy as edges in the graph for rank assignment.
///
/// This is the Rust equivalent of dagre's `nestingGraph.run(g)`.
///
/// Returns a `NestingState` that must be passed to `cleanup()` after rank
/// assignment to remove the synthetic nodes/edges.
pub fn run(
    graph: &mut DiGraph<NodeData, EdgeData>,
    ast: &FlowchartAst,
    membership: &SubgraphMembership,
) -> NestingState {
    if ast.subgraphs.is_empty() {
        // No compound structure — add a root node connected to all real nodes
        // with weight 0 so the graph is connected for rank assignment.
        let root = graph.add_node(NodeData {
            id: "__nesting_root".to_string(),
            label: String::new(),
            shape: NodeShape::Rectangle,
            style: Default::default(),
            width: 0.0,
            height: 0.0,
        });
        let mut nesting_edges = Vec::new();
        for node in graph.node_indices().collect::<Vec<_>>() {
            if node == root {
                continue;
            }
            let ei = graph.add_edge(
                root,
                node,
                EdgeData {
                    label: None,
                    line_style: crate::ast::flowchart::LineStyle::Solid,
                    arrow_start: crate::ast::flowchart::ArrowEnd::None,
                    arrow_end: crate::ast::flowchart::ArrowEnd::None,
                    label_width: 0.0,
                    label_height: 0.0,
                    weight: 0,
                    minlen: 1,
                },
            );
            nesting_edges.push(ei);
        }
        return NestingState {
            root,
            nesting_edges,
            original_minlens: HashMap::new(),
            subgraph_borders: HashMap::new(),
            node_rank_factor: 1,
        };
    }

    let (depths, max_depth) = compute_tree_depths(ast);

    // height = max depth (following dagre: max of all tree depths)
    let height = max_depth;

    // nodeSep = 2 * height + 1 — ensures real nodes land on ranks that are
    // multiples of nodeSep, leaving room for border nodes in between.
    let node_sep = 2 * height + 1;

    // Step 1: Scale existing edge minlens
    let mut original_minlens = HashMap::new();
    for ei in graph.edge_indices().collect::<Vec<_>>() {
        let orig = graph[ei].minlen;
        original_minlens.insert(ei, orig);
        graph[ei].minlen = orig * node_sep;
    }

    // Step 2: Compute nesting weight = sum of all edge weights + 1
    // This ensures nesting edges dominate any single real edge.
    let total_weight: i64 = graph.edge_indices().map(|ei| graph[ei].weight).sum();
    let nesting_weight = total_weight + 1;

    // Step 3: Add synthetic root node
    let root = graph.add_node(NodeData {
        id: "__nesting_root".to_string(),
        label: String::new(),
        shape: NodeShape::Rectangle,
        style: Default::default(),
        width: 0.0,
        height: 0.0,
    });

    let mut nesting_edges = Vec::new();
    let mut subgraph_borders: HashMap<String, (NodeIndex, NodeIndex)> = HashMap::new();

    // Step 4: DFS over subgraph hierarchy, adding border nodes and nesting edges
    fn dfs(
        subgraphs: &[SubgraphDef],
        parent_path: &[String],
        root: NodeIndex,
        graph: &mut DiGraph<NodeData, EdgeData>,
        membership: &SubgraphMembership,
        depths: &HashMap<String, usize>,
        height: usize,
        nesting_weight: i64,
        node_sep: usize,
        nesting_edges: &mut Vec<EdgeIndex>,
        subgraph_borders: &mut HashMap<String, (NodeIndex, NodeIndex)>,
    ) {
        for sg in subgraphs {
            let mut path = parent_path.to_vec();
            path.push(sg.id.clone());

            let sg_depth = depths.get(&sg.id).copied().unwrap_or(1);

            // Create borderTop and borderBottom dummy nodes for this subgraph
            let border_top = graph.add_node(NodeData {
                id: format!("__nesting_bt_{}", sg.id),
                label: String::new(),
                shape: NodeShape::Rectangle,
                style: Default::default(),
                width: 0.0,
                height: 0.0,
            });
            let border_bottom = graph.add_node(NodeData {
                id: format!("__nesting_bb_{}", sg.id),
                label: String::new(),
                shape: NodeShape::Rectangle,
                style: Default::default(),
                width: 0.0,
                height: 0.0,
            });
            subgraph_borders.insert(sg.id.clone(), (border_top, border_bottom));

            // Recurse into child subgraphs first
            dfs(
                &sg.subgraphs,
                &path,
                root,
                graph,
                membership,
                depths,
                height,
                nesting_weight,
                node_sep,
                nesting_edges,
                subgraph_borders,
            );

            // Process each direct child of this subgraph
            // Direct children = nodes whose membership path is exactly `path`,
            // plus child subgraphs.
            let direct_child_nodes: Vec<NodeIndex> = graph
                .node_indices()
                .filter(|&ni| {
                    let id = &graph[ni].id;
                    // Skip nesting infrastructure nodes
                    if id.starts_with("__nesting_") || id.starts_with("__dummy_") {
                        return false;
                    }
                    // Node's membership path must be exactly this subgraph's path
                    if let Some(node_path) = membership.get(id) {
                        *node_path == path
                    } else {
                        false
                    }
                })
                .collect();

            for &child_node in &direct_child_nodes {
                // Leaf child (not a subgraph):
                // dagre: `thisWeight = childNode.borderTop ? weight : 2 * weight`
                // For leaf nodes, borderTop is undefined, so thisWeight = 2 * weight.
                // dagre: `minlen = childTop !== childBottom ? 1 : height - depths[v] + 1`
                // For leaf nodes, childTop === childBottom === child, so minlen = height - depth + 1.
                let child_w = 2 * nesting_weight;
                let child_minlen = height - sg_depth + 1;

                let e1 = graph.add_edge(
                    border_top,
                    child_node,
                    make_nesting_edge(child_w, child_minlen),
                );
                let e2 = graph.add_edge(
                    child_node,
                    border_bottom,
                    make_nesting_edge(child_w, child_minlen),
                );
                nesting_edges.push(e1);
                nesting_edges.push(e2);

                // dagre: for leaf nodes, also adds root → v with weight=0, minlen=nodeSep.
                // This ensures all leaf nodes have the same minimum distance from root,
                // allowing network simplex to freely optimize their positions.
                let e_root = graph.add_edge(root, child_node, make_nesting_edge(0, node_sep));
                nesting_edges.push(e_root);
            }

            for child_sg in &sg.subgraphs {
                if let Some(&(child_bt, child_bb)) = subgraph_borders.get(&child_sg.id) {
                    // Child subgraph: connect to its border nodes
                    let child_w = nesting_weight;
                    let child_minlen = 1;

                    let e1 = graph.add_edge(
                        border_top,
                        child_bt,
                        make_nesting_edge(child_w, child_minlen),
                    );
                    let e2 = graph.add_edge(
                        child_bb,
                        border_bottom,
                        make_nesting_edge(child_w, child_minlen),
                    );
                    nesting_edges.push(e1);
                    nesting_edges.push(e2);
                }
            }

            // If this is a top-level subgraph, connect root → borderTop
            if parent_path.is_empty() {
                let e = graph.add_edge(root, border_top, make_nesting_edge(0, height + sg_depth));
                nesting_edges.push(e);
            }
        }
    }

    dfs(
        &ast.subgraphs,
        &[],
        root,
        graph,
        membership,
        &depths,
        height,
        nesting_weight,
        node_sep,
        &mut nesting_edges,
        &mut subgraph_borders,
    );

    // Step 5: Connect root to any top-level leaf nodes (not in any subgraph)
    for ni in graph.node_indices().collect::<Vec<_>>() {
        if ni == root {
            continue;
        }
        let id = &graph[ni].id;
        if id.starts_with("__nesting_") {
            continue;
        }
        // Check if this node is a top-level node (empty membership path or no membership)
        let is_top_level = membership
            .get(id)
            .map(|path| path.is_empty())
            .unwrap_or(true);
        if is_top_level {
            let e = graph.add_edge(root, ni, make_nesting_edge(0, node_sep));
            nesting_edges.push(e);
        }
    }

    NestingState {
        root,
        nesting_edges,
        original_minlens,
        subgraph_borders,
        node_rank_factor: node_sep,
    }
}

/// Remove nesting graph infrastructure from the graph.
///
/// Removes:
/// - All nesting edges
/// - The synthetic root node
/// - All nesting border nodes (bt/bb)
///
/// NOTE: In dagre, bt/bb stay in the graph because dagre's graphlib supports
/// compound graphs where these are proper children of subgraph nodes. In our
/// petgraph-based system, keeping them causes ordering/coordinate issues
/// since they appear as regular leaf nodes. So we remove them, but the caller
/// should read their ranks BEFORE calling cleanup (via assignRankMinMax).
pub fn cleanup(
    graph: &mut DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    state: &NestingState,
) {
    // Collect all nesting infrastructure nodes to remove.
    let mut to_remove: Vec<NodeIndex> = Vec::new();
    to_remove.push(state.root);
    for (_, &(bt, bb)) in &state.subgraph_borders {
        to_remove.push(bt);
        to_remove.push(bb);
    }

    // Deduplicate and sort descending by index (petgraph uses swap-remove,
    // so removing from highest index first avoids index invalidation).
    to_remove.sort_by(|a, b| b.index().cmp(&a.index()));
    to_remove.dedup();

    for ni in to_remove {
        if graph.node_weight(ni).is_none() {
            continue;
        }

        // Handle petgraph's swap-remove: when removing a node that isn't the
        // last, the last node takes the removed node's index.
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

/// Create a nesting edge with the given weight and minlen.
fn make_nesting_edge(weight: i64, minlen: usize) -> EdgeData {
    EdgeData {
        label: None,
        line_style: crate::ast::flowchart::LineStyle::Solid,
        arrow_start: crate::ast::flowchart::ArrowEnd::None,
        arrow_end: crate::ast::flowchart::ArrowEnd::None,
        label_width: 0.0,
        label_height: 0.0,
        weight,
        minlen,
    }
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
    fn test_no_subgraphs() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(make_node_data("A"));
        let b = graph.add_node(make_node_data("B"));
        graph.add_edge(a, b, make_edge_data());

        let ast = FlowchartAst {
            subgraphs: vec![],
            ..Default::default()
        };
        let membership = SubgraphMembership::new();

        let state = run(&mut graph, &ast, &membership);

        // Should have added a root node
        assert_eq!(graph.node_count(), 3); // A, B, root
                                           // Root should have edges to A and B
        assert_eq!(state.nesting_edges.len(), 2);
        // No border nodes
        assert!(state.subgraph_borders.is_empty());

        // Cleanup
        let mut ranks = HashMap::new();
        ranks.insert(a, 0);
        ranks.insert(b, 1);
        ranks.insert(state.root, 0);
        cleanup(&mut graph, &mut ranks, &state);

        // Root should be removed, back to A and B
        assert_eq!(graph.node_count(), 2);
    }

    #[test]
    fn test_single_subgraph() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(make_node_data("A"));
        let b = graph.add_node(make_node_data("B"));
        graph.add_edge(a, b, make_edge_data());

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

        let state = run(&mut graph, &ast, &membership);

        // Should have root + border_top + border_bottom = 3 new nodes
        assert!(state.subgraph_borders.contains_key("SG"));
        let (bt, bb) = state.subgraph_borders["SG"];
        assert_ne!(bt, bb);

        // Edge minlen should have been scaled
        // Original minlen was 1, node_sep = 2*1+1 = 3, so scaled = 3
        assert_eq!(state.node_rank_factor, 3);
    }

    #[test]
    fn test_nested_subgraphs() {
        let mut graph = DiGraph::new();
        let _a = graph.add_node(make_node_data("A"));
        let _b = graph.add_node(make_node_data("B"));

        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "Outer".to_string(),
                label: None,
                direction: None,
                nodes: vec![],
                edges: vec![],
                subgraphs: vec![SubgraphDef {
                    id: "Inner".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![NodeDef {
                        id: "A".into(),
                        label: Some("A".into()),
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    }],
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
        membership.insert("B".to_string(), vec![]);

        let state = run(&mut graph, &ast, &membership);

        // Both Outer and Inner should have border nodes
        assert!(state.subgraph_borders.contains_key("Outer"));
        assert!(state.subgraph_borders.contains_key("Inner"));

        // max_depth = 2, height = 2, node_sep = 2*2+1 = 5
        assert_eq!(state.node_rank_factor, 5);

        // B is top-level, should be connected to root
        let root = state.root;
        let b_connected = graph
            .neighbors_directed(root, petgraph::Direction::Outgoing)
            .any(|tgt| graph[tgt].id == "B");
        assert!(b_connected, "top-level node B should be connected to root");
    }

    #[test]
    fn test_run_and_cleanup_roundtrip() {
        let mut graph = DiGraph::new();
        let a = graph.add_node(make_node_data("A"));
        let b = graph.add_node(make_node_data("B"));
        let c = graph.add_node(make_node_data("C"));
        let _e1 = graph.add_edge(a, b, make_edge_data());
        let _e2 = graph.add_edge(b, c, make_edge_data());

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
        membership.insert("C".to_string(), vec![]);

        let orig_node_count = graph.node_count();
        let state = run(&mut graph, &ast, &membership);

        // After run: graph should have extra nodes
        assert!(graph.node_count() > orig_node_count);

        // Simulate rank assignment results
        let mut ranks: HashMap<NodeIndex, usize> = HashMap::new();
        // In reality, network simplex would assign these, but for the test
        // we just need valid ranks for the cleanup to work.
        for ni in graph.node_indices() {
            ranks.insert(ni, ni.index());
        }

        cleanup(&mut graph, &mut ranks, &state);

        // After cleanup: should be back to original 3 nodes
        assert_eq!(
            graph.node_count(),
            orig_node_count,
            "cleanup should restore original node count"
        );

        // All remaining nodes should be real nodes (A, B, C)
        let ids: Vec<String> = graph
            .node_indices()
            .map(|ni| graph[ni].id.clone())
            .collect();
        for id in &["A", "B", "C"] {
            assert!(ids.contains(&id.to_string()), "missing node {}", id);
        }
    }
}
