use crate::ast::flowchart::{EdgeDef, EdgeSide, NodeShape};
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

/// Route edges using dummy-node bend points for long edges and direct segments
/// for short edges.
pub fn route_edges(
    positioned_nodes: &[PositionedNode],
    edges: &[EdgeDef],
    is_horizontal: bool,
    raw_points: &HashMap<(String, String), Vec<(f64, f64)>>,
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
            let points = if is_rect_like(from.shape) && is_rect_like(to.shape) {
                if let Some(raw) = raw_points.get(&key) {
                    raw.clone()
                } else {
                    route_short_edge(from, to, positioned_nodes, is_horizontal, None, None)
                }
            } else if let Some(bps) = bend_points.get(&key) {
                // Long edge: use dummy-node positions as waypoints
                route_with_bend_points(from, to, bps, is_horizontal, positioned_nodes, None, None)
            } else {
                // Short edge: direct segment between node-boundary intersections.
                route_short_edge(from, to, positioned_nodes, is_horizontal, None, None)
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
                line_style: edge.line_style,
                arrow_start: edge.arrow_start,
                arrow_end: edge.arrow_end,
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

fn is_rect_like(shape: NodeShape) -> bool {
    matches!(
        shape,
        NodeShape::Rectangle
            | NodeShape::RoundedRectangle
            | NodeShape::Stadium
            | NodeShape::Subroutine
            | NodeShape::Cylinder
            | NodeShape::Asymmetric
            | NodeShape::Parallelogram
            | NodeShape::ParallelogramAlt
            | NodeShape::Trapezoid
            | NodeShape::TrapezoidAlt
    )
}

/// Route a long edge through its bend points (from dummy node positions).
/// Computes endpoint intersections with node shapes and passes the raw
/// control points to the B-spline curve generator for smooth rendering.
fn route_with_bend_points(
    from: &PositionedNode,
    to: &PositionedNode,
    bend_points: &[(f64, f64)],
    _is_horizontal: bool,
    _nodes: &[PositionedNode],
    from_side: Option<EdgeSide>,
    to_side: Option<EdgeSide>,
) -> Vec<(f64, f64)> {
    // First bend point direction determines exit angle from source
    let first_target = bend_points.first().copied().unwrap_or((to.x, to.y));
    let start = from_side
        .map(|s| intersect_shape_with_fixed_side(from, s))
        .unwrap_or_else(|| intersect_shape(from, first_target.0, first_target.1));

    // Last bend point direction determines entry angle into target
    let last_target = bend_points.last().copied().unwrap_or((from.x, from.y));
    let end = to_side
        .map(|s| intersect_shape_with_fixed_side(to, s))
        .unwrap_or_else(|| intersect_shape(to, last_target.0, last_target.1));

    // Build the waypoint sequence: start + bend points + end.
    // The B-spline curve generator handles smoothing, so we pass
    // only the raw control points — no densification needed.
    let mut points = Vec::with_capacity(bend_points.len() + 2);
    points.push(start);
    points.extend_from_slice(bend_points);
    points.push(end);

    points
}

/// Route a short (single-rank-span) edge with intersect_rect endpoints.
fn route_short_edge(
    from: &PositionedNode,
    to: &PositionedNode,
    _nodes: &[PositionedNode],
    _is_horizontal: bool,
    from_side: Option<EdgeSide>,
    to_side: Option<EdgeSide>,
) -> Vec<(f64, f64)> {
    let start = from_side
        .map(|s| intersect_shape_with_fixed_side(from, s))
        .unwrap_or_else(|| intersect_shape(from, to.x, to.y));

    // When the source is inside the target (e.g. node inside a subgraph),
    // the standard center-to-center intersection exits the target on the
    // wrong side (toward the source). Instead, intersect outward: from
    // the target center in the same direction as source→target.
    let end = to_side
        .map(|s| intersect_shape_with_fixed_side(to, s))
        .unwrap_or_else(|| {
            if node_contains(to, from.x, from.y) {
                // Source inside target: exit target on the far side
                let far_x = to.x + (to.x - from.x);
                let far_y = to.y + (to.y - from.y);
                intersect_shape(to, far_x, far_y)
            } else {
                intersect_shape(to, from.x, from.y)
            }
        });
    vec![start, end]
}

/// Check if a point (px, py) is inside a node's bounding box.
fn node_contains(node: &PositionedNode, px: f64, py: f64) -> bool {
    let hw = node.width / 2.0;
    let hh = node.height / 2.0;
    px >= node.x - hw && px <= node.x + hw && py >= node.y - hh && py <= node.y + hh
}

fn intersect_shape_with_fixed_side(node: &PositionedNode, side: EdgeSide) -> (f64, f64) {
    let hw = node.width / 2.0;
    let hh = node.height / 2.0;
    match side {
        EdgeSide::Top => (node.x, node.y - hh),
        EdgeSide::Bottom => (node.x, node.y + hh),
        EdgeSide::Left => (node.x - hw, node.y),
        EdgeSide::Right => (node.x + hw, node.y),
    }
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
#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use crate::ast::flowchart::EdgeSide;

    use super::*;
    use crate::ast::flowchart::{ArrowEnd, EdgeDef, LineStyle, NodeShape};

    fn make_node(id: &str, x: f64, y: f64, w: f64, h: f64, shape: NodeShape) -> PositionedNode {
        PositionedNode {
            id: id.to_string(),
            label: id.to_string(),
            shape,
            style: Default::default(),
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn make_rect_node(id: &str, x: f64, y: f64) -> PositionedNode {
        make_node(id, x, y, 80.0, 40.0, NodeShape::Rectangle)
    }

    // -----------------------------------------------------------------------
    // intersect_shape tests (via route_edges / route_short_edge)
    // -----------------------------------------------------------------------

    #[test]
    fn test_intersect_diamond_from_above() {
        let node = make_node("D", 200.0, 200.0, 100.0, 100.0, NodeShape::Diamond);
        // Ray from center upward (target above)
        let (ix, iy) = intersect_shape(&node, 200.0, 0.0);
        // Diamond top vertex is at (cx, cy - ry) = (200, 150)
        assert!((ix - 200.0).abs() < 1.0, "x should be ~200, got {ix}");
        assert!((iy - 150.0).abs() < 1.0, "y should be ~150, got {iy}");
    }

    #[test]
    fn test_intersect_diamond_from_below() {
        let node = make_node("D", 200.0, 200.0, 100.0, 100.0, NodeShape::Diamond);
        let (ix, iy) = intersect_shape(&node, 200.0, 400.0);
        // Bottom vertex at (200, 250)
        assert!((ix - 200.0).abs() < 1.0, "x should be ~200, got {ix}");
        assert!((iy - 250.0).abs() < 1.0, "y should be ~250, got {iy}");
    }

    #[test]
    fn test_intersect_diamond_from_left() {
        let node = make_node("D", 200.0, 200.0, 100.0, 100.0, NodeShape::Diamond);
        let (ix, iy) = intersect_shape(&node, 0.0, 200.0);
        // Left vertex at (150, 200)
        assert!((ix - 150.0).abs() < 1.0, "x should be ~150, got {ix}");
        assert!((iy - 200.0).abs() < 1.0, "y should be ~200, got {iy}");
    }

    #[test]
    fn test_intersect_diamond_from_right() {
        let node = make_node("D", 200.0, 200.0, 100.0, 100.0, NodeShape::Diamond);
        let (ix, iy) = intersect_shape(&node, 400.0, 200.0);
        // Right vertex at (250, 200)
        assert!((ix - 250.0).abs() < 1.0, "x should be ~250, got {ix}");
        assert!((iy - 200.0).abs() < 1.0, "y should be ~200, got {iy}");
    }

    #[test]
    fn test_intersect_diamond_diagonal() {
        let node = make_node("D", 200.0, 200.0, 100.0, 100.0, NodeShape::Diamond);
        // Target at 45 degrees (upper-right)
        let (ix, iy) = intersect_shape(&node, 400.0, 0.0);
        // Diamond boundary: |dx|/rx + |dy|/ry = 1
        // rx=50, ry=50, direction = (200, -200), normalized via diamond formula
        let dx = ix - 200.0;
        let dy = iy - 200.0;
        let boundary = dx.abs() / 50.0 + dy.abs() / 50.0;
        assert!(
            (boundary - 1.0).abs() < 0.01,
            "point ({ix},{iy}) should lie on diamond boundary, |dx|/rx + |dy|/ry = {boundary}"
        );
    }

    #[test]
    fn test_intersect_diamond_degenerate_center() {
        let node = make_node("D", 200.0, 200.0, 100.0, 100.0, NodeShape::Diamond);
        // Target at center — degenerate case
        let (ix, iy) = intersect_shape(&node, 200.0, 200.0);
        // Should return (cx, cy + height/2) as fallback
        assert!((ix - 200.0).abs() < 1.0);
        assert!((iy - 250.0).abs() < 1.0);
    }

    #[test]
    fn test_intersect_circle() {
        let node = make_node("C", 100.0, 100.0, 60.0, 60.0, NodeShape::Circle);
        // Radius = 30; target to the right
        let (ix, iy) = intersect_shape(&node, 200.0, 100.0);
        assert!((ix - 130.0).abs() < 1.0, "x should be ~130, got {ix}");
        assert!((iy - 100.0).abs() < 1.0, "y should be ~100, got {iy}");
    }

    #[test]
    fn test_intersect_circle_degenerate() {
        let node = make_node("C", 100.0, 100.0, 60.0, 60.0, NodeShape::Circle);
        let (ix, iy) = intersect_shape(&node, 100.0, 100.0);
        assert!((ix - 100.0).abs() < 1.0);
        assert!((iy - 130.0).abs() < 1.0);
    }

    #[test]
    fn test_intersect_rect() {
        let node = make_rect_node("R", 100.0, 100.0);
        // Target to the right; hw=40, hh=20
        let (ix, iy) = intersect_shape(&node, 300.0, 100.0);
        assert!((ix - 140.0).abs() < 1.0, "x should be ~140, got {ix}");
        assert!((iy - 100.0).abs() < 1.0, "y should be ~100, got {iy}");
    }

    #[test]
    fn test_intersect_rect_exit_top() {
        // abs_dy*hw > abs_dx*hh, dy < 0 -> exit top (sy = -hh)
        let node = make_rect_node("R", 100.0, 100.0);
        let (_ix, iy) = intersect_shape(&node, 100.0, 0.0);
        assert!((iy - 80.0).abs() < 1.0, "should exit top, y ~80, got {iy}");
    }

    #[test]
    fn test_intersect_rect_exit_bottom() {
        // abs_dy*hw > abs_dx*hh, dy > 0 -> exit bottom (sy = hh)
        let node = make_rect_node("R", 100.0, 100.0);
        let (_ix, iy) = intersect_shape(&node, 100.0, 200.0);
        assert!(
            (iy - 120.0).abs() < 1.0,
            "should exit bottom, y ~120, got {iy}"
        );
    }

    #[test]
    fn test_intersect_rect_exit_left() {
        // else branch, dx < 0 -> exit left (sx = -hw)
        let node = make_rect_node("R", 100.0, 100.0);
        let (ix, _iy) = intersect_shape(&node, 0.0, 100.0);
        assert!((ix - 60.0).abs() < 1.0, "should exit left, x ~60, got {ix}");
    }

    // -----------------------------------------------------------------------
    // route_short_edge (direct short-edge routing)
    // -----------------------------------------------------------------------

    #[test]
    fn test_route_short_edge_aligned_vertical() {
        // Two nodes vertically aligned -> straight line
        let from = make_rect_node("A", 100.0, 50.0);
        let to = make_rect_node("B", 100.0, 200.0);
        let nodes = vec![from.clone(), to.clone()];

        let points = route_short_edge(&from, &to, &nodes, false, None, None);
        // Should be a simple 2-point line
        assert_eq!(points.len(), 2, "axis-aligned should produce 2 points");
        assert!(
            (points[0].0 - points[1].0).abs() < 1.0,
            "x coords should be nearly identical"
        );
    }

    #[test]
    fn test_route_short_edge_non_aligned_vertical() {
        // Two nodes NOT vertically aligned but unobstructed -> direct segment
        let from = make_rect_node("A", 100.0, 50.0);
        let to = make_rect_node("B", 250.0, 200.0);
        let nodes = vec![from.clone(), to.clone()];

        let points = route_short_edge(&from, &to, &nodes, false, None, None);
        assert_eq!(
            points.len(),
            2,
            "non-aligned unobstructed edge should be direct, got {} points",
            points.len()
        );
    }

    #[test]
    fn test_route_short_edge_non_aligned_horizontal() {
        // Horizontal layout with non-aligned nodes
        let from = make_rect_node("A", 50.0, 100.0);
        let to = make_rect_node("B", 200.0, 250.0);
        let nodes = vec![from.clone(), to.clone()];

        let points = route_short_edge(&from, &to, &nodes, true, None, None);
        assert_eq!(
            points.len(),
            2,
            "non-aligned unobstructed horizontal edge should be direct, got {} points",
            points.len()
        );
    }

    #[test]
    fn test_route_short_edge_avoids_intermediate_node() {
        // Mermaid parity: short edges stay direct even with intermediate nodes.
        let from = make_rect_node("A", 100.0, 50.0);
        let to = make_rect_node("B", 100.0, 250.0);
        let blocker = make_rect_node("C", 100.0, 150.0);
        let nodes = vec![from.clone(), to.clone(), blocker];

        let points = route_short_edge(&from, &to, &nodes, false, None, None);
        assert_eq!(
            points.len(),
            2,
            "short edge should remain direct for mermaid parity, got {} points",
            points.len()
        );
    }

    #[test]
    fn test_route_short_edge_last_resort() {
        // Dense blockers do not affect short-edge direct routing.
        let from = make_rect_node("A", 50.0, 50.0);
        let to = make_rect_node("B", 250.0, 250.0);
        let blockers: Vec<PositionedNode> = (0..20)
            .flat_map(|i| {
                (0..20).map(move |j| {
                    make_rect_node(
                        &format!("b{i}_{j}"),
                        70.0 + i as f64 * 10.0,
                        70.0 + j as f64 * 10.0,
                    )
                })
            })
            .collect();
        let mut nodes = vec![from.clone(), to.clone()];
        nodes.extend(blockers);

        let points = route_short_edge(&from, &to, &nodes, false, None, None);
        assert_eq!(points.len(), 2);
    }

    #[test]
    fn test_route_short_edge_honors_fixed_sides() {
        let from = make_rect_node("A", 100.0, 100.0); // half-width=40, half-height=20
        let to = make_rect_node("B", 300.0, 100.0);
        let nodes = vec![from.clone(), to.clone()];

        let points = route_short_edge(
            &from,
            &to,
            &nodes,
            false,
            Some(EdgeSide::Right),
            Some(EdgeSide::Left),
        );
        assert_eq!(points.len(), 2);
        assert!((points[0].0 - (from.x + from.width / 2.0)).abs() < 1e-6);
        assert!((points[0].1 - from.y).abs() < 1e-6);
        assert!((points[1].0 - (to.x - to.width / 2.0)).abs() < 1e-6);
        assert!((points[1].1 - to.y).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // adjust_labels_for_subgraph_boundaries
    // -----------------------------------------------------------------------

    #[test]
    fn test_adjust_labels_no_subgraphs() {
        // No subgraphs -> labels unchanged
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(100.0),
            label_y: Some(100.0),
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(50.0, 50.0), (150.0, 150.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[]);
        assert!((edges[0].label_x.unwrap() - 100.0).abs() < 0.1);
        assert!((edges[0].label_y.unwrap() - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_adjust_labels_straddling_top_border() {
        // Label center is just below the subgraph top border, but label top is above it
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 50.0,
            y: 100.0, // top border at y=100
            width: 200.0,
            height: 200.0,
            style: Default::default(),
        };

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(150.0), // horizontally inside subgraph
            label_y: Some(105.0), // center below border, but label_top = 105 - 10 = 95 < 100
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(50.0, 50.0), (250.0, 150.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_y = edges[0].label_y.unwrap();
        // Label center was below border -> pushed below title area
        let title_bottom = 100.0 + SUBGRAPH_TITLE_HEIGHT + SUBGRAPH_PADDING;
        assert!(
            new_y > title_bottom,
            "label should be pushed below title area (title_bottom={title_bottom}, got y={new_y})"
        );
    }

    #[test]
    fn test_adjust_labels_straddling_bottom_border() {
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 50.0,
            y: 100.0,
            width: 200.0,
            height: 200.0, // bottom border at y=300
            style: Default::default(),
        };

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(150.0),
            label_y: Some(305.0), // center below border; label_top=295 < 300, label_bottom=315 > 300
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(50.0, 50.0), (250.0, 350.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_y = edges[0].label_y.unwrap();
        // Center below border (cur_y > sg_bottom) -> should be pushed fully below
        assert!(
            new_y > 300.0 + 10.0,
            "label should be pushed below bottom border, got y={new_y}"
        );
    }

    #[test]
    fn test_adjust_labels_straddling_bottom_border_center_above() {
        // Label straddles bottom border with center ABOVE it (cur_y <= sg_bottom)
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 50.0,
            y: 100.0,
            width: 200.0,
            height: 200.0, // bottom border at y=300
            style: Default::default(),
        };

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(150.0),
            label_y: Some(295.0), // center above border; label_bottom=305 > 300
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(150.0, 250.0), (150.0, 350.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_y = edges[0].label_y.unwrap();
        // Center above -> pushed inside (sg_bottom - hh - clearance)
        assert!(
            new_y < 300.0,
            "label center above border should be pushed inside, got y={new_y}"
        );
    }

    #[test]
    fn test_adjust_labels_straddling_left_border_center_left() {
        // Label straddles left border with center LEFT of it (cur_x < sg.x)
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 100.0,
            y: 50.0,
            width: 200.0,
            height: 200.0,
            style: Default::default(),
        };

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(95.0), // center left of border; label_right=115 > 100
            label_y: Some(150.0),
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(50.0, 150.0), (200.0, 150.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_x = edges[0].label_x.unwrap();
        assert!(
            new_x < 100.0 - 15.0,
            "label center left of border should be pushed further left, got x={new_x}"
        );
    }

    #[test]
    fn test_adjust_labels_straddling_right_border_center_right() {
        // Label straddles right border with center RIGHT of it (cur_x > sg_right)
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 100.0,
            y: 50.0,
            width: 200.0, // right border at x=300
            height: 200.0,
            style: Default::default(),
        };

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(305.0), // center right of border; label_left=285 < 300
            label_y: Some(150.0),
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(200.0, 150.0), (400.0, 150.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_x = edges[0].label_x.unwrap();
        assert!(
            new_x > 300.0 + 15.0,
            "label center right of border should be pushed further right, got x={new_x}"
        );
    }

    #[test]
    fn test_adjust_labels_straddling_left_border() {
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 100.0, // left border at x=100
            y: 50.0,
            width: 200.0,
            height: 200.0,
            style: Default::default(),
        };

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(105.0), // center right of border; label_left=85 < 100, label_right=125 > 100
            label_y: Some(150.0), // vertically inside subgraph
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(50.0, 150.0), (200.0, 150.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_x = edges[0].label_x.unwrap();
        // Center is right of border -> pushed further right
        assert!(
            new_x > 100.0 + 20.0,
            "label should be pushed right of left border, got x={new_x}"
        );
    }

    #[test]
    fn test_adjust_labels_straddling_right_border() {
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 100.0,
            y: 50.0,
            width: 200.0, // right border at x=300
            height: 200.0,
            style: Default::default(),
        };

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(295.0), // label_left=275 < 300, label_right=315 > 300
            label_y: Some(150.0),
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(200.0, 150.0), (400.0, 150.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_x = edges[0].label_x.unwrap();
        // Center is left of right border -> pushed left
        assert!(
            new_x < 300.0 - 15.0,
            "label should be pushed left of right border, got x={new_x}"
        );
    }

    #[test]
    fn test_adjust_labels_title_area_overlap() {
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 50.0,
            y: 100.0,
            width: 200.0,
            height: 200.0,
            style: Default::default(),
        };
        let title_bottom = 100.0 + SUBGRAPH_TITLE_HEIGHT + SUBGRAPH_PADDING;

        // Label inside subgraph but overlapping the title area
        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(150.0),
            label_y: Some(title_bottom - 5.0), // label_top inside title area
            label_width: Some(40.0),
            label_height: Some(20.0),
            points: vec![(50.0, 50.0), (250.0, 250.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        let new_y = edges[0].label_y.unwrap();
        assert!(
            new_y >= title_bottom,
            "label should be pushed below title area (title_bottom={title_bottom}, got y={new_y})"
        );
    }

    #[test]
    fn test_adjust_labels_no_label() {
        // Edge with no label dimensions -> skip
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 50.0,
            y: 100.0,
            width: 200.0,
            height: 200.0,
            style: Default::default(),
        };

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![(50.0, 50.0), (250.0, 250.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        assert!(edges[0].label_x.is_none());
        assert!(edges[0].label_y.is_none());
    }

    #[test]
    fn test_adjust_labels_zero_size_label() {
        // Edge with zero-size label -> skip
        let sg = PositionedSubgraph {
            id: "sg1".into(),
            label: Some("SG".into()),
            x: 50.0,
            y: 100.0,
            width: 200.0,
            height: 200.0,
            style: Default::default(),
        };

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("tiny".into()),
            label_x: Some(150.0),
            label_y: Some(100.0),
            label_width: Some(0.5), // < 1.0
            label_height: Some(0.5),
            points: vec![(50.0, 50.0), (250.0, 250.0)],
        }];

        adjust_labels_for_subgraph_boundaries(&mut edges, &[sg]);
        // Should be unchanged because label width/height are < 1.0
        assert!((edges[0].label_x.unwrap() - 150.0).abs() < 0.1);
        assert!((edges[0].label_y.unwrap() - 100.0).abs() < 0.1);
    }

    // -----------------------------------------------------------------------
    // route_edges with bend points (long edge routing)
    // -----------------------------------------------------------------------

    #[test]
    fn test_route_with_bend_points() {
        let from = make_rect_node("A", 100.0, 50.0);
        let to = make_rect_node("B", 100.0, 350.0);
        let nodes = vec![from.clone(), to.clone()];

        let bps = vec![(100.0, 150.0), (100.0, 250.0)];
        let points = route_with_bend_points(&from, &to, &bps, false, &nodes, None, None);

        // Should start near from and end near to, passing through bend points
        assert!(points.len() > 2, "should have more than 2 points");
        // First point should be near the edge of from (y should be ~ from.y + hh)
        assert!(
            (points[0].0 - 100.0).abs() < 5.0,
            "start x should be near from.x"
        );
        // Last point should be near the edge of to
        let last = points.last().unwrap();
        assert!((last.0 - 100.0).abs() < 5.0, "end x should be near to.x");
    }

    #[test]
    fn test_route_with_bend_points_honors_fixed_sides() {
        let from = make_rect_node("A", 100.0, 100.0); // hh=20
        let to = make_rect_node("B", 100.0, 300.0); // hh=20
        let nodes = vec![from.clone(), to.clone()];
        let bps = vec![(120.0, 180.0), (120.0, 220.0)];

        let points = route_with_bend_points(
            &from,
            &to,
            &bps,
            false,
            &nodes,
            Some(EdgeSide::Bottom),
            Some(EdgeSide::Top),
        );
        assert!(!points.is_empty());
        let start = points.first().copied().unwrap();
        let end = points.last().copied().unwrap();
        assert!((start.0 - from.x).abs() < 1e-6);
        assert!((start.1 - (from.y + from.height / 2.0)).abs() < 1e-6);
        assert!((end.0 - to.x).abs() < 1e-6);
        assert!((end.1 - (to.y - to.height / 2.0)).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // route_edges integration test
    // -----------------------------------------------------------------------

    #[test]
    fn test_route_edges_with_label_positions() {
        let from = make_rect_node("A", 100.0, 50.0);
        let to = make_rect_node("B", 100.0, 200.0);
        let nodes = vec![from.clone(), to.clone()];

        let edges = vec![EdgeDef {
            from: "A".into(),
            to: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("yes".into()),
            from_side: None,
            to_side: None,
        }];

        let mut label_positions = HashMap::new();
        label_positions.insert(("A".to_string(), "B".to_string()), (100.0, 125.0));
        let mut label_dimensions = HashMap::new();
        label_dimensions.insert(("A".to_string(), "B".to_string()), (30.0, 15.0));

        let result = route_edges(
            &nodes,
            &edges,
            false,
            &HashMap::new(),
            &HashMap::new(),
            &label_positions,
            &label_dimensions,
        );
        assert_eq!(result.len(), 1);
        assert!(result[0].label_x.is_some());
        assert!(result[0].label_y.is_some());
        assert!((result[0].label_x.unwrap() - 100.0).abs() < 0.1);
        assert!((result[0].label_y.unwrap() - 125.0).abs() < 0.1);
        assert!((result[0].label_width.unwrap() - 30.0).abs() < 0.1);
        assert!((result[0].label_height.unwrap() - 15.0).abs() < 0.1);
    }

    #[test]
    fn test_route_edges_label_fallback_to_anchor() {
        let from = make_rect_node("A", 100.0, 50.0);
        let to = make_rect_node("B", 100.0, 200.0);
        let nodes = vec![from.clone(), to.clone()];

        let edges = vec![EdgeDef {
            from: "A".into(),
            to: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("yes".into()),
            from_side: None,
            to_side: None,
        }];

        // No label_positions provided -> should fall back to edge_label_anchor
        let result = route_edges(
            &nodes,
            &edges,
            false,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(result.len(), 1);
        assert!(result[0].label_x.is_some());
        assert!(result[0].label_y.is_some());
        // Anchor is midpoint of longest segment
        assert!(result[0].label_width.is_none());
        assert!(result[0].label_height.is_none());
    }

    #[test]
    fn test_route_edges_no_label() {
        let from = make_rect_node("A", 100.0, 50.0);
        let to = make_rect_node("B", 100.0, 200.0);
        let nodes = vec![from.clone(), to.clone()];

        let edges = vec![EdgeDef {
            from: "A".into(),
            to: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            from_side: None,
            to_side: None,
        }];

        let result = route_edges(
            &nodes,
            &edges,
            false,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(result.len(), 1);
        assert!(result[0].label_x.is_none());
        assert!(result[0].label_y.is_none());
    }

    #[test]
    fn test_route_edges_missing_node() {
        // Edge references a node that doesn't exist -> filtered out
        let from = make_rect_node("A", 100.0, 50.0);
        let nodes = vec![from.clone()];

        let edges = vec![EdgeDef {
            from: "A".into(),
            to: "NONEXISTENT".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            from_side: None,
            to_side: None,
        }];

        let result = route_edges(
            &nodes,
            &edges,
            false,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(
            result.len(),
            0,
            "edge with missing node should be filtered out"
        );
    }

    #[test]
    fn test_route_edges_raw_points_do_not_force_fixed_sides() {
        let from = make_rect_node("A", 100.0, 100.0); // hw=40, hh=20
        let to = make_rect_node("B", 300.0, 100.0); // hw=40, hh=20
        let nodes = vec![from.clone(), to.clone()];

        let edges = vec![EdgeDef {
            from: "A".into(),
            to: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            from_side: Some(EdgeSide::Bottom),
            to_side: Some(EdgeSide::Top),
        }];

        let mut raw_points = HashMap::new();
        raw_points.insert(
            ("A".to_string(), "B".to_string()),
            vec![(140.0, 100.0), (260.0, 100.0)],
        );

        let result = route_edges(
            &nodes,
            &edges,
            false,
            &raw_points,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(result.len(), 1);
        let p = &result[0].points;
        assert_eq!(p.len(), 2);
        // For raw_points, keep dagre-provided endpoints to avoid route regressions.
        assert!((p[0].0 - 140.0).abs() < 1e-6);
        assert!((p[0].1 - 100.0).abs() < 1e-6);
        assert!((p[1].0 - 260.0).abs() < 1e-6);
        assert!((p[1].1 - 100.0).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // edge_label_anchor
    // -----------------------------------------------------------------------

    #[test]
    fn test_edge_label_anchor_empty() {
        assert_eq!(edge_label_anchor(&[]), (0.0, 0.0));
    }

    #[test]
    fn test_edge_label_anchor_single_point() {
        assert_eq!(edge_label_anchor(&[(5.0, 10.0)]), (5.0, 10.0));
    }

    #[test]
    fn test_edge_label_anchor_two_points() {
        let anchor = edge_label_anchor(&[(0.0, 0.0), (100.0, 0.0)]);
        assert!((anchor.0 - 50.0).abs() < 0.1);
        assert!((anchor.1 - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_edge_label_anchor_longest_segment() {
        // Three points: first segment is short, second is long
        let anchor = edge_label_anchor(&[(0.0, 0.0), (10.0, 0.0), (110.0, 0.0)]);
        // Longest segment is (10,0) -> (110,0), midpoint = (60, 0)
        assert!((anchor.0 - 60.0).abs() < 0.1);
    }

    // -----------------------------------------------------------------------
    // path_avoids_nodes
    // -----------------------------------------------------------------------

    #[test]
    fn test_path_avoids_nodes_clear_path() {
        let nodes = vec![
            make_rect_node("A", 100.0, 50.0),
            make_rect_node("B", 100.0, 200.0),
            make_rect_node("C", 300.0, 125.0), // far to the right, not blocking
        ];
        let path = vec![(100.0, 70.0), (100.0, 180.0)];
        assert!(path_avoids_nodes(&path, "A", "B", &nodes));
    }

    #[test]
    fn test_path_avoids_nodes_blocked() {
        let nodes = vec![
            make_rect_node("A", 100.0, 50.0),
            make_rect_node("B", 100.0, 300.0),
            make_rect_node("C", 100.0, 175.0), // directly in the path
        ];
        let path = vec![(100.0, 70.0), (100.0, 280.0)];
        assert!(!path_avoids_nodes(&path, "A", "B", &nodes));
    }

    // -----------------------------------------------------------------------
    // segment_intersects_rect
    // -----------------------------------------------------------------------

    #[test]
    fn test_segment_intersects_rect_horizontal_hit() {
        assert!(segment_intersects_rect(
            (0.0, 5.0),
            (10.0, 5.0),
            3.0,
            0.0,
            7.0,
            10.0
        ));
    }

    #[test]
    fn test_segment_intersects_rect_horizontal_miss() {
        assert!(!segment_intersects_rect(
            (0.0, 15.0),
            (10.0, 15.0),
            3.0,
            0.0,
            7.0,
            10.0
        ));
    }

    #[test]
    fn test_segment_intersects_rect_vertical_hit() {
        assert!(segment_intersects_rect(
            (5.0, 0.0),
            (5.0, 10.0),
            3.0,
            3.0,
            7.0,
            7.0
        ));
    }

    #[test]
    fn test_segment_intersects_rect_vertical_miss() {
        assert!(!segment_intersects_rect(
            (1.0, 0.0),
            (1.0, 10.0),
            3.0,
            3.0,
            7.0,
            7.0
        ));
    }

    #[test]
    fn test_segment_intersects_rect_diagonal_hit() {
        assert!(segment_intersects_rect(
            (0.0, 0.0),
            (10.0, 10.0),
            3.0,
            3.0,
            7.0,
            7.0
        ));
    }

    #[test]
    fn test_segment_intersects_rect_diagonal_miss() {
        assert!(!segment_intersects_rect(
            (0.0, 0.0),
            (2.0, 2.0),
            5.0,
            5.0,
            10.0,
            10.0
        ));
    }
}
