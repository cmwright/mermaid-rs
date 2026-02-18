use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

use crate::ast::flowchart::Direction;
use crate::layout::graph_builder::SubgraphMembership;
use crate::layout::types::*;

/// Size-aware coordinate placement.
/// - Main axis: accumulate max_thickness_in_rank + RANK_SEP
/// - Cross axis: place with node_width/2 + NODE_SEP + next_width/2 minimum spacing
/// - Barycenter refinement passes
/// - Extra gap at subgraph boundaries
/// - Direction handling (TB/BT/LR/RL)
pub fn assign_coordinates(
    graph: &DiGraph<NodeData, EdgeData>,
    layers: &[Vec<NodeIndex>],
    direction: Direction,
    membership: &SubgraphMembership,
) -> HashMap<NodeIndex, (f64, f64)> {
    let is_horizontal = matches!(direction, Direction::LeftToRight | Direction::RightToLeft);
    let empty_path: Vec<String> = Vec::new();

    let mut positions: HashMap<NodeIndex, (f64, f64)> = HashMap::new();

    // Initial placement: position nodes along main and cross axes
    let mut rank_offset = 0.0;

    for layer in layers {
        let max_thickness = layer
            .iter()
            .map(|&idx| {
                let node = &graph[idx];
                if is_horizontal {
                    node.width
                } else {
                    node.height
                }
            })
            .fold(0.0f64, f64::max);

        let mut cross_offset = 0.0;
        let mut prev_path: Option<&Vec<String>> = None;

        for &idx in layer {
            let node = &graph[idx];
            let node_path = membership.get(&node.id).unwrap_or(&empty_path);

            // Add spacing at subgraph boundaries
            if let Some(prev) = prev_path {
                if prev != node_path {
                    let common = prev
                        .iter()
                        .zip(node_path.iter())
                        .take_while(|(a, b)| a == b)
                        .count();
                    let divergence = prev.len().max(node_path.len()) - common;
                    cross_offset += SUBGRAPH_GROUP_GAP * divergence as f64;
                }
            }

            let cross_size = if is_horizontal {
                node.height
            } else {
                node.width
            };

            let cross_center = cross_offset + cross_size / 2.0;

            let (x, y) = if is_horizontal {
                (rank_offset + max_thickness / 2.0, cross_center)
            } else {
                (cross_center, rank_offset + max_thickness / 2.0)
            };

            positions.insert(idx, (x, y));
            cross_offset = cross_center + cross_size / 2.0 + NODE_SEP;
            prev_path = Some(node_path);
        }

        rank_offset += max_thickness + RANK_SEP;
    }

    // Barycenter refinement: shift nodes toward average neighbor position
    for _pass in 0..5 {
        // Down sweep
        for layer in layers.iter().skip(1) {
            refine_layer(graph, layer, &mut positions, is_horizontal, membership, &empty_path);
        }
        // Up sweep
        for layer in layers.iter().rev().skip(1) {
            refine_layer(graph, layer, &mut positions, is_horizontal, membership, &empty_path);
        }
    }

    // For BT or RL directions, mirror the positions
    if matches!(direction, Direction::BottomToTop | Direction::RightToLeft) {
        let max_coord = if is_horizontal {
            positions
                .values()
                .map(|&(x, _)| x)
                .fold(0.0f64, f64::max)
                + graph
                    .node_indices()
                    .filter_map(|ni| positions.get(&ni).map(|_| graph[ni].width))
                    .fold(0.0f64, f64::max)
        } else {
            positions
                .values()
                .map(|&(_, y)| y)
                .fold(0.0f64, f64::max)
                + graph
                    .node_indices()
                    .filter_map(|ni| positions.get(&ni).map(|_| graph[ni].height))
                    .fold(0.0f64, f64::max)
        };

        for (_, pos) in positions.iter_mut() {
            if is_horizontal {
                pos.0 = max_coord - pos.0;
            } else {
                pos.1 = max_coord - pos.1;
            }
        }
    }

    positions
}

