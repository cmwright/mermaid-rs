use std::fmt::Write;

use crate::ast::sequence::{ArrowType, BlockKind, ParticipantKind};
use crate::error::Result;
use crate::layout::sequence::*;
use crate::render::html_util;
use crate::render::svg_util::escape_xml;
use crate::render::theme::Theme;

const SVG_PADDING: f64 = 8.0;

/// Render a positioned sequence diagram layout to an SVG string.
pub fn render_svg(layout: &SequenceLayout, theme: &Theme) -> Result<String> {
    let view_w = (layout.width + 2.0 * SVG_PADDING).ceil();
    let view_h = (layout.height + 2.0 * SVG_PADDING).ceil();

    // Estimate capacity based on layout complexity
    let est_capacity = 2048
        + layout.actors.len() * 400
        + layout.messages.len() * 200
        + layout.blocks.len() * 300
        + layout.notes.len() * 200
        + layout.activations.len() * 100;
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
  .seq-text {{ font-family: {}; font-size: {}px; }}
  .seq-label {{ font-family: {}; font-size: {}px; }}
  .seq-note {{ font-family: {}; font-size: {}px; }}
</style>"#,
        theme.background.to_css(),
        theme.font_family,
        theme.font_size,
        theme.font_family,
        theme.font_size * 0.9,
        theme.font_family,
        theme.font_size * 0.85,
    );
    svg.push('\n');

    // Defs: markers
    build_sequence_defs(&mut svg, theme);

    // Content group
    let _ = write!(
        svg,
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    );
    svg.push('\n');

    // 1. Block backgrounds
    for block in &layout.blocks {
        render_block_bg(&mut svg, block, theme);
    }

    // 2. Lifelines
    for lifeline in &layout.lifelines {
        render_lifeline(&mut svg, lifeline, theme);
    }

    // 3. Activation rectangles
    for activation in &layout.activations {
        render_activation(&mut svg, activation, theme);
    }

    // 4. Notes
    for note in &layout.notes {
        render_note(&mut svg, note, theme);
    }

    // 5. Message arrows (lines only, no labels yet)
    for msg in &layout.messages {
        render_message_line(&mut svg, msg, theme);
    }

    // 6. Top actor boxes/figures
    for actor in &layout.actors {
        render_actor(&mut svg, actor, actor.top_y, theme);
    }

    // 7. Bottom actor boxes/figures
    for actor in &layout.actors {
        render_actor(&mut svg, actor, actor.bottom_y, theme);
    }

    // 8. Block labels (rendered after actors so they appear on top)
    for block in &layout.blocks {
        render_block_label(&mut svg, block, theme);
    }

    // 9. Message labels (rendered last so they appear on top)
    for msg in &layout.messages {
        render_message_label(&mut svg, msg, theme);
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn build_sequence_defs(svg: &mut String, theme: &Theme) {
    let line_color = theme.line_color.to_css();

    let _ = write!(
        svg,
        r#"<defs>
  <marker id="seq-arrowhead" viewBox="0 0 10 10" markerWidth="8" markerHeight="8" refX="9" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 0 0 L 10 5 L 0 10 z" fill="{line_color}"/>
  </marker>
  <marker id="seq-arrowhead-open" viewBox="0 0 10 10" markerWidth="8" markerHeight="8" refX="9" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 0 0 L 10 5 L 0 10" fill="none" stroke="{line_color}" stroke-width="1.5"/>
  </marker>
  <marker id="seq-cross" viewBox="0 0 10 10" markerWidth="10" markerHeight="10" refX="5" refY="5" orient="auto" markerUnits="userSpaceOnUse">
    <path d="M 1 1 L 9 9 M 1 9 L 9 1" fill="none" stroke="{line_color}" stroke-width="2"/>
  </marker>
</defs>
"#,
    );
}

fn render_actor(svg: &mut String, actor: &PositionedActor, y: f64, theme: &Theme) {
    match actor.kind {
        ParticipantKind::Participant => {
            // Rectangle box with centered label
            let x = actor.center_x - actor.box_width / 2.0;
            let _ = write!(
                svg,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="2"/>"#,
                x,
                y,
                actor.box_width,
                actor.box_height,
                theme.sequence.actor_fill.to_css(),
                theme.sequence.actor_border.to_css(),
            );
            svg.push('\n');

            // Label (handle multi-line via <br/>)
            let display = html_util::normalize_br(&actor.display_name);
            let lines: Vec<&str> = display.split('\n').collect();
            let text_y = y + actor.box_height / 2.0;

            if lines.len() == 1 {
                let clean = html_util::strip_html_tags(lines[0]);
                let _ = write!(
                    svg,
                    r#"<text class="seq-text" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
                    actor.center_x,
                    text_y,
                    theme.sequence.actor_text.to_css(),
                    escape_xml(&clean),
                );
                svg.push('\n');
            } else {
                let line_height = 1.2_f64;
                let start_dy = -((lines.len() as f64 - 1.0) / 2.0) * line_height;
                let _ = write!(
                    svg,
                    r#"<text class="seq-text" x="{}" y="{}" text-anchor="middle" fill="{}">"#,
                    actor.center_x,
                    text_y,
                    theme.sequence.actor_text.to_css(),
                );
                svg.push('\n');
                for (i, line) in lines.iter().enumerate() {
                    let clean = html_util::strip_html_tags(line);
                    if i == 0 {
                        let _ = write!(
                            svg,
                            r#"  <tspan x="{}" dy="{}em" dominant-baseline="central">{}</tspan>"#,
                            actor.center_x,
                            start_dy,
                            escape_xml(&clean),
                        );
                    } else {
                        let _ = write!(
                            svg,
                            r#"  <tspan x="{}" dy="{}em" dominant-baseline="central">{}</tspan>"#,
                            actor.center_x,
                            line_height,
                            escape_xml(&clean),
                        );
                    }
                    svg.push('\n');
                }
                svg.push_str("</text>\n");
            }
        }
        ParticipantKind::Actor => {
            // Stick figure
            let cx = actor.center_x;
            let fig_top = y;
            let head_r = 8.0;
            let head_cy = fig_top + head_r + 2.0;
            let body_top = head_cy + head_r;
            let body_bottom = body_top + 14.0;
            let arms_y = body_top + 5.0;
            let arm_span = 12.0;
            let leg_bottom = body_bottom + 10.0;
            let leg_span = 10.0;

            let stroke = theme.sequence.actor_border.to_css();
            // Head
            let _ = write!(
                svg,
                r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{}" stroke-width="2"/>"#,
                cx, head_cy, head_r, stroke,
            );
            svg.push('\n');
            // Body
            let _ = write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"#,
                cx, body_top, cx, body_bottom, stroke,
            );
            svg.push('\n');
            // Arms
            let _ = write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"#,
                cx - arm_span,
                arms_y,
                cx + arm_span,
                arms_y,
                stroke,
            );
            svg.push('\n');
            // Left leg
            let _ = write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"#,
                cx,
                body_bottom,
                cx - leg_span,
                leg_bottom,
                stroke,
            );
            svg.push('\n');
            // Right leg
            let _ = write!(
                svg,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"#,
                cx,
                body_bottom,
                cx + leg_span,
                leg_bottom,
                stroke,
            );
            svg.push('\n');

            // Label below figure
            let label_y = fig_top + 40.0 + 4.0;
            let clean = html_util::strip_html_tags(&html_util::normalize_br(&actor.display_name));
            let _ = write!(
                svg,
                r#"<text class="seq-text" x="{}" y="{}" text-anchor="middle" dominant-baseline="hanging" fill="{}">{}</text>"#,
                cx,
                label_y,
                theme.sequence.actor_text.to_css(),
                escape_xml(&clean),
            );
            svg.push('\n');
        }
    }
}

