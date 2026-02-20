use crate::error::Result;
use crate::layout::pie::*;
use crate::render::svg_util::escape_xml;
use crate::render::theme::Theme;

const SVG_PADDING: f64 = 8.0;

// Default color palette for pie slices (matching mermaid.js default theme)
const PIE_COLORS: &[&str] = &[
    "#ECECFF", // light purple
    "#ffffde", // light yellow
    "#cdffb2", // light green
    "#ffc7c7", // light red
    "#c7e8ff", // light blue
    "#ffe4c7", // light orange
    "#e8c7ff", // light magenta
    "#c7ffe8", // light cyan
    "#ffd4e5", // pink
    "#d4e5ff", // light steel
    "#e5ffd4", // lime
    "#ffe5d4", // peach
];

/// Render a positioned pie chart layout to an SVG string.
pub fn render_svg(layout: &PieLayout, theme: &Theme) -> Result<String> {
    let view_w = (layout.width + 2.0 * SVG_PADDING).ceil();
    let view_h = (layout.height + 2.0 * SVG_PADDING).ceil();

    let mut svg = String::with_capacity(8192);

    // SVG header
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        view_w as i64, view_h as i64, view_w as i64, view_h as i64,
    ));
    svg.push('\n');

    // Style block
    svg.push_str(&format!(
        r#"<style>
  svg {{ background: {}; }}
  .pie-text {{ font-family: {}; font-size: {}px; }}
  .pie-title {{ font-family: {}; font-size: {}px; font-weight: bold; }}
  .pie-label {{ font-family: {}; font-size: {}px; }}
  .pie-legend {{ font-family: {}; font-size: {}px; }}
</style>"#,
        theme.background.to_css(),
        theme.font_family,
        theme.font_size,
        theme.font_family,
        theme.font_size * 1.2,
        theme.font_family,
        theme.font_size * 0.9,
        theme.font_family,
        theme.font_size,
    ));
    svg.push('\n');

    // Content group
    svg.push_str(&format!(
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    ));
    svg.push('\n');

    // 1. Title
    if let Some(title) = &layout.title {
        svg.push_str(&format!(
            r#"<text class="pie-title" x="{}" y="{}" text-anchor="middle" fill="{}">{}</text>"#,
            layout.pie_center_x,
            layout.title_y,
            theme.text_color.to_css(),
            escape_xml(title),
        ));
        svg.push('\n');
    }

    // 2. Pie slices
    for slice in &layout.slices {
        svg.push_str(&render_slice(slice, layout, theme));
    }

    // 3. Slice labels (percentages)
    for slice in &layout.slices {
        svg.push_str(&render_slice_label(slice, theme));
    }

    // 4. Legend
    for item in &layout.legend {
        svg.push_str(&render_legend_item(item, layout, theme));
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn render_slice(slice: &PositionedSlice, layout: &PieLayout, _theme: &Theme) -> String {
    let cx = layout.pie_center_x;
    let cy = layout.pie_center_y;
    let r = layout.pie_radius;

    // Calculate start and end points on the circle
    let x1 = cx + r * slice.start_angle.cos();
    let y1 = cy + r * slice.start_angle.sin();
    let x2 = cx + r * slice.end_angle.cos();
    let y2 = cy + r * slice.end_angle.sin();

    // Determine if the arc should be the long way around (> 180 degrees)
    let angle_span = slice.end_angle - slice.start_angle;
    let large_arc = if angle_span > std::f64::consts::PI {
        1
    } else {
        0
    };

    // Build the SVG path
    // Move to center, line to start point, arc to end point, close path
    let color = get_slice_color(slice.color_index);

    format!(
        r##"<path d="M {} {} L {} {} A {} {} 0 {} 1 {} {} Z" fill="{}" stroke="#333" stroke-width="1"/>"##,
        cx,
        cy, // Move to center
        x1,
        y1, // Line to start point
        r,
        r,         // Radius (rx, ry)
        large_arc, // Large arc flag
        x2,
        y2, // End point
        color,
    ) + "\n"
}

fn render_slice_label(slice: &PositionedSlice, theme: &Theme) -> String {
    // Only show percentage label if slice is large enough (> 3%)
    if slice.percentage < 3.0 {
        return String::new();
    }

    let percentage_text = format!("{:.0}%", slice.percentage);

    format!(
        r#"<text class="pie-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
        slice.label_x,
        slice.label_y,
        theme.text_color.to_css(),
        escape_xml(&percentage_text),
    ) + "\n"
}

fn render_legend_item(item: &LegendItem, layout: &PieLayout, theme: &Theme) -> String {
    let x = layout.legend_x;
    let y = layout.legend_y + item.y;
    let color = get_slice_color(item.color_index);

    let mut s = String::new();

    // Color box
    s.push_str(&format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="#333" stroke-width="1"/>"##,
        x,
        y,
        16.0, // LEGEND_BOX_SIZE
        16.0,
        color,
    ));
    s.push('\n');

    // Label text
    s.push_str(&format!(
        r#"<text class="pie-legend" x="{}" y="{}" text-anchor="start" dominant-baseline="central" fill="{}">{}</text>"#,
        x + 24.0, // LEGEND_TEXT_OFFSET
        y + 8.0,
        theme.text_color.to_css(),
        escape_xml(&item.label),
    ));
    s.push('\n');

    s
}