/// Refine a single layer by shifting nodes toward their neighbors' average
/// position, while respecting minimum spacing constraints.
fn refine_layer(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &[NodeIndex],
    positions: &mut HashMap<NodeIndex, (f64, f64)>,
    is_horizontal: bool,
    membership: &SubgraphMembership,
    empty_path: &Vec<String>,
) {
    // Compute desired positions based on neighbor barycenters
    let mut desired: Vec<(NodeIndex, f64)> = Vec::new();

    for &idx in layer {
        let neighbors_in: Vec<NodeIndex> = graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .collect();
        let neighbors_out: Vec<NodeIndex> = graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .collect();

        let all_neighbors: Vec<&NodeIndex> = neighbors_in
            .iter()
            .chain(neighbors_out.iter())
            .collect();

        if all_neighbors.is_empty() {
            continue;
        }

        let avg_cross: f64 = all_neighbors
            .iter()
            .filter_map(|&&n| positions.get(&n))
            .map(|&(x, y)| if is_horizontal { y } else { x })
            .sum::<f64>()
            / all_neighbors.len() as f64;

        desired.push((idx, avg_cross));
    }

    // Apply desired positions
    for (idx, target) in &desired {
        if let Some(pos) = positions.get_mut(idx) {
            if is_horizontal {
                pos.1 = *target;
            } else {
                pos.0 = *target;
            }
        }
    }

    // Enforce minimum spacing (no overlaps)
    enforce_spacing(graph, layer, positions, is_horizontal, membership, empty_path);
}

/// Ensure nodes in a layer don't overlap, respecting minimum spacing.
fn enforce_spacing(
    graph: &DiGraph<NodeData, EdgeData>,
    layer: &[NodeIndex],
    positions: &mut HashMap<NodeIndex, (f64, f64)>,
    is_horizontal: bool,
    membership: &SubgraphMembership,
    empty_path: &Vec<String>,
) {
    if layer.len() < 2 {
        return;
    }

    // Sort layer by current cross-axis position
    let mut sorted: Vec<NodeIndex> = layer.to_vec();
    sorted.sort_by(|a, b| {
        let ca = positions
            .get(a)
            .map(|&(x, y)| if is_horizontal { y } else { x })
            .unwrap_or(0.0);
        let cb = positions
            .get(b)
            .map(|&(x, y)| if is_horizontal { y } else { x })
            .unwrap_or(0.0);
        ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
    });

    for i in 1..sorted.len() {
        let prev_idx = sorted[i - 1];
        let curr_idx = sorted[i];

        let prev_node = &graph[prev_idx];
        let curr_node = &graph[curr_idx];

        let prev_cross = positions
            .get(&prev_idx)
            .map(|&(x, y)| if is_horizontal { y } else { x })
            .unwrap_or(0.0);
        let prev_size = if is_horizontal {
            prev_node.height
        } else {
            prev_node.width
        };
        let curr_size = if is_horizontal {
            curr_node.height
        } else {
            curr_node.width
        };

        let prev_path = membership.get(&prev_node.id).unwrap_or(empty_path);
        let curr_path = membership.get(&curr_node.id).unwrap_or(empty_path);
        let extra_gap = if prev_path != curr_path {
            let common = prev_path
                .iter()
                .zip(curr_path.iter())
                .take_while(|(a, b)| a == b)
                .count();
            let divergence = prev_path.len().max(curr_path.len()) - common;
            SUBGRAPH_GROUP_GAP * divergence as f64
        } else {
            0.0
        };

        let min_center_dist = prev_size / 2.0 + curr_size / 2.0 + NODE_SEP + extra_gap;
        let curr_cross = positions
            .get(&curr_idx)
            .map(|&(x, y)| if is_horizontal { y } else { x })
            .unwrap_or(0.0);

        if curr_cross - prev_cross < min_center_dist {
            let new_cross = prev_cross + min_center_dist;
            if let Some(pos) = positions.get_mut(&curr_idx) {
                if is_horizontal {
                    pos.1 = new_cross;
                } else {
                    pos.0 = new_cross;
                }
            }
        }
    }
}
