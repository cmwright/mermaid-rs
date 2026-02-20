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
    ("#d8d8e8", "0.6"), // darker band
    ("#ececf4", "0.4"), // lighter band
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
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
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
  .gantt-dependency-halo {{ fill: none; stroke: {}; stroke-width: 4; opacity: 0.7; }}
  .gantt-dependency {{ fill: none; stroke: {}; stroke-width: 1.5; opacity: 0.9; }}
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
        theme.background.to_css(),
        theme.text_color.to_css(),
    );
    svg.push('\n');

    // Marker defs
    let _ = write!(
        svg,
        r#"<defs>
  <marker id="gantt-dependency-arrow" markerWidth="4" markerHeight="3" refX="3.5" refY="1.5" orient="auto">
    <path d="M 0 0 L 4 1.5 L 0 3 z" fill="{}"/>
  </marker>
</defs>"#,
        theme.text_color.to_css(),
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
            layout.width,
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

    // 6. Dependency edges (on top of bars, below labels)
    let task_count = layout.tasks.len();
    let mut out_totals = vec![0usize; task_count];
    let mut in_totals = vec![0usize; task_count];
    for edge in &layout.dependency_edges {
        if edge.from_task_index < task_count && edge.to_task_index < task_count {
            out_totals[edge.from_task_index] += 1;
            in_totals[edge.to_task_index] += 1;
        }
    }
    let mut out_seen = vec![0usize; task_count];
    let mut in_seen = vec![0usize; task_count];
    for edge in &layout.dependency_edges {
        if edge.from_task_index >= task_count || edge.to_task_index >= task_count {
            continue;
        }
        let source_slot = out_seen[edge.from_task_index];
        let target_slot = in_seen[edge.to_task_index];
        out_seen[edge.from_task_index] += 1;
        in_seen[edge.to_task_index] += 1;

        let from_task = &layout.tasks[edge.from_task_index];
        let to_task = &layout.tasks[edge.to_task_index];
        render_dependency_edge(
            &mut svg,
            from_task,
            to_task,
            source_slot,
            out_totals[edge.from_task_index],
            target_slot,
            in_totals[edge.to_task_index],
        );
    }

    // 7. Task labels — inside bars when they fit, otherwise to the right
    for task in &layout.tasks {
        let label_pad = 8.0;
        let label_fits_inside =
            task.label_width + label_pad * 2.0 <= task.width && !task.is_milestone;

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

    // 8. Section labels
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
            cx,
            cy - half, // top
            cx + half,
            cy, // right
            cx,
            cy + half, // bottom
            cx - half,
            cy, // left
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
        task.x, task.y, task.width, task.height, fill, stroke, stroke_width,
    );
    svg.push('\n');
}

fn render_dependency_edge(
    svg: &mut String,
    from_task: &PositionedTask,
    to_task: &PositionedTask,
    source_slot: usize,
    source_total: usize,
    target_slot: usize,
    target_total: usize,
) {
    let (from_x, from_y) = task_end_anchor(from_task, source_slot, source_total);
    let (to_x, to_y) = task_start_anchor(to_task, target_slot, target_total);
    let spacing_nudge = (source_slot as f64 - (source_total.saturating_sub(1) as f64) / 2.0) * 4.0;
    let target_nudge = (target_slot as f64 - (target_total.saturating_sub(1) as f64) / 2.0) * 3.0;
    let from_y = from_y + spacing_nudge;
    let to_y = to_y + target_nudge;
    let from_stub_x = from_x + 6.0;
    let to_stub_x = to_x - 6.0;
    let dx = to_x - from_x;
    let control = (dx.abs() * 0.45).clamp(24.0, 100.0);
    let c1x = from_stub_x + control;
    let c2x = to_stub_x - control;
    let path_d = format!(
        "M {:.1} {:.1} L {:.1} {:.1} C {:.1} {:.1}, {:.1} {:.1}, {:.1} {:.1} L {:.1} {:.1}",
        from_x, from_y, from_stub_x, from_y, c1x, from_y, c2x, to_y, to_stub_x, to_y, to_x, to_y
    );

    let _ = write!(
        svg,
        r#"<path class="gantt-dependency-halo" d="{}"/>"#,
        path_d
    );
    svg.push('\n');
    let _ = write!(
        svg,
        r#"<path class="gantt-dependency" d="{}" marker-end="url(#gantt-dependency-arrow)"/>"#,
        path_d
    );
    svg.push('\n');
}

fn task_start_anchor(task: &PositionedTask, slot: usize, total: usize) -> (f64, f64) {
    let y = port_y(task, slot, total);
    if task.is_milestone {
        let cx = task.x + task.width / 2.0;
        let half = task.height / 3.0;
        (cx - half, y)
    } else {
        (task.x, y)
    }
}

