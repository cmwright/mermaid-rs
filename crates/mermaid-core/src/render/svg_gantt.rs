use std::fmt::Write;

use crate::error::Result;
use crate::layout::gantt::*;
use crate::render::svg_util::escape_xml;
use crate::render::theme::Theme;

const SVG_PADDING: f64 = 8.0;

// Task bar colors per section (cycling)
const TASK_FILL: &[&str] = &["#8a90dd", "#d3d3de", "#a3c4d1", "#ffcc00"];
const TASK_DONE_FILL: &[&str] = &["#b8bedd", "#e8e8eb", "#c8dbe3", "#ffe566"];
const TASK_ACTIVE_FILL: &[&str] = &["#5b61c2", "#a0a0b2", "#7aa8bd", "#e6b800"];
const CRIT_STROKE: &str = "#ff3333";
const MILESTONE_FILL: &str = "#e83737";

// Section band colors (alternating backgrounds with visible contrast)
const SECTION_BAND_COLORS: &[(&str, &str)] = &[
    ("#d8d8e8", "0.6"),  // darker band
    ("#ececf4", "0.4"),  // lighter band
];

/// Compute relative luminance of a hex color (e.g. "#8a90dd") using WCAG formula.
/// Returns a value between 0.0 (black) and 1.0 (white).
fn hex_luminance(hex: &str) -> f64 {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return 0.5; // fallback for malformed colors
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128) as f64 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128) as f64 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128) as f64 / 255.0;

    // sRGB to linear
    let linearize = |c: f64| -> f64 {
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// Get the fill color for a task bar based on its tags and section index.
fn task_fill_color(task: &PositionedTask) -> &'static str {
    let si = task.section_index;
    if task.tags.done {
        TASK_DONE_FILL[si % TASK_DONE_FILL.len()]
    } else if task.tags.active {
        TASK_ACTIVE_FILL[si % TASK_ACTIVE_FILL.len()]
    } else {
        TASK_FILL[si % TASK_FILL.len()]
    }
}

