pub mod coordinate_assignment;
pub mod cycle_removal;
pub mod dummy_nodes;
pub mod ordering;
pub mod rank_assignment;

use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};

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

    // Record which edges were reversed (by their current src->tgt after reversal).
    // After reversal, the "original" back-edge A->B becomes B->A in the graph.
    // When insert_dummy_nodes processes B->A, it records original_source=B,
    // original_target=A.  We mark those chains so sync_dummy_positions knows
    // their dummies are intentionally far from the direct corridor.
    let reversed_endpoints: HashSet<(NodeIndex, NodeIndex)> = reversed
        .iter()
        .filter_map(|&ei| graph.edge_endpoints(ei))
        .collect();

    // Phase 2: Assign ranks
    let mut ranks = rank_assignment::assign_ranks(graph);

    // Phase 2b: Align sibling subgraph ranks
    rank_assignment::align_sibling_subgraph_ranks(graph, &mut ranks, ast, membership);

    // Phase 2b-ii: Align peer nodes within each subgraph.
    // Cross-subgraph edges can push some nodes to higher ranks even though
    // they are peers (same internal depth) inside their subgraph.
    rank_assignment::align_within_subgraph_peers(graph, &mut ranks, membership, ast);

    // Phase 2c: Double all ranks to create interstitial label ranks.
    // This ensures every edge spans ≥2 ranks and gets at least 1 dummy node,
    // so labeled edges always have a midpoint dummy to host the label.
    for rank in ranks.values_mut() {
        *rank *= 2;
    }

    // Phase 3: Insert dummy nodes for long edges
    let mut dummy_chains = dummy_nodes::insert_dummy_nodes(graph, &mut ranks);

    // Mark dummy chains from reversed back-edges
    for chain in &mut dummy_chains {
        if reversed_endpoints.contains(&(chain.original_source, chain.original_target)) {
            chain.is_reversed = true;
        }
    }

    // Phase 4: Convert ranks to layers and minimize crossings (with dummy nodes)
    let mut layers = rank_assignment::ranks_to_layers(graph, &ranks);
    ordering::minimize_crossings(graph, &mut layers, membership, 24);

    // Phase 4b: Subgraph-local ordering refinement
    ordering::refine_subgraph_ordering(graph, &mut layers, membership, ast, &dummy_chains);

    // Phase 4c: Remove empty layers.
    // Alignment passes can leave gaps where no node (real or dummy) exists.
    // Empty layers waste vertical space since coordinate assignment allocates
    // rank_sep for each layer regardless.
    layers.retain(|layer| !layer.is_empty());

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