fn task_end_anchor(task: &PositionedTask, slot: usize, total: usize) -> (f64, f64) {
    let y = port_y(task, slot, total);
    if task.is_milestone {
        let cx = task.x + task.width / 2.0;
        let half = task.height / 3.0;
        (cx + half, y)
    } else {
        (task.x + task.width, y)
    }
}

fn port_y(task: &PositionedTask, slot: usize, total: usize) -> f64 {
    if total <= 1 {
        return task.y + task.height / 2.0;
    }
    let slot = slot.min(total - 1) as f64;
    let total = total as f64;
    let frac = (slot + 1.0) / (total + 1.0);
    task.y + task.height * frac
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
                    id: "a1".to_string(),
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
                    id: "b1".to_string(),
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
                    id: "m1".to_string(),
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
            dependency_edges: vec![DependencyEdge {
                from_task_index: 0,
                to_task_index: 1,
            }],
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
    fn test_render_dependency_edges() {
        let layout = create_test_layout();
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("gantt-dependency-arrow"));
        assert!(svg.contains("class=\"gantt-dependency\""));
        assert!(svg.contains("marker-end=\"url(#gantt-dependency-arrow)\""));
    }

    #[test]
    fn test_dependency_edge_with_invalid_index_skipped() {
        // Exercise the continue branch when edge.from_task_index >= task_count
        // or edge.to_task_index >= task_count (line 209-210)
        let layout = GanttLayout {
            width: 800.0,
            height: 200.0,
            title: None,
            title_y: 0.0,
            tasks: vec![PositionedTask {
                name: "Only Task".to_string(),
                id: "t1".to_string(),
                x: 220.0,
                y: 80.0,
                width: 100.0,
                height: 24.0,
                tags: TaskTags::default(),
                section_index: 0,
                is_milestone: false,
                label_width: 60.0,
            }],
            sections: vec![PositionedSection {
                name: "S1".to_string(),
                y_start: 80.0,
                y_end: 120.0,
                index: 0,
            }],
            grid_lines: vec![],
            dependency_edges: vec![
                DependencyEdge {
                    from_task_index: 0,
                    to_task_index: 99, // invalid: >= task_count (1)
                },
                DependencyEdge {
                    from_task_index: 99, // invalid
                    to_task_index: 0,
                },
            ],
            today_x: None,
            chart_x: 200.0,
            chart_y: 50.0,
            chart_width: 580.0,
            chart_height: 100.0,
            axis_format: "%Y-%m-%d".to_string(),
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // Should render without panic; invalid edges are skipped
        assert!(svg.contains("Only Task"));
        assert!(svg.contains("<svg"));
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

    #[test]
    fn test_hex_luminance_malformed() {
        let lum = hex_luminance("#ab");
        assert!((lum - 0.5).abs() < 0.01, "malformed hex should fallback to 0.5");
    }

    #[test]
    fn test_hex_luminance_bright_color() {
        // #ffffff is white, linearize path where c > 0.03928
        let lum = hex_luminance("#ffffff");
        assert!(lum > 0.99);
    }

    #[test]
    fn test_hex_luminance_dark_color_low_srgb() {
        // #050505 → r=g=b≈0.0196, which is <= 0.03928 → linearize via c/12.92
        let lum = hex_luminance("#050505");
        assert!(lum < 0.01);
    }

    #[test]
    fn test_empty_section_name_skipped() {
        let layout = GanttLayout {
            width: 800.0,
            height: 200.0,
            title: None,
            title_y: 0.0,
            tasks: vec![],
            sections: vec![PositionedSection {
                name: String::new(),
                y_start: 50.0,
                y_end: 100.0,
                index: 0,
            }],
            grid_lines: vec![],
            dependency_edges: vec![],
            today_x: None,
            chart_x: 100.0,
            chart_y: 40.0,
            chart_width: 600.0,
            chart_height: 100.0,
            axis_format: "%Y-%m-%d".to_string(),
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        let after_style = svg.split("</style>").nth(1).unwrap_or("");
        assert!(
            !after_style.contains("gantt-section-label"),
            "empty section name should not render a section label text element"
        );
    }

    #[test]
    fn test_grid_line_hidden_label() {
        let layout = GanttLayout {
            width: 800.0,
            height: 200.0,
            title: None,
            title_y: 0.0,
            tasks: vec![],
            sections: vec![],
            grid_lines: vec![GridLine {
                x: 200.0,
                label: "Hidden".to_string(),
                show_label: false,
            }],
            dependency_edges: vec![],
            today_x: None,
            chart_x: 100.0,
            chart_y: 40.0,
            chart_width: 600.0,
            chart_height: 100.0,
            axis_format: "%Y-%m-%d".to_string(),
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        assert!(
            !svg.contains("Hidden"),
            "hidden grid line should not render its label text"
        );
    }

    #[test]
    fn test_no_today_marker() {
        let layout = GanttLayout {
            width: 800.0,
            height: 200.0,
            title: None,
            title_y: 0.0,
            tasks: vec![],
            sections: vec![],
            grid_lines: vec![],
            dependency_edges: vec![],
            today_x: None,
            chart_x: 100.0,
            chart_y: 40.0,
            chart_width: 600.0,
            chart_height: 100.0,
            axis_format: "%Y-%m-%d".to_string(),
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        let after_style = svg.split("</style>").nth(1).unwrap_or("");
        assert!(
            !after_style.contains("gantt-today"),
            "no today_x should produce no today marker line element"
        );
    }

    #[test]
    fn test_done_task_uses_done_fill() {
        // Task with done=true should use TASK_DONE_FILL (#b8bedd for section 0)
        let mut layout = create_test_layout();
        layout.tasks[0].tags.done = true;
        layout.tasks[0].width = 200.0; // ensure label fits inside
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // TASK_DONE_FILL[0] is #b8bedd
        assert!(
            svg.contains("#b8bedd"),
            "done task should use TASK_DONE_FILL color"
        );
    }

    #[test]
    fn test_port_y_multiple_dependency_edges() {
        // Task with 2+ outgoing edges exercises port_y when total > 1
        let layout = GanttLayout {
            width: 800.0,
            height: 300.0,
            title: None,
            title_y: 0.0,
            tasks: vec![
                PositionedTask {
                    name: "A".to_string(),
                    id: "a1".to_string(),
                    x: 100.0,
                    y: 80.0,
                    width: 80.0,
                    height: 24.0,
                    tags: TaskTags::default(),
                    section_index: 0,
                    is_milestone: false,
                    label_width: 20.0,
                },
                PositionedTask {
                    name: "B".to_string(),
                    id: "b1".to_string(),
                    x: 220.0,
                    y: 80.0,
                    width: 80.0,
                    height: 24.0,
                    tags: TaskTags::default(),
                    section_index: 0,
                    is_milestone: false,
                    label_width: 20.0,
                },
                PositionedTask {
                    name: "C".to_string(),
                    id: "c1".to_string(),
                    x: 220.0,
                    y: 108.0,
                    width: 80.0,
                    height: 24.0,
                    tags: TaskTags::default(),
                    section_index: 0,
                    is_milestone: false,
                    label_width: 20.0,
                },
            ],
            sections: vec![PositionedSection {
                name: "S".to_string(),
                y_start: 80.0,
                y_end: 140.0,
                index: 0,
            }],
            grid_lines: vec![],
            dependency_edges: vec![
                DependencyEdge {
                    from_task_index: 0,
                    to_task_index: 1,
                },
                DependencyEdge {
                    from_task_index: 0,
                    to_task_index: 2,
                },
            ],
            today_x: None,
            chart_x: 80.0,
            chart_y: 50.0,
            chart_width: 300.0,
            chart_height: 90.0,
            axis_format: "%Y-%m-%d".to_string(),
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();
        // Multiple edges from task 0 should produce distinct paths (port_y varies by slot)
        assert!(
            svg.contains("gantt-dependency"),
            "multiple dependency edges should render"
        );
        let path_count = svg.matches("class=\"gantt-dependency\"").count();
        assert_eq!(path_count, 2, "should have 2 dependency paths");
    }

    #[test]
    fn test_label_rendered_inside_wide_bar() {
        // Create a wide task with a short label so the label fits inside the bar.
        // The condition is: label_width + 16.0 <= width && !is_milestone
        // With label_width=40.0 and width=300.0, 40+16=56 <= 300 => fits inside.
        let layout = GanttLayout {
            width: 800.0,
            height: 200.0,
            title: None,
            title_y: 0.0,
            tasks: vec![PositionedTask {
                name: "Wide Task".to_string(),
                id: "w1".to_string(),
                x: 100.0,
                y: 50.0,
                width: 300.0,
                height: 24.0,
                tags: TaskTags::default(),
                section_index: 0,
                is_milestone: false,
                label_width: 40.0,
            }],
            sections: vec![],
            grid_lines: vec![],
            dependency_edges: vec![],
            today_x: None,
            chart_x: 100.0,
            chart_y: 40.0,
            chart_width: 600.0,
            chart_height: 100.0,
            axis_format: "%Y-%m-%d".to_string(),
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // The inside-label path uses text-anchor="middle" and renders the label text
        assert!(
            svg.contains(r#"text-anchor="middle"#),
            "Inside label should use text-anchor=\"middle\""
        );
        assert!(
            svg.contains("Wide Task"),
            "SVG should contain the task label text"
        );
        // Verify it is NOT using text-anchor="start" for this task label (that's the outside path)
        // The inside label is centered at task.x + task.width / 2.0 = 250.0
        assert!(
            svg.contains("x=\"250.0\""),
            "Inside label x should be centered at task midpoint"
        );
    }
}
