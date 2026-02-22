pub mod border_segments;
pub mod coordinate_assignment;
pub mod cycle_removal;
pub mod dummy_nodes;
pub mod nesting_graph;
pub mod ordering;
pub mod parent_dummy_chains;
pub mod rank_assignment;
pub mod recursive_ordering;

use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, HashSet};

use crate::ast::flowchart::{Direction, FlowchartAst};
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;

use self::border_segments::BorderSegments;
use self::dummy_nodes::DummyChain;

/// Result of the Sugiyama layout pipeline.
pub struct SugiyamaResult {
    /// The layer ordering (rank → list of nodes in order).
    pub layers: Vec<Vec<NodeIndex>>,
    /// Coordinate positions for each node.
    pub positions: HashMap<NodeIndex, (f64, f64)>,
    /// Dummy node chains for long edge reconstruction.
    pub dummy_chains: Vec<DummyChain>,
    /// Border segment information for subgraph bounding boxes.
    pub border_segments: BorderSegments,
}

/// Run the full Sugiyama pipeline:
/// 1. Cycle removal (DFS-based back-edge reversal)
/// 2. Nesting graph construction (encodes subgraph hierarchy for rank assignment)
/// 3. Rank assignment (network simplex with nesting constraints)
/// 4. Nesting graph cleanup (removes synthetic nodes, restores minlens)
/// 5. Dummy nodes for multi-rank edges
/// 6. Ordering (barycenter crossing minimization with subgraph contiguity)
/// 7. Coordinate assignment (size-aware placement)
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

    // Phase 2: Build nesting graph — encodes subgraph hierarchy as weighted
    // edges so that network simplex naturally keeps children within their
    // parent's rank range. This replaces the old align_sibling_subgraph_ranks
    // and align_within_subgraph_peers hacks.
    let nesting_state = nesting_graph::run(graph, ast, membership);

    // Phase 3: Assign ranks (with nesting constraints in the graph)
    let mut ranks = rank_assignment::assign_ranks(graph);

    // Phase 4: Clean up nesting graph — removes synthetic root, border nodes,
    // nesting edges, and restores original edge minlens. Also compacts empty
    // ranks and normalizes to zero-based.
    nesting_graph::cleanup(graph, &mut ranks, &nesting_state);

    // Phase 4b: Temporary compatibility: align sibling subgraph ranks.
    // The nesting graph ensures containment but doesn't enforce vertical
    // separation between sibling subgraphs. Border segments + recursive
    // ordering (not yet implemented) will handle this properly. Until then,
    // keep the alignment hacks as a fallback.
    rank_assignment::align_sibling_subgraph_ranks(graph, &mut ranks, ast, membership);
    rank_assignment::align_within_subgraph_peers(graph, &mut ranks, membership, ast);

    // Phase 4c: Add border segments — left/right border dummy nodes at every
    // rank within each subgraph's range, connected vertically by edges.
    // These are used by the ordering algorithm to enforce subgraph contiguity.
    let border_segments = border_segments::add_border_segments(graph, &mut ranks, ast, membership);

    // Phase 5: Double all ranks to create interstitial label ranks.
    // This ensures every edge spans ≥2 ranks and gets at least 1 dummy node,
    // so labeled edges always have a midpoint dummy to host the label.
    for rank in ranks.values_mut() {
        *rank *= 2;
    }

    // Phase 5: Insert dummy nodes for long edges
    let mut dummy_chains = dummy_nodes::insert_dummy_nodes(graph, &mut ranks);

    // Mark dummy chains from reversed back-edges
    for chain in &mut dummy_chains {
        if reversed_endpoints.contains(&(chain.original_source, chain.original_target)) {
            chain.is_reversed = true;
        }
    }

    // Phase 5b: Parent dummy chains — assign dummy nodes to the correct
    // subgraph in the membership map. This ensures dummies participate in
    // the right subgraph's ordering during crossing minimization.
    {
        let node_ids: HashMap<NodeIndex, String> = graph
            .node_indices()
            .map(|ni| (ni, graph[ni].id.clone()))
            .collect();
        let mut membership_mut = membership.clone();
        parent_dummy_chains::parent_dummy_chains(
            &dummy_chains,
            &ranks,
            &mut membership_mut,
            ast,
            &border_segments,
            &node_ids,
        );
        // Use the updated membership for ordering.
        // Note: we pass `membership` (immutable) to functions below, so we need
        // to use the mutated copy. We'll shadow the binding.
        let membership = &membership_mut;

        // Phase 6: Convert ranks to layers and minimize crossings (with dummy nodes)
        let mut layers = rank_assignment::ranks_to_layers(graph, &ranks);

        // Use the recursive ordering algorithm that properly handles compound
        // graphs. Falls back to the flat barycenter first to get an initial
        // ordering, then refines with the recursive algorithm.
        ordering::minimize_crossings(graph, &mut layers, membership, 24);
        recursive_ordering::minimize_crossings_recursive(
            graph,
            &mut layers,
            membership,
            &border_segments,
            24,
        );

        // Phase 6c: Remove empty layers.
        layers.retain(|layer| !layer.is_empty());

        // Phase 7: Coordinate assignment
        let positions = coordinate_assignment::assign_coordinates(
            graph,
            &layers,
            direction,
            membership,
            RANK_SEP / 2.0,
        );

        // Phase 8: Remove border segment nodes from the graph.
        border_segments::remove_border_segments(graph, &mut ranks, &border_segments);

        // Restore reversed edges (doesn't affect positions)
        cycle_removal::restore_cycles(graph, &reversed);

        SugiyamaResult {
            layers,
            positions,
            dummy_chains,
            border_segments,
        }
    }
}
