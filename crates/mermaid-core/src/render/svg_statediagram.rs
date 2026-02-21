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

    // Title label
    if let Some(label) = &composite.label {
        let label_x = composite.x + composite.width / 2.0;
        let label_y = composite.y + 18.0;
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

    let _ = write!(
        svg,
        r#"<g transform="translate({}, {})">"#,
        note.x, note.y
    );
    svg.push('\n');

    // Note body (rectangle with corner fold)
    let _ = write!(
        svg,
        r#"  <path d="M {} {} L {} {} L {} {} L {} {} L {} {} Z" fill="{}" stroke="{}" stroke-width="1"/>"#,
        -hw, -hh,              // top-left
        hw - fold, -hh,        // top-right before fold
        hw, -hh + fold,        // fold corner
        hw, hh,                // bottom-right
        -hw, hh,               // bottom-left
        theme.sequence.note_fill.to_css(),
        theme.sequence.note_border.to_css(),
    );
    svg.push('\n');

    // Fold triangle
    let _ = write!(
        svg,
        r#"  <path d="M {} {} L {} {} L {} {} Z" fill="none" stroke="{}" stroke-width="1"/>"#,
        hw - fold, -hh,
        hw - fold, -hh + fold,
        hw, -hh + fold,
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
    if let (Some(label), Some(lx), Some(ly)) = (&transition.label, transition.label_x, transition.label_y) {
        let label_w = transition.label_width.unwrap_or(label.len() as f64 * 8.0 + 10.0);
        let label_h = transition.label_height.unwrap_or(20.0);
        let _ = write!(
            svg,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="rgba(232,232,232,0.8)"/>"#,
            lx - label_w / 2.0,
            ly - label_h / 2.0,
            label_w,
            label_h,
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