fn render_lifeline(svg: &mut String, lifeline: &Lifeline, theme: &Theme) {
    let _ = write!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"#,
        lifeline.x,
        lifeline.y_start,
        lifeline.x,
        lifeline.y_end,
        theme.sequence.lifeline_color.to_css(),
    );
    svg.push('\n');
}

fn render_activation(svg: &mut String, act: &PositionedActivation, theme: &Theme) {
    let w = 10.0;
    let x_offset = act.depth as f64 * 3.0;
    let x = act.x - w / 2.0 + x_offset;
    let _ = write!(
        svg,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
        x,
        act.y_start,
        w,
        act.y_end - act.y_start,
        theme.sequence.activation_fill.to_css(),
        theme.sequence.activation_border.to_css(),
    );
    svg.push('\n');
}

fn render_note(svg: &mut String, note: &PositionedNote, theme: &Theme) {
    // Note rectangle
    let _ = write!(
        svg,
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
        note.x,
        note.y,
        note.width,
        note.height,
        theme.sequence.note_fill.to_css(),
        theme.sequence.note_border.to_css(),
    );
    svg.push('\n');

    // Note text (already normalized: <br/> → \n by layout phase)
    let text_x = note.x + note.width / 2.0;
    let text_color = theme.sequence.note_text.to_css();
    let lines: Vec<&str> = note.text.split('\n').collect();

    if lines.len() == 1 {
        let _ = write!(
            svg,
            r#"<text class="seq-note" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
            text_x,
            note.y + note.height / 2.0,
            text_color,
            escape_xml(lines[0]),
        );
        svg.push('\n');
    } else {
        let line_height = 1.2_f64;
        let total_em = (lines.len() as f64 - 1.0) * line_height;
        // Center the text block vertically in the note box
        let start_y = note.y + note.height / 2.0;
        let start_dy = -(total_em / 2.0);

        let _ = write!(
            svg,
            r#"<text class="seq-note" x="{}" y="{}" text-anchor="middle" fill="{}">"#,
            text_x, start_y, text_color,
        );
        svg.push('\n');
        for (i, line) in lines.iter().enumerate() {
            if i == 0 {
                let _ = write!(
                    svg,
                    r#"  <tspan x="{}" dy="{}em" dominant-baseline="central">{}</tspan>"#,
                    text_x,
                    start_dy,
                    escape_xml(line),
                );
            } else {
                let _ = write!(
                    svg,
                    r#"  <tspan x="{}" dy="{}em" dominant-baseline="central">{}</tspan>"#,
                    text_x,
                    line_height,
                    escape_xml(line),
                );
            }
            svg.push('\n');
        }
        svg.push_str("</text>\n");
    }
}

