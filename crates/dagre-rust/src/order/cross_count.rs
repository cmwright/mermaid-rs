//! Cross count using accumulator tree.
//! Port of dagre's `order/cross-count.js`.

use crate::graph::LayoutGraph;
use std::collections::HashMap;

/// Counts weighted edge crossings across the entire layering.
pub fn cross_count(g: &LayoutGraph, layering: &[Vec<String>]) -> i64 {
    let mut cc: i64 = 0;
    for i in 1..layering.len() {
        cc += two_layer_cross_count(g, &layering[i - 1], &layering[i]);
    }
    cc
}

fn two_layer_cross_count(g: &LayoutGraph, north_layer: &[String], south_layer: &[String]) -> i64 {
    // Map south layer nodes to positions
    let south_pos: HashMap<&str, usize> = south_layer
        .iter()
        .enumerate()
        .map(|(i, v)| (v.as_str(), i))
        .collect();

    // Collect sorted entries
    let mut south_entries: Vec<(usize, f64)> = Vec::new();
    for v in north_layer {
        let mut entries: Vec<(usize, f64)> = Vec::new();
        if let Some(out_edge_ids) = g.out_edge_ids(v) {
            for edge_id in out_edge_ids {
                let Some(e) = g.edge_obj_by_id(edge_id) else {
                    continue;
                };
                if let Some(&pos) = south_pos.get(e.w.as_str()) {
                    let weight = g.edge_label_by_id(edge_id).map(|l| l.weight).unwrap_or(0.0);
                    entries.push((pos, weight));
                }
            }
        }
        entries.sort_by_key(|e| e.0);
        south_entries.extend(entries);
    }

    // Build accumulator tree
    let mut first_index = 1usize;
    while first_index < south_layer.len() {
        first_index <<= 1;
    }
    let tree_size = 2 * first_index - 1;
    let first_index = first_index - 1;
    let mut tree = vec![0.0f64; tree_size];

    let mut cc = 0.0f64;
    for (pos, weight) in &south_entries {
        let mut index = pos + first_index;
        if index >= tree.len() {
            continue;
        }
        tree[index] += weight;
        let mut weight_sum = 0.0;
        while index > 0 {
            if !index.is_multiple_of(2) && index + 1 < tree.len() {
                weight_sum += tree[index + 1];
            }
            index = (index - 1) >> 1;
            tree[index] += weight;
        }
        cc += weight * weight_sum;
    }

    cc as i64
}
