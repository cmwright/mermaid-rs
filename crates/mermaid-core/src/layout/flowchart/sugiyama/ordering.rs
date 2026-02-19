use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;

/// Barycenter heuristic with alternating up/down sweeps.
/// Enforces subgraph contiguity: nodes belonging to the same subgraph
/// remain contiguous within each rank.
pub fn minimize_crossings(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &mut [Vec<NodeIndex>],
    membership: &SubgraphMembership,
    num_iterations: usize,
) {
    let empty_path: Vec<String> = Vec::new();

    for iteration in 0..num_iterations {
        if iteration % 2 == 0 {
            // Down sweep: process layers top to bottom
            for i in 1..layers.len() {
                let prev_positions = build_position_map(&layers[i - 1]);
                sort_layer_by_barycenter(
                    graph,
                    &mut layers[i],
                    &prev_positions,
                    petgraph::Direction::Incoming,
                    membership,
                    &empty_path,
                );
            }
        } else {
            // Up sweep: process layers bottom to top
            for i in (0..layers.len().saturating_sub(1)).rev() {
                let next_positions = build_position_map(&layers[i + 1]);
                sort_layer_by_barycenter(
                    graph,
                    &mut layers[i],
                    &next_positions,
                    petgraph::Direction::Outgoing,
                    membership,
                    &empty_path,
                );
            }
        }
    }
}

/// Build a map from NodeIndex to its position within the layer.
fn build_position_map(layer: &[NodeIndex]) -> HashMap<NodeIndex, usize> {
    layer
        .iter()
        .enumerate()
        .map(|(pos, &node)| (node, pos))
        .collect()
}

/// Sort a layer using barycenter heuristic while maintaining subgraph contiguity.
fn sort_layer_by_barycenter(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &mut Vec<NodeIndex>,
    adjacent_positions: &HashMap<NodeIndex, usize>,
    direction: petgraph::Direction,
    membership: &SubgraphMembership,
    empty_path: &Vec<String>,
) {
    // Compute barycenter for each node, using enumeration index as fallback
    let barycenters: HashMap<NodeIndex, f64> = layer
        .iter()
        .enumerate()
        .map(|(idx, &node)| {
            let neighbors: Vec<usize> = graph
                .neighbors_directed(node, direction)
                .filter_map(|n| adjacent_positions.get(&n).copied())
                .collect();

            let bc = if neighbors.is_empty() {
                // Keep current relative position as fallback (use index directly)
                idx as f64
            } else {
                neighbors.iter().sum::<usize>() as f64 / neighbors.len() as f64
            };
            (node, bc)
        })
        .collect();

    // Group nodes by subgraph membership path, preserving order.
    // Dummy nodes get unique paths so each is an independent singleton —
    // this prevents all dummies from clumping into one block, letting each
    // be placed freely at its individual barycenter position.
    let mut groups: Vec<(Vec<String>, Vec<NodeIndex>)> = Vec::new();
    for &node in layer.iter() {
        let is_dummy = graph[node].id.starts_with("__dummy_");
        let path = if is_dummy {
            vec![graph[node].id.clone()]
        } else {
            membership
                .get(&graph[node].id)
                .unwrap_or(empty_path)
                .clone()
        };

        if !is_dummy {
            // Linear scan is typically fastest for small group counts (1-5 groups per layer)
            if let Some(group) = groups.iter_mut().find(|(p, _)| *p == path) {
                group.1.push(node);
            } else {
                groups.push((path, vec![node]));
            }
        } else {
            // Each dummy is its own group
            groups.push((path, vec![node]));
        }
    }

    // Sort nodes within each group by barycenter
    for (_, members) in &mut groups {
        members.sort_by(|a, b| {
            let ba = barycenters.get(a).copied().unwrap_or(0.0);
            let bb = barycenters.get(b).copied().unwrap_or(0.0);
            ba.partial_cmp(&bb).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Sort groups by average barycenter of their members
    groups.sort_by(|(_, a_members), (_, b_members)| {
        let avg_a = a_members
            .iter()
            .filter_map(|n| barycenters.get(n))
            .sum::<f64>()
            / a_members.len().max(1) as f64;
        let avg_b = b_members
            .iter()
            .filter_map(|n| barycenters.get(n))
            .sum::<f64>()
            / b_members.len().max(1) as f64;
        avg_a
            .partial_cmp(&avg_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Flatten back into layer
    *layer = groups
        .into_iter()
        .flat_map(|(_, members)| members)
        .collect();
}
