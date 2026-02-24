pub mod border_segments;
pub mod coordinate_assignment;
pub mod cycle_removal;
pub mod dummy_nodes;
pub mod nesting_graph;
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

/// Run the full Sugiyama pipeline matching dagre's exact sequence:
///
/// 1. makeSpaceForEdgeLabels (double edge minlens)
/// 2. removeSelfEdges (TODO: not yet needed)
/// 3. acyclic.run (cycle removal)
/// 4. nestingGraph.run (encode subgraph hierarchy as edges)
/// 5. rank(asNonCompoundGraph(g)) (network simplex with simplify)
/// 6. injectEdgeLabelProxies (pin label ranks for removeEmptyRanks)
/// 7. removeEmptyRanks (compact empty ranks respecting nodeRankFactor)
/// 8. nestingGraph.cleanup (remove root + nesting edges)
/// 9. normalizeRanks (shift to 0-based)
/// 10. assignRankMinMax (record min/max rank on compound nodes)
/// 11. removeEdgeLabelProxies (transfer label rank to edges)
/// 12. normalize.run (insert dummy chains for long edges)
/// 13. parentDummyChains (assign dummies to correct subgraphs)
/// 14. addBorderSegments (create left/right border nodes)
/// 15. order (crossing minimization)
/// 16. position (coordinate assignment)
pub fn layout(
    graph: &mut DiGraph<NodeData, EdgeData>,
    direction: Direction,
    membership: &SubgraphMembership,
    ast: &FlowchartAst,
) -> SugiyamaResult {
    // ── Step 1: makeSpaceForEdgeLabels ──────────────────────────────────
    // Double all edge minlens so there's room for label ranks between nodes.
    // dagre also halves ranksep here; we handle that in coordinate assignment.
    for ei in graph.edge_indices().collect::<Vec<_>>() {
        graph[ei].minlen *= 2;
    }

    // ── Step 3: acyclic.run ─────────────────────────────────────────────
    let reversed = cycle_removal::remove_cycles(graph);

    // Record which edges were reversed (by their current src->tgt after reversal).
    let reversed_endpoints: HashSet<(NodeIndex, NodeIndex)> = reversed
        .iter()
        .filter_map(|&ei| graph.edge_endpoints(ei))
        .collect();

    // ── Step 4: nestingGraph.run ────────────────────────────────────────
    // Encodes subgraph hierarchy as weighted edges so network simplex
    // naturally keeps children within their parent's rank range.
    // Also multiplies existing edge minlens by nodeSep.
    let nesting_state = nesting_graph::run(graph, ast, membership);

    // ── Step 5: rank(asNonCompoundGraph(g)) ─────────────────────────────
    // Assign ranks using network simplex. We strip compound (nesting) nodes
    // first, matching dagre's asNonCompoundGraph, and merge parallel edges
    // (simplify) inside network simplex.
    let mut ranks = rank_assignment::assign_ranks_non_compound(graph, &nesting_state);

    // ── Step 6: injectEdgeLabelProxies ──────────────────────────────────
    // For edges with labels (width > 0 && height > 0), inject a temporary
    // dummy node at the midpoint rank. This ensures removeEmptyRanks doesn't
    // collapse the rank that the label needs.
    let edge_label_proxies = inject_edge_label_proxies(graph, &mut ranks);

    // ── Step 7: removeEmptyRanks ────────────────────────────────────────
    // Compact empty ranks while preserving structural gaps at nodeRankFactor
    // boundaries (for border node placement).
    if nesting_state.node_rank_factor > 1 {
        remove_empty_ranks(&mut ranks, nesting_state.node_rank_factor);
    }

    // ── Step 10: assignRankMinMax (BEFORE cleanup, since we remove bt/bb) ─
    // Read bt/bb ranks to compute subgraph min/max rank ranges.
    // These are used later by parentDummyChains.
    let mut subgraph_rank_ranges =
        compute_subgraph_rank_ranges_from_nesting(&nesting_state, &ranks);

    // ── Step 8: nestingGraph.cleanup ────────────────────────────────────
    // Remove synthetic root node, nesting edges, and bt/bb nodes.
    nesting_graph::cleanup(graph, &mut ranks, &nesting_state);

    // ── Step 9: normalizeRanks ──────────────────────────────────────────
    // Shift all ranks so the minimum rank is 0. Apply same shift to rank ranges.
    if let Some(&min_rank) = ranks.values().min() {
        if min_rank > 0 {
            for rank in ranks.values_mut() {
                *rank -= min_rank;
            }
            // Normalize subgraph rank ranges by the same amount
            for borders in subgraph_rank_ranges.subgraphs.values_mut() {
                borders.min_rank = borders.min_rank.saturating_sub(min_rank);
                borders.max_rank = borders.max_rank.saturating_sub(min_rank);
            }
        }
    }

    // ── Post-rank alignment ─────────────────────────────────────────────
    // Our network simplex can find different optima than dagre's, causing
    // sibling subgraphs to land at different rank levels. These alignment
    // passes compensate until our NS implementation exactly matches dagre's.
    rank_assignment::align_sibling_subgraph_ranks(graph, &mut ranks, ast, membership);
    rank_assignment::align_within_subgraph_peers(graph, &mut ranks, membership, ast);

    // Compact gaps left by nesting cleanup and alignment (bt/bb removal
    // can leave empty ranks). Map occupied ranks to consecutive integers
    // so that rank doubling below doesn't amplify the gaps.
    {
        let mut unique_ranks: Vec<usize> = ranks.values().copied().collect();
        // Include subgraph range endpoints so they map through compaction too.
        for borders in subgraph_rank_ranges.subgraphs.values() {
            unique_ranks.push(borders.min_rank);
            unique_ranks.push(borders.max_rank);
        }
        unique_ranks.sort_unstable();
        unique_ranks.dedup();
        let rank_map: HashMap<usize, usize> = unique_ranks
            .into_iter()
            .enumerate()
            .map(|(new, old)| (old, new))
            .collect();
        for rank in ranks.values_mut() {
            *rank = rank_map[rank];
        }
        for borders in subgraph_rank_ranges.subgraphs.values_mut() {
            borders.min_rank = rank_map[&borders.min_rank];
            borders.max_rank = rank_map[&borders.max_rank];
        }
    }

    // Double all ranks to create interstitial ranks for edge labels and
    // ensure enough vertical space for subgraph padding between adjacent
    // subgraphs. With rank_sep/2 per rank, doubling gives rank_sep between
    // adjacent real nodes (2 * rank_sep/2 = rank_sep), which provides enough
    // space for subgraph title + padding.
    for rank in ranks.values_mut() {
        *rank *= 2;
    }
    // Apply the same doubling to subgraph rank ranges.
    // Extend max_rank by +1 to cover the interstitial rank below the doubled
    // max. Doubling creates odd "gap" ranks between even content ranks; without
    // this extension, the gap rank between this subgraph's last rank and the
    // next subgraph falls outside any border range, leaving dummy chains
    // uncontained at root level during ordering. In dagre this isn't needed
    // because ranks aren't doubled — nodeSep-based scaling already provides
    // sufficient resolution.
    for borders in subgraph_rank_ranges.subgraphs.values_mut() {
        borders.min_rank *= 2;
        borders.max_rank = borders.max_rank * 2 + 1;
    }

    // ── Step 11: removeEdgeLabelProxies ─────────────────────────────────
    // Remove the temporary edge-proxy nodes, transferring their computed
    // rank back to the edge as labelRank.
    remove_edge_label_proxies(graph, &mut ranks, &edge_label_proxies);

    // ── Step 12: normalize.run (insert dummy chains) ────────────────────
    let mut dummy_chains = dummy_nodes::insert_dummy_nodes(graph, &mut ranks);

    // Mark dummy chains from reversed back-edges
    for chain in &mut dummy_chains {
        if reversed_endpoints.contains(&(chain.original_source, chain.original_target)) {
            chain.is_reversed = true;
        }
    }

    // ── Step 13: parentDummyChains ──────────────────────────────────────
    // Assign dummy nodes to the correct subgraph membership. This must run
    // BEFORE addBorderSegments because border segment rank ranges need to
    // account for dummy nodes that belong to each subgraph.
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
        &subgraph_rank_ranges,
        &node_ids,
    );
    let membership = &membership_mut;

    // ── Step 14: addBorderSegments ──────────────────────────────────────
    // Create left/right border dummy nodes at every rank in each subgraph's
    // range. Use the nesting-derived rank ranges (subgraph_rank_ranges) to
    // match dagre's addBorderSegments, which reads minRank/maxRank from
    // compound node properties set by assignRankMinMax, NOT from membership.
    let border_segments = border_segments::add_border_segments_with_ranges(
        graph,
        &mut ranks,
        ast,
        membership,
        Some(&subgraph_rank_ranges),
    );

    // ── Step 15: order (crossing minimization) ──────────────────────────
    let mut layers = rank_assignment::ranks_to_layers(graph, &ranks);

    // Recursive subgraph-aware ordering matching dagre's order() module.
    recursive_ordering::minimize_crossings_recursive(
        graph,
        &mut layers,
        membership,
        &border_segments,
        ast,
    );

    // Remove empty layers.
    layers.retain(|layer| !layer.is_empty());

    // ── Step 16: position (coordinate assignment) ───────────────────────
    let positions = coordinate_assignment::assign_coordinates(
        graph,
        &layers,
        direction,
        membership,
        RANK_SEP / 2.0,
    );

    // Border segment nodes remain in the graph but are filtered out by
    // build_positioned_nodes (which skips __dummy_ and __border_ prefixed
    // nodes). We do NOT remove them to avoid invalidating NodeIndex values
    // stored in DummyChains and the positions map.

    // Restore reversed edges (doesn't affect positions)
    cycle_removal::restore_cycles(graph, &reversed);

    SugiyamaResult {
        layers,
        positions,
        dummy_chains,
        border_segments,
    }
}

