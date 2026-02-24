//! Position assignment module.
//! Port of dagre's `position/index.js`.

pub mod bk;

use crate::graph::LayoutGraph;
use crate::types::*;
use crate::util;

/// Assigns x and y coordinates to all nodes.
pub fn position(g: &mut LayoutGraph) {
    let mut ncg = util::as_non_compound_graph(g);
    position_y(&mut ncg);
    let xs = bk::position_x(&ncg);

    // Copy positions back to the original graph
    for (v, x) in &xs {
        if let Some(node) = g.node_mut(v) {
            node.x = Some(*x);
        }
    }
    // Copy y from ncg to g
    for v in ncg.nodes() {
        if let Some(y) = ncg.node(&v).and_then(|n| n.y)
            && let Some(node) = g.node_mut(&v)
        {
            node.y = Some(y);
        }
    }
}

fn position_y(g: &mut LayoutGraph) {
    let layering = util::build_layer_matrix(g);
    let ranksep = g.graph().ranksep;
    let rankalign = g.graph().rankalign;

    let mut prev_y = 0.0;

    for layer in &layering {
        let max_height = layer
            .iter()
            .filter(|v| !v.is_empty())
            .map(|v| g.node(v).map(|n| n.height).unwrap_or(0.0))
            .fold(0.0f64, f64::max);

        for v in layer {
            if v.is_empty() {
                continue;
            }
            if let Some(node) = g.node_mut(v) {
                let height = node.height;
                let y = match rankalign {
                    RankAlign::Top => prev_y + height / 2.0,
                    RankAlign::Bottom => prev_y + max_height - height / 2.0,
                    RankAlign::Center => prev_y + max_height / 2.0,
                };
                node.y = Some(y);
            }
        }

        prev_y += max_height + ranksep;
    }
}