fn render_message_line(svg: &mut String, msg: &PositionedMessage, theme: &Theme) {
    let line_color = theme.line_color.to_css();

    if msg.is_self {
        // Self-message: polyline looping right then back down
        let x = msg.from_x;
        let sw = msg.self_width;
        let sh = msg.self_height;
        let y1 = msg.y;
        let y2 = y1 + sh;

        let (dasharray, marker) = arrow_attrs(msg.arrow);

        let _ = write!(
            svg,
            r#"<polyline points="{},{} {},{} {},{} {},{}" fill="none" stroke="{}" stroke-width="2"{}{}/>"#,
            x,
            y1,
            x + sw,
            y1,
            x + sw,
            y2,
            x,
            y2,
            line_color,
            dasharray,
            marker,
        );
        svg.push('\n');
    } else {
        // Regular message: horizontal line with arrowhead
        let (dasharray, marker) = arrow_attrs(msg.arrow);

        let _ = write!(
            svg,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"{}{}/>"#,
            msg.from_x, msg.y, msg.to_x, msg.y, line_color, dasharray, marker,
        );
        svg.push('\n');
    }

    // Autonumber circle (part of the line layer)
    if let Some(num) = msg.number {
        let num_x = if msg.is_self {
            msg.from_x - 12.0
        } else {
            msg.from_x.min(msg.to_x) - 5.0
        };
        let num_y = msg.y;
        let _ = write!(
            svg,
            r#"<circle cx="{}" cy="{}" r="8" fill="{}"/>"#,
            num_x,
            num_y,
            theme.sequence.actor_fill.to_css(),
        );
        svg.push('\n');
        let _ = write!(
            svg,
            r#"<text class="seq-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}" font-size="10">{}</text>"#,
            num_x,
            num_y,
            theme.sequence.actor_text.to_css(),
            num,
        );
        svg.push('\n');
    }
}

fn render_message_label(svg: &mut String, msg: &PositionedMessage, theme: &Theme) {
    if msg.is_self {
        // Self-message label centered above the self-message
        let x = msg.from_x;
        let sw = msg.self_width;
        let y1 = msg.y;
        let mid_x = x + sw / 2.0;
        let _ = write!(
            svg,
            r#"<text class="seq-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="auto" fill="{}">{}</text>"#,
            mid_x,
            y1 - 5.0,
            theme.text_color.to_css(),
            escape_xml(&msg.label),
        );
        svg.push('\n');
    } else {
        // Regular message label centered above the arrow
        let mid_x = (msg.from_x + msg.to_x) / 2.0;
        let _ = write!(
            svg,
            r#"<text class="seq-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="auto" fill="{}">{}</text>"#,
            mid_x,
            msg.y - 5.0,
            theme.text_color.to_css(),
            escape_xml(&msg.label),
        );
        svg.push('\n');
    }
}

