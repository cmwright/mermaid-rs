use std::collections::{HashMap, HashSet};

use crate::ast::common::StyleProperties;
use crate::ast::flowchart::{EdgeDef, FlowchartAst, StyleOverride, SubgraphDef};
use crate::layout::flowchart::graph_builder::SubgraphMembership;
use crate::layout::flowchart::types::*;
use crate::layout::text_measure::TextMeasurer;

const SUBGRAPH_TITLE_SIDE_PADDING: f64 = 18.0;

/// Position subgraphs as bounding boxes around their contained nodes.
/// Recursively processes nested subgraphs from innermost to outermost.
pub fn position_subgraphs(
    subgraphs: &[SubgraphDef],
    positioned_nodes: &[PositionedNode],
    style_overrides: &[StyleOverride],
    measurer: &TextMeasurer<'_>,
) -> Vec<PositionedSubgraph> {
    let node_pos: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let mut result = Vec::new();
    position_subgraphs_recursive(subgraphs, &node_pos, style_overrides, measurer, &mut result);
    result
}

fn position_subgraphs_recursive(
    subgraphs: &[SubgraphDef],
    node_pos: &HashMap<&str, &PositionedNode>,
    style_overrides: &[StyleOverride],
    measurer: &TextMeasurer<'_>,
    result: &mut Vec<PositionedSubgraph>,
) {
    for sg in subgraphs {
        position_subgraphs_recursive(&sg.subgraphs, node_pos, style_overrides, measurer, result);

        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut has_content = false;

        for node in &sg.nodes {
            if let Some(pn) = node_pos.get(node.id.as_str()) {
                min_x = min_x.min(pn.x - pn.width / 2.0);
                min_y = min_y.min(pn.y - pn.height / 2.0);
                max_x = max_x.max(pn.x + pn.width / 2.0);
                max_y = max_y.max(pn.y + pn.height / 2.0);
                has_content = true;
            }
        }

        for child_sg in &sg.subgraphs {
            if let Some(child_pos) = result.iter().find(|ps| ps.id == child_sg.id) {
                min_x = min_x.min(child_pos.x);
                min_y = min_y.min(child_pos.y);
                max_x = max_x.max(child_pos.x + child_pos.width);
                max_y = max_y.max(child_pos.y + child_pos.height);
                has_content = true;
            }
        }

        for edge in &sg.edges {
            for id in [&edge.from, &edge.to] {
                if let Some(pn) = node_pos.get(id.as_str()) {
                    min_x = min_x.min(pn.x - pn.width / 2.0);
                    min_y = min_y.min(pn.y - pn.height / 2.0);
                    max_x = max_x.max(pn.x + pn.width / 2.0);
                    max_y = max_y.max(pn.y + pn.height / 2.0);
                    has_content = true;
                }
            }
        }

        if has_content {
            let title_height = if let Some(ref label) = sg.label {
                let normalized = crate::render::html_util::normalize_br(label);
                let line_count = normalized.split('\n').count();
                SUBGRAPH_TITLE_HEIGHT + (line_count.saturating_sub(1) as f64) * 16.0
            } else {
                SUBGRAPH_TITLE_HEIGHT
            };
            let title_text = sg.label.as_deref().unwrap_or(&sg.id);
            let title_width = measure_subgraph_title_width(title_text, measurer);
            let content_width = max_x - min_x;
            let min_required_width = title_width + 2.0 * SUBGRAPH_TITLE_SIDE_PADDING;
            if content_width < min_required_width {
                let extra = (min_required_width - content_width) / 2.0;
                min_x -= extra;
                max_x += extra;
            }

            let mut style = StyleProperties::default();
            for so in style_overrides {
                if so.node_id == sg.id {
                    style = style.merge(&so.properties);
                }
            }

            let label = sg.label.clone().or_else(|| Some(sg.id.clone()));

            result.push(PositionedSubgraph {
                id: sg.id.clone(),
                label,
                x: min_x - SUBGRAPH_PADDING,
                y: min_y - SUBGRAPH_PADDING - title_height,
                width: (max_x - min_x) + 2.0 * SUBGRAPH_PADDING,
                height: (max_y - min_y) + 2.0 * SUBGRAPH_PADDING + title_height,
                style,
            });
        }
    }
}