fn get_slice_color(index: usize) -> &'static str {
    PIE_COLORS.get(index).copied().unwrap_or("#cccccc")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_layout() -> PieLayout {
        PieLayout {
            width: 600.0,
            height: 500.0,
            title: Some("Test Pie".to_string()),
            title_y: 20.0,
            pie_center_x: 200.0,
            pie_center_y: 250.0,
            pie_radius: 150.0,
            slices: vec![
                PositionedSlice {
                    label: "A".to_string(),
                    value: 50.0,
                    percentage: 50.0,
                    start_angle: -std::f64::consts::PI / 2.0,
                    end_angle: std::f64::consts::PI / 2.0,
                    label_x: 200.0,
                    label_y: 175.0,
                    color_index: 0,
                },
                PositionedSlice {
                    label: "B".to_string(),
                    value: 50.0,
                    percentage: 50.0,
                    start_angle: std::f64::consts::PI / 2.0,
                    end_angle: 3.0 * std::f64::consts::PI / 2.0,
                    label_x: 200.0,
                    label_y: 325.0,
                    color_index: 1,
                },
            ],
            legend: vec![
                LegendItem {
                    label: "A".to_string(),
                    color_index: 0,
                    y: 0.0,
                },
                LegendItem {
                    label: "B".to_string(),
                    color_index: 1,
                    y: 25.0,
                },
            ],
            legend_x: 400.0,
            legend_y: 225.0,
        }
    }

    #[test]
    fn test_render_svg_structure() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Check basic SVG structure
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>\n"));
        assert!(svg.contains("Test Pie"));
        assert!(svg.contains("50%"));
    }

    #[test]
    fn test_slice_path_generation() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Should contain path elements for slices
        assert!(svg.contains("<path"));
        assert!(svg.contains("A 150 150")); // Arc command with radius
    }

    #[test]
    fn test_legend_rendering() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Should contain legend items
        assert!(svg.contains("<rect"));
        // Legend labels should be present
        assert!(svg.contains(">A</text>"));
        assert!(svg.contains(">B</text>"));
    }

    #[test]
    fn test_get_slice_color_fallback() {
        // PIE_COLORS has 12 entries; index 100 should exceed the array and return the fallback
        assert_eq!(get_slice_color(100), "#cccccc");
    }

    #[test]
    fn test_large_arc_flag_for_dominant_slice() {
        let layout = PieLayout {
            width: 600.0,
            height: 500.0,
            title: None,
            title_y: 20.0,
            pie_center_x: 200.0,
            pie_center_y: 250.0,
            pie_radius: 150.0,
            slices: vec![
                PositionedSlice {
                    label: "Big".to_string(),
                    value: 80.0,
                    percentage: 80.0,
                    start_angle: -std::f64::consts::PI / 2.0,
                    end_angle: -std::f64::consts::PI / 2.0 + 2.0 * std::f64::consts::PI * 0.8,
                    label_x: 200.0,
                    label_y: 200.0,
                    color_index: 0,
                },
                PositionedSlice {
                    label: "Small".to_string(),
                    value: 20.0,
                    percentage: 20.0,
                    start_angle: -std::f64::consts::PI / 2.0 + 2.0 * std::f64::consts::PI * 0.8,
                    end_angle: 3.0 * std::f64::consts::PI / 2.0,
                    label_x: 200.0,
                    label_y: 350.0,
                    color_index: 1,
                },
            ],
            legend: vec![],
            legend_x: 400.0,
            legend_y: 225.0,
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // The large slice (80%) spans > 180°, so the large_arc flag should be 1
        assert!(svg.contains(" 1 1 "), "expected large_arc=1 for >180° slice");
    }

    #[test]
    fn test_tiny_slice_label_hidden() {
        let layout = PieLayout {
            width: 600.0,
            height: 500.0,
            title: None,
            title_y: 20.0,
            pie_center_x: 200.0,
            pie_center_y: 250.0,
            pie_radius: 150.0,
            slices: vec![PositionedSlice {
                label: "Tiny".to_string(),
                value: 2.0,
                percentage: 2.0,
                start_angle: 0.0,
                end_angle: 0.04 * std::f64::consts::PI,
                label_x: 200.0,
                label_y: 250.0,
                color_index: 0,
            }],
            legend: vec![],
            legend_x: 400.0,
            legend_y: 225.0,
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(!svg.contains("2%"), "tiny slice (<3%) should not show percentage label");
    }
}
