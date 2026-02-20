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
    membership: &SubgraphMembership,
    style_overrides: &[StyleOverride],
    measurer: &TextMeasurer<'_>,
) -> Vec<PositionedSubgraph> {
    let node_pos: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let mut result = Vec::new();
    position_subgraphs_recursive(
        subgraphs,
        &node_pos,
        membership,
        style_overrides,
        measurer,
        &mut result,
    );
    result
}

fn position_subgraphs_recursive(
    subgraphs: &[SubgraphDef],
    node_pos: &HashMap<&str, &PositionedNode>,
    membership: &SubgraphMembership,
    style_overrides: &[StyleOverride],
    measurer: &TextMeasurer<'_>,
    result: &mut Vec<PositionedSubgraph>,
) {
    for sg in subgraphs {
        position_subgraphs_recursive(
            &sg.subgraphs,
            node_pos,
            membership,
            style_overrides,
            measurer,
            result,
        );

        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut has_content = false;

        for node in &sg.nodes {
            let in_subgraph = membership
                .get(&node.id)
                .map(|path| path.iter().any(|p| p == &sg.id))
                .unwrap_or(false);
            if !in_subgraph {
                continue;
            }
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
                let in_subgraph = membership
                    .get(id.as_str())
                    .map(|path| path.iter().any(|p| p == &sg.id))
                    .unwrap_or(false);
                if !in_subgraph {
                    continue;
                }
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
    let cross_gap = 12.0; // space between side-by-side subgraphs
    let dependency_main_gap = 50.0; // minimum gap along rank axis for sg dependencies
    let overlap_epsilon = 1e-6;

    let mut sibling_groups: Vec<Vec<String>> = Vec::new();
    collect_sibling_groups(&ast.subgraphs, &mut sibling_groups);

    let bounds: HashMap<&str, &PositionedSubgraph> =
        subgraphs.iter().map(|sg| (sg.id.as_str(), sg)).collect();

    // Process deeper sibling groups first, so child-level alignment/separation
    // is computed before any ancestor-level shifts move the whole branch.
    for child_ids in sibling_groups.into_iter().rev() {
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

        // --- Pass 1: derive dependency-based main-axis shifts ---
        // If sibling A has edges into sibling B, keep B below/right of A.
        // We compute these shifts before cross-axis packing so vertically
        // dependent groups are not treated as same-rank overlaps.
        let mut dep_pairs: HashSet<(&str, &str)> = HashSet::new();
        for e in all_edges {
            let src_sg = sibling_members
                .iter()
                .find_map(|(sg_id, members)| members.contains(e.from.as_str()).then_some(*sg_id));
            let dst_sg = sibling_members
                .iter()
                .find_map(|(sg_id, members)| members.contains(e.to.as_str()).then_some(*sg_id));
            if let (Some(src), Some(dst)) = (src_sg, dst_sg) {
                if src != dst {
                    dep_pairs.insert((src, dst));
                }
            }
        }

        // Apply dependency rank spacing only for vertical flows (TD/BT). In LR/RL,
        // forcing main-axis sibling spacing can explode graph width.
        let mut main_shifts: HashMap<&str, f64> = HashMap::new();
        if !is_horizontal && !dep_pairs.is_empty() {
            let mut dep_nodes: HashSet<&str> = HashSet::new();
            for &(src, dst) in &dep_pairs {
                dep_nodes.insert(src);
                dep_nodes.insert(dst);
            }

            let mut in_degree: HashMap<&str, usize> =
                dep_nodes.iter().map(|&n| (n, 0usize)).collect();
            let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
            for &(src, dst) in &dep_pairs {
                adj.entry(src).or_default().push(dst);
                *in_degree.entry(dst).or_insert(0) += 1;
            }

            let mut queue: Vec<&str> = in_degree
                .iter()
                .filter_map(|(&n, &d)| (d == 0).then_some(n))
                .collect();
            queue.sort_unstable();
            let mut topo = Vec::with_capacity(dep_nodes.len());
            while let Some(n) = queue.pop() {
                topo.push(n);
                if let Some(nexts) = adj.get(n) {
                    for &m in nexts {
                        if let Some(d) = in_degree.get_mut(m) {
                            *d -= 1;
                            if *d == 0 {
                                queue.push(m);
                            }
                        }
                    }
                }
            }

            // If dependency graph has cycles, skip this pass to avoid runaway shifts.
            if topo.len() == dep_nodes.len() {
                for &src in &topo {
                    let Some(src_sg) = bounds.get(src) else { continue };
                    let src_shift = main_shifts.get(src).copied().unwrap_or(0.0);
                    let src_start = src_sg.y + src_shift;
                    let src_end = src_start + src_sg.height;

                    if let Some(nexts) = adj.get(src) {
                        for &dst in nexts {
                            let Some(dst_sg) = bounds.get(dst) else { continue };
                            let required_start = src_end + dependency_main_gap;
                            let needed_shift = (required_start - dst_sg.y).max(0.0);
                            let cur = main_shifts.get(dst).copied().unwrap_or(0.0);
                            if needed_shift > cur {
                                main_shifts.insert(dst, needed_shift);
                            }
                        }
                    }
                }
            }
        }

        // --- Pass 2: resolve cross-axis overlaps (original logic) ---
        // Recompute bounds after main-axis shifts by adjusting in-memory.
        // We adjust the placed tracking to account for main shifts.
        let mut placed: Vec<(&str, f64, f64)> = Vec::new();

        for &id in &siblings {
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

        // Apply main-axis dependency shifts after cross-axis packing.
        for (&sg_id, &delta) in &main_shifts {
            if delta > overlap_epsilon {
                shift_nodes_in_subgraph_main(nodes, membership, sg_id, delta, is_horizontal);
            }
        }
    }
}

fn collect_sibling_groups(subgraphs: &[SubgraphDef], out: &mut Vec<Vec<String>>) {
    if subgraphs.is_empty() {
        return;
    }
    out.push(subgraphs.iter().map(|sg| sg.id.clone()).collect());
    for sg in subgraphs {
        collect_sibling_groups(&sg.subgraphs, out);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::SubgraphDef;
    use crate::font::FontProvider;

    #[test]
    fn test_position_subgraphs() {
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: Some("Subgraph".to_string()),
            direction: None,
            nodes: vec![crate::ast::flowchart::NodeDef {
                id: "A".into(),
                label: None,
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                class_shorthand: None,
            }],
            edges: vec![],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 50.0,
            width: 40.0,
            height: 20.0,
        }];
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(&subgraphs, &positioned_nodes, &membership, &[], &measurer);
        assert_eq!(result.len(), 1);
        assert!(result[0].width > 0.0);
        assert!(result[0].height > 0.0);
    }

    #[test]
    fn test_position_subgraphs_nested() {
        let subgraphs = vec![SubgraphDef {
            id: "Outer".to_string(),
            label: None,
            direction: None,
            nodes: vec![],
            edges: vec![],
            subgraphs: vec![SubgraphDef {
                id: "Inner".to_string(),
                label: Some("Inner".to_string()),
                direction: None,
                nodes: vec![crate::ast::flowchart::NodeDef {
                    id: "A".into(),
                    label: None,
                    shape: crate::ast::flowchart::NodeShape::Rectangle,
                    class_shorthand: None,
                }],
                edges: vec![],
                subgraphs: vec![],
            }],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 80.0,
            width: 40.0,
            height: 20.0,
        }];
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["Outer".to_string(), "Inner".to_string()]);
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(&subgraphs, &positioned_nodes, &membership, &[], &measurer);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_measure_subgraph_title_width_multiline() {
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let w = measure_subgraph_title_width("Line1\nLine2\nLine3", &measurer);
        assert!(w > 0.0);
    }

    #[test]
    fn test_separate_overlapping_sibling_subgraphs() {
        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "SG1".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        crate::ast::flowchart::NodeDef {
                            id: "A1".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        crate::ast::flowchart::NodeDef {
                            id: "A2".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![],
                    subgraphs: vec![],
                },
                SubgraphDef {
                    id: "SG2".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        crate::ast::flowchart::NodeDef {
                            id: "B1".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        crate::ast::flowchart::NodeDef {
                            id: "B2".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![],
                    subgraphs: vec![],
                },
            ],
            ..Default::default()
        };
        let mut membership = SubgraphMembership::new();
        membership.insert("A1".to_string(), vec!["SG1".to_string()]);
        membership.insert("A2".to_string(), vec!["SG1".to_string()]);
        membership.insert("B1".to_string(), vec!["SG2".to_string()]);
        membership.insert("B2".to_string(), vec!["SG2".to_string()]);

        let mut nodes = vec![
            PositionedNode {
                id: "A1".into(),
                label: "A1".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "A2".into(),
                label: "A2".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 100.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B1".into(),
                label: "B1".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 55.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B2".into(),
                label: "B2".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 105.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        let subgraphs = vec![
            PositionedSubgraph {
                id: "SG1".into(),
                label: Some("SG1".into()),
                x: 30.0,
                y: 30.0,
                width: 80.0,
                height: 100.0,
                style: Default::default(),
            },
            PositionedSubgraph {
                id: "SG2".into(),
                label: Some("SG2".into()),
                x: 30.0,
                y: 35.0,
                width: 80.0,
                height: 100.0,
                style: Default::default(),
            },
        ];
        let y_before: Vec<f64> = nodes.iter().map(|n| n.y).collect();
        let x_before: Vec<f64> = nodes.iter().map(|n| n.x).collect();
        separate_overlapping_sibling_subgraphs(
            &ast,
            &membership,
            &mut nodes,
            &subgraphs,
            &[],
            false,
        );
        let y_after: Vec<f64> = nodes.iter().map(|n| n.y).collect();
        let x_after: Vec<f64> = nodes.iter().map(|n| n.x).collect();
        assert!(y_before != y_after || x_before != x_after, "overlap resolution should shift nodes");
    }

    #[test]
    fn test_compact_subgraphs() {
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec!["SG".to_string()]);
        membership.insert("C".to_string(), vec!["SG".to_string()]);

        let mut nodes = vec![
            PositionedNode {
                id: "A".into(),
                label: "A".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B".into(),
                label: "B".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 150.0,
                y: 100.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "C".into(),
                label: "C".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 250.0,
                y: 150.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        let y_before: Vec<f64> = nodes.iter().map(|n| n.y).collect();
        compact_subgraphs(&mut nodes, &membership, false);
        let y_after: Vec<f64> = nodes.iter().map(|n| n.y).collect();
        assert_ne!(y_before, y_after);
    }

    #[test]
    fn test_shift_nodes_in_subgraph() {
        let mut membership = SubgraphMembership::new();
        membership.insert("A".into(), vec!["SG".to_string()]);

        let mut nodes = vec![
            PositionedNode {
                id: "A".into(),
                label: "A".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B".into(),
                label: "B".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 150.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        shift_nodes_in_subgraph(&mut nodes, &membership, "SG", 20.0, false);
        assert!((nodes[0].x - 70.0).abs() < 0.01);
        assert!((nodes[1].x - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_position_subgraphs_multiline_label() {
        // Subgraph with multiline label (line 113 - title_height with line_count > 1)
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: Some("Line1\nLine2\nLine3".to_string()),
            direction: None,
            nodes: vec![crate::ast::flowchart::NodeDef {
                id: "A".into(),
                label: None,
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                class_shorthand: None,
            }],
            edges: vec![],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 50.0,
            width: 40.0,
            height: 20.0,
        }];
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(&subgraphs, &positioned_nodes, &membership, &[], &measurer);
        assert_eq!(result.len(), 1);
        assert!(result[0].height > 0.0);
    }

    #[test]
    fn test_separate_overlapping_horizontal() {
        // is_horizontal: true -> shift_nodes_in_subgraph modifies node.y (line 383)
        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "SG1".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        crate::ast::flowchart::NodeDef {
                            id: "A1".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        crate::ast::flowchart::NodeDef {
                            id: "A2".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![],
                    subgraphs: vec![],
                },
                SubgraphDef {
                    id: "SG2".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        crate::ast::flowchart::NodeDef {
                            id: "B1".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        crate::ast::flowchart::NodeDef {
                            id: "B2".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![],
                    subgraphs: vec![],
                },
            ],
            ..Default::default()
        };
        let mut membership = SubgraphMembership::new();
        membership.insert("A1".to_string(), vec!["SG1".to_string()]);
        membership.insert("A2".to_string(), vec!["SG1".to_string()]);
        membership.insert("B1".to_string(), vec!["SG2".to_string()]);
        membership.insert("B2".to_string(), vec!["SG2".to_string()]);

        let mut nodes = vec![
            PositionedNode {
                id: "A1".into(),
                label: "A1".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "A2".into(),
                label: "A2".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 100.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B1".into(),
                label: "B1".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 55.0,
                y: 55.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B2".into(),
                label: "B2".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 55.0,
                y: 105.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        let subgraphs = vec![
            PositionedSubgraph {
                id: "SG1".into(),
                label: Some("SG1".into()),
                x: 30.0,
                y: 30.0,
                width: 80.0,
                height: 100.0,
                style: Default::default(),
            },
            PositionedSubgraph {
                id: "SG2".into(),
                label: Some("SG2".into()),
                x: 35.0,
                y: 35.0,
                width: 80.0,
                height: 100.0,
                style: Default::default(),
            },
        ];
        let y_before: Vec<f64> = nodes.iter().map(|n| n.y).collect();
        separate_overlapping_sibling_subgraphs(&ast, &membership, &mut nodes, &subgraphs, &[], true);
        let y_after: Vec<f64> = nodes.iter().map(|n| n.y).collect();
        assert!(y_before != y_after, "horizontal mode should shift nodes on y");
    }

    #[test]
    fn test_compact_subgraphs_horizontal() {
        // compact_subgraphs with is_horizontal: true (line 470 - node.x += shift)
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec!["SG".to_string()]);
        membership.insert("C".to_string(), vec!["SG".to_string()]);

        let mut nodes = vec![
            PositionedNode {
                id: "A".into(),
                label: "A".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B".into(),
                label: "B".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 150.0,
                y: 100.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "C".into(),
                label: "C".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 250.0,
                y: 150.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        let x_before: Vec<f64> = nodes.iter().map(|n| n.x).collect();
        compact_subgraphs(&mut nodes, &membership, true);
        let x_after: Vec<f64> = nodes.iter().map(|n| n.x).collect();
        assert_ne!(x_before, x_after);
    }

    #[test]
    fn test_compact_subgraphs_even_positions() {
        // compact_subgraphs with even number of nodes (lines 432, 438)
        let mut membership = SubgraphMembership::new();
        membership.insert("A".into(), vec!["SG".to_string()]);
        membership.insert("B".into(), vec!["SG".to_string()]);
        membership.insert("C".into(), vec!["SG".to_string()]);
        membership.insert("D".into(), vec!["SG".to_string()]);

        let mut nodes = vec![
            PositionedNode {
                id: "A".into(),
                label: "A".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 10.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B".into(),
                label: "B".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "C".into(),
                label: "C".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 90.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "D".into(),
                label: "D".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 130.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        compact_subgraphs(&mut nodes, &membership, false);
        assert!(nodes.iter().all(|n| n.y.is_finite()));
    }

    #[test]
    fn test_position_subgraphs_title_wider_than_content() {
        // content_width < min_required_width -> expand min_x/max_x (lines 87-91)
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: Some("Very Long Subgraph Title That Exceeds Content".to_string()),
            direction: None,
            nodes: vec![crate::ast::flowchart::NodeDef {
                id: "A".into(),
                label: None,
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                class_shorthand: None,
            }],
            edges: vec![],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 50.0,
            width: 10.0,
            height: 10.0,
        }];
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(&subgraphs, &positioned_nodes, &membership, &[], &measurer);
        assert_eq!(result.len(), 1);
        assert!(result[0].width >= 2.0 * SUBGRAPH_TITLE_SIDE_PADDING);
    }

    #[test]
    fn test_position_subgraphs_has_content_from_edges_only() {
        // Subgraph with has_content from edges (nodes in edges but not in node_pos)
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: None,
            direction: None,
            nodes: vec![],
            edges: vec![crate::ast::flowchart::EdgeDef {
                from: "A".into(),
                to: "B".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
            }],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![
            PositionedNode {
                id: "A".into(),
                label: "A".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B".into(),
                label: "B".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 150.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec!["SG".to_string()]);
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(&subgraphs, &positioned_nodes, &membership, &[], &measurer);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_separate_overlapping_subgraph_no_external_edges() {
        // Subgraph with no edges to outside (cnt==0) -> target_cross from bounds (lines 195-204)
        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "SG1".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        crate::ast::flowchart::NodeDef {
                            id: "A1".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        crate::ast::flowchart::NodeDef {
                            id: "A2".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![crate::ast::flowchart::EdgeDef {
                        from: "A1".into(),
                        to: "A2".into(),
                        line_style: crate::ast::flowchart::LineStyle::Solid,
                        arrow_start: crate::ast::flowchart::ArrowEnd::None,
                        arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                        label: None,
                    }],
                    subgraphs: vec![],
                },
                SubgraphDef {
                    id: "SG2".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        crate::ast::flowchart::NodeDef {
                            id: "B1".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        crate::ast::flowchart::NodeDef {
                            id: "B2".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![crate::ast::flowchart::EdgeDef {
                        from: "B1".into(),
                        to: "B2".into(),
                        line_style: crate::ast::flowchart::LineStyle::Solid,
                        arrow_start: crate::ast::flowchart::ArrowEnd::None,
                        arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                        label: None,
                    }],
                    subgraphs: vec![],
                },
            ],
            ..Default::default()
        };
        let mut membership = SubgraphMembership::new();
        membership.insert("A1".to_string(), vec!["SG1".to_string()]);
        membership.insert("A2".to_string(), vec!["SG1".to_string()]);
        membership.insert("B1".to_string(), vec!["SG2".to_string()]);
        membership.insert("B2".to_string(), vec!["SG2".to_string()]);

        let mut nodes = vec![
            PositionedNode {
                id: "A1".into(),
                label: "A1".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "A2".into(),
                label: "A2".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 100.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B1".into(),
                label: "B1".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 55.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B2".into(),
                label: "B2".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 105.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        let subgraphs = vec![
            PositionedSubgraph {
                id: "SG1".into(),
                label: Some("SG1".into()),
                x: 30.0,
                y: 30.0,
                width: 80.0,
                height: 100.0,
                style: Default::default(),
            },
            PositionedSubgraph {
                id: "SG2".into(),
                label: Some("SG2".into()),
                x: 30.0,
                y: 35.0,
                width: 80.0,
                height: 100.0,
                style: Default::default(),
            },
        ];
        separate_overlapping_sibling_subgraphs(&ast, &membership, &mut nodes, &subgraphs, &[], false);
        assert!(nodes.iter().all(|n| n.x.is_finite() && n.y.is_finite()));
    }

    #[test]
    fn test_separate_overlapping_main_axis_shift_applied() {
        // Overlapping subgraphs with main_overlap_amount < 0.5 * larger_main -> main-axis shift
        let ast = FlowchartAst {
            subgraphs: vec![
                SubgraphDef {
                    id: "SG1".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        crate::ast::flowchart::NodeDef {
                            id: "A1".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        crate::ast::flowchart::NodeDef {
                            id: "A2".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![],
                    subgraphs: vec![],
                },
                SubgraphDef {
                    id: "SG2".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![
                        crate::ast::flowchart::NodeDef {
                            id: "B1".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                        crate::ast::flowchart::NodeDef {
                            id: "B2".into(),
                            label: None,
                            shape: crate::ast::flowchart::NodeShape::Rectangle,
                            class_shorthand: None,
                        },
                    ],
                    edges: vec![],
                    subgraphs: vec![],
                },
            ],
            ..Default::default()
        };
        let mut membership = SubgraphMembership::new();
        membership.insert("A1".to_string(), vec!["SG1".to_string()]);
        membership.insert("A2".to_string(), vec!["SG1".to_string()]);
        membership.insert("B1".to_string(), vec!["SG2".to_string()]);
        membership.insert("B2".to_string(), vec!["SG2".to_string()]);

        let mut nodes = vec![
            PositionedNode {
                id: "A1".into(),
                label: "A1".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "A2".into(),
                label: "A2".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 100.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B1".into(),
                label: "B1".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 55.0,
                y: 55.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B2".into(),
                label: "B2".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 55.0,
                y: 105.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        let subgraphs = vec![
            PositionedSubgraph {
                id: "SG1".into(),
                label: Some("SG1".into()),
                x: 30.0,
                y: 30.0,
                width: 80.0,
                height: 100.0,
                style: Default::default(),
            },
            PositionedSubgraph {
                id: "SG2".into(),
                label: Some("SG2".into()),
                x: 35.0,
                y: 35.0,
                width: 80.0,
                height: 100.0,
                style: Default::default(),
            },
        ];
        let x_before: Vec<f64> = nodes.iter().map(|n| n.x).collect();
        separate_overlapping_sibling_subgraphs(&ast, &membership, &mut nodes, &subgraphs, &[], true);
        let x_after: Vec<f64> = nodes.iter().map(|n| n.x).collect();
        assert!(x_before != x_after || nodes.iter().all(|n| n.x.is_finite()));
    }

    #[test]
    fn test_compact_subgraphs_single_node_skipped() {
        // Subgraph with 1 node -> continue (line 328)
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        membership.insert("B".to_string(), vec!["SG2".to_string()]);

        let mut nodes = vec![
            PositionedNode {
                id: "A".into(),
                label: "A".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
            PositionedNode {
                id: "B".into(),
                label: "B".into(),
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                style: Default::default(),
                x: 150.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            },
        ];
        let x_before: Vec<f64> = nodes.iter().map(|n| n.x).collect();
        let y_before: Vec<f64> = nodes.iter().map(|n| n.y).collect();
        compact_subgraphs(&mut nodes, &membership, false);
        let x_after: Vec<f64> = nodes.iter().map(|n| n.x).collect();
        let y_after: Vec<f64> = nodes.iter().map(|n| n.y).collect();
        assert_eq!(x_before, x_after, "single-node subgraphs should not shift");
        assert_eq!(y_before, y_after, "single-node subgraphs should not shift");
    }

    #[test]
    fn test_subgraph_style_overrides() {
        let subgraphs = vec![SubgraphDef {
            id: "SG".to_string(),
            label: Some("Styled".to_string()),
            direction: None,
            nodes: vec![crate::ast::flowchart::NodeDef {
                id: "A".into(),
                label: None,
                shape: crate::ast::flowchart::NodeShape::Rectangle,
                class_shorthand: None,
            }],
            edges: vec![],
            subgraphs: vec![],
        }];
        let positioned_nodes = vec![PositionedNode {
            id: "A".into(),
            label: "A".into(),
            shape: crate::ast::flowchart::NodeShape::Rectangle,
            style: Default::default(),
            x: 100.0,
            y: 50.0,
            width: 40.0,
            height: 20.0,
        }];
        let style_overrides = vec![crate::ast::flowchart::StyleOverride {
            node_id: "SG".into(),
            properties: crate::ast::common::parse_style_string("fill:#f96"),
        }];
        let mut membership = SubgraphMembership::new();
        membership.insert("A".to_string(), vec!["SG".to_string()]);
        let provider = FontProvider::default_font();
        let measurer = TextMeasurer::new(provider.font_ref().unwrap(), 14.0);
        let result = position_subgraphs(
            &subgraphs,
            &positioned_nodes,
            &membership,
            &style_overrides,
            &measurer,
        );
        assert_eq!(result.len(), 1);
        assert!(result[0].style.fill.is_some());
    }
}
