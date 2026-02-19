pub mod coordinate_assignment;
pub mod cycle_removal;
pub mod dummy_nodes;
pub mod ordering;
pub mod rank_assignment;

use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

use crate::ast::flowchart::{Direction, FlowchartAst};
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;

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

    // Phase 2c: Double all ranks to create interstitial label ranks.
    // This ensures every edge spans ≥2 ranks and gets at least 1 dummy node,
    // so labeled edges always have a midpoint dummy to host the label.
    for rank in ranks.values_mut() {
        *rank *= 2;
    }

    // Phase 3: Insert dummy nodes for long edges
    let dummy_chains = dummy_nodes::insert_dummy_nodes(graph, &mut ranks);

    // Phase 4: Convert ranks to layers and minimize crossings (with dummy nodes).
    // Try two initial orderings (DFS and Kahn's topo-sort) and keep whichever
    // converges to fewer crossings — DFS is better for complex dependency graphs,
    // Kahn's can be better for simpler graphs with subgraphs.
    let layers_dfs = rank_assignment::ranks_to_layers(graph, &ranks);
    let layers_topo = rank_assignment::ranks_to_layers_alt(graph, &ranks);
    let mut layers = ordering::minimize_crossings_best_of(
        graph,
        &[layers_dfs, layers_topo],
        membership,
        48,
    );

    // Phase 4b: Subgraph-local ordering refinement
    ordering::refine_subgraph_ordering(graph, &mut layers, membership, ast, &dummy_chains);

    // Phase 5: Coordinate assignment — dummy nodes participate fully (like dagre).
    // Dummies get real positions via Brandes-Köpf with EDGE_SEP separation,
    // and their coordinates become edge waypoints.
    // Use halved RANK_SEP because ranks were doubled to create interstitial label ranks.
    let positions = coordinate_assignment::assign_coordinates(
        graph,
        &layers,
        direction,
        membership,
        RANK_SEP / 2.0,
    );

    // Restore reversed edges (doesn't affect positions)
    cycle_removal::restore_cycles(graph, &reversed);

    SugiyamaResult {
        layers,
        positions,
        dummy_chains,
    }
}
