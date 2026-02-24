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
            let in_edges = g.in_edges(v, None).unwrap_or_default();
            if in_edges.is_empty() {
                return BarycenterEntry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                };
            }

            let mut sum = 0.0;
            let mut weight = 0.0;
            for e in &in_edges {
                let edge_weight = g.edge_by_obj(e).map(|l| l.weight).unwrap_or(0.0);
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
