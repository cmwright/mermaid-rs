use std::fmt::Write;

/// Escape special XML characters in text content.
/// Single-pass implementation that avoids creating multiple intermediate Strings.
pub fn escape_xml(s: &str) -> String {
    // Fast path: if no special characters, return as-is
    if !s
        .bytes()
        .any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\''))
    {
        return s.to_string();
    }

    let mut result = String::with_capacity(s.len() + s.len() / 8);
    for ch in s.chars() {
        match ch {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            _ => result.push(ch),
        }
    }
    result
}

/// Build an SVG path using cubic B-spline interpolation (matches d3.curveBasis).
/// The curve starts and ends exactly at the first/last points, but smoothly
/// approximates (doesn't pass through) intermediate control points.
pub fn build_basis_curve_path(points: &[(f64, f64)]) -> String {
    if points.is_empty() {
        return String::new();
    }
    if points.len() == 1 {
        let mut path = String::with_capacity(32);
        let _ = write!(path, "M {} {}", points[0].0, points[0].1);
        return path;
    }
    if points.len() == 2 {
        let mut path = String::with_capacity(64);
        let _ = write!(
            path,
            "M {} {} L {} {}",
            points[0].0, points[0].1, points[1].0, points[1].1
        );
        return path;
    }

    // Estimate capacity: ~40 bytes per point for bezier commands
    let mut path = String::with_capacity(points.len() * 40 + 64);

    let _ = write!(path, "M {} {}", points[0].0, points[0].1);

    // d3.curveBasis state: x0/y0 = two points ago, x1/y1 = one point ago
    let mut x0 = points[0].0;
    let mut y0 = points[0].1;
    let mut x1 = points[1].0;
    let mut y1 = points[1].1;

    // After point 0 (MoveTo) and point 1 (stored), process point 2:
    // Line to weighted position near the start, then first bezier
    let _ = write!(
        path,
        " L {} {}",
        (5.0 * x0 + x1) / 6.0,
        (5.0 * y0 + y1) / 6.0
    );
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
    let _ = write!(path, " L {} {}", x1, y1);

    path
}

/// Emit one cubic bezier segment for the B-spline basis function.
/// Control points are weighted averages of three consecutive input points.
#[inline]
fn basis_bezier(path: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, x: f64, y: f64) {
    let _ = write!(
        path,
        " C {} {} {} {} {} {}",
        (2.0 * x0 + x1) / 3.0,
        (2.0 * y0 + y1) / 3.0,
        (x0 + 2.0 * x1) / 3.0,
        (y0 + 2.0 * y1) / 3.0,
        (x0 + 4.0 * x1 + x) / 6.0,
        (y0 + 4.0 * y1 + y) / 6.0,
    );
}

