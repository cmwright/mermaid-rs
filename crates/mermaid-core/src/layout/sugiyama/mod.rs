pub mod coordinate_assignment;
pub mod cycle_removal;
pub mod dummy_nodes;
pub mod ordering;
pub mod rank_assignment;

use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use std::collections::HashMap;

use crate::ast::flowchart::{Direction, FlowchartAst};
use crate::layout::graph_builder::SubgraphMembership;
use crate::layout::types::*;

use self::dummy_nodes::DummyChain;

/// Result of the Sugiyama layout pipeline.
pub struct SugiyamaResult {
    /// The layer ordering (rank → list of nodes in order).
    pub layers: Vec<Vec<NodeIndex>>,
    /// Coordinate positions for each node.
    pub positions: HashMap<NodeIndex, (f64, f64)>,
    /// Dummy node chains for long edge reconstruction.
    pub dummy_chains: Vec<DummyChain>,
}

/// Run the full Sugiyama pipeline:
/// 1. Cycle removal (DFS-based back-edge reversal)
/// 2. Rank assignment (longest-path)
/// 3. Dummy nodes for multi-rank edges
/// 4. Ordering (barycenter crossing minimization with subgraph contiguity)
/// 5. Coordinate assignment (size-aware placement)
pub fn layout(
    graph: &mut DiGraph<NodeData, EdgeData>,
    direction: Direction,
    membership: &SubgraphMembership,
    ast: &FlowchartAst,
) -> SugiyamaResult {
    // Phase 1: Remove cycles
    let reversed = cycle_removal::remove_cycles(graph);

    // Phase 2: Assign ranks
    let mut ranks = rank_assignment::assign_ranks(graph);

    // Phase 2b: Align sibling subgraph ranks
    rank_assignment::align_sibling_subgraph_ranks(graph, &mut ranks, ast);

    // Phase 3: Insert dummy nodes for long edges
    let dummy_chains = dummy_nodes::insert_dummy_nodes(graph, &mut ranks);

    // Phase 4: Convert ranks to layers and minimize crossings (with dummy nodes)
    let mut layers = rank_assignment::ranks_to_layers(&ranks);
    ordering::minimize_crossings(graph, &mut layers, membership, 24);

    // Phase 5: Position real nodes only — dummy nodes helped with crossing
    // minimization but shouldn't influence node placement.
    //
    // Add direct edges for long-edge endpoints so they still attract each other
    // during barycenter refinement, then strip dummies from layers.
    let direct_edges: Vec<EdgeIndex> = dummy_chains
        .iter()
        .map(|chain| {
            graph.add_edge(
                chain.original_source,
                chain.original_target,
                chain.edge_data.clone(),
            )
        })
        .collect();

    let real_layers: Vec<Vec<NodeIndex>> = layers
        .iter()
        .map(|layer| {
            layer
                .iter()
                .filter(|&&idx| !graph[idx].id.starts_with("__dummy_"))
                .copied()
                .collect()
        })
        .filter(|layer: &Vec<NodeIndex>| !layer.is_empty())
        .collect();

    let positions =
        coordinate_assignment::assign_coordinates(graph, &real_layers, direction, membership);

    // Clean up temporary edges
    for ei in direct_edges.into_iter().rev() {
        graph.remove_edge(ei);
    }

    // Restore reversed edges (doesn't affect positions)
    cycle_removal::restore_cycles(graph, &reversed);

    SugiyamaResult {
        layers,
        positions,
        dummy_chains,
    }
}
