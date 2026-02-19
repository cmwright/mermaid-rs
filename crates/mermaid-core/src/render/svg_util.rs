/// Escape special XML characters in text content.
pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Build an SVG path using cubic B-spline interpolation (matches d3.curveBasis).
/// The curve starts and ends exactly at the first/last points, but smoothly
/// approximates (doesn't pass through) intermediate control points.
pub fn build_basis_curve_path(points: &[(f64, f64)]) -> String {
    if points.is_empty() {
        return String::new();
    }
    if points.len() == 1 {
        return format!("M {} {}", points[0].0, points[0].1);
    }
    if points.len() == 2 {
        return format!(
            "M {} {} L {} {}",
            points[0].0, points[0].1, points[1].0, points[1].1
        );
    }

    let mut path = format!("M {} {}", points[0].0, points[0].1);

    // d3.curveBasis state: x0/y0 = two points ago, x1/y1 = one point ago
    let mut x0 = points[0].0;
    let mut y0 = points[0].1;
    let mut x1 = points[1].0;
    let mut y1 = points[1].1;

    // After point 0 (MoveTo) and point 1 (stored), process point 2:
    // Line to weighted position near the start, then first bezier
    path.push_str(&format!(
        " L {} {}",
        (5.0 * x0 + x1) / 6.0,
        (5.0 * y0 + y1) / 6.0
    ));
    basis_bezier(&mut path, x0, y0, x1, y1, points[2].0, points[2].1);
    x0 = x1;
    y0 = y1;
    x1 = points[2].0;
    y1 = points[2].1;

    // Process remaining intermediate points
    for pt in points.iter().skip(3) {
        basis_bezier(&mut path, x0, y0, x1, y1, pt.0, pt.1);
        x0 = x1;
        y0 = y1;
        x1 = pt.0;
        y1 = pt.1;
    }

    // Final bezier closing toward the last point, then line to exact endpoint
    basis_bezier(&mut path, x0, y0, x1, y1, x1, y1);
    path.push_str(&format!(" L {} {}", x1, y1));

    path
}

/// Emit one cubic bezier segment for the B-spline basis function.
/// Control points are weighted averages of three consecutive input points.
#[inline]
fn basis_bezier(path: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, x: f64, y: f64) {
    path.push_str(&format!(
        " C {} {} {} {} {} {}",
        (2.0 * x0 + x1) / 3.0,
        (2.0 * y0 + y1) / 3.0,
        (x0 + 2.0 * x1) / 3.0,
        (y0 + 2.0 * y1) / 3.0,
        (x0 + 4.0 * x1 + x) / 6.0,
        (y0 + 4.0 * y1 + y) / 6.0,
    ));
}

/// Build an SVG path with rounded 90-degree orthogonal corners (like mermaid.js git graph).
/// Creates a path with horizontal and vertical segments connected by rounded corners.
pub fn build_orthogonal_path(points: &[(f64, f64)]) -> String {
    if points.is_empty() {
        return String::new();
    }
    if points.len() == 1 {
        return format!("M {} {}", points[0].0, points[0].1);
    }

    let radius: f64 = 8.0; // Corner radius for rounded turns
    let mut path = format!("M {} {}", points[0].0, points[0].1);

    for i in 1..points.len() {
        let (x1, y1) = points[i - 1];
        let (x2, y2) = points[i];

        if i == 1 {
            // First segment with rounded corner
            if (x2 - x1).abs() > (y2 - y1).abs() {
                // Horizontal then vertical
                let dx: f64 = (x2 - x1).abs();
                let dy: f64 = (y2 - y1).abs();
                let r: f64 = radius.min(dx / 2.0).min(dy / 2.0);
                let corner_x = if x2 > x1 { x2 - r } else { x2 + r };
                let corner_y = if y2 > y1 { y1 + r } else { y1 - r };
                path.push_str(&format!(" L {} {}", corner_x, y1));
                path.push_str(&format!(" Q {} {} {} {}", x2, y1, x2, corner_y));
                path.push_str(&format!(" L {} {}", x2, y2));
            } else {
                // Vertical then horizontal
                let dx: f64 = (x2 - x1).abs();
                let dy: f64 = (y2 - y1).abs();
                let r: f64 = radius.min(dx / 2.0).min(dy / 2.0);
                let corner_y = if y2 > y1 { y2 - r } else { y2 + r };
                let corner_x = if x2 > x1 { x1 + r } else { x1 - r };
                path.push_str(&format!(" L {} {}", x1, corner_y));
                path.push_str(&format!(" Q {} {} {} {}", x1, y2, corner_x, y2));
                path.push_str(&format!(" L {} {}", x2, y2));
            }
        } else {
            // Subsequent segments with rounded corners
            let (x0, y0) = points[i - 2];

            // Determine direction of previous segment
            let prev_horizontal = (x1 - x0).abs() > (y1 - y0).abs();
            let dx: f64 = (x2 - x1).abs();
            let dy: f64 = (y2 - y1).abs();
            let r: f64 = radius.min(dx / 2.0).min(dy / 2.0);

            if prev_horizontal {
                // Previous was horizontal, coming in horizontally
                // Go vertical first with rounded corner
                let corner_y = if y2 > y1 { y1 + r } else { y1 - r };
                path.push_str(&format!(" L {} {}", x1, corner_y));
                if x2 != x1 {
                    let corner_x = if x2 > x1 { x1 + r } else { x1 - r };
                    path.push_str(&format!(" Q {} {} {} {}", x1, y2, corner_x, y2));
                }
            } else {
                // Previous was vertical, coming in vertically
                // Go horizontal first with rounded corner
                let corner_x = if x2 > x1 { x1 + r } else { x1 - r };
                path.push_str(&format!(" L {} {}", corner_x, y1));
                if y2 != y1 {
                    let corner_y = if y2 > y1 { y1 + r } else { y1 - r };
                    path.push_str(&format!(" Q {} {} {} {}", x2, y1, x2, corner_y));
                }
            }
            path.push_str(&format!(" L {} {}", x2, y2));
        }
    }

    path
}