fn measure_subgraph_title_width(label: &str, measurer: &TextMeasurer<'_>) -> f64 {
    let normalized = crate::render::html_util::normalize_br(label);
    normalized
        .split('\n')
        .map(crate::render::html_util::strip_html_tags)
        .map(|line| measurer.measure(&line).width)
        .fold(0.0, f64::max)
}

/// Ensure sibling subgraphs do not overlap.
///
/// Two passes:
/// 1. Main-axis pass: when sibling subgraphs have a small main-axis overlap
///    (i.e. they are mostly stacked), push the lower/right one further along
///    the main axis so their bounding boxes no longer intersect.
/// 2. Cross-axis pass: for subgraphs that still overlap on the main axis
///    (i.e. they live at overlapping ranks), push on the cross axis.
pub fn separate_overlapping_sibling_subgraphs(
    ast: &FlowchartAst,
    membership: &SubgraphMembership,
    nodes: &mut [PositionedNode],
    subgraphs: &[PositionedSubgraph],
    all_edges: &[EdgeDef],
    is_horizontal: bool,
) {
    let main_gap = 50.0; // space between stacked subgraphs (room for edge labels)
    let cross_gap = 12.0; // space between side-by-side subgraphs
    let overlap_epsilon = 1e-6;

    let mut parent_children: HashMap<Option<String>, Vec<String>> = HashMap::new();
    collect_subgraph_parent_map(&ast.subgraphs, None, &mut parent_children);

    let bounds: HashMap<&str, &PositionedSubgraph> =
        subgraphs.iter().map(|sg| (sg.id.as_str(), sg)).collect();

    for child_ids in parent_children.values() {
        let mut siblings: Vec<&str> = child_ids
            .iter()
            .map(|s| s.as_str())
            .filter(|id| bounds.contains_key(id))
            .collect();

        let node_by_id: HashMap<&str, &PositionedNode> =
            nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let mut sibling_members: HashMap<&str, HashSet<&str>> = HashMap::new();
        for &sg_id in &siblings {
            let members: HashSet<&str> = membership
                .iter()
                .filter(|(_, path)| path.iter().any(|p| p == sg_id))
                .map(|(id, _)| id.as_str())
                .collect();
            sibling_members.insert(sg_id, members);
        }
        let mut target_cross: HashMap<&str, f64> = HashMap::new();
        for &sg_id in &siblings {
            let Some(members) = sibling_members.get(sg_id) else {
                continue;
            };
            let mut acc = 0.0;
            let mut cnt = 0usize;
            for e in all_edges {
                let from_in = members.contains(e.from.as_str());
                let to_in = members.contains(e.to.as_str());
                if from_in == to_in {
                    continue;
                }
                let other = if from_in {
                    e.to.as_str()
                } else {
                    e.from.as_str()
                };
                if let Some(n) = node_by_id.get(other) {
                    acc += if is_horizontal { n.y } else { n.x };
                    cnt += 1;
                }
            }
            if cnt > 0 {
                target_cross.insert(sg_id, acc / cnt as f64);
            } else if let Some(sg) = bounds.get(sg_id) {
                target_cross.insert(
                    sg_id,
                    if is_horizontal {
                        sg.y + sg.height / 2.0
                    } else {
                        sg.x + sg.width / 2.0
                    },
                );
            }
        }

        siblings.sort_by(|a, b| {
            let ta = target_cross.get(a).copied().unwrap_or(0.0);
            let tb = target_cross.get(b).copied().unwrap_or(0.0);
            let primary = ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal);
            if primary != std::cmp::Ordering::Equal {
                return primary;
            }
            let sa = bounds
                .get(a)
                .map(|sg| if is_horizontal { sg.y } else { sg.x })
                .unwrap_or(0.0);
            let sb = bounds
                .get(b)
                .map(|sg| if is_horizontal { sg.y } else { sg.x })
                .unwrap_or(0.0);
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        });

        // --- Pass 1: resolve main-axis overlaps ---
        // Track cumulative main-axis shifts per subgraph so we can apply them.
        let mut main_shifts: HashMap<&str, f64> = HashMap::new();

        // Sort siblings by main-axis start for this pass.
        let mut by_main: Vec<&str> = siblings.clone();
        by_main.sort_by(|a, b| {
            let ma = bounds
                .get(a)
                .map(|sg| if is_horizontal { sg.x } else { sg.y })
                .unwrap_or(0.0);
            let mb = bounds
                .get(b)
                .map(|sg| if is_horizontal { sg.x } else { sg.y })
                .unwrap_or(0.0);
            ma.partial_cmp(&mb).unwrap_or(std::cmp::Ordering::Equal)
        });

        for (i, &id) in by_main.iter().enumerate() {
            let Some(sg) = bounds.get(id) else { continue };
            let my_main_start = if is_horizontal { sg.x } else { sg.y }
                + main_shifts.get(id).copied().unwrap_or(0.0);
            let my_main_end = my_main_start + if is_horizontal { sg.width } else { sg.height };
            let my_cross_start = if is_horizontal { sg.y } else { sg.x };
            let my_cross_end = my_cross_start + if is_horizontal { sg.height } else { sg.width };

            for &prev_id in &by_main[..i] {
                let Some(prev_sg) = bounds.get(prev_id) else {
                    continue;
                };
                let prev_main_start = if is_horizontal { prev_sg.x } else { prev_sg.y }
                    + main_shifts.get(prev_id).copied().unwrap_or(0.0);
                let prev_main_end = prev_main_start
                    + if is_horizontal {
                        prev_sg.width
                    } else {
                        prev_sg.height
                    };
                let prev_cross_start = if is_horizontal { prev_sg.y } else { prev_sg.x };
                let prev_cross_end = prev_cross_start
                    + if is_horizontal {
                        prev_sg.height
                    } else {
                        prev_sg.width
                    };

                // Check for 2D overlap (both axes)
                let main_overlap = my_main_start < prev_main_end - overlap_epsilon
                    && my_main_end > prev_main_start + overlap_epsilon;
                let cross_overlap = my_cross_start < prev_cross_end - overlap_epsilon
                    && my_cross_end > prev_cross_start + overlap_epsilon;

                if main_overlap && cross_overlap {
                    // Determine the smaller fix: push on main or cross axis.
                    // If the main-axis overlap is small relative to the total main span,
                    // push on the main axis (they're mostly stacked).
                    let main_overlap_amount =
                        my_main_end.min(prev_main_end) - my_main_start.max(prev_main_start);
                    let my_main_size = my_main_end - my_main_start;
                    let prev_main_size = prev_main_end - prev_main_start;
                    let larger_main = my_main_size.max(prev_main_size);

                    // If overlap is less than half the larger subgraph's main size,
                    // it's a stacking situation — resolve on main axis.
                    if main_overlap_amount < larger_main * 0.5 {
                        let needed = prev_main_end + main_gap - my_main_start;
                        if needed > 0.0 {
                            let cur = main_shifts.entry(id).or_insert(0.0);
                            *cur = cur.max(needed);
                        }
                    }
                }
            }
        }

        // Apply main-axis shifts
        for (&sg_id, &delta) in &main_shifts {
            if delta > overlap_epsilon {
                shift_nodes_in_subgraph_main(nodes, membership, sg_id, delta, is_horizontal);
            }
        }

        // --- Pass 2: resolve cross-axis overlaps (original logic) ---
        // Recompute bounds after main-axis shifts by adjusting in-memory.
        // We adjust the placed tracking to account for main shifts.
        let mut placed: Vec<(&str, f64, f64)> = Vec::new();

        for id in siblings {
            let Some(sg) = bounds.get(id) else { continue };
            let cross_start = if is_horizontal { sg.y } else { sg.x };
            let cross_size = if is_horizontal { sg.height } else { sg.width };
            let cross_end = cross_start + cross_size;

            let main_shift = main_shifts.get(id).copied().unwrap_or(0.0);
            let main_start = (if is_horizontal { sg.x } else { sg.y }) + main_shift;
            let main_end = main_start + if is_horizontal { sg.width } else { sg.height };

            let mut required_cross_start = cross_start;
            for (placed_id, _placed_cross_start, placed_cross_end) in &placed {
                let Some(placed_sg) = bounds.get(placed_id) else {
                    continue;
                };
                let placed_main_shift = main_shifts.get(placed_id).copied().unwrap_or(0.0);
                let placed_main_start = (if is_horizontal {
                    placed_sg.x
                } else {
                    placed_sg.y
                }) + placed_main_shift;
                let placed_main_end = placed_main_start
                    + if is_horizontal {
                        placed_sg.width
                    } else {
                        placed_sg.height
                    };

                let main_overlap = main_start < placed_main_end - overlap_epsilon
                    && main_end > placed_main_start + overlap_epsilon;
                if main_overlap {
                    required_cross_start = required_cross_start.max(*placed_cross_end + cross_gap);
                }
            }

            if required_cross_start > cross_start {
                let delta = required_cross_start - cross_start;
                shift_nodes_in_subgraph(nodes, membership, id, delta, is_horizontal);
                placed.push((id, cross_start + delta, cross_end + delta));
            } else {
                placed.push((id, cross_start, cross_end));
            }
        }
    }
}

