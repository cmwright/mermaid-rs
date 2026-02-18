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
    for i in 3..points.len() {
        basis_bezier(&mut path, x0, y0, x1, y1, points[i].0, points[i].1);
        x0 = x1;
        y0 = y1;
        x1 = points[i].0;
        y1 = points[i].1;
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
