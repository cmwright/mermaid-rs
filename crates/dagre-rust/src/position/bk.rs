//! Coordinate assignment using Brandes-Kopf algorithm.
//! Port of dagre's `position/bk.js`.

use crate::graph::{Graph, LayoutGraph};
use crate::types::*;
use crate::util;
use std::collections::{HashMap, HashSet};

/// The block graph uses no node labels and f64 edge labels (separation values).
type BlockGraph = Graph<(), f64, ()>;

/// Boxed neighbor function used in alignment passes.
type NeighborFn<'a> = Box<dyn Fn(&str) -> Vec<String> + 'a>;

/// Main entry: assigns x coordinates using 4 alignment passes + median.
pub fn position_x(g: &LayoutGraph) -> HashMap<String, f64> {
    let layering = util::build_layer_matrix(g);
    let mut conflicts = find_type1_conflicts(g, &layering);
    let type2 = find_type2_conflicts(g, &layering);
    for (k, v) in type2 {
        let inner = conflicts.entry(k).or_default();
        for k2 in v {
            inner.insert(k2);
        }
    }

    let mut xss: HashMap<String, HashMap<String, f64>> = HashMap::new();

    for vert in &["u", "d"] {
        let adjusted_layering: Vec<Vec<String>> = if *vert == "u" {
            layering.clone()
        } else {
            layering.iter().rev().cloned().collect()
        };

        for horiz in &["l", "r"] {
            let adj = if *horiz == "r" {
                adjusted_layering
                    .iter()
                    .map(|layer| layer.iter().rev().cloned().collect())
                    .collect()
            } else {
                adjusted_layering.clone()
            };

            let neighbor_fn: NeighborFn = if *vert == "u" {
                Box::new(|v: &str| g.predecessors(v).unwrap_or_default())
            } else {
                Box::new(|v: &str| g.successors(v).unwrap_or_default())
            };

            let align = vertical_alignment(g, &adj, &conflicts, &*neighbor_fn);
            let mut xs = horizontal_compaction(g, &adj, &align.root, &align.align, *horiz == "r");

            if *horiz == "r" {
                xs = xs.into_iter().map(|(k, v)| (k, -v)).collect();
            }

            let key = format!("{}{}", vert, horiz);
            xss.insert(key, xs);
        }
    }

    let smallest_width = find_smallest_width_alignment(g, &xss);
    align_coordinates(&mut xss, &smallest_width);

    let align_opt = g.graph().align;

    balance(&xss, align_opt)
}

// === Type-1 and Type-2 conflict detection ===

type Conflicts = HashMap<String, HashSet<String>>;

fn find_type1_conflicts(g: &LayoutGraph, layering: &[Vec<String>]) -> Conflicts {
    let mut conflicts: Conflicts = HashMap::new();

    if layering.len() < 2 {
        return conflicts;
    }

    for li in 1..layering.len() {
        let prev_layer = &layering[li - 1];
        let layer = &layering[li];
        let mut k0: usize = 0;
        let mut scan_pos: usize = 0;
        let prev_layer_length = prev_layer.len();
        let last_node = layer.last().cloned().unwrap_or_default();

        for (i, v) in layer.iter().enumerate() {
            let w = find_other_inner_segment_node(g, v);
            let k1 = match &w {
                Some(w_node) => g
                    .node(w_node)
                    .and_then(|n| n.order)
                    .map(|o| o as u64)
                    .unwrap_or(prev_layer_length as u64) as usize,
                None => prev_layer_length,
            };

            if w.is_some() || *v == last_node {
                for scan_idx in scan_pos..=i {
                    if scan_idx < layer.len() {
                        let scan_node = &layer[scan_idx];
                        for u in g.predecessors(scan_node).unwrap_or_default() {
                            let u_pos = g
                                .node(&u)
                                .and_then(|n| n.order)
                                .map(|o| o as u64)
                                .unwrap_or(0) as usize;
                            let u_dummy = g.node(&u).map(|n| n.dummy.is_some()).unwrap_or(false);
                            let scan_dummy = g
                                .node(scan_node)
                                .map(|n| n.dummy.is_some())
                                .unwrap_or(false);
                            if (u_pos < k0 || k1 < u_pos) && !(u_dummy && scan_dummy) {
                                add_conflict(&mut conflicts, &u, scan_node);
                            }
                        }
                    }
                }
                scan_pos = i + 1;
                k0 = k1;
            }
        }
    }

    conflicts
}

