//! Nesting graph support for compound graphs.
//! Port of dagre's `nesting-graph.js`.

use crate::graph::LayoutGraph;
use crate::types::*;
use crate::util::{add_border_node, add_dummy_node};
use std::collections::HashMap;

/// Creates the nesting graph structure for compound graphs.
pub fn run(g: &mut LayoutGraph) {
    let root = add_dummy_node(g, DummyType::Root, NodeLabel::default(), "_root");
    let depths = tree_depths(g);
    let height = depths.values().copied().max().unwrap_or(1) as i64 - 1;
    let node_sep = 2 * height + 1;

    // Set nestingRoot on graph label
    g.graph_mut().nesting_root = Some(root.clone());

    // Multiply minlen by nodeSep
    let edges = g.edges();
    for e in &edges {
        if let Some(label) = g.edge_mut_by_obj(e) {
            label.minlen *= node_sep as f64;
        }
    }

    // Calculate weight sufficient to keep subgraphs compact
    let weight = sum_weights(g) + 1.0;

    // Create border nodes and link them
    let root_children = g.children(None).unwrap_or_default();
    for child in root_children {
        dfs(g, &root, node_sep, weight, height, &depths, &child);
    }

    // Save nodeRankFactor
    g.graph_mut().node_rank_factor = Some(node_sep);
}

fn dfs(
    g: &mut LayoutGraph,
    root: &str,
    node_sep: i64,
    weight: f64,
    height: i64,
    depths: &HashMap<String, usize>,
    v: &str,
) {
    let children = g.children(Some(v)).unwrap_or_default();
    if children.is_empty() {
        if v != root {
            g.set_edge(
                root,
                v,
                Some(EdgeLabel {
                    weight: 0.0,
                    minlen: node_sep as f64,
                    ..Default::default()
                }),
                None,
            );
        }
        return;
    }

    let top = add_border_node(g, "_bt", None, None);
    let bottom = add_border_node(g, "_bb", None, None);

    // Set borderTop/borderBottom on parent label
    if let Some(label) = g.node_mut(v) {
        label.border_top = Some(top.clone());
        label.border_bottom = Some(bottom.clone());
    }

    g.set_parent(&top, Some(v));
    g.set_parent(&bottom, Some(v));

    for child in &children {
        dfs(g, root, node_sep, weight, height, depths, child);

        let child_node = g.node(child).cloned().unwrap_or_default();
        let child_top = child_node
            .border_top
            .as_deref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| child.clone());
        let child_bottom = child_node
            .border_bottom
            .as_deref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| child.clone());
        let this_weight = if child_node.border_top.is_some() {
            weight
        } else {
            2.0 * weight
        };
        let minlen = if child_top != child_bottom {
            1
        } else {
            let depth_v = *depths.get(v).unwrap_or(&0) as i64;
            height - depth_v + 1
        };

        g.set_edge(
            &top,
            &child_top,
            Some(EdgeLabel {
                weight: this_weight,
                minlen: minlen as f64,
                nesting_edge: true,
                ..Default::default()
            }),
            None,
        );
        g.set_edge(
            &child_bottom,
            &bottom,
            Some(EdgeLabel {
                weight: this_weight,
                minlen: minlen as f64,
                nesting_edge: true,
                ..Default::default()
            }),
            None,
        );
    }

    if g.parent(v).is_none() {
        let depth_v = *depths.get(v).unwrap_or(&0) as i64;
        g.set_edge(
            root,
            &top,
            Some(EdgeLabel {
                weight: 0.0,
                minlen: (height + depth_v) as f64,
                ..Default::default()
            }),
            None,
        );
    }
}

fn tree_depths(g: &LayoutGraph) -> HashMap<String, usize> {
    let mut depths = HashMap::new();

    fn dfs_depth(g: &LayoutGraph, v: &str, depth: usize, depths: &mut HashMap<String, usize>) {
        let children = g.children(Some(v)).unwrap_or_default();
        if !children.is_empty() {
            for child in &children {
                dfs_depth(g, child, depth + 1, depths);
            }
        }
        depths.insert(v.to_string(), depth);
    }

    let root_children = g.children(None).unwrap_or_default();
    for v in &root_children {
        dfs_depth(g, v, 1, &mut depths);
    }
    depths
}

fn sum_weights(g: &LayoutGraph) -> f64 {
    g.edges()
        .iter()
        .filter_map(|e| g.edge_by_obj(e).map(|l| l.weight))
        .sum()
}

/// Removes the nesting graph structure.
pub fn cleanup(g: &mut LayoutGraph) {
    let nesting_root = g.graph().nesting_root.clone();

    if let Some(root) = nesting_root {
        g.remove_node(&root);
    }

    g.graph_mut().nesting_root = None;

    // Remove nesting edges
    let nesting_edges: Vec<_> = g
        .edges()
        .into_iter()
        .filter(|e| g.edge_by_obj(e).map(|l| l.nesting_edge).unwrap_or(false))
        .collect();

    for e in nesting_edges {
        g.remove_edge_by_obj(&e);
    }
}
