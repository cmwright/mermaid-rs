//! Recursive subgraph sorting.
//! Port of dagre's `order/sort-subgraph.js`.

use crate::graph::{ConstraintGraph, LayoutGraph};
use crate::order::barycenter;
use crate::order::resolve_conflicts;
use crate::order::sort;
use std::collections::HashMap;

/// Recursively sorts a subgraph's children.
pub fn sort_subgraph(
    g: &LayoutGraph,
    v: &str,
    cg: &ConstraintGraph,
    bias_right: bool,
) -> sort::SortResult {
    let mut movable = g.children(Some(v)).unwrap_or_default();
    let node = g.node(v).cloned();

    let bl = node
        .as_ref()
        .and_then(|n| n.border_left.first())
        .and_then(|o| o.as_deref())
        .map(|s| s.to_string());
    let br = node
        .as_ref()
        .and_then(|n| n.border_right.first())
        .and_then(|o| o.as_deref())
        .map(|s| s.to_string());

    if let (Some(bl_str), Some(br_str)) = (&bl, &br) {
        movable.retain(|w| w != bl_str && w != br_str);
    }

    let mut barycenters = barycenter::barycenter(g, &movable);
    let mut subgraphs: HashMap<String, sort::SortResult> = HashMap::new();

    for entry in &mut barycenters {
        let children = g.children(Some(&entry.v)).unwrap_or_default();
        if !children.is_empty() {
            let sub_result = sort_subgraph(g, &entry.v, cg, bias_right);
            subgraphs.insert(entry.v.clone(), sub_result);

            if let Some(sr) = subgraphs.get(&entry.v)
                && sr.barycenter.is_some()
            {
                merge_barycenters(entry, sr);
            }
        }
    }

    let entries = resolve_conflicts::resolve_conflicts(&barycenters, cg);

    // Expand subgraphs
    let expanded: Vec<resolve_conflicts::ResolvedEntry> = entries
        .into_iter()
        .map(|mut entry| {
            entry.vs = entry
                .vs
                .iter()
                .flat_map(|v| {
                    if let Some(sr) = subgraphs.get(v) {
                        sr.vs.clone()
                    } else {
                        vec![v.clone()]
                    }
                })
                .collect();
            entry
        })
        .collect();

    let mut result = sort::sort(&expanded, bias_right);

    if let (Some(bl_str), Some(br_str)) = (&bl, &br) {
        let mut new_vs = vec![bl_str.clone()];
        new_vs.extend(result.vs);
        new_vs.push(br_str.clone());
        result.vs = new_vs;

        // Adjust barycenter for border predecessors
        let bl_preds = g.predecessors(bl_str).unwrap_or_default();
        let br_preds = g.predecessors(br_str).unwrap_or_default();
        if !bl_preds.is_empty() {
            let bl_pred_order = g.node(&bl_preds[0]).and_then(|n| n.order).unwrap_or(0) as f64;
            let br_pred_order = g.node(&br_preds[0]).and_then(|n| n.order).unwrap_or(0) as f64;

            let (bc, wt) = match (result.barycenter, result.weight) {
                (Some(bc), Some(wt)) => (bc, wt),
                _ => (0.0, 0.0),
            };

            result.barycenter = Some((bc * wt + bl_pred_order + br_pred_order) / (wt + 2.0));
            result.weight = Some(wt + 2.0);
        }
    }

    result
}

fn merge_barycenters(target: &mut barycenter::BarycenterEntry, other: &sort::SortResult) {
    if let (Some(tb), Some(tw)) = (target.barycenter, target.weight) {
        if let (Some(ob), Some(ow)) = (other.barycenter, other.weight) {
            target.barycenter = Some((tb * tw + ob * ow) / (tw + ow));
            target.weight = Some(tw + ow);
        }
    } else {
        target.barycenter = other.barycenter;
        target.weight = other.weight;
    }
}
