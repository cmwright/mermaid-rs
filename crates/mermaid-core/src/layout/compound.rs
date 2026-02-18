use std::collections::{HashMap, HashSet};

use crate::ast::common::StyleProperties;
use crate::ast::flowchart::{EdgeDef, FlowchartAst, StyleOverride, SubgraphDef};
use crate::layout::graph_builder::SubgraphMembership;
use crate::layout::types::*;

/// Position subgraphs as bounding boxes around their contained nodes.
/// Recursively processes nested subgraphs from innermost to outermost.
pub fn position_subgraphs(
    subgraphs: &[SubgraphDef],
    positioned_nodes: &[PositionedNode],
    style_overrides: &[StyleOverride],
) -> Vec<PositionedSubgraph> {
    let node_pos: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let mut result = Vec::new();
    position_subgraphs_recursive(subgraphs, &node_pos, style_overrides, &mut result);
    result
}

fn position_subgraphs_recursive(
    subgraphs: &[SubgraphDef],
    node_pos: &HashMap<&str, &PositionedNode>,
    style_overrides: &[StyleOverride],
    result: &mut Vec<PositionedSubgraph>,
) {
    for sg in subgraphs {
        position_subgraphs_recursive(&sg.subgraphs, node_pos, style_overrides, result);

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

/// Ensure sibling subgraphs do not overlap.
pub fn separate_overlapping_sibling_subgraphs(
    ast: &FlowchartAst,
    membership: &SubgraphMembership,
    nodes: &mut [PositionedNode],
    subgraphs: &[PositionedSubgraph],
    all_edges: &[EdgeDef],
    is_horizontal: bool,
) {
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

        let mut placed: Vec<(&str, f64, f64)> = Vec::new();

        for id in siblings {
            let Some(sg) = bounds.get(id) else { continue };
            let cross_start = if is_horizontal { sg.y } else { sg.x };
            let cross_size = if is_horizontal { sg.height } else { sg.width };
            let cross_end = cross_start + cross_size;

            let main_start = if is_horizontal { sg.x } else { sg.y };
            let main_end = if is_horizontal {
                sg.x + sg.width
            } else {
                sg.y + sg.height
            };

            let mut required_cross_start = cross_start;
            for (placed_id, _placed_cross_start, placed_cross_end) in &placed {
                let Some(placed_sg) = bounds.get(placed_id) else {
                    continue;
                };
                let placed_main_start = if is_horizontal {
                    placed_sg.x
                } else {
                    placed_sg.y
                };
                let placed_main_end = if is_horizontal {
                    placed_sg.x + placed_sg.width
                } else {
                    placed_sg.y + placed_sg.height
                };

                let main_overlap = main_start < placed_main_end - overlap_epsilon
                    && main_end > placed_main_start + overlap_epsilon;
                if main_overlap {
                    required_cross_start = required_cross_start.max(*placed_cross_end + 8.0);
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
