use std::fmt::Write;

use crate::ast::flowchart::{ArrowEnd, LineStyle};
use crate::ast::statediagram::StateKind;
use crate::error::Result;
use crate::layout::statediagram::types::*;
use crate::render::html_util;
use crate::render::svg_util::{build_basis_curve_path, escape_xml};
use crate::render::theme::Theme;

const SVG_PADDING: f64 = 8.0;

/// Render a positioned state diagram to an SVG string.
pub fn render_svg(diagram: &PositionedStateDiagram, theme: &Theme) -> Result<String> {
    let view_w = diagram.width + 2.0 * SVG_PADDING;
    let view_h = diagram.height + 2.0 * SVG_PADDING;

    let est_capacity = 1024
        + diagram.states.len() * 200
        + diagram.transitions.len() * 300
        + diagram.composites.len() * 200
        + diagram.notes.len() * 200;
    let mut svg = String::with_capacity(est_capacity);

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
  .node-text {{ font-family: {}; font-size: {}px; }}
  .edge-label {{ font-family: {}; font-size: {}px; }}
</style>"#,
        theme.background.to_css(),
        theme.font_family,
        theme.font_size,
        theme.font_family,
        theme.font_size * 0.85,
    );
    svg.push('\n');

    // Defs: arrowhead marker
    build_defs(&mut svg, theme);

    // Content group with padding offset
    let _ = write!(
        svg,
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    );
    svg.push('\n');

    // Render order: composites (background) -> transitions -> notes -> states (foreground)
    for composite in &diagram.composites {
        render_composite(&mut svg, composite, theme);
    }
    for transition in &diagram.transitions {
        render_transition(&mut svg, transition, theme);
    }
    for note in &diagram.notes {
        render_note(&mut svg, note, theme);
    }
    for state in &diagram.states {
        render_state(&mut svg, state, theme);
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn build_defs(svg: &mut String, theme: &Theme) {
    let line_color = theme.line_color.to_css();
    let sz = theme.flowchart.arrowhead_size.max(8.0);
    let mw = sz * 0.8;

    let _ = write!(
        svg,
        r#"<defs>
  <marker id="arrowhead" viewBox="0 0 10 10" markerWidth="{mw}" markerHeight="{mw}" refX="8.5" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 0 0 L 10 5 L 0 10 z" fill="{line_color}"/>
  </marker>
</defs>
"#,
    );
}

fn render_state(svg: &mut String, state: &PositionedState, theme: &Theme) {
    let _ = write!(
        svg,
        r#"<g transform="translate({}, {})">"#,
        state.x, state.y
    );
    svg.push('\n');

    match state.kind {
        StateKind::Start => {
            // Filled black circle
            let r = 7.0;
            let _ = write!(
                svg,
                r#"  <circle cx="0" cy="0" r="{}" fill="{}" stroke="none"/>"#,
                r,
                theme.line_color.to_css()
            );
            svg.push('\n');
        }
        StateKind::End => {
            // Donut: outer circle with inner filled circle
            let outer_r = 10.0;
            let inner_r = 5.0;
            let _ = write!(
                svg,
                r#"  <circle cx="0" cy="0" r="{}" fill="none" stroke="{}" stroke-width="1.5"/>"#,
                outer_r,
                theme.line_color.to_css()
            );
            svg.push('\n');
            let _ = write!(
                svg,
                r#"  <circle cx="0" cy="0" r="{}" fill="{}" stroke="none"/>"#,
                inner_r,
                theme.line_color.to_css()
            );
            svg.push('\n');
        }
        StateKind::Fork | StateKind::Join => {
            // Black filled horizontal bar
            let bar_width = 70.0;
            let bar_height = 6.0;
            let _ = write!(
                svg,
                r#"  <rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{}" stroke="none"/>"#,
                -bar_width / 2.0,
                -bar_height / 2.0,
                bar_width,
                bar_height,
                theme.line_color.to_css()
            );
            svg.push('\n');
        }
        StateKind::Choice => {
            // Filled black diamond
            let hs = 14.0;
            let _ = write!(
                svg,
                r#"  <polygon points="0,{} {},0 0,{} {},0" fill="{}" stroke="none"/>"#,
                -hs,
                hs,
                hs,
                -hs,
                theme.line_color.to_css()
            );
            svg.push('\n');
        }
        StateKind::Normal => {
            let fill = state
                .style
                .fill
                .as_ref()
                .map(|c| c.to_css())
                .unwrap_or_else(|| theme.flowchart.primary_color.to_css());
            let stroke = state
                .style
                .stroke
                .as_ref()
                .map(|c| c.to_css())
                .unwrap_or_else(|| theme.flowchart.primary_border.to_css());
            let stroke_width = state
                .style
                .stroke_width
                .unwrap_or(theme.flowchart.node_border_width);
            let text_color = state
                .style
                .color
                .as_ref()
                .map(|c| c.to_css())
                .unwrap_or_else(|| theme.flowchart.primary_text.to_css());

            let hw = state.width / 2.0;
            let hh = state.height / 2.0;
            let rx = (state.height.min(state.width) * 0.2).min(8.0);

            // Rounded rectangle
            let _ = write!(
                svg,
                r#"  <rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                -hw, -hh, state.width, state.height, rx, rx, fill, stroke, stroke_width
            );
            svg.push('\n');

            // Label text
            if !state.label.is_empty() {
                let label_lines: Vec<String> = html_util::normalize_br(&state.label)
                    .lines()
                    .map(|l| l.to_string())
                    .collect();

                if label_lines.len() <= 1 {
                    let _ = write!(
                        svg,
                        r#"  <text class="node-text" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
                        text_color,
                        escape_xml(&state.label),
                    );
                    svg.push('\n');
                } else {
                    let line_height = 1.2_f64;
                    let start_dy = -((label_lines.len() as f64 - 1.0) / 2.0) * line_height;

                    let _ = write!(
                        svg,
                        r#"  <text class="node-text" text-anchor="middle" fill="{}">"#,
                        text_color,
                    );
                    svg.push('\n');

                    for (i, line) in label_lines.iter().enumerate() {
                        let dy = if i == 0 {
                            format!("{}em", start_dy)
                        } else {
                            format!("{}em", line_height)
                        };
                        let _ = write!(
                            svg,
                            r#"    <tspan x="0" dy="{}" dominant-baseline="central">{}</tspan>"#,
                            dy,
                            escape_xml(&html_util::strip_html_tags(line)),
                        );
                        svg.push('\n');
                    }
                    svg.push_str("  </text>\n");
                }
            }
        }
    }

    svg.push_str("</g>\n");
}

fn render_composite(svg: &mut String, composite: &PositionedComposite, theme: &Theme) {
    let fill = composite
        .style
        .fill
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.flowchart.subgraph_fill.to_css());
    let stroke = composite
        .style
        .stroke
        .as_ref()
        .map(|c| c.to_css())
        .unwrap_or_else(|| theme.flowchart.subgraph_border.to_css());

    // Rounded rectangle background
    let _ = write!(
        svg,
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" stroke="{}" stroke-width="1"/>"#,
        composite.x, composite.y, composite.width, composite.height, fill, stroke,
    );
    svg.push('\n');

    // Title separator + label (stateDiagram-v2 style)
    let header_h = 24.0;
    let _ = write!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
        composite.x,
        composite.y + header_h,
        composite.x + composite.width,
        composite.y + header_h,
        stroke,
    );
    svg.push('\n');

    // Title label
    if let Some(label) = &composite.label {
        let label_x = composite.x + composite.width / 2.0;
        let label_y = composite.y + header_h / 2.0;
        let _ = write!(
            svg,
            r#"<text x="{}" y="{}" text-anchor="middle" font-family="{}" font-size="{}" font-weight="bold" fill="{}">{}</text>"#,
            label_x,
            label_y,
            theme.font_family,
            theme.font_size,
            theme.flowchart.subgraph_text.to_css(),
            escape_xml(label),
        );
        svg.push('\n');
    }
}

