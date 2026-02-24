//! Resolve conflicts between barycenters and constraint graph.
//! Port of dagre's `order/resolve-conflicts.js`.

use crate::graph::ConstraintGraph;
use crate::order::barycenter::BarycenterEntry;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ResolvedEntry {
    pub vs: Vec<String>,
    pub i: usize,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone)]
struct MappedEntry {
    indegree: usize,
    in_entries: Vec<usize>, // indices into mapped_entries
    out_entries: Vec<usize>,
    vs: Vec<String>,
    i: usize,
    barycenter: Option<f64>,
    weight: Option<f64>,
    merged: bool,
}

/// Resolves conflicts between barycenters and the constraint graph.
pub fn resolve_conflicts(entries: &[BarycenterEntry], cg: &ConstraintGraph) -> Vec<ResolvedEntry> {
    let mut mapped: HashMap<String, usize> = HashMap::new();
    let mut entries_vec: Vec<MappedEntry> = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        let me = MappedEntry {
            indegree: 0,
            in_entries: Vec::new(),
            out_entries: Vec::new(),
            vs: vec![entry.v.clone()],
            i,
            barycenter: entry.barycenter,
            weight: entry.weight,
            merged: false,
        };
        let idx = entries_vec.len();
        entries_vec.push(me);
        mapped.insert(entry.v.clone(), idx);
    }

    // Add constraint edges
    for e in cg.edges() {
        if let (Some(&v_idx), Some(&w_idx)) = (mapped.get(&e.v), mapped.get(&e.w)) {
            entries_vec[w_idx].indegree += 1;
            entries_vec[v_idx].out_entries.push(w_idx);
        }
    }

    // Source set: entries with indegree 0
    let mut source_set: Vec<usize> = entries_vec
        .iter()
        .enumerate()
        .filter(|(_, e)| e.indegree == 0)
        .map(|(i, _)| i)
        .collect();

    let mut result_order: Vec<usize> = Vec::new();

    while let Some(idx) = source_set.pop() {
        result_order.push(idx);

        // Handle in-entries (merge if needed)
        let in_entries = entries_vec[idx].in_entries.clone();
        for &u_idx in in_entries.iter().rev() {
            if entries_vec[u_idx].merged {
                continue;
            }
            let should_merge = entries_vec[u_idx].barycenter.is_none()
                || entries_vec[idx].barycenter.is_none()
                || entries_vec[u_idx].barycenter.unwrap() >= entries_vec[idx].barycenter.unwrap();

            if should_merge {
                // Merge u_idx into idx
                merge_entries(&mut entries_vec, idx, u_idx);
            }
        }

        // Handle out-entries
        let out_entries = entries_vec[idx].out_entries.clone();
        for &w_idx in &out_entries {
            entries_vec[w_idx].in_entries.push(idx);
            entries_vec[w_idx].indegree -= 1;
            if entries_vec[w_idx].indegree == 0 {
                source_set.push(w_idx);
            }
        }
    }

    result_order
        .into_iter()
        .filter(|&idx| !entries_vec[idx].merged)
        .map(|idx| {
            let e = &entries_vec[idx];
            ResolvedEntry {
                vs: e.vs.clone(),
                i: e.i,
                barycenter: e.barycenter,
                weight: e.weight,
            }
        })
        .collect()
}

fn merge_entries(entries: &mut [MappedEntry], target: usize, source: usize) {
    let mut sum = 0.0;
    let mut weight = 0.0;

    if let Some(tw) = entries[target].weight {
        sum += entries[target].barycenter.unwrap_or(0.0) * tw;
        weight += tw;
    }
    if let Some(sw) = entries[source].weight {
        sum += entries[source].barycenter.unwrap_or(0.0) * sw;
        weight += sw;
    }

    let source_vs = entries[source].vs.clone();
    let source_i = entries[source].i;

    // target.vs = source.vs.concat(target.vs)
    let mut new_vs = source_vs;
    new_vs.extend(entries[target].vs.clone());
    entries[target].vs = new_vs;

    if weight > 0.0 {
        entries[target].barycenter = Some(sum / weight);
        entries[target].weight = Some(weight);
    }
    entries[target].i = entries[target].i.min(source_i);
    entries[source].merged = true;
}
