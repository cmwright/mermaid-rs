//! Barycenter computation for ordering.
//! Port of dagre's `order/barycenter.js`.

use crate::graph::LayoutGraph;
#[derive(Debug, Clone)]
pub struct BarycenterEntry {
    pub v: String,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

/// Computes barycenters for movable nodes based on predecessor positions.
pub fn barycenter(g: &LayoutGraph, movable: &[String]) -> Vec<BarycenterEntry> {
    movable
        .iter()
        .map(|v| {
            let Some(in_edge_ids) = g.in_edge_ids(v) else {
                return BarycenterEntry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                };
            };
            if in_edge_ids.is_empty() {
                return BarycenterEntry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                };
            }

            let mut sum = 0.0;
            let mut weight = 0.0;
            for edge_id in in_edge_ids {
                let Some(e) = g.edge_obj_by_id(edge_id) else {
                    continue;
                };
                let edge_weight = g.edge_label_by_id(edge_id).map(|l| l.weight).unwrap_or(0.0);
                let node_order = g.node(&e.v).and_then(|n| n.order).unwrap_or(0) as f64;
                sum += edge_weight * node_order;
                weight += edge_weight;
            }

            BarycenterEntry {
                v: v.clone(),
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        })
        .collect()
}