fn render_note(svg: &mut String, note: &PositionedNote, theme: &Theme) {
    let hw = note.width / 2.0;
    let hh = note.height / 2.0;
    let fold = 8.0;

    let _ = write!(svg, r#"<g transform="translate({}, {})">"#, note.x, note.y);
    svg.push('\n');

    // Note body (rectangle with corner fold)
    let _ = write!(
        svg,
        r#"  <path d="M {} {} L {} {} L {} {} L {} {} L {} {} Z" fill="{}" stroke="{}" stroke-width="1"/>"#,
        -hw,
        -hh, // top-left
        hw - fold,
        -hh, // top-right before fold
        hw,
        -hh + fold, // fold corner
        hw,
        hh, // bottom-right
        -hw,
        hh, // bottom-left
        theme.sequence.note_fill.to_css(),
        theme.sequence.note_border.to_css(),
    );
    svg.push('\n');

    // Fold triangle
    let _ = write!(
        svg,
        r#"  <path d="M {} {} L {} {} L {} {} Z" fill="none" stroke="{}" stroke-width="1"/>"#,
        hw - fold,
        -hh,
        hw - fold,
        -hh + fold,
        hw,
        -hh + fold,
        theme.sequence.note_border.to_css(),
    );
    svg.push('\n');

    // Note text (supports multi-line from word wrapping)
    let lines: Vec<&str> = note.text.lines().collect();
    if lines.len() <= 1 {
        let _ = write!(
            svg,
            r#"  <text class="node-text" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
            theme.sequence.note_text.to_css(),
            escape_xml(&note.text),
        );
        svg.push('\n');
    } else {
        let line_height = 1.2_f64;
        let start_dy = -((lines.len() as f64 - 1.0) / 2.0) * line_height;

        let _ = write!(
            svg,
            r#"  <text class="node-text" text-anchor="middle" fill="{}">"#,
            theme.sequence.note_text.to_css(),
        );
        svg.push('\n');

        for (i, line) in lines.iter().enumerate() {
            let dy = if i == 0 {
                format!("{}em", start_dy)
            } else {
                format!("{}em", line_height)
            };
            let _ = write!(
                svg,
                r#"    <tspan x="0" dy="{}" dominant-baseline="central">{}</tspan>"#,
                dy,
                escape_xml(line),
            );
            svg.push('\n');
        }
        svg.push_str("  </text>\n");
    }

    svg.push_str("</g>\n");
}

fn render_transition(svg: &mut String, transition: &PositionedTransition, theme: &Theme) {
    if transition.points.len() < 2 {
        return;
    }

    let line_color = theme.line_color.to_css();
    // Use pre-computed cubic bezier path for bowed edges, otherwise basis curve
    let path_d = match &transition.raw_path_d {
        Some(raw) => raw.clone(),
        None => build_basis_curve_path(&transition.points),
    };
    let stroke_width = theme.flowchart.edge_width.max(1.75);

    let dash_attr = match transition.line_style {
        LineStyle::Dotted => r#" stroke-dasharray="5,5""#,
        _ => "",
    };
    let marker_attr = match transition.arrow_end {
        ArrowEnd::None => "",
        _ => r#" marker-end="url(#arrowhead)""#,
    };

    let _ = write!(
        svg,
        r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}"{}{} stroke-linecap="round" stroke-linejoin="round"/>"#,
        path_d, line_color, stroke_width, dash_attr, marker_attr,
    );
    svg.push('\n');

    // Transition label
    if let (Some(label), Some(lx), Some(ly)) =
        (&transition.label, transition.label_x, transition.label_y)
    {
        let label_w = transition
            .label_width
            .unwrap_or(label.len() as f64 * 8.0 + 10.0);
        let label_h = transition.label_height.unwrap_or(20.0);
        let _ = write!(
            svg,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{}"/>"#,
            lx - label_w / 2.0,
            ly - label_h / 2.0,
            label_w,
            label_h,
            theme.edge_label_background.to_css(),
        );
        svg.push('\n');

        let clean = html_util::normalize_br(label);
        let _ = write!(
            svg,
            r#"<text class="edge-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
            lx,
            ly,
            theme.text_color.to_css(),
            escape_xml(&html_util::strip_html_tags(&clean)),
        );
        svg.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::flowchart::{ArrowEnd, Direction, LineStyle};
    use crate::ast::statediagram::StateKind;
    use crate::layout::statediagram::types::{
        PositionedComposite, PositionedNote, PositionedState, PositionedStateDiagram,
        PositionedTransition,
    };
    use crate::render::theme::Theme;

    fn make_state(id: &str, label: &str, kind: StateKind) -> PositionedState {
        PositionedState {
            id: id.to_string(),
            label: label.to_string(),
            kind,
            style: Default::default(),
            x: 100.0,
            y: 100.0,
            width: 80.0,
            height: 40.0,
        }
    }

    fn make_transition(from: &str, to: &str, label: Option<&str>) -> PositionedTransition {
        PositionedTransition {
            from_id: from.to_string(),
            to_id: to.to_string(),
            line_style: LineStyle::Solid,
            arrow_end: ArrowEnd::Arrow,
            label: label.map(|s| s.to_string()),
            label_x: label.map(|_| 150.0),
            label_y: label.map(|_| 75.0),
            label_width: label.map(|_| 50.0),
            label_height: label.map(|_| 20.0),
            points: vec![(50.0, 100.0), (150.0, 100.0)],
            raw_path_d: None,
        }
    }

    fn empty_diagram() -> PositionedStateDiagram {
        PositionedStateDiagram {
            states: vec![],
            transitions: vec![],
            composites: vec![],
            notes: vec![],
            width: 300.0,
            height: 200.0,
            direction: Direction::TopToBottom,
        }
    }

    #[test]
    fn render_svg_produces_valid_svg() {
        let diagram = empty_diagram();
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn render_svg_includes_style_and_defs() {
        let diagram = empty_diagram();
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("<style>"));
        assert!(svg.contains(".node-text"));
        assert!(svg.contains(".edge-label"));
        assert!(svg.contains("<defs>"));
        assert!(svg.contains("arrowhead"));
    }

    #[test]
    fn render_normal_state_with_label() {
        let mut diagram = empty_diagram();
        diagram
            .states
            .push(make_state("idle", "Idle", StateKind::Normal));
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("Idle"), "should contain state label");
        assert!(
            svg.contains("<rect"),
            "normal state should render as rounded rect"
        );
        assert!(
            svg.contains("rx="),
            "normal state rect should have rounded corners"
        );
    }

    #[test]
    fn render_start_state_as_filled_circle() {
        let mut diagram = empty_diagram();
        diagram
            .states
            .push(make_state("start", "", StateKind::Start));
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("<circle"), "start state should be a circle");
        assert!(
            svg.contains("stroke=\"none\""),
            "start state circle should have no stroke"
        );
    }

    #[test]
    fn render_end_state_as_donut() {
        let mut diagram = empty_diagram();
        diagram.states.push(make_state("end", "", StateKind::End));
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        let circle_count = svg.matches("<circle").count();
        assert!(
            circle_count >= 2,
            "end state should have outer + inner circle (donut), got {} circles",
            circle_count
        );
    }

    #[test]
    fn render_fork_join_as_bar() {
        for kind in [StateKind::Fork, StateKind::Join] {
            let mut diagram = empty_diagram();
            diagram.states.push(make_state("fj", "", kind));
            let theme = Theme::default();
            let svg = render_svg(&diagram, &theme).unwrap();

            assert!(
                svg.contains("<rect"),
                "{:?} should render as a rectangle bar",
                kind
            );
        }
    }

    #[test]
    fn render_transition_without_label_no_label_elements() {
        let mut diagram = empty_diagram();
        diagram.transitions.push(make_transition("A", "B", None));
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        // The class exists in the style block, but no <text class="edge-label"> should appear
        // in the rendered content
        let content_after_style = svg.split("</style>").nth(1).unwrap_or("");
        assert!(
            !content_after_style.contains("edge-label"),
            "no label text should be rendered when transition has no label"
        );
    }

    #[test]
    fn render_transition_with_arrow_marker() {
        let mut diagram = empty_diagram();
        diagram.transitions.push(make_transition("A", "B", None));
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("<path"), "should have path for transition");
        assert!(
            svg.contains("marker-end=\"url(#arrowhead)\""),
            "should have arrowhead marker"
        );
    }

    #[test]
    fn render_transition_with_label_shows_text_and_bg() {
        let mut diagram = empty_diagram();
        diagram
            .transitions
            .push(make_transition("A", "B", Some("go")));
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("go"), "should contain transition label text");
        assert!(
            svg.contains("#e8e8e8cc"),
            "should have label background rect"
        );
    }

    #[test]
    fn render_transition_dotted_line_style() {
        let mut diagram = empty_diagram();
        diagram.transitions.push(PositionedTransition {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Dotted,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![(0.0, 0.0), (100.0, 100.0)],
            raw_path_d: None,
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(
            svg.contains("stroke-dasharray"),
            "dotted line style should produce stroke-dasharray"
        );
    }

    #[test]
    fn render_transition_no_arrow_end() {
        let mut diagram = empty_diagram();
        diagram.transitions.push(PositionedTransition {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_end: ArrowEnd::None,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![(0.0, 0.0), (100.0, 100.0)],
            raw_path_d: None,
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(
            !svg.contains("marker-end"),
            "ArrowEnd::None should not produce marker-end"
        );
    }

    #[test]
    fn render_transition_with_raw_path_d() {
        let mut diagram = empty_diagram();
        diagram.transitions.push(PositionedTransition {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![(0.0, 0.0), (50.0, 30.0), (100.0, 0.0)],
            raw_path_d: Some("M 0 0 C 33 30 67 30 100 0".into()),
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(
            svg.contains("M 0 0 C 33 30 67 30 100 0"),
            "should use raw_path_d when provided"
        );
    }

    #[test]
    fn render_transition_skips_when_too_few_points() {
        let mut diagram = empty_diagram();
        diagram.transitions.push(PositionedTransition {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![(0.0, 0.0)], // only 1 point
            raw_path_d: None,
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        // The defs block has a <path> for the arrowhead marker, but the content
        // area should not contain any additional path for this transition
        let content_after_defs = svg.split("</defs>").nth(1).unwrap_or("");
        assert!(
            !content_after_defs.contains("<path"),
            "should not render path element for transition with < 2 points"
        );
    }

    #[test]
    fn render_composite_with_label() {
        let mut diagram = empty_diagram();
        diagram.composites.push(PositionedComposite {
            id: "active".into(),
            label: Some("Active".into()),
            x: 20.0,
            y: 20.0,
            width: 200.0,
            height: 150.0,
            style: Default::default(),
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("Active"), "composite should show its label");
        assert!(
            svg.contains("<rect"),
            "composite should have background rect"
        );
        assert!(
            svg.contains("<line"),
            "composite should have header separator line"
        );
        assert!(
            svg.contains("font-weight=\"bold\""),
            "composite title should be bold"
        );
    }

    #[test]
    fn render_composite_without_label() {
        let mut diagram = empty_diagram();
        diagram.composites.push(PositionedComposite {
            id: "comp".into(),
            label: None,
            x: 20.0,
            y: 20.0,
            width: 200.0,
            height: 150.0,
            style: Default::default(),
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        // Should still render the rect and line, just no text
        assert!(svg.contains("<rect"));
        assert!(svg.contains("<line"));
    }

    #[test]
    fn render_note_with_fold() {
        let mut diagram = empty_diagram();
        diagram.notes.push(PositionedNote {
            id: "n1".into(),
            text: "Important note".into(),
            x: 200.0,
            y: 100.0,
            width: 120.0,
            height: 50.0,
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("Important note"), "should show note text");
        // Note has two path elements: body and fold triangle
        let path_count = svg.matches("<path").count();
        assert!(
            path_count >= 2,
            "note should have body path and fold triangle, got {} paths",
            path_count
        );
    }

    #[test]
    fn render_multiline_note() {
        let mut diagram = empty_diagram();
        diagram.notes.push(PositionedNote {
            id: "n1".into(),
            text: "Line one\nLine two\nLine three".into(),
            x: 200.0,
            y: 100.0,
            width: 120.0,
            height: 60.0,
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        let tspan_count = svg.matches("<tspan").count();
        assert_eq!(
            tspan_count, 3,
            "three-line note should produce 3 <tspan> elements"
        );
        assert!(svg.contains("Line one"));
        assert!(svg.contains("Line two"));
        assert!(svg.contains("Line three"));
    }

    #[test]
    fn render_multiline_normal_state_label() {
        let mut diagram = empty_diagram();
        diagram.states.push(PositionedState {
            id: "s1".into(),
            label: "First\nSecond".into(),
            kind: StateKind::Normal,
            style: Default::default(),
            x: 100.0,
            y: 100.0,
            width: 100.0,
            height: 50.0,
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        let tspan_count = svg.matches("<tspan").count();
        assert_eq!(
            tspan_count, 2,
            "two-line state label should produce 2 <tspan> elements"
        );
    }

    #[test]
    fn render_normal_state_custom_style() {
        let mut diagram = empty_diagram();
        diagram.states.push(PositionedState {
            id: "styled".into(),
            label: "Styled".into(),
            kind: StateKind::Normal,
            style: crate::ast::common::StyleProperties {
                fill: Some(crate::ast::common::Color::Hex("#ff0000".into())),
                stroke: Some(crate::ast::common::Color::Hex("#00ff00".into())),
                color: Some(crate::ast::common::Color::Hex("#0000ff".into())),
                stroke_width: Some(3.0),
                ..Default::default()
            },
            x: 100.0,
            y: 100.0,
            width: 80.0,
            height: 40.0,
        });
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        assert!(svg.contains("#ff0000"), "custom fill color should appear");
        assert!(svg.contains("#00ff00"), "custom stroke color should appear");
        assert!(svg.contains("#0000ff"), "custom text color should appear");
    }

    #[test]
    fn render_order_composites_before_states() {
        let mut diagram = empty_diagram();
        diagram.composites.push(PositionedComposite {
            id: "comp".into(),
            label: Some("Comp".into()),
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 200.0,
            style: Default::default(),
        });
        diagram
            .states
            .push(make_state("inner", "Inner", StateKind::Normal));
        let theme = Theme::default();
        let svg = render_svg(&diagram, &theme).unwrap();

        let comp_pos = svg.find("Comp").unwrap();
        let state_pos = svg.find("Inner").unwrap();
        assert!(
            comp_pos < state_pos,
            "composite should be rendered before states (background)"
        );
    }
}