/// Inject edge label proxy nodes at label midpoint ranks.
/// Returns a list of (proxy_node, edge_endpoints) for later removal.
fn inject_edge_label_proxies(
    graph: &mut DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
) -> Vec<(NodeIndex, (NodeIndex, NodeIndex))> {
    let mut proxies = Vec::new();

    // Collect edges with labels
    let labeled_edges: Vec<_> = graph
        .edge_indices()
        .filter_map(|ei| {
            let edge = &graph[ei];
            if edge.label_width > 0.0 && edge.label_height > 0.0 {
                let (v, w) = graph.edge_endpoints(ei)?;
                let v_rank = *ranks.get(&v)?;
                let w_rank = *ranks.get(&w)?;
                Some((v, w, v_rank, w_rank))
            } else {
                None
            }
        })
        .collect();

    for (v, w, v_rank, w_rank) in labeled_edges {
        let mid_rank = (v_rank + w_rank) / 2;
        let proxy = graph.add_node(NodeData {
            id: format!("__edge_proxy_{}_{}_{}", graph[v].id, graph[w].id, mid_rank),
            label: String::new(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            width: 0.0,
            height: 0.0,
        });
        ranks.insert(proxy, mid_rank);
        proxies.push((proxy, (v, w)));
    }

    proxies
}

/// Remove edge label proxy nodes, transferring their rank to the edge.
fn remove_edge_label_proxies(
    graph: &mut DiGraph<NodeData, EdgeData>,
    ranks: &mut HashMap<NodeIndex, usize>,
    proxies: &[(NodeIndex, (NodeIndex, NodeIndex))],
) {
    // Remove proxies in reverse order to maintain valid indices
    // (petgraph uses swap-remove, so removing from highest index first is safest)
    let mut to_remove: Vec<(NodeIndex, usize)> = proxies
        .iter()
        .filter_map(|&(proxy, _)| {
            let label_rank = ranks.get(&proxy).copied()?;
            Some((proxy, label_rank))
        })
        .collect();
    to_remove.sort_by(|a, b| b.0.index().cmp(&a.0.index()));

    for (proxy, _label_rank) in to_remove {
        if graph.node_weight(proxy).is_none() {
            continue;
        }
        // Handle petgraph's swap-remove: when we remove a node, the last node
        // takes the removed node's index.
        let last_idx = NodeIndex::new(graph.node_count() - 1);
        if proxy != last_idx {
            if let Some(rank) = ranks.remove(&last_idx) {
                ranks.insert(proxy, rank);
            }
        }
        ranks.remove(&proxy);
        graph.remove_node(proxy);
    }
}

/// Remove empty ranks, matching dagre's removeEmptyRanks.
///
/// Compacts empty ranks while preserving structural gaps at nodeRankFactor
/// boundaries. This is called AFTER rank assignment and BEFORE cleanup.
fn remove_empty_ranks(ranks: &mut HashMap<NodeIndex, usize>, node_rank_factor: usize) {
    if ranks.is_empty() {
        return;
    }

    let min_rank = *ranks.values().min().unwrap();
    let max_rank = *ranks.values().max().unwrap();
    let num_layers = max_rank - min_rank + 1;

    let mut occupied = vec![false; num_layers];
    for &rank in ranks.values() {
        occupied[rank - min_rank] = true;
    }

    // Compute cumulative delta (negative shift to compact empty ranks).
    // Preserve empty ranks at multiples of node_rank_factor.
    let mut delta: Vec<i64> = vec![0; num_layers];
    let mut d: i64 = 0;
    for i in 0..num_layers {
        if !occupied[i] && i % node_rank_factor != 0 {
            d -= 1;
        }
        delta[i] = d;
    }

    for rank in ranks.values_mut() {
        let idx = *rank - min_rank;
        let new_rank = (*rank as i64 + delta[idx]) as usize;
        *rank = new_rank;
    }
}

fn compute_subgraph_rank_ranges_from_nesting(
    nesting_state: &nesting_graph::NestingState,
    ranks: &HashMap<NodeIndex, usize>,
) -> BorderSegments {
    use border_segments::SubgraphBorders;

    let mut segments = BorderSegments {
        subgraphs: HashMap::new(),
    };

    for (sg_id, &(bt, bb)) in &nesting_state.subgraph_borders {
        if let (Some(&bt_rank), Some(&bb_rank)) = (ranks.get(&bt), ranks.get(&bb)) {
            let min_rank = bt_rank.min(bb_rank);
            let max_rank = bt_rank.max(bb_rank);
            segments.subgraphs.insert(
                sg_id.clone(),
                SubgraphBorders {
                    border_left: HashMap::new(),
                    border_right: HashMap::new(),
                    min_rank,
                    max_rank,
                },
            );
        }
    }

    segments
}
