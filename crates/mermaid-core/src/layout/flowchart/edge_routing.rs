use crate::ast::flowchart::{EdgeDef, NodeShape};
use crate::layout::flowchart::types::*;
use std::collections::HashMap;

/// Adjust edge label positions to prevent overlap with subgraph borders and titles.
/// For each label, checks if its bounding box straddles any subgraph border line
/// or overlaps the title area, and pushes it to the nearest clear position.
pub fn adjust_labels_for_subgraph_boundaries(
    edges: &mut [PositionedEdge],
    subgraphs: &[PositionedSubgraph],
) {
    let clearance = 4.0;

    for edge in edges.iter_mut() {
        let Some(lx) = edge.label_x else { continue };
        let Some(ly) = edge.label_y else { continue };
        let lw = edge.label_width.unwrap_or(0.0);
        let lh = edge.label_height.unwrap_or(0.0);
        if lw < 1.0 || lh < 1.0 {
            continue;
        }

        let mut cur_x = lx;
        let mut cur_y = ly;

        // Multiple passes to handle cascading adjustments from nested subgraphs
        for _ in 0..3 {
            let prev_x = cur_x;
            let prev_y = cur_y;

            for sg in subgraphs.iter() {
                let hw = lw / 2.0;
                let hh = lh / 2.0;

                let sg_right = sg.x + sg.width;
                let sg_bottom = sg.y + sg.height;
                let title_bottom = sg.y + SUBGRAPH_TITLE_HEIGHT + SUBGRAPH_PADDING;

                // --- Horizontal borders (top/bottom of subgraph) ---
                // Only relevant if label horizontally overlaps the subgraph
                if cur_x + hw > sg.x && cur_x - hw < sg_right {
                    // Top border: label straddles the border line at sg.y
                    let label_top = cur_y - hh;
                    let label_bottom = cur_y + hh;
                    if label_top < sg.y && label_bottom > sg.y {
                        if cur_y < sg.y {
                            // Center above border → push label fully above
                            cur_y = sg.y - hh - clearance;
                        } else {
                            // Center below border → push label below title area
                            cur_y = title_bottom + hh + clearance;
                        }
                    } else if label_top >= sg.y && label_top < title_bottom && cur_y > sg.y {
                        // Label inside subgraph but overlapping title area → push below title
                        cur_y = title_bottom + hh + clearance;
                    }

                    // Bottom border: label straddles the border line at sg_bottom
                    let label_top = cur_y - hh;
                    let label_bottom = cur_y + hh;
                    if label_top < sg_bottom && label_bottom > sg_bottom {
                        if cur_y > sg_bottom {
                            cur_y = sg_bottom + hh + clearance;
                        } else {
                            cur_y = sg_bottom - hh - clearance;
                        }
                    }
                }

                // --- Vertical borders (left/right of subgraph) ---
                // Only relevant if label vertically overlaps the subgraph
                let label_top = cur_y - hh;
                let label_bottom = cur_y + hh;
                if label_top < sg_bottom && label_bottom > sg.y {
                    // Left border at sg.x
                    let label_left = cur_x - hw;
                    let label_right = cur_x + hw;
                    if label_left < sg.x && label_right > sg.x {
                        if cur_x < sg.x {
                            cur_x = sg.x - hw - clearance;
                        } else {
                            cur_x = sg.x + hw + clearance;
                        }
                    }

                    // Right border at sg_right
                    let label_left = cur_x - hw;
                    let label_right = cur_x + hw;
                    if label_left < sg_right && label_right > sg_right {
                        if cur_x > sg_right {
                            cur_x = sg_right + hw + clearance;
                        } else {
                            cur_x = sg_right - hw - clearance;
                        }
                    }
                }
            }

            if (cur_x - prev_x).abs() < 0.1 && (cur_y - prev_y).abs() < 0.1 {
                break;
            }
        }

        edge.label_x = Some(cur_x);
        edge.label_y = Some(cur_y);
    }
}