fn find_type2_conflicts(g: &LayoutGraph, layering: &[Vec<String>]) -> Conflicts {
    let mut conflicts: Conflicts = HashMap::new();

    if layering.len() < 2 {
        return conflicts;
    }

    for li in 1..layering.len() {
        let north = &layering[li - 1];
        let south = &layering[li];
        let mut prev_north_pos: i64 = -1;
        let mut next_north_pos: i64 = 0;
        let mut south_pos: usize = 0;

        for (south_lookahead, v) in south.iter().enumerate() {
            let is_border = g.node(v).and_then(|n| n.dummy) == Some(DummyType::Border);

            if is_border {
                let preds = g.predecessors(v).unwrap_or_default();
                if !preds.is_empty() {
                    next_north_pos = g.node(&preds[0]).and_then(|n| n.order).unwrap_or(0);
                    scan_type2(
                        g,
                        &mut conflicts,
                        south,
                        south_pos,
                        south_lookahead,
                        prev_north_pos,
                        next_north_pos,
                    );
                    south_pos = south_lookahead;
                    prev_north_pos = next_north_pos;
                }
            }
            scan_type2(
                g,
                &mut conflicts,
                south,
                south_pos,
                south.len(),
                next_north_pos,
                north.len() as i64,
            );
        }
    }

    conflicts
}

fn scan_type2(
    g: &LayoutGraph,
    conflicts: &mut Conflicts,
    south: &[String],
    south_pos: usize,
    south_end: usize,
    prev_north_border: i64,
    next_north_border: i64,
) {
    for i in south_pos..south_end {
        if i >= south.len() {
            break;
        }
        let v = &south[i];
        let is_dummy = g.node(v).map(|n| n.dummy.is_some()).unwrap_or(false);
        if is_dummy {
            for u in g.predecessors(v).unwrap_or_default() {
                let u_dummy = g.node(&u).map(|n| n.dummy.is_some()).unwrap_or(false);
                if u_dummy {
                    let u_order = g.node(&u).and_then(|n| n.order).unwrap_or(0);
                    if u_order < prev_north_border || u_order > next_north_border {
                        add_conflict(conflicts, &u, v);
                    }
                }
            }
        }
    }
}

fn find_other_inner_segment_node(g: &LayoutGraph, v: &str) -> Option<String> {
    let is_dummy = g.node(v).map(|n| n.dummy.is_some()).unwrap_or(false);
    if is_dummy {
        g.predecessors(v)
            .unwrap_or_default()
            .into_iter()
            .find(|u| g.node(u).map(|n| n.dummy.is_some()).unwrap_or(false))
    } else {
        None
    }
}

fn add_conflict(conflicts: &mut Conflicts, v: &str, w: &str) {
    let (v, w) = if v > w { (w, v) } else { (v, w) };
    conflicts
        .entry(v.to_string())
        .or_default()
        .insert(w.to_string());
}

fn has_conflict(conflicts: &Conflicts, v: &str, w: &str) -> bool {
    let (v, w) = if v > w { (w, v) } else { (v, w) };
    conflicts
        .get(v)
        .map(|inner| inner.contains(w))
        .unwrap_or(false)
}

// === Vertical alignment ===

struct AlignResult {
    root: HashMap<String, String>,
    align: HashMap<String, String>,
}

fn vertical_alignment(
    _g: &LayoutGraph,
    layering: &[Vec<String>],
    conflicts: &Conflicts,
    neighbor_fn: &dyn Fn(&str) -> Vec<String>,
) -> AlignResult {
    let mut root: HashMap<String, String> = HashMap::new();
    let mut align: HashMap<String, String> = HashMap::new();
    let mut pos: HashMap<String, usize> = HashMap::new();

    for layer in layering {
        for (order, v) in layer.iter().enumerate() {
            root.insert(v.clone(), v.clone());
            align.insert(v.clone(), v.clone());
            pos.insert(v.clone(), order);
        }
    }

    for layer in layering {
        let mut prev_idx: i64 = -1;
        for v in layer {
            let mut ws = neighbor_fn(v);
            if !ws.is_empty() {
                ws.sort_by_key(|a| *pos.get(a).unwrap_or(&0));
                let mp = (ws.len() as f64 - 1.0) / 2.0;
                let i_start = mp.floor() as usize;
                let i_end = mp.ceil() as usize;
                for i in i_start..=i_end {
                    if i >= ws.len() {
                        break;
                    }
                    let w = &ws[i];
                    let w_pos = *pos.get(w).unwrap_or(&0) as i64;
                    if align.get(v).map(|a| a == v).unwrap_or(false)
                        && prev_idx < w_pos
                        && !has_conflict(conflicts, v, w)
                    {
                        align.insert(w.clone(), v.clone());
                        let rw = root.get(w).cloned().unwrap_or_else(|| w.clone());
                        align.insert(v.clone(), rw.clone());
                        root.insert(v.clone(), rw);
                        prev_idx = w_pos;
                    }
                }
            }
        }
    }

    AlignResult { root, align }
}