/// Render a positioned Gantt chart layout to an SVG string.
pub fn render_svg(layout: &GanttLayout, theme: &Theme) -> Result<String> {
    let view_w = (layout.width + 2.0 * SVG_PADDING).ceil();
    let view_h = (layout.height + 2.0 * SVG_PADDING).ceil();

    let mut svg = String::with_capacity(8192);

    // SVG header
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        view_w as i64, view_h as i64, view_w as i64, view_h as i64,
    );
    svg.push('\n');

    // Style block
    let _ = write!(
        svg,
        r#"<style>
  svg {{ background: {}; }}
  .gantt-text {{ font-family: {}; font-size: {:.0}px; }}
  .gantt-title {{ font-family: {}; font-size: {:.0}px; font-weight: bold; }}
  .gantt-section-label {{ font-family: {}; font-size: {:.0}px; font-weight: bold; }}
  .gantt-task-label {{ font-family: {}; font-size: {:.0}px; }}
  .gantt-grid-label {{ font-family: {}; font-size: {:.0}px; }}
  .gantt-today {{ stroke: #f66; stroke-width: 2; stroke-dasharray: 5,5; }}
</style>"#,
        theme.background.to_css(),
        theme.font_family,
        theme.font_size,
        theme.font_family,
        theme.font_size * 1.2,
        theme.font_family,
        theme.font_size * 0.9,
        theme.font_family,
        theme.font_size * 0.85,
        theme.font_family,
        theme.font_size * 0.8,
    );
    svg.push('\n');

    // Content group
    let _ = write!(
        svg,
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING,
    );
    svg.push('\n');

    // 1. Title
    if let Some(ref title) = layout.title {
        let _ = write!(
            svg,
            r#"<text class="gantt-title" x="{}" y="{}" text-anchor="middle" fill="{}">{}</text>"#,
            layout.width / 2.0,
            layout.title_y,
            theme.text_color.to_css(),
            escape_xml(title),
        );
        svg.push('\n');
    }

    // 2. Section bands (alternating background) — extend full width to include labels
    for section in &layout.sections {
        let (band_color, band_opacity) =
            SECTION_BAND_COLORS[section.index % SECTION_BAND_COLORS.len()];
        let _ = write!(
            svg,
            r#"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="{}"/>"#,
            section.y_start,
            layout.chart_x + layout.chart_width,
            section.y_end - section.y_start,
            band_color,
            band_opacity,
        );
        svg.push('\n');
    }

    // 3. Grid lines — always render tick marks, only show labels when they fit
    for gl in &layout.grid_lines {
        // Vertical tick line (always shown)
        let _ = write!(
            svg,
            r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#ddd" stroke-width="1"/>"##,
            gl.x,
            layout.chart_y,
            gl.x,
            layout.chart_y + layout.chart_height,
        );
        svg.push('\n');

        // Date label at top (only when it fits without overlapping)
        if gl.show_label {
            let _ = write!(
                svg,
                r#"<text class="gantt-grid-label" x="{:.1}" y="{:.1}" text-anchor="middle" fill="{}">{}</text>"#,
                gl.x,
                layout.chart_y - 5.0,
                theme.text_color.to_css(),
                escape_xml(&gl.label),
            );
            svg.push('\n');
        }
    }

    // 4. Today marker
    if let Some(today_x) = layout.today_x {
        let _ = write!(
            svg,
            r#"<line class="gantt-today" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#,
            today_x,
            layout.chart_y,
            today_x,
            layout.chart_y + layout.chart_height,
        );
        svg.push('\n');
    }

    // 5. Task bars
    for task in &layout.tasks {
        render_task_bar(&mut svg, task, theme);
    }

    // 6. Task labels — inside bars when they fit, otherwise to the right
    for task in &layout.tasks {
        let label_pad = 8.0;
        let label_fits_inside = task.label_width + label_pad * 2.0 <= task.width
            && !task.is_milestone;

        if label_fits_inside {
            // Choose text color based on bar darkness
            let fill = task_fill_color(task);
            let text_color = if hex_luminance(fill) < 0.4 {
                "#ffffff"
            } else {
                &theme.text_color.to_css()
            };
            // Centered inside the bar
            let _ = write!(
                svg,
                r#"<text class="gantt-task-label" x="{:.1}" y="{:.1}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
                task.x + task.width / 2.0,
                task.y + task.height / 2.0,
                text_color,
                escape_xml(&task.name),
            );
        } else {
            // To the right of the bar (or diamond)
            let label_x = if task.is_milestone {
                task.x + task.height / 3.0 + label_pad
            } else {
                task.x + task.width + label_pad
            };
            let _ = write!(
                svg,
                r#"<text class="gantt-task-label" x="{:.1}" y="{:.1}" text-anchor="start" dominant-baseline="central" fill="{}">{}</text>"#,
                label_x,
                task.y + task.height / 2.0,
                theme.text_color.to_css(),
                escape_xml(&task.name),
            );
        }
        svg.push('\n');
    }

    // 7. Section labels
    for section in &layout.sections {
        if section.name.is_empty() {
            continue;
        }
        let mid_y = (section.y_start + section.y_end) / 2.0;
        let _ = write!(
            svg,
            r#"<text class="gantt-section-label" x="{:.1}" y="{:.1}" text-anchor="end" dominant-baseline="central" fill="{}">{}</text>"#,
            layout.chart_x - 30.0,
            mid_y,
            theme.text_color.to_css(),
            escape_xml(&section.name),
        );
        svg.push('\n');
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn render_task_bar(svg: &mut String, task: &PositionedTask, _theme: &Theme) {
    if task.is_milestone {
        // Milestone: diamond shape (rotated square)
        let cx = task.x + task.width / 2.0;
        let cy = task.y + task.height / 2.0;
        let half = task.height / 3.0;

        let _ = write!(
            svg,
            r##"<polygon points="{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}" fill="{}" stroke="#333" stroke-width="1"/>"##,
            cx, cy - half,        // top
            cx + half, cy,        // right
            cx, cy + half,        // bottom
            cx - half, cy,        // left
            MILESTONE_FILL,
        );
        svg.push('\n');
        return;
    }

    // Determine fill color based on tags
    let fill = task_fill_color(task);

    // Determine stroke
    let (stroke, stroke_width) = if task.tags.crit {
        (CRIT_STROKE, "2")
    } else {
        ("#333", "1")
    };

    let _ = write!(
        svg,
        r#"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="3" ry="3" fill="{}" stroke="{}" stroke-width="{}"/>"#,
        task.x,
        task.y,
        task.width,
        task.height,
        fill,
        stroke,
        stroke_width,
    );
    svg.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::gantt::TaskTags;

    fn create_test_layout() -> GanttLayout {
        GanttLayout {
            width: 800.0,
            height: 300.0,
            title: Some("Test Gantt".to_string()),
            title_y: 20.0,
            tasks: vec![
                PositionedTask {
                    name: "Task A".to_string(),
                    x: 220.0,
                    y: 80.0,
                    width: 100.0,
                    height: 24.0,
                    tags: TaskTags::default(),
                    section_index: 0,
                    is_milestone: false,
                    label_width: 40.0,
                },
                PositionedTask {
                    name: "Task B".to_string(),
                    x: 320.0,
                    y: 108.0,
                    width: 80.0,
                    height: 24.0,
                    tags: TaskTags {
                        done: true,
                        ..Default::default()
                    },
                    section_index: 0,
                    is_milestone: false,
                    label_width: 40.0,
                },
                PositionedTask {
                    name: "Milestone".to_string(),
                    x: 400.0,
                    y: 136.0,
                    width: 0.0,
                    height: 24.0,
                    tags: TaskTags {
                        milestone: true,
                        ..Default::default()
                    },
                    section_index: 0,
                    is_milestone: true,
                    label_width: 60.0,
                },
            ],
            sections: vec![PositionedSection {
                name: "Section A".to_string(),
                y_start: 80.0,
                y_end: 160.0,
                index: 0,
            }],
            grid_lines: vec![
                GridLine {
                    x: 220.0,
                    label: "Jan 01".to_string(),
                    show_label: true,
                },
                GridLine {
                    x: 320.0,
                    label: "Jan 08".to_string(),
                    show_label: true,
                },
            ],
            today_x: Some(350.0),
            chart_x: 200.0,
            chart_y: 50.0,
            chart_width: 580.0,
            chart_height: 110.0,
            axis_format: "%Y-%m-%d".to_string(),
        }
    }

    #[test]
    fn test_render_svg_structure() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>\n"));
        assert!(svg.contains("Test Gantt"));
        assert!(svg.contains("gantt-title"));
    }

    #[test]
    fn test_render_tasks() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Should have task bars
        assert!(svg.contains("<rect"));
        // Should have task labels
        assert!(svg.contains("Task A"));
        assert!(svg.contains("Task B"));
    }

    #[test]
    fn test_render_milestone() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Should have milestone diamond
        assert!(svg.contains("<polygon"));
    }

    #[test]
    fn test_render_today_marker() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Should have today marker line
        assert!(svg.contains("gantt-today"));
    }

    #[test]
    fn test_render_grid_lines() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Should have grid line labels
        assert!(svg.contains("Jan 01"));
        assert!(svg.contains("Jan 08"));
    }

    #[test]
    fn test_render_section_labels() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("Section A"));
    }

    #[test]
    fn test_hex_luminance() {
        // Black should be ~0
        assert!(hex_luminance("#000000") < 0.01);
        // White should be ~1
        assert!(hex_luminance("#ffffff") > 0.99);
        // Dark purple (active fill) should be dark
        assert!(hex_luminance("#5b61c2") < 0.4);
        // Light done fill should be light
        assert!(hex_luminance("#b8bedd") > 0.4);
    }

    #[test]
    fn test_dark_bar_gets_white_text() {
        let mut layout = create_test_layout();
        // Make task active (dark fill #5b61c2)
        layout.tasks[0].tags.active = true;
        layout.tasks[0].width = 200.0; // ensure label fits inside
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // The inside label for the active task should use white text
        assert!(svg.contains(r##"fill="#ffffff">Task A</text>"##));
    }

    #[test]
    fn test_render_crit_task() {
        let mut layout = create_test_layout();
        layout.tasks[0].tags.crit = true;
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // Crit tasks should have red stroke
        assert!(svg.contains(CRIT_STROKE));
    }
}