/// Route edges using dummy-node bend points for long edges and S-curve fallback
/// for short edges.
pub fn route_edges(
    positioned_nodes: &[PositionedNode],
    edges: &[EdgeDef],
    is_horizontal: bool,
    bend_points: &HashMap<(String, String), Vec<(f64, f64)>>,
    label_positions: &HashMap<(String, String), (f64, f64)>,
    label_dimensions: &HashMap<(String, String), (f64, f64)>,
) -> Vec<PositionedEdge> {
    let node_pos: HashMap<&str, &PositionedNode> = positioned_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    edges
        .iter()
        .filter_map(|edge| {
            let from = node_pos.get(edge.from.as_str())?;
            let to = node_pos.get(edge.to.as_str())?;

            let key = (edge.from.clone(), edge.to.clone());
            let points = if let Some(bps) = bend_points.get(&key) {
                // Long edge: use dummy-node positions as waypoints
                route_with_bend_points(from, to, bps, is_horizontal, positioned_nodes)
            } else {
                // Short edge: intersect_rect endpoints + S-curve if needed
                route_short_edge(from, to, positioned_nodes, is_horizontal)
            };

            // Use pre-computed label position from label dummy if available,
            // otherwise fall back to edge_label_anchor
            let (label_x, label_y, label_width, label_height) = if edge.label.is_some() {
                if let Some(&(lx, ly)) = label_positions.get(&key) {
                    let (lw, lh) = label_dimensions.get(&key).copied().unwrap_or((0.0, 0.0));
                    (Some(lx), Some(ly), Some(lw), Some(lh))
                } else {
                    let anchor = edge_label_anchor(&points);
                    (Some(anchor.0), Some(anchor.1), None, None)
                }
            } else {
                (None, None, None, None)
            };

            Some(PositionedEdge {
                from_id: edge.from.clone(),
                to_id: edge.to.clone(),
                edge_type: edge.edge_type,
                label: edge.label.clone(),
                label_x,
                label_y,
                label_width,
                label_height,
                points,
            })
        })
        .collect()
}

/// Route a long edge through its bend points (from dummy node positions).
/// Densifies segments with linearly-interpolated control points so the
/// B-spline (which only approximates interior control points) stays pinned
/// to the desired path — especially for straight vertical/horizontal runs.
/// After densification, enforces clearance from intermediate node boxes.
fn route_with_bend_points(
    from: &PositionedNode,
    to: &PositionedNode,
    bend_points: &[(f64, f64)],
    _is_horizontal: bool,
    _nodes: &[PositionedNode],
) -> Vec<(f64, f64)> {
    // First bend point direction determines exit angle from source
    let first_target = bend_points.first().copied().unwrap_or((to.x, to.y));
    let start = intersect_shape(from, first_target.0, first_target.1);

    // Last bend point direction determines entry angle into target
    let last_target = bend_points.last().copied().unwrap_or((from.x, from.y));
    let end = intersect_shape(to, last_target.0, last_target.1);

    // Build the raw waypoint sequence
    let mut raw = Vec::with_capacity(bend_points.len() + 2);
    raw.push(start);
    raw.extend_from_slice(bend_points);
    raw.push(end);

    // Densify: insert linearly-interpolated points between each consecutive
    // pair so that the B-spline has enough control points to stay on track.
    let max_gap = 30.0;
    let mut points = Vec::new();
    for pair in raw.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        let dx = x1 - x0;
        let dy = y1 - y0;
        let dist = (dx * dx + dy * dy).sqrt();
        let n = (dist / max_gap).ceil().max(1.0) as usize;
        for i in 0..n {
            let t = i as f64 / n as f64;
            points.push((x0 + dx * t, y0 + dy * t));
        }
    }
    // Push the final point
    if let Some(&last) = raw.last() {
        points.push(last);
    }

    points
}