/// Build an SVG path with rounded 90-degree orthogonal corners (like mermaid.js git graph).
/// Creates a path with horizontal and vertical segments connected by rounded corners.
pub fn build_orthogonal_path(points: &[(f64, f64)]) -> String {
    if points.is_empty() {
        return String::new();
    }
    if points.len() == 1 {
        let mut path = String::with_capacity(32);
        let _ = write!(path, "M {} {}", points[0].0, points[0].1);
        return path;
    }

    let radius: f64 = 8.0; // Corner radius for rounded turns
    let mut path = String::with_capacity(points.len() * 60 + 32);
    let _ = write!(path, "M {} {}", points[0].0, points[0].1);

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
                let _ = write!(path, " L {} {}", corner_x, y1);
                let _ = write!(path, " Q {} {} {} {}", x2, y1, x2, corner_y);
                let _ = write!(path, " L {} {}", x2, y2);
            } else {
                // Vertical then horizontal
                let dx: f64 = (x2 - x1).abs();
                let dy: f64 = (y2 - y1).abs();
                let r: f64 = radius.min(dx / 2.0).min(dy / 2.0);
                let corner_y = if y2 > y1 { y2 - r } else { y2 + r };
                let corner_x = if x2 > x1 { x1 + r } else { x1 - r };
                let _ = write!(path, " L {} {}", x1, corner_y);
                let _ = write!(path, " Q {} {} {} {}", x1, y2, corner_x, y2);
                let _ = write!(path, " L {} {}", x2, y2);
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
                let _ = write!(path, " L {} {}", x1, corner_y);
                if x2 != x1 {
                    let corner_x = if x2 > x1 { x1 + r } else { x1 - r };
                    let _ = write!(path, " Q {} {} {} {}", x1, y2, corner_x, y2);
                }
            } else {
                // Previous was vertical, coming in vertically
                // Go horizontal first with rounded corner
                let corner_x = if x2 > x1 { x1 + r } else { x1 - r };
                let _ = write!(path, " L {} {}", corner_x, y1);
                if y2 != y1 {
                    let corner_y = if y2 > y1 { y1 + r } else { y1 - r };
                    let _ = write!(path, " Q {} {} {} {}", x2, y1, x2, corner_y);
                }
            }
            let _ = write!(path, " L {} {}", x2, y2);
        }
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // escape_xml
    // ---------------------------------------------------------------

    #[test]
    fn escape_xml_fast_path_no_special_chars() {
        // Fast path: no special characters, should return the input unchanged.
        assert_eq!(escape_xml("plain text"), "plain text");
    }

    #[test]
    fn escape_xml_all_special_chars() {
        assert_eq!(
            escape_xml("a & b < c > d \"e\" 'f'"),
            "a &amp; b &lt; c &gt; d &quot;e&quot; &#39;f&#39;"
        );
    }

    // ---------------------------------------------------------------
    // build_basis_curve_path
    // ---------------------------------------------------------------

    #[test]
    fn basis_curve_empty_points() {
        assert_eq!(build_basis_curve_path(&[]), "");
    }

    #[test]
    fn basis_curve_single_point() {
        assert_eq!(build_basis_curve_path(&[(5.0, 10.0)]), "M 5 10");
    }

    #[test]
    fn basis_curve_two_points() {
        assert_eq!(
            build_basis_curve_path(&[(0.0, 0.0), (10.0, 10.0)]),
            "M 0 0 L 10 10"
        );
    }

    #[test]
    fn basis_curve_three_points_has_cubic_bezier() {
        let path = build_basis_curve_path(&[(0.0, 0.0), (10.0, 20.0), (30.0, 40.0)]);
        assert!(path.starts_with("M "), "path should start with M: {path}");
        assert!(
            path.contains("C "),
            "path should contain cubic bezier C: {path}"
        );
    }

    #[test]
    fn basis_curve_five_points_multiple_cubics() {
        let path = build_basis_curve_path(&[
            (0.0, 0.0),
            (10.0, 20.0),
            (30.0, 40.0),
            (50.0, 10.0),
            (70.0, 30.0),
        ]);
        assert!(path.starts_with("M "), "path should start with M: {path}");
        // With 5 points there should be multiple C segments
        let c_count = path.matches(" C ").count();
        assert!(
            c_count >= 2,
            "expected multiple C segments, got {c_count}: {path}"
        );
    }

    // ---------------------------------------------------------------
    // build_orthogonal_path
    // ---------------------------------------------------------------

    #[test]
    fn orthogonal_empty_points() {
        assert_eq!(build_orthogonal_path(&[]), "");
    }

    #[test]
    fn orthogonal_single_point() {
        assert_eq!(build_orthogonal_path(&[(5.0, 10.0)]), "M 5 10");
    }

    #[test]
    fn orthogonal_two_points_horizontal_first() {
        // dx (50) > dy (5) => horizontal-first branch
        let path = build_orthogonal_path(&[(0.0, 0.0), (50.0, 5.0)]);
        assert!(
            path.starts_with("M 0 0"),
            "path should start with M 0 0: {path}"
        );
        assert!(path.contains("Q "), "path should contain a Q arc: {path}");
        assert!(path.contains("L 50 5"), "path should end at target: {path}");
    }

    #[test]
    fn orthogonal_two_points_vertical_first() {
        // dy (50) > dx (5) => vertical-first branch (the else at line ~139)
        let path = build_orthogonal_path(&[(0.0, 0.0), (5.0, 50.0)]);
        assert!(
            path.starts_with("M 0 0"),
            "path should start with M 0 0: {path}"
        );
        assert!(path.contains("Q "), "path should contain a Q arc: {path}");
        assert!(path.contains("L 5 50"), "path should end at target: {path}");
    }

    #[test]
    fn orthogonal_three_points_prev_horizontal() {
        // First segment: dx=50 > dy=10  => horizontal first, so prev_horizontal=true
        // Second segment: from (50,10) to (55,80) => prev_horizontal branch in "else" block
        let path = build_orthogonal_path(&[(0.0, 0.0), (50.0, 10.0), (55.0, 80.0)]);
        assert!(
            path.starts_with("M 0 0"),
            "path should start with M 0 0: {path}"
        );
        assert!(
            path.contains("L 55 80"),
            "path should reach final point: {path}"
        );
    }

    #[test]
    fn orthogonal_three_points_prev_vertical() {
        // First segment: dy=50 > dx=5  => vertical first, so prev_horizontal=false
        // Second segment: from (5,50) to (80,55) => !prev_horizontal branch
        let path = build_orthogonal_path(&[(0.0, 0.0), (5.0, 50.0), (80.0, 55.0)]);
        assert!(
            path.starts_with("M 0 0"),
            "path should start with M 0 0: {path}"
        );
        assert!(
            path.contains("L 80 55"),
            "path should reach final point: {path}"
        );
    }

    #[test]
    fn orthogonal_subsequent_prev_horizontal_x2_eq_x1() {
        // First segment: dx=50 > dy=5 => horizontal, prev_horizontal=true
        // Second segment: from (50,5) to (50,60) => x2==x1, skips the Q arc in the
        //   prev_horizontal branch (line 165 condition).
        let path = build_orthogonal_path(&[(0.0, 0.0), (50.0, 5.0), (50.0, 60.0)]);
        assert!(
            path.starts_with("M 0 0"),
            "path should start with M 0 0: {path}"
        );
        assert!(
            path.contains("L 50 60"),
            "path should reach final point: {path}"
        );

        // The second segment should NOT have a Q for the corner because x2==x1.
        // Count Q commands - only the first segment should produce one.
        let q_count = path.matches("Q ").count();
        assert_eq!(
            q_count, 1,
            "expected exactly 1 Q (from first segment only): {path}"
        );
    }

    #[test]
    fn orthogonal_subsequent_prev_vertical_y2_eq_y1() {
        // First segment: dy=50 > dx=5 => vertical, prev_horizontal=false
        // Second segment: from (5,50) to (60,50) => y2==y1, skips the Q arc in the
        //   !prev_horizontal branch (line 174 condition).
        let path = build_orthogonal_path(&[(0.0, 0.0), (5.0, 50.0), (60.0, 50.0)]);
        assert!(
            path.starts_with("M 0 0"),
            "path should start with M 0 0: {path}"
        );
        assert!(
            path.contains("L 60 50"),
            "path should reach final point: {path}"
        );

        // The second segment should NOT have a Q for the corner because y2==y1.
        let q_count = path.matches("Q ").count();
        assert_eq!(
            q_count, 1,
            "expected exactly 1 Q (from first segment only): {path}"
        );
    }
}
