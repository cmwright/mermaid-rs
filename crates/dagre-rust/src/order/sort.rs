//! Sort entries by barycenter with bias.
//! Port of dagre's `order/sort.js`.

use crate::order::resolve_conflicts::ResolvedEntry;

#[derive(Debug, Clone)]
pub struct SortResult {
    pub vs: Vec<String>,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

/// Sorts entries by barycenter, interleaving unsortable entries.
pub fn sort(entries: &[ResolvedEntry], bias_right: bool) -> SortResult {
    let mut sortable: Vec<&ResolvedEntry> = Vec::new();
    let mut unsortable: Vec<&ResolvedEntry> = Vec::new();

    for entry in entries {
        if entry.barycenter.is_some() {
            sortable.push(entry);
        } else {
            unsortable.push(entry);
        }
    }

    // Sort unsortable by descending i
    unsortable.sort_by_key(|b| std::cmp::Reverse(b.i));

    // Sort sortable by barycenter with bias
    sortable.sort_by(|a, b| {
        let ab = a.barycenter.unwrap_or(0.0);
        let bb = b.barycenter.unwrap_or(0.0);
        if ab < bb {
            std::cmp::Ordering::Less
        } else if ab > bb {
            std::cmp::Ordering::Greater
        } else if !bias_right {
            a.i.cmp(&b.i)
        } else {
            b.i.cmp(&a.i)
        }
    });

    let mut vs: Vec<Vec<String>> = Vec::new();
    let mut sum = 0.0;
    let mut weight = 0.0;
    let mut vs_index: usize = 0;

    // Consume unsortable entries at the beginning
    vs_index = consume_unsortable(&mut vs, &mut unsortable, vs_index);

    for entry in &sortable {
        vs_index += entry.vs.len();
        vs.push(entry.vs.clone());
        sum += entry.barycenter.unwrap_or(0.0) * entry.weight.unwrap_or(0.0);
        weight += entry.weight.unwrap_or(0.0);
        vs_index = consume_unsortable(&mut vs, &mut unsortable, vs_index);
    }

    let flat: Vec<String> = vs.into_iter().flatten().collect();

    SortResult {
        vs: flat,
        barycenter: if weight > 0.0 {
            Some(sum / weight)
        } else {
            None
        },
        weight: if weight > 0.0 { Some(weight) } else { None },
    }
}

fn consume_unsortable(
    vs: &mut Vec<Vec<String>>,
    unsortable: &mut Vec<&ResolvedEntry>,
    mut index: usize,
) -> usize {
    while let Some(last) = unsortable.last() {
        if last.i <= index {
            let entry = unsortable.pop().unwrap();
            vs.push(entry.vs.clone());
            index += 1;
        } else {
            break;
        }
    }
    index
}