/// Route a short (single-rank-span) edge with intersect_rect endpoints.
fn route_short_edge(
    from: &PositionedNode,
    to: &PositionedNode,
    nodes: &[PositionedNode],
    is_horizontal: bool,
) -> Vec<(f64, f64)> {
    let start = intersect_shape(from, to.x, to.y);
    let end = intersect_shape(to, from.x, from.y);

    let eps = 1e-6;

    // Straight line for axis-aligned edges
    let aligned = if is_horizontal {
        (start.1 - end.1).abs() < eps
    } else {
        (start.0 - end.0).abs() < eps
    };

    if aligned && path_avoids_nodes(&[start, end], &from.id, &to.id, nodes) {
        return vec![start, end];
    }

    // S-curve fallback for non-aligned short edges
    let step = 30.0;
    let (main_s, cross_s, main_e, cross_e) = if is_horizontal {
        (start.0, start.1, end.0, end.1)
    } else {
        (start.1, start.0, end.1, end.0)
    };

    let main_dist = (main_e - main_s).abs();
    let num_steps = (main_dist / step).ceil().max(6.0) as usize;

    let offsets = [0.0, 30.0, -30.0, 60.0, -60.0, 100.0, -100.0];

    for &off in &offsets {
        let points = build_smooth_waypoints(
            start,
            end,
            main_s,
            cross_s,
            main_e,
            cross_e,
            num_steps,
            off,
            is_horizontal,
        );

        if path_avoids_nodes(&points, &from.id, &to.id, nodes) {
            return points;
        }
    }

    // Last resort
    build_smooth_waypoints(
        start,
        end,
        main_s,
        cross_s,
        main_e,
        cross_e,
        num_steps,
        0.0,
        is_horizontal,
    )
}

/// Shape-aware intersection: finds where a ray from the node's center toward
/// (target_x, target_y) exits the node's actual shape boundary.
fn intersect_shape(node: &PositionedNode, target_x: f64, target_y: f64) -> (f64, f64) {
    let dx = target_x - node.x;
    let dy = target_y - node.y;

    // Degenerate case: target is at node center
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return (node.x, node.y + node.height / 2.0);
    }

    match node.shape {
        NodeShape::Circle | NodeShape::DoubleCircle => {
            intersect_circle(node.x, node.y, node.width / 2.0, target_x, target_y)
        }
        NodeShape::Diamond => intersect_diamond(
            node.x,
            node.y,
            node.width / 2.0,
            node.height / 2.0,
            target_x,
            target_y,
        ),
        _ => intersect_rect_impl(node.x, node.y, node.width / 2.0, node.height / 2.0, dx, dy),
    }
}

/// Circle intersection: normalize direction vector and scale by radius.
fn intersect_circle(cx: f64, cy: f64, r: f64, tx: f64, ty: f64) -> (f64, f64) {
    let dx = tx - cx;
    let dy = ty - cy;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return (cx, cy + r);
    }
    (cx + r * dx / len, cy + r * dy / len)
}

/// Diamond intersection: diamond boundary is |x/rx| + |y/ry| = 1.
/// Scale the direction vector so the point lies on this boundary.
fn intersect_diamond(cx: f64, cy: f64, rx: f64, ry: f64, tx: f64, ty: f64) -> (f64, f64) {
    let dx = tx - cx;
    let dy = ty - cy;
    // t satisfies |t*dx|/rx + |t*dy|/ry = 1
    let denom = dx.abs() / rx + dy.abs() / ry;
    if denom < 1e-9 {
        return (cx, cy + ry);
    }
    let t = 1.0 / denom;
    (cx + t * dx, cy + t * dy)
}

