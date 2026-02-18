use crate::ast::sequence::{ArrowType, BlockKind, ParticipantKind};
use crate::error::Result;
use crate::layout::sequence_layout::*;
use crate::render::html_util;
use crate::render::svg_util::escape_xml;
use crate::render::theme::Theme;

const SVG_PADDING: f64 = 8.0;

/// Render a positioned sequence diagram layout to an SVG string.
pub fn render_svg(layout: &SequenceLayout, theme: &Theme) -> Result<String> {
    let view_w = layout.width + 2.0 * SVG_PADDING;
    let view_h = layout.height + 2.0 * SVG_PADDING;

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
    ));
    svg.push('\n');

    // Defs: markers
    svg.push_str(&build_sequence_defs(theme));

    // Content group
    svg.push_str(&format!(
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    ));
    svg.push('\n');

    // 1. Block backgrounds
    for block in &layout.blocks {
        svg.push_str(&render_block(block, theme));
    }

    // 2. Lifelines
    for lifeline in &layout.lifelines {
        svg.push_str(&render_lifeline(lifeline, theme));
    }

    // 3. Activation rectangles
    for activation in &layout.activations {
        svg.push_str(&render_activation(activation, theme));
    }

    // 4. Notes
    for note in &layout.notes {
        svg.push_str(&render_note(note, theme));
    }

    // 5. Message arrows and labels
    for msg in &layout.messages {
        svg.push_str(&render_message(msg, theme));
    }

    // 6. Top actor boxes/figures
    for actor in &layout.actors {
        svg.push_str(&render_actor(actor, actor.top_y, theme));
    }

    // 7. Bottom actor boxes/figures
    for actor in &layout.actors {
        svg.push_str(&render_actor(actor, actor.bottom_y, theme));
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn build_sequence_defs(theme: &Theme) -> String {
    let line_color = theme.line_color.to_css();

    format!(
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
        line_color = line_color,
    )
}

fn render_actor(actor: &PositionedActor, y: f64, theme: &Theme) -> String {
    let mut s = String::new();

    match actor.kind {
        ParticipantKind::Participant => {
            // Rectangle box with centered label
            let x = actor.center_x - actor.box_width / 2.0;
            s.push_str(&format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="2"/>"#,
                x, y, actor.box_width, actor.box_height,
                theme.actor_fill.to_css(),
                theme.actor_border.to_css(),
            ));
            s.push('\n');

            // Label (handle multi-line via <br/>)
            let display = html_util::normalize_br(&actor.display_name);
            let lines: Vec<&str> = display.split('\n').collect();
            let text_y = y + actor.box_height / 2.0;

            if lines.len() == 1 {
                let clean = html_util::strip_html_tags(lines[0]);
                s.push_str(&format!(
                    r#"<text class="seq-text" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
                    actor.center_x,
                    text_y,
                    theme.actor_text.to_css(),
                    escape_xml(&clean),
                ));
                s.push('\n');
            } else {
                let line_height = 1.2_f64;
                let start_dy = -((lines.len() as f64 - 1.0) / 2.0) * line_height;
                s.push_str(&format!(
                    r#"<text class="seq-text" x="{}" y="{}" text-anchor="middle" fill="{}">"#,
                    actor.center_x, text_y, theme.actor_text.to_css(),
                ));
                s.push('\n');
                for (i, line) in lines.iter().enumerate() {
                    let clean = html_util::strip_html_tags(line);
                    let dy = if i == 0 {
                        format!("{}em", start_dy)
                    } else {
                        format!("{}em", line_height)
                    };
                    s.push_str(&format!(
                        r#"  <tspan x="{}" dy="{}" dominant-baseline="central">{}</tspan>"#,
                        actor.center_x, dy, escape_xml(&clean),
                    ));
                    s.push('\n');
                }
                s.push_str("</text>\n");
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

            let stroke = theme.actor_border.to_css();
            // Head
            s.push_str(&format!(
                r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{}" stroke-width="2"/>"#,
                cx, head_cy, head_r, stroke,
            ));
            s.push('\n');
            // Body
            s.push_str(&format!(
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"#,
                cx, body_top, cx, body_bottom, stroke,
            ));
            s.push('\n');
            // Arms
            s.push_str(&format!(
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"#,
                cx - arm_span, arms_y, cx + arm_span, arms_y, stroke,
            ));
            s.push('\n');
            // Left leg
            s.push_str(&format!(
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"#,
                cx, body_bottom, cx - leg_span, leg_bottom, stroke,
            ));
            s.push('\n');
            // Right leg
            s.push_str(&format!(
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"#,
                cx, body_bottom, cx + leg_span, leg_bottom, stroke,
            ));
            s.push('\n');

            // Label below figure
            let label_y = fig_top + 40.0 + 4.0;
            let clean = html_util::strip_html_tags(&html_util::normalize_br(&actor.display_name));
            s.push_str(&format!(
                r#"<text class="seq-text" x="{}" y="{}" text-anchor="middle" dominant-baseline="hanging" fill="{}">{}</text>"#,
                cx, label_y, theme.actor_text.to_css(), escape_xml(&clean),
            ));
            s.push('\n');
        }
    }

    s
}

fn render_lifeline(lifeline: &Lifeline, theme: &Theme) -> String {
    format!(
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-dasharray="5,5"/>"#,
        lifeline.x, lifeline.y_start, lifeline.x, lifeline.y_end,
        theme.line_color.to_css(),
    ) + "\n"
}

fn render_activation(act: &PositionedActivation, theme: &Theme) -> String {
    let w = 10.0;
    let x_offset = act.depth as f64 * 3.0;
    let x = act.x - w / 2.0 + x_offset;
    format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
        x, act.y_start, w, act.y_end - act.y_start,
        theme.activation_fill.to_css(),
        theme.activation_border.to_css(),
    ) + "\n"
}