// === Horizontal compaction ===

fn horizontal_compaction(
    g: &LayoutGraph,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    align: &HashMap<String, String>,
    reverse_sep: bool,
) -> HashMap<String, f64> {
    let mut xs: HashMap<String, f64> = HashMap::new();
    let block_g = build_block_graph(g, layering, root, reverse_sep);

    // Pass 1: assign smallest coordinates (topological order using predecessors)
    let mut stack: Vec<String> = block_g.nodes();
    let mut visited: HashSet<String> = HashSet::new();

    while let Some(elem) = stack.pop() {
        if visited.contains(&elem) {
            // Process: xs[elem] = max of (xs[pred] + edge weight)
            let in_edges = block_g.in_edges(&elem, None).unwrap_or_default();
            let x = in_edges
                .iter()
                .map(|e| {
                    let pred_x = xs.get(&e.v).copied().unwrap_or(0.0);
                    let edge_val = block_g.edge_by_obj(e).copied().unwrap_or(0.0);
                    pred_x + edge_val
                })
                .fold(0.0f64, f64::max);
            xs.insert(elem, x);
        } else {
            visited.insert(elem.clone());
            stack.push(elem.clone());
            for pred in block_g.predecessors(&elem).unwrap_or_default() {
                stack.push(pred);
            }
        }
    }

    // Pass 2: assign greatest coordinates
    let skip_border = if reverse_sep {
        BorderType::Left
    } else {
        BorderType::Right
    };

    stack = block_g.nodes();
    visited.clear();

    while let Some(elem) = stack.pop() {
        if visited.contains(&elem) {
            let out_edges = block_g.out_edges(&elem, None).unwrap_or_default();
            let min = out_edges
                .iter()
                .map(|e| {
                    let succ_x = xs.get(&e.w).copied().unwrap_or(0.0);
                    let edge_val = block_g.edge_by_obj(e).copied().unwrap_or(0.0);
                    succ_x - edge_val
                })
                .fold(f64::INFINITY, f64::min);

            let node_border = g.node(&elem).and_then(|n| n.border_type);

            if min != f64::INFINITY && node_border != Some(skip_border) {
                let current = xs.get(&elem).copied().unwrap_or(0.0);
                xs.insert(elem, current.max(min));
            }
        } else {
            visited.insert(elem.clone());
            stack.push(elem.clone());
            for succ in block_g.successors(&elem).unwrap_or_default() {
                stack.push(succ);
            }
        }
    }

    // Assign x coordinates to all nodes via their root
    for v in align.keys() {
        let r = root.get(v).unwrap_or(v);
        let x = xs.get(r).copied().unwrap_or(0.0);
        xs.insert(v.clone(), x);
    }

    xs
}

fn build_block_graph(
    g: &LayoutGraph,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    reverse_sep: bool,
) -> BlockGraph {
    let mut block_graph: BlockGraph = Graph::new();
    let nodesep = g.graph().nodesep;
    let edgesep = g.graph().edgesep;

    for layer in layering {
        let mut u: Option<String> = None;
        for v in layer {
            let v_root = root.get(v).cloned().unwrap_or_else(|| v.clone());
            block_graph.set_node(&v_root, None);
            if let Some(ref u_node) = u {
                let u_root = root.get(u_node).cloned().unwrap_or_else(|| u_node.clone());
                let prev_max = block_graph
                    .edge(&u_root, &v_root, None)
                    .copied()
                    .unwrap_or(0.0);
                let sep_val = sep(g, v, u_node, nodesep, edgesep, reverse_sep);
                let new_val = sep_val.max(prev_max);
                block_graph.set_edge(&u_root, &v_root, Some(new_val), None);
            }
            u = Some(v.clone());
        }
    }

    block_graph
}

