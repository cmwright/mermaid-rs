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
    for v in g.nodes() {
        if let Some(node) = g.node_mut(&v) {
            std::mem::swap(&mut node.width, &mut node.height);
        }
    }
    for e in g.edges() {
        if let Some(label) = g.edge_mut_by_obj(&e) {
            std::mem::swap(&mut label.width, &mut label.height);
        }
    }
}

fn reverse_y(g: &mut LayoutGraph) {
    for v in g.nodes() {
        if let Some(node) = g.node_mut(&v)
            && let Some(y) = node.y
        {
            node.y = Some(-y);
        }
    }
    for e in g.edges() {
        if let Some(label) = g.edge_mut_by_obj(&e) {
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
    for v in g.nodes() {
        if let Some(node) = g.node_mut(&v) {
            std::mem::swap(&mut node.x, &mut node.y);
        }
    }
    for e in g.edges() {
        if let Some(label) = g.edge_mut_by_obj(&e) {
            for p in &mut label.points {
                std::mem::swap(&mut p.x, &mut p.y);
            }
            std::mem::swap(&mut label.x, &mut label.y);
        }
    }
}