fn collect_subgraph_parent_map(
    subgraphs: &[SubgraphDef],
    parent: Option<String>,
    out: &mut HashMap<Option<String>, Vec<String>>,
) {
    for sg in subgraphs {
        out.entry(parent.clone()).or_default().push(sg.id.clone());
        collect_subgraph_parent_map(&sg.subgraphs, Some(sg.id.clone()), out);
    }
}

fn shift_nodes_in_subgraph(
    nodes: &mut [PositionedNode],
    membership: &SubgraphMembership,
    subgraph_id: &str,
    delta: f64,
    is_horizontal: bool,
) {
    for node in nodes {
        let in_subgraph = membership
            .get(&node.id)
            .map(|path| path.iter().any(|sg| sg == subgraph_id))
            .unwrap_or(false);
        if in_subgraph {
            if is_horizontal {
                node.y += delta;
            } else {
                node.x += delta;
            }
        }
    }
}

/// Shift nodes in a subgraph along the main axis (y in TD, x in LR).
fn shift_nodes_in_subgraph_main(
    nodes: &mut [PositionedNode],
    membership: &SubgraphMembership,
    subgraph_id: &str,
    delta: f64,
    is_horizontal: bool,
) {
    for node in nodes {
        let in_subgraph = membership
            .get(&node.id)
            .map(|path| path.iter().any(|sg| sg == subgraph_id))
            .unwrap_or(false);
        if in_subgraph {
            if is_horizontal {
                node.x += delta;
            } else {
                node.y += delta;
            }
        }
    }
}