fn sep(g: &LayoutGraph, v: &str, w: &str, nodesep: f64, edgesep: f64, reverse_sep: bool) -> f64 {
    let v_label = g.node(v).unwrap();
    let w_label = g.node(w).unwrap();

    let v_width = v_label.width;
    let w_width = w_label.width;
    let v_dummy = v_label.dummy.is_some();
    let w_dummy = w_label.dummy.is_some();

    let mut sum = 0.0;

    sum += v_width / 2.0;

    if let Some(labelpos) = v_label.label_pos {
        let delta = match labelpos {
            LabelPos::Left => -v_width / 2.0,
            LabelPos::Right => v_width / 2.0,
            LabelPos::Center => 0.0,
        };
        if delta != 0.0 {
            sum += if reverse_sep { delta } else { -delta };
        }
    }

    sum += (if v_dummy { edgesep } else { nodesep }) / 2.0;
    sum += (if w_dummy { edgesep } else { nodesep }) / 2.0;

    sum += w_width / 2.0;

    if let Some(labelpos) = w_label.label_pos {
        let delta = match labelpos {
            LabelPos::Left => w_width / 2.0,
            LabelPos::Right => -w_width / 2.0,
            LabelPos::Center => 0.0,
        };
        if delta != 0.0 {
            sum += if reverse_sep { delta } else { -delta };
        }
    }

    sum
}

fn find_smallest_width_alignment(
    g: &LayoutGraph,
    xss: &HashMap<String, HashMap<String, f64>>,
) -> HashMap<String, f64> {
    let mut best_width = f64::INFINITY;
    let mut best: Option<&HashMap<String, f64>> = None;

    for xs in xss.values() {
        let mut max = f64::NEG_INFINITY;
        let mut min = f64::INFINITY;
        for (v, &x) in xs {
            let half_width = g.node(v).map(|n| n.width).unwrap_or(0.0) / 2.0;
            max = max.max(x + half_width);
            min = min.min(x - half_width);
        }
        let w = max - min;
        if w < best_width {
            best_width = w;
            best = Some(xs);
        }
    }

    best.cloned().unwrap_or_default()
}

fn align_coordinates(
    xss: &mut HashMap<String, HashMap<String, f64>>,
    align_to: &HashMap<String, f64>,
) {
    let align_to_min = align_to.values().copied().fold(f64::INFINITY, f64::min);
    let align_to_max = align_to.values().copied().fold(f64::NEG_INFINITY, f64::max);

    for vert in &["u", "d"] {
        for horiz in &["l", "r"] {
            let key = format!("{}{}", vert, horiz);
            // Skip if this is the alignment we're aligning to
            let xs = match xss.get(&key) {
                Some(xs) => xs.clone(),
                None => continue,
            };

            // Check if this IS the align_to map (by value equality)
            if &xs == align_to {
                continue;
            }

            let xs_min = xs.values().copied().fold(f64::INFINITY, f64::min);
            let xs_max = xs.values().copied().fold(f64::NEG_INFINITY, f64::max);

            let delta = if *horiz == "l" {
                align_to_min - xs_min
            } else {
                align_to_max - xs_max
            };

            if delta != 0.0 {
                let shifted: HashMap<String, f64> =
                    xs.into_iter().map(|(k, v)| (k, v + delta)).collect();
                xss.insert(key, shifted);
            }
        }
    }
}

fn balance(
    xss: &HashMap<String, HashMap<String, f64>>,
    align: Option<Align>,
) -> HashMap<String, f64> {
    let ul = xss.get("ul").cloned().unwrap_or_default();
    let mut result = HashMap::new();

    for v in ul.keys() {
        if let Some(a) = align {
            let key = match a {
                Align::UL => "ul",
                Align::UR => "ur",
                Align::DL => "dl",
                Align::DR => "dr",
            }
            .to_string();
            result.insert(
                v.clone(),
                xss.get(&key)
                    .and_then(|xs| xs.get(v))
                    .copied()
                    .unwrap_or(0.0),
            );
        } else {
            let mut vals: Vec<f64> = xss.values().filter_map(|xs| xs.get(v)).copied().collect();
            vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if vals.len() >= 4 {
                result.insert(v.clone(), (vals[1] + vals[2]) / 2.0);
            } else if vals.len() >= 2 {
                result.insert(v.clone(), (vals[0] + vals[vals.len() - 1]) / 2.0);
            } else if !vals.is_empty() {
                result.insert(v.clone(), vals[0]);
            }
        }
    }

    result
}