fn render_note(note: &PositionedNote, theme: &Theme) -> String {
    let mut s = String::new();

    // Note rectangle
    s.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
        note.x, note.y, note.width, note.height,
        theme.note_fill.to_css(),
        theme.note_border.to_css(),
    ));
    s.push('\n');

    // Note text
    s.push_str(&format!(
        r#"<text class="seq-note" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}">{}</text>"#,
        note.x + note.width / 2.0,
        note.y + note.height / 2.0,
        theme.note_text.to_css(),
        escape_xml(&note.text),
    ));
    s.push('\n');

    s
}

fn render_message(msg: &PositionedMessage, theme: &Theme) -> String {
    let mut s = String::new();
    let line_color = theme.line_color.to_css();

    if msg.is_self {
        // Self-message: polyline looping right then back down
        let x = msg.from_x;
        let sw = msg.self_width;
        let sh = msg.self_height;
        let y1 = msg.y;
        let y2 = y1 + sh;

        let (dasharray, marker) = arrow_attrs(msg.arrow);

        s.push_str(&format!(
            r#"<polyline points="{},{} {},{} {},{} {},{}" fill="none" stroke="{}" stroke-width="2"{}{}/>"#,
            x, y1,
            x + sw, y1,
            x + sw, y2,
            x, y2,
            line_color,
            dasharray,
            marker,
        ));
        s.push('\n');

        // Label to the right
        s.push_str(&format!(
            r#"<text class="seq-label" x="{}" y="{}" text-anchor="start" dominant-baseline="auto" fill="{}">{}</text>"#,
            x + sw + 5.0,
            y1 + sh / 2.0 + 4.0,
            theme.text_color.to_css(),
            escape_xml(&msg.label),
        ));
        s.push('\n');
    } else {
        // Regular message: horizontal line with arrowhead
        let (dasharray, marker) = arrow_attrs(msg.arrow);

        s.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"{}{}/>"#,
            msg.from_x, msg.y, msg.to_x, msg.y,
            line_color,
            dasharray,
            marker,
        ));
        s.push('\n');

        // Label centered above the arrow
        let mid_x = (msg.from_x + msg.to_x) / 2.0;
        s.push_str(&format!(
            r#"<text class="seq-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="auto" fill="{}">{}</text>"#,
            mid_x,
            msg.y - 5.0,
            theme.text_color.to_css(),
            escape_xml(&msg.label),
        ));
        s.push('\n');
    }

    // Autonumber
    if let Some(num) = msg.number {
        let num_x = if msg.is_self {
            msg.from_x - 12.0
        } else {
            msg.from_x.min(msg.to_x) - 5.0
        };
        let num_y = msg.y;
        s.push_str(&format!(
            r#"<circle cx="{}" cy="{}" r="8" fill="{}"/>"#,
            num_x, num_y, theme.actor_fill.to_css(),
        ));
        s.push('\n');
        s.push_str(&format!(
            r#"<text class="seq-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="central" fill="{}" font-size="10">{}</text>"#,
            num_x, num_y, theme.actor_text.to_css(), num,
        ));
        s.push('\n');
    }

    s
}

