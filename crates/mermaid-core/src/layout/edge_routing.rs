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

    // Only use a direct line when source and target are axis-aligned
    // (the line is already perpendicular to both node faces).
    let aligned = if is_horizontal {
        (start.1 - end.1).abs() < eps
    } else {
        (start.0 - end.0).abs() < eps
    };

    if aligned && path_avoids_nodes(&[start, end], from_id, to_id, nodes) {
        return vec![start, end];
    }

    // Z-routes: always perpendicular at both source and target.
    // For TB: vertical → horizontal → vertical
    // For LR: horizontal → vertical → horizontal
    let offsets = [0.0, 30.0, -30.0, 60.0, -60.0, 100.0, -100.0, 150.0, -150.0, 200.0, -200.0];

    for off in offsets {
        let points = if is_horizontal {
            let mid_x = (start.0 + end.0) / 2.0 + off;
            vec![start, (mid_x, start.1), (mid_x, end.1), end]
        } else {
            let mid_y = (start.1 + end.1) / 2.0 + off;
            vec![start, (start.0, mid_y), (end.0, mid_y), end]
        };

        if path_avoids_nodes(&points, from_id, to_id, nodes) {
            return dedupe_adjacent_points(points);
        }
    }

    // Last resort: Z-route at midpoint without collision check
    let points = if is_horizontal {
        let mid_x = (start.0 + end.0) / 2.0;
        vec![start, (mid_x, start.1), (mid_x, end.1), end]
    } else {
        let mid_y = (start.1 + end.1) / 2.0;
        vec![start, (start.0, mid_y), (end.0, mid_y), end]
    };
    dedupe_adjacent_points(points)
}

fn dedupe_adjacent_points(points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(points.len());
    for p in points {
        if out
            .last()
            .map(|q| (q.0 - p.0).abs() < 1e-6 && (q.1 - p.1).abs() < 1e-6)
            .unwrap_or(false)
        {
            continue;
        }
        out.push(p);
    }
    out
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