fn arrow_attrs(arrow: ArrowType) -> (&'static str, &'static str) {
    match arrow {
        ArrowType::SolidArrow => ("", r#" marker-end="url(#seq-arrowhead)""#),
        ArrowType::DottedArrow => (
            r#" stroke-dasharray="5,5""#,
            r#" marker-end="url(#seq-arrowhead)""#,
        ),
        ArrowType::SolidOpen => ("", r#" marker-end="url(#seq-arrowhead-open)""#),
        ArrowType::DottedOpen => (
            r#" stroke-dasharray="5,5""#,
            r#" marker-end="url(#seq-arrowhead-open)""#,
        ),
        ArrowType::SolidParen => ("", ""),
        ArrowType::DottedParen => (r#" stroke-dasharray="5,5""#, ""),
        ArrowType::SolidCross => ("", r#" marker-end="url(#seq-cross)""#),
        ArrowType::DottedCross => (
            r#" stroke-dasharray="5,5""#,
            r#" marker-end="url(#seq-cross)""#,
        ),
    }
}

fn render_block_bg(svg: &mut String, block: &PositionedBlock, theme: &Theme) {
    // Block background
    let _ = write!(
        svg,
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{}" stroke="{}" stroke-width="1"/>"#,
        block.x,
        block.y,
        block.width,
        block.height,
        theme.sequence.loop_fill.to_css(),
        theme.sequence.loop_line.to_css(),
    );
    svg.push('\n');

    // Block kind label tab background (just the background, text will be in label layer)
    let kind_text = block_kind_str(block.kind);
    let tab_w = kind_text.len() as f64 * 7.5 + 16.0;
    let tab_h = 20.0;

    // Label tab background (polygon with angled bottom-right corner)
    let _ = write!(
        svg,
        r#"<polygon points="{},{} {},{} {},{} {},{} {},{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
        block.x,
        block.y,
        block.x + tab_w,
        block.y,
        block.x + tab_w,
        block.y + tab_h - 4.0,
        block.x + tab_w - 4.0,
        block.y + tab_h,
        block.x,
        block.y + tab_h,
        theme.sequence.label_box_fill.to_css(),
        theme.sequence.loop_line.to_css(),
    );
    svg.push('\n');

    // Section dividers (dashed lines only)
    for divider in &block.sections {
        let _ = write!(
            svg,
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-dasharray="5,5"/>"#,
            block.x,
            divider.y,
            block.x + block.width,
            divider.y,
            theme.sequence.loop_line.to_css(),
        );
        svg.push('\n');
    }
}

fn render_block_label(svg: &mut String, block: &PositionedBlock, theme: &Theme) {
    // Block kind label tab (just the kind keyword)
    let kind_text = block_kind_str(block.kind);
    let tab_h = 20.0;

    // Kind text inside tab
    let _ = write!(
        svg,
        r#"<text class="seq-label" x="{}" y="{}" text-anchor="start" dominant-baseline="central" fill="{}" font-weight="bold">{}</text>"#,
        block.x + 6.0,
        block.y + tab_h / 2.0,
        theme.text_color.to_css(),
        escape_xml(kind_text),
    );
    svg.push('\n');

    // Condition label centered in the block header area
    if !block.label.is_empty() {
        let center_x = block.x + block.width / 2.0;
        let _ = write!(
            svg,
            r#"<text class="seq-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">[{}]</text>"#,
            center_x,
            block.y + tab_h / 2.0,
            theme.text_color.to_css(),
            escape_xml(&block.label),
        );
        svg.push('\n');
    }

    // Divider labels
    for divider in &block.sections {
        if let Some(label) = &divider.label {
            let center_x = block.x + block.width / 2.0;
            let _ = write!(
                svg,
                r#"<text class="seq-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="hanging" fill="{}">[{}]</text>"#,
                center_x,
                divider.y + 4.0,
                theme.text_color.to_css(),
                escape_xml(label),
            );
            svg.push('\n');
        }
    }
}

#[inline]
fn block_kind_str(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Alt => "alt",
        BlockKind::Loop => "loop",
        BlockKind::Opt => "opt",
        BlockKind::Par => "par",
        BlockKind::Critical => "critical",
        BlockKind::Break => "break",
        BlockKind::Rect => "rect",
    }
}