fn arrow_attrs(arrow: ArrowType) -> (&'static str, &'static str) {
    match arrow {
        ArrowType::SolidArrow => (
            "",
            r#" marker-end="url(#seq-arrowhead)""#,
        ),
        ArrowType::DottedArrow => (
            r#" stroke-dasharray="5,5""#,
            r#" marker-end="url(#seq-arrowhead)""#,
        ),
        ArrowType::SolidOpen => (
            "",
            r#" marker-end="url(#seq-arrowhead-open)""#,
        ),
        ArrowType::DottedOpen => (
            r#" stroke-dasharray="5,5""#,
            r#" marker-end="url(#seq-arrowhead-open)""#,
        ),
        ArrowType::SolidCross => (
            "",
            r#" marker-end="url(#seq-cross)""#,
        ),
        ArrowType::DottedCross => (
            r#" stroke-dasharray="5,5""#,
            r#" marker-end="url(#seq-cross)""#,
        ),
    }
}

fn render_block(block: &PositionedBlock, theme: &Theme) -> String {
    let mut s = String::new();

    // Block background
    s.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{}" stroke="{}" stroke-width="1"/>"#,
        block.x, block.y, block.width, block.height,
        theme.loop_fill.to_css(),
        theme.loop_line.to_css(),
    ));
    s.push('\n');

    // Block kind label tab
    let kind_text = match block.kind {
        BlockKind::Alt => "alt",
        BlockKind::Loop => "loop",
        BlockKind::Opt => "opt",
        BlockKind::Par => "par",
        BlockKind::Critical => "critical",
        BlockKind::Break => "break",
        BlockKind::Rect => "rect",
    };
    let tab_text = if block.label.is_empty() {
        kind_text.to_string()
    } else {
        format!("{} [{}]", kind_text, block.label)
    };
    let tab_w = tab_text.len() as f64 * 7.5 + 16.0;
    let tab_h = 18.0;

    // Label tab background
    s.push_str(&format!(
        r#"<polygon points="{},{} {},{} {},{} {},{} {},{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
        block.x, block.y,
        block.x + tab_w, block.y,
        block.x + tab_w, block.y + tab_h - 4.0,
        block.x + tab_w - 4.0, block.y + tab_h,
        block.x, block.y + tab_h,
        theme.label_box_fill.to_css(),
        theme.loop_line.to_css(),
    ));
    s.push('\n');

    // Label text
    s.push_str(&format!(
        r#"<text class="seq-label" x="{}" y="{}" text-anchor="start" dominant-baseline="central" fill="{}" font-weight="bold">{}</text>"#,
        block.x + 4.0,
        block.y + tab_h / 2.0,
        theme.text_color.to_css(),
        escape_xml(&tab_text),
    ));
    s.push('\n');

    // Section dividers
    for divider in &block.sections {
        // Dashed line
        s.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-dasharray="5,5"/>"#,
            block.x, divider.y, block.x + block.width, divider.y,
            theme.loop_line.to_css(),
        ));
        s.push('\n');

        // Divider label
        if let Some(label) = &divider.label {
            // Small label tab
            let div_text = format!("[{}]", label);
            s.push_str(&format!(
                r#"<text class="seq-label" x="{}" y="{}" text-anchor="start" dominant-baseline="auto" fill="{}" font-style="italic">{}</text>"#,
                block.x + 8.0,
                divider.y + 14.0,
                theme.text_color.to_css(),
                escape_xml(&div_text),
            ));
            s.push('\n');
        }
    }

    s
}
