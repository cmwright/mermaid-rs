//! Coordinate system adjustment for different rank directions.
//! Port of dagre's `coordinate-system.js`.

use crate::graph::LayoutGraph;

/// Adjusts the graph for the given rankdir. For LR/RL, swaps width/height.
pub fn adjust(g: &mut LayoutGraph) {
    if g.graph().rankdir.is_lr_or_rl() {
        swap_width_height(g);
    }
}

/// Undoes coordinate system adjustments after layout.
pub fn undo(g: &mut LayoutGraph) {
    if g.graph().rankdir.is_bt_or_rl() {
        reverse_y(g);
    }

    if g.graph().rankdir.is_lr_or_rl() {
        swap_xy(g);
        swap_width_height(g);
    }
}

fn swap_width_height(g: &mut LayoutGraph) {
    let node_ids: Vec<String> = g.node_ids().to_vec();
    for v in &node_ids {
        if let Some(node) = g.node_mut(v) {
            std::mem::swap(&mut node.width, &mut node.height);
        }
    }
    let edge_ids: Vec<String> = g.edge_ids().to_vec();
    for eid in &edge_ids {
        if let Some(label) = g.edge_label_mut_by_id(eid) {
            std::mem::swap(&mut label.width, &mut label.height);
        }
    }
}

fn reverse_y(g: &mut LayoutGraph) {
    let node_ids: Vec<String> = g.node_ids().to_vec();
    for v in &node_ids {
        if let Some(node) = g.node_mut(v)
            && let Some(y) = node.y
        {
            node.y = Some(-y);
        }
    }
    let edge_ids: Vec<String> = g.edge_ids().to_vec();
    for eid in &edge_ids {
        if let Some(label) = g.edge_label_mut_by_id(eid) {
            for p in &mut label.points {
                p.y = -p.y;
            }
            if let Some(y) = label.y {
                label.y = Some(-y);
            }
        }
    }
}

fn swap_xy(g: &mut LayoutGraph) {
    let node_ids: Vec<String> = g.node_ids().to_vec();
    for v in &node_ids {
        if let Some(node) = g.node_mut(v) {
            std::mem::swap(&mut node.x, &mut node.y);
        }
    }
    let edge_ids: Vec<String> = g.edge_ids().to_vec();
    for eid in &edge_ids {
        if let Some(label) = g.edge_label_mut_by_id(eid) {
            for p in &mut label.points {
                std::mem::swap(&mut p.x, &mut p.y);
            }
            std::mem::swap(&mut label.x, &mut label.y);
        }
    }
}