/// Post-processing step to compact subgraph nodes by shifting them toward
/// the subgraph's centroid. This helps keep nodes in the same subgraph closer
/// together after the Sugiyama layout has spread them across ranks.
pub fn compact_subgraphs(
    nodes: &mut [PositionedNode],
    membership: &SubgraphMembership,
    is_horizontal: bool,
) {
    use std::collections::HashMap;

    // Group nodes by their immediate subgraph
    let mut subgraph_nodes: HashMap<&str, Vec<&mut PositionedNode>> = HashMap::new();

    for node in nodes.iter_mut() {
        if let Some(path) = membership.get(&node.id) {
            if let Some(sg_id) = path.first() {
                subgraph_nodes.entry(sg_id).or_default().push(node);
            }
        }
    }

    // For each subgraph, shift nodes toward the median position
    for (_sg_id, mut sg_nodes) in subgraph_nodes {
        if sg_nodes.len() <= 1 {
            continue;
        }

        // Calculate median position on the main axis
        let mut positions: Vec<f64> = sg_nodes
            .iter()
            .map(|n| if is_horizontal { n.x } else { n.y })
            .collect();
        positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median = if positions.len() % 2 == 1 {
            positions[positions.len() / 2]
        } else {
            (positions[positions.len() / 2 - 1] + positions[positions.len() / 2]) / 2.0
        };

        // Calculate current centroid
        let min_pos = *positions.first().unwrap();
        let max_pos = *positions.last().unwrap();
        let centroid = (min_pos + max_pos) / 2.0;

        // Shift nodes toward median (but preserve relative ordering)
        // Use a weighted approach: nodes further from centroid move more
        for node in &mut sg_nodes {
            let pos = if is_horizontal { node.x } else { node.y };
            let dist_from_centroid = (pos - centroid).abs();
            let max_dist = (max_pos - min_pos).max(1.0);
            let factor = 0.4 * (dist_from_centroid / max_dist); // Move up to 40% toward median

            let shift = (median - pos) * factor;

            if is_horizontal {
                node.x += shift;
            } else {
                node.y += shift;
            }
        }
    }
}
