use crate::ast::flowchart::EdgeDef;
use crate::layout::types::*;
use std::collections::HashMap;

/// Route edges using dummy-node bend points for long edges and S-curve fallback
/// for short edges.
pub fn route_edges(
    positioned_nodes: &[PositionedNode],
    edges: &[EdgeDef],
    is_horizontal: bool,
    bend_points: &HashMap<(String, String), Vec<(f64, f64)>>,
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

            let label_anchor = edge_label_anchor(&points);

            Some(PositionedEdge {
                from_id: edge.from.clone(),
                to_id: edge.to.clone(),
                edge_type: edge.edge_type,
                label: edge.label.clone(),
                label_x: edge.label.as_ref().map(|_| label_anchor.0),
                label_y: edge.label.as_ref().map(|_| label_anchor.1),
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
    nodes: &[PositionedNode],
) -> Vec<(f64, f64)> {
    // First bend point direction determines exit angle from source
    let first_target = bend_points.first().copied().unwrap_or((to.x, to.y));
    let start = intersect_rect(from, first_target.0, first_target.1);

    // Last bend point direction determines entry angle into target
    let last_target = bend_points.last().copied().unwrap_or((from.x, from.y));
    let end = intersect_rect(to, last_target.0, last_target.1);

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

    // Enforce clearance: push any control points that are inside or too close
    // to intermediate node boxes away to the nearest edge + padding.
    let pad = 6.0;
    for pt in points.iter_mut() {
        for n in nodes {
            if n.id == from.id || n.id == to.id {
                continue;
            }
            let min_x = n.x - n.width / 2.0 - pad;
            let max_x = n.x + n.width / 2.0 + pad;
            let min_y = n.y - n.height / 2.0 - pad;
            let max_y = n.y + n.height / 2.0 + pad;

            if pt.0 > min_x && pt.0 < max_x && pt.1 > min_y && pt.1 < max_y {
                // Point is inside the padded box — push to nearest edge
                let d_left = pt.0 - min_x;
                let d_right = max_x - pt.0;
                let d_top = pt.1 - min_y;
                let d_bottom = max_y - pt.1;
                let min_d = d_left.min(d_right).min(d_top).min(d_bottom);
                if min_d == d_left {
                    pt.0 = min_x;
                } else if min_d == d_right {
                    pt.0 = max_x;
                } else if min_d == d_top {
                    pt.1 = min_y;
                } else {
                    pt.1 = max_y;
                }
            }
        }
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
    let start = intersect_rect(from, to.x, to.y);
    let end = intersect_rect(to, from.x, from.y);

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
            start, end, main_s, cross_s, main_e, cross_e, num_steps, off, is_horizontal,
        );

        if path_avoids_nodes(&points, &from.id, &to.id, nodes) {
            return points;
        }
    }

    // Last resort
    build_smooth_waypoints(
        start, end, main_s, cross_s, main_e, cross_e, num_steps, 0.0, is_horizontal,
    )
}

/// Dagre-style ray-rect intersection: finds where a ray from the rectangle's
/// center toward (target_x, target_y) exits the rectangle boundary.
fn intersect_rect(node: &PositionedNode, target_x: f64, target_y: f64) -> (f64, f64) {
    let hw = node.width / 2.0;
    let hh = node.height / 2.0;
    let dx = target_x - node.x;
    let dy = target_y - node.y;

    // Degenerate case: target is at node center
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return (node.x, node.y + hh);
    }

    let abs_dx = dx.abs();
    let abs_dy = dy.abs();

    // Determine which edge the ray hits first
    let (sx, sy) = if abs_dy * hw > abs_dx * hh {
        // Hits top or bottom edge
        let sy = if dy > 0.0 { hh } else { -hh };
        let sx = if abs_dy > 1e-9 { sy * dx / dy } else { 0.0 };
        (sx, sy)
    } else {
        // Hits left or right edge
        let sx = if dx > 0.0 { hw } else { -hw };
        let sy = if abs_dx > 1e-9 { sx * dy / dx } else { 0.0 };
        (sx, sy)
    };

    (node.x + sx, node.y + sy)
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