/// Rectangle intersection: ray from center exits the rectangular boundary.
fn intersect_rect_impl(cx: f64, cy: f64, hw: f64, hh: f64, dx: f64, dy: f64) -> (f64, f64) {
    let abs_dx = dx.abs();
    let abs_dy = dy.abs();

    let (sx, sy) = if abs_dy * hw > abs_dx * hh {
        let sy = if dy > 0.0 { hh } else { -hh };
        let sx = if abs_dy > 1e-9 { sy * dx / dy } else { 0.0 };
        (sx, sy)
    } else {
        let sx = if dx > 0.0 { hw } else { -hw };
        let sy = if abs_dx > 1e-9 { sx * dy / dx } else { 0.0 };
        (sx, sy)
    };

    (cx + sx, cy + sy)
}

fn edge_label_anchor(points: &[(f64, f64)]) -> (f64, f64) {
    if points.is_empty() {
        return (0.0, 0.0);
    }
    if points.len() == 1 {
        return points[0];
    }
    let mut best_len = -1.0;
    let mut best_mid = (
        (points[0].0 + points[1].0) / 2.0,
        (points[0].1 + points[1].1) / 2.0,
    );
    for w in points.windows(2) {
        let dx = w[1].0 - w[0].0;
        let dy = w[1].1 - w[0].1;
        let len = dx * dx + dy * dy;
        if len > best_len {
            best_len = len;
            best_mid = ((w[0].0 + w[1].0) / 2.0, (w[0].1 + w[1].1) / 2.0);
        }
    }
    best_mid
}

/// Build waypoints along a smooth curve between start and end.
#[allow(clippy::too_many_arguments)]
fn build_smooth_waypoints(
    start: (f64, f64),
    end: (f64, f64),
    main_s: f64,
    cross_s: f64,
    main_e: f64,
    cross_e: f64,
    num_steps: usize,
    offset: f64,
    is_horizontal: bool,
) -> Vec<(f64, f64)> {
    let mut points = Vec::with_capacity(num_steps + 1);
    points.push(start);

    let turn_frac = 0.25;

    for i in 1..num_steps {
        let t = i as f64 / num_steps as f64;
        let main = main_s + (main_e - main_s) * t;

        let cross = if t <= turn_frac {
            let u = t / turn_frac;
            let su = u * u * (3.0 - 2.0 * u);
            cross_s + (cross_e - cross_s) * su
        } else {
            cross_e
        } + offset * 4.0 * t * (1.0 - t);

        if is_horizontal {
            points.push((main, cross));
        } else {
            points.push((cross, main));
        }
    }

    points.push(end);
    points
}

fn path_avoids_nodes(
    points: &[(f64, f64)],
    from_id: &str,
    to_id: &str,
    nodes: &[PositionedNode],
) -> bool {
    for seg in points.windows(2) {
        for n in nodes {
            if n.id == from_id || n.id == to_id {
                continue;
            }
            let min_x = n.x - n.width / 2.0 - 4.0;
            let max_x = n.x + n.width / 2.0 + 4.0;
            let min_y = n.y - n.height / 2.0 - 4.0;
            let max_y = n.y + n.height / 2.0 + 4.0;
            if segment_intersects_rect(seg[0], seg[1], min_x, min_y, max_x, max_y) {
                return false;
            }
        }
    }
    true
}

fn segment_intersects_rect(
    a: (f64, f64),
    b: (f64, f64),
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) -> bool {
    let eps = 1e-6;
    if (a.1 - b.1).abs() < eps {
        let y = a.1;
        if y < min_y || y > max_y {
            return false;
        }
        let (x1, x2) = if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) };
        x2 > min_x && x1 < max_x
    } else if (a.0 - b.0).abs() < eps {
        let x = a.0;
        if x < min_x || x > max_x {
            return false;
        }
        let (y1, y2) = if a.1 <= b.1 { (a.1, b.1) } else { (b.1, a.1) };
        y2 > min_y && y1 < max_y
    } else {
        let seg_min_x = a.0.min(b.0);
        let seg_max_x = a.0.max(b.0);
        let seg_min_y = a.1.min(b.1);
        let seg_max_y = a.1.max(b.1);
        seg_max_x > min_x && seg_min_x < max_x && seg_max_y > min_y && seg_min_y < max_y
    }
}
