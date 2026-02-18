use crate::ast::flowchart::EdgeDef;
use crate::layout::types::*;
use std::collections::HashMap;

/// Route edges with orthogonal segments and basic node-collision avoidance.
pub fn route_edges(
    positioned_nodes: &[PositionedNode],
    edges: &[EdgeDef],
    is_horizontal: bool,
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

            let (from_x, from_y) = preferred_port_point(from, to.x, to.y, is_horizontal);
            let (to_x, to_y) = preferred_port_point(to, from.x, from.y, is_horizontal);

            let points = route_orthogonal_with_avoidance(
                from.id.as_str(),
                to.id.as_str(),
                (from_x, from_y),
                (to_x, to_y),
                positioned_nodes,
                is_horizontal,
            );

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

fn preferred_port_point(
    node: &PositionedNode,
    target_x: f64,
    target_y: f64,
    is_horizontal: bool,
) -> (f64, f64) {
    let hw = node.width / 2.0;
    let hh = node.height / 2.0;

    if is_horizontal {
        if target_x >= node.x {
            (node.x + hw, node.y)
        } else {
            (node.x - hw, node.y)
        }
    } else {
        if target_y >= node.y {
            (node.x, node.y + hh)
        } else {
            (node.x, node.y - hh)
        }
    }
}

fn route_orthogonal_with_avoidance(
    from_id: &str,
    to_id: &str,
    start: (f64, f64),
    end: (f64, f64),
    nodes: &[PositionedNode],
    is_horizontal: bool,
) -> Vec<(f64, f64)> {
    let eps = 1e-6;

    // Straight line for axis-aligned edges
    let aligned = if is_horizontal {
        (start.1 - end.1).abs() < eps
    } else {
        (start.0 - end.0).abs() < eps
    };

    if aligned && path_avoids_nodes(&[start, end], from_id, to_id, nodes) {
        return vec![start, end];
    }

    // Generate smooth curve with many intermediate waypoints.
    // Uses smoothstep on the cross-axis so the edge departs/arrives
    // perpendicular to the node face, with a gradual S-curve in between.
    // Many closely-spaced points keep the B-spline tight (like mermaid/dagre).
    let step = 30.0;

    let (main_s, cross_s, main_e, cross_e) = if is_horizontal {
        (start.0, start.1, end.0, end.1)
    } else {
        (start.1, start.0, end.1, end.0)
    };

    let main_dist = (main_e - main_s).abs();
    let num_steps = (main_dist / step).ceil().max(6.0) as usize;

    // Try smooth curve with optional cross-axis offset for avoidance
    let offsets = [0.0, 30.0, -30.0, 60.0, -60.0, 100.0, -100.0];

    for &off in &offsets {
        let points = build_smooth_waypoints(
            start, end, main_s, cross_s, main_e, cross_e, num_steps, off, is_horizontal,
        );

        if path_avoids_nodes(&points, from_id, to_id, nodes) {
            return points;
        }
    }

    // Last resort: smooth curve without avoidance check
    build_smooth_waypoints(
        start, end, main_s, cross_s, main_e, cross_e, num_steps, 0.0, is_horizontal,
    )
}

/// Build waypoints along a smooth curve between start and end.
/// The edge turns quickly to the target's cross-axis position near the
/// start, then goes straight down to the target — matching how dagre's
/// dummy nodes settle near the target x through barycenter ordering.
/// `offset` adds a parabolic bulge to the cross-axis for node avoidance.
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

    // Turn from source_x to target_x in the first ~25% of the path,
    // then go straight at target_x for the remaining ~75%.
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
