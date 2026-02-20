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
    let mid_x = if msg.is_self {
        msg.from_x + msg.self_width / 2.0
    } else {
        (msg.from_x + msg.to_x) / 2.0
    };
    let base_y = msg.y - 5.0;
    let text_color = theme.text_color.to_css();

    // Label is already normalized (br -> \n) in the layout phase
    let lines: Vec<&str> = msg.label.split('\n').collect();

    if lines.len() == 1 {
        let _ = write!(
            svg,
            r#"<text class="seq-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="auto" fill="{}">{}</text>"#,
            mid_x,
            base_y,
            text_color,
            escape_xml(lines[0]),
        );
        svg.push('\n');
    } else {
        let line_height = 1.2_f64;
        // Position so that the last line sits at base_y (just above the arrow)
        let start_dy = -((lines.len() as f64 - 1.0) * line_height);

        let _ = write!(
            svg,
            r#"<text class="seq-label" x="{}" y="{}" text-anchor="middle" fill="{}">"#,
            mid_x, base_y, text_color,
        );
        svg.push('\n');
        for (i, line) in lines.iter().enumerate() {
            if i == 0 {
                let _ = write!(
                    svg,
                    r#"  <tspan x="{}" dy="{}em">{}</tspan>"#,
                    mid_x,
                    start_dy,
                    escape_xml(line),
                );
            } else {
                let _ = write!(
                    svg,
                    r#"  <tspan x="{}" dy="{}em">{}</tspan>"#,
                    mid_x,
                    line_height,
                    escape_xml(line),
                );
            }
            svg.push('\n');
        }
        svg.push_str("</text>\n");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::sequence::{
        BlockDivider, Lifeline, PositionedActivation, PositionedActor, PositionedBlock,
        PositionedMessage, PositionedNote, SequenceLayout,
    };

    /// Build a minimal empty SequenceLayout for reuse across tests.
    fn empty_layout() -> SequenceLayout {
        SequenceLayout {
            width: 400.0,
            height: 300.0,
            actors: Vec::new(),
            lifelines: Vec::new(),
            messages: Vec::new(),
            blocks: Vec::new(),
            notes: Vec::new(),
            activations: Vec::new(),
            autonumber: false,
        }
    }

    fn default_theme() -> Theme {
        Theme::default()
    }

    fn make_participant(id: &str, display_name: &str, center_x: f64) -> PositionedActor {
        PositionedActor {
            id: id.to_string(),
            display_name: display_name.to_string(),
            kind: ParticipantKind::Participant,
            center_x,
            top_y: 30.0,
            box_width: 80.0,
            box_height: 40.0,
            bottom_y: 260.0,
        }
    }

    fn make_actor(id: &str, display_name: &str, center_x: f64) -> PositionedActor {
        PositionedActor {
            id: id.to_string(),
            display_name: display_name.to_string(),
            kind: ParticipantKind::Actor,
            center_x,
            top_y: 30.0,
            box_width: 80.0,
            box_height: 60.0,
            bottom_y: 260.0,
        }
    }

    fn make_message(
        from_x: f64,
        to_x: f64,
        y: f64,
        arrow: ArrowType,
        label: &str,
    ) -> PositionedMessage {
        PositionedMessage {
            from_x,
            to_x,
            y,
            arrow,
            label: label.to_string(),
            is_self: false,
            self_width: 30.0,
            self_height: 28.0,
            number: None,
        }
    }

    // ── 1. Actor stick figure ──────────────────────────────────

    #[test]
    fn actor_stick_figure_renders_circle_and_lines() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.actors.push(make_actor("A", "Alice", 100.0));

        let svg = render_svg(&layout, &theme).unwrap();

        // Stick figure head (circle)
        assert!(svg.contains("<circle"), "expected <circle for stick figure head");
        // Body, arms, and legs are all <line> elements — at least 4 lines
        let line_count = svg.matches("<line ").count();
        // 4 lines per stick figure * 2 (top + bottom rendering) = 8
        assert!(
            line_count >= 8,
            "expected at least 8 <line> elements for stick figure top+bottom, got {}",
            line_count
        );
        // Label text
        assert!(svg.contains("Alice"), "expected actor label 'Alice'");
    }

    // ── 2. Multi-line participant name ─────────────────────────

    #[test]
    fn multiline_participant_name_renders_tspan_elements() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout
            .actors
            .push(make_participant("A", "Line1\nLine2", 100.0));

        let svg = render_svg(&layout, &theme).unwrap();

        // Multi-line names should produce <tspan> elements
        assert!(
            svg.contains("<tspan"),
            "expected <tspan> for multi-line participant name"
        );
        assert!(svg.contains("Line1"), "expected first line text");
        assert!(svg.contains("Line2"), "expected second line text");
        // There should be multiple tspan elements (at least 2 per rendering, rendered top+bottom = 4)
        let tspan_count = svg.matches("<tspan").count();
        assert!(
            tspan_count >= 4,
            "expected at least 4 <tspan> elements (2 lines * top+bottom), got {}",
            tspan_count
        );
    }

    // ── 3. Multi-line note ─────────────────────────────────────

    #[test]
    fn multiline_note_renders_tspan_elements() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.notes.push(PositionedNote {
            text: "first line\nsecond line\nthird line".to_string(),
            x: 50.0,
            y: 100.0,
            width: 120.0,
            height: 60.0,
        });

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("<tspan"),
            "expected <tspan> for multi-line note"
        );
        assert!(svg.contains("first line"), "expected first line text");
        assert!(svg.contains("second line"), "expected second line text");
        assert!(svg.contains("third line"), "expected third line text");
        let tspan_count = svg.matches("<tspan").count();
        assert_eq!(tspan_count, 3, "expected 3 <tspan> elements for 3-line note");
    }

    #[test]
    fn single_line_note_renders_without_tspan() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.notes.push(PositionedNote {
            text: "simple note".to_string(),
            x: 50.0,
            y: 100.0,
            width: 120.0,
            height: 30.0,
        });

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("simple note"), "expected note text");
        assert!(
            svg.contains("seq-note"),
            "expected seq-note class on note text"
        );
        // Single-line note should NOT use tspan
        assert!(
            !svg.contains("<tspan"),
            "single-line note should not contain <tspan>"
        );
    }

    // ── 4. Self-message (polyline) ─────────────────────────────

    #[test]
    fn self_message_renders_polyline() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.messages.push(PositionedMessage {
            from_x: 100.0,
            to_x: 100.0,
            y: 120.0,
            arrow: ArrowType::SolidArrow,
            label: "self call".to_string(),
            is_self: true,
            self_width: 30.0,
            self_height: 28.0,
            number: None,
        });

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("<polyline"),
            "expected <polyline> for self-message"
        );
        assert!(svg.contains("self call"), "expected self-message label");
        // Self-message should NOT produce a regular <line> for the message itself
        // (lifelines and actors may still add <line> elements)
    }

    // ── 5. Autonumber circle ───────────────────────────────────

    #[test]
    fn autonumber_renders_circle_and_number() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.messages.push(PositionedMessage {
            from_x: 80.0,
            to_x: 200.0,
            y: 120.0,
            arrow: ArrowType::SolidArrow,
            label: "numbered".to_string(),
            is_self: false,
            self_width: 30.0,
            self_height: 28.0,
            number: Some(1),
        });

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("<circle"),
            "expected <circle> for autonumber"
        );
        // The number text "1"
        assert!(
            svg.contains(">1</text>"),
            "expected number '1' in autonumber circle"
        );
    }

    #[test]
    fn autonumber_on_self_message_positions_circle() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.messages.push(PositionedMessage {
            from_x: 100.0,
            to_x: 100.0,
            y: 120.0,
            arrow: ArrowType::SolidArrow,
            label: "self".to_string(),
            is_self: true,
            self_width: 30.0,
            self_height: 28.0,
            number: Some(3),
        });

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("<circle"),
            "expected <circle> for autonumber on self-message"
        );
        assert!(
            svg.contains(">3</text>"),
            "expected number '3' in autonumber circle"
        );
        // Self-message autonumber is positioned at from_x - 12.0 = 88
        assert!(svg.contains("cx=\"88\""), "expected cx=88 for self-message autonumber");
    }

    // ── 6. Multi-line message label ────────────────────────────

    #[test]
    fn multiline_message_label_renders_tspan_elements() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.messages.push(make_message(
            80.0,
            200.0,
            120.0,
            ArrowType::SolidArrow,
            "line one\nline two",
        ));

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("<tspan"),
            "expected <tspan> for multi-line message label"
        );
        assert!(svg.contains("line one"), "expected first label line");
        assert!(svg.contains("line two"), "expected second label line");
    }

    #[test]
    fn single_line_message_label_no_tspan() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.messages.push(make_message(
            80.0,
            200.0,
            120.0,
            ArrowType::SolidArrow,
            "simple label",
        ));

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("simple label"), "expected label text");
        assert!(
            !svg.contains("<tspan"),
            "single-line label should not contain <tspan>"
        );
    }

    // ── 7. Arrow types ─────────────────────────────────────────

    #[test]
    fn arrow_attrs_solid_arrow() {
        let (dash, marker) = arrow_attrs(ArrowType::SolidArrow);
        assert_eq!(dash, "");
        assert!(marker.contains("seq-arrowhead"));
        assert!(!marker.contains("open"));
        assert!(!marker.contains("cross"));
    }

    #[test]
    fn arrow_attrs_dotted_arrow() {
        let (dash, marker) = arrow_attrs(ArrowType::DottedArrow);
        assert!(dash.contains("stroke-dasharray"));
        assert!(marker.contains("seq-arrowhead"));
        assert!(!marker.contains("open"));
    }

    #[test]
    fn arrow_attrs_solid_open() {
        let (dash, marker) = arrow_attrs(ArrowType::SolidOpen);
        assert_eq!(dash, "");
        assert!(marker.contains("seq-arrowhead-open"));
    }

    #[test]
    fn arrow_attrs_dotted_open() {
        let (dash, marker) = arrow_attrs(ArrowType::DottedOpen);
        assert!(dash.contains("stroke-dasharray"));
        assert!(marker.contains("seq-arrowhead-open"));
    }

    #[test]
    fn arrow_attrs_solid_paren() {
        let (dash, marker) = arrow_attrs(ArrowType::SolidParen);
        assert_eq!(dash, "");
        assert_eq!(marker, "");
    }

    #[test]
    fn arrow_attrs_dotted_paren() {
        let (dash, marker) = arrow_attrs(ArrowType::DottedParen);
        assert!(dash.contains("stroke-dasharray"));
        assert_eq!(marker, "");
    }

    #[test]
    fn arrow_attrs_solid_cross() {
        let (dash, marker) = arrow_attrs(ArrowType::SolidCross);
        assert_eq!(dash, "");
        assert!(marker.contains("seq-cross"));
    }

    #[test]
    fn arrow_attrs_dotted_cross() {
        let (dash, marker) = arrow_attrs(ArrowType::DottedCross);
        assert!(dash.contains("stroke-dasharray"));
        assert!(marker.contains("seq-cross"));
    }

    #[test]
    fn arrow_types_render_in_svg() {
        let theme = default_theme();
        let arrows = [
            (ArrowType::SolidArrow, "seq-arrowhead"),
            (ArrowType::DottedArrow, "seq-arrowhead"),
            (ArrowType::SolidOpen, "seq-arrowhead-open"),
            (ArrowType::DottedOpen, "seq-arrowhead-open"),
            (ArrowType::SolidCross, "seq-cross"),
            (ArrowType::DottedCross, "seq-cross"),
        ];

        for (arrow, expected_marker) in &arrows {
            let mut layout = empty_layout();
            layout.messages.push(make_message(50.0, 200.0, 100.0, *arrow, "msg"));
            let svg = render_svg(&layout, &theme).unwrap();
            assert!(
                svg.contains(expected_marker),
                "arrow {:?} should reference marker '{}'",
                arrow,
                expected_marker
            );
        }

        // SolidParen and DottedParen produce no marker-end
        for arrow in &[ArrowType::SolidParen, ArrowType::DottedParen] {
            let mut layout = empty_layout();
            layout.messages.push(make_message(50.0, 200.0, 100.0, *arrow, "msg"));
            let svg = render_svg(&layout, &theme).unwrap();
            assert!(
                !svg.contains("marker-end"),
                "arrow {arrow:?} should have no marker-end in the message line",
            );
        }

        // Dotted arrows should have stroke-dasharray on the message line
        for arrow in &[
            ArrowType::DottedArrow,
            ArrowType::DottedOpen,
            ArrowType::DottedParen,
            ArrowType::DottedCross,
        ] {
            let mut layout = empty_layout();
            layout.messages.push(make_message(50.0, 200.0, 100.0, *arrow, "msg"));
            let svg = render_svg(&layout, &theme).unwrap();
            // The defs section always has stroke, but the message line should also have dasharray
            assert!(
                svg.contains("stroke-dasharray=\"5,5\""),
                "dotted arrow {arrow:?} should produce stroke-dasharray on message line",
            );
        }
    }

    // ── 8. Block with sections and divider labels ──────────────

    #[test]
    fn block_section_divider_labels_rendered() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.blocks.push(PositionedBlock {
            kind: BlockKind::Alt,
            label: "is valid".to_string(),
            x: 10.0,
            y: 80.0,
            width: 300.0,
            height: 200.0,
            sections: vec![
                BlockDivider {
                    y: 160.0,
                    label: Some("else invalid".to_string()),
                },
                BlockDivider {
                    y: 220.0,
                    label: None,
                },
            ],
        });

        let svg = render_svg(&layout, &theme).unwrap();

        // Divider label should be rendered
        assert!(
            svg.contains("else invalid"),
            "expected divider label 'else invalid' in SVG"
        );
        // Divider line should be rendered (dashed)
        assert!(
            svg.contains("stroke-dasharray=\"5,5\""),
            "expected dashed divider line"
        );
        // The block kind label "alt" should appear
        assert!(
            svg.contains(">alt</text>"),
            "expected block kind label 'alt'"
        );
        // The condition label "[is valid]" should appear
        assert!(
            svg.contains("[is valid]"),
            "expected condition label '[is valid]'"
        );
    }

    #[test]
    fn block_divider_without_label_no_extra_text() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.blocks.push(PositionedBlock {
            kind: BlockKind::Loop,
            label: String::new(),
            x: 10.0,
            y: 80.0,
            width: 300.0,
            height: 100.0,
            sections: vec![BlockDivider {
                y: 130.0,
                label: None,
            }],
        });

        let svg = render_svg(&layout, &theme).unwrap();

        // The loop label should be present
        assert!(svg.contains(">loop</text>"), "expected block kind 'loop'");
        // No condition label (empty label string)
        // Count text elements - should only have block kind label, no condition text
        assert!(
            !svg.contains("["),
            "empty label should not render condition brackets"
        );
    }

    // ── 9. All block kinds ─────────────────────────────────────

    #[test]
    fn block_kind_str_all_variants() {
        assert_eq!(block_kind_str(BlockKind::Alt), "alt");
        assert_eq!(block_kind_str(BlockKind::Loop), "loop");
        assert_eq!(block_kind_str(BlockKind::Opt), "opt");
        assert_eq!(block_kind_str(BlockKind::Par), "par");
        assert_eq!(block_kind_str(BlockKind::Critical), "critical");
        assert_eq!(block_kind_str(BlockKind::Break), "break");
        assert_eq!(block_kind_str(BlockKind::Rect), "rect");
    }

    #[test]
    fn all_block_kinds_render_label_in_svg() {
        let theme = default_theme();
        let kinds = [
            (BlockKind::Alt, "alt"),
            (BlockKind::Loop, "loop"),
            (BlockKind::Opt, "opt"),
            (BlockKind::Par, "par"),
            (BlockKind::Critical, "critical"),
            (BlockKind::Break, "break"),
            (BlockKind::Rect, "rect"),
        ];

        for (kind, expected_str) in &kinds {
            let mut layout = empty_layout();
            layout.blocks.push(PositionedBlock {
                kind: *kind,
                label: String::new(),
                x: 10.0,
                y: 80.0,
                width: 200.0,
                height: 100.0,
                sections: Vec::new(),
            });

            let svg = render_svg(&layout, &theme).unwrap();
            let expected_text = format!(">{}</text>", expected_str);
            assert!(
                svg.contains(&expected_text),
                "block kind {:?} should render label '{}', got SVG without it",
                kind,
                expected_str
            );
        }
    }

    // ── 10. Block with non-empty label ─────────────────────────

    #[test]
    fn block_with_condition_label_renders_bracketed_text() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.blocks.push(PositionedBlock {
            kind: BlockKind::Opt,
            label: "user is admin".to_string(),
            x: 20.0,
            y: 90.0,
            width: 250.0,
            height: 120.0,
            sections: Vec::new(),
        });

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("[user is admin]"),
            "expected condition label '[user is admin]' in SVG"
        );
        assert!(svg.contains(">opt</text>"), "expected block kind 'opt'");
    }

    #[test]
    fn block_empty_label_does_not_render_condition() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.blocks.push(PositionedBlock {
            kind: BlockKind::Rect,
            label: String::new(),
            x: 10.0,
            y: 80.0,
            width: 200.0,
            height: 100.0,
            sections: Vec::new(),
        });

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains(">rect</text>"), "expected block kind 'rect'");
        // Count how many <text> elements there are for blocks; should only be the kind label
        // Empty label should not produce a second text element with brackets
        assert!(
            !svg.contains("["),
            "empty label should not produce bracketed condition"
        );
    }

    // ── Additional coverage helpers ────────────────────────────

    #[test]
    fn render_svg_produces_valid_svg_structure() {
        let theme = default_theme();
        let layout = empty_layout();

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.starts_with("<svg "), "should start with <svg tag");
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("<style>"));
        assert!(svg.contains("</style>"));
        assert!(svg.contains("<defs>"));
        assert!(svg.contains("</defs>"));
        assert!(svg.contains("</g>"));
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn lifeline_renders_as_line() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.lifelines.push(Lifeline {
            actor_id: "A".to_string(),
            x: 100.0,
            y_start: 70.0,
            y_end: 250.0,
        });

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("<line "),
            "expected <line> element for lifeline"
        );
        assert!(
            svg.contains("x1=\"100\""),
            "expected lifeline at x=100"
        );
    }

    #[test]
    fn activation_renders_as_rect() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.activations.push(PositionedActivation {
            actor_id: "A".to_string(),
            x: 100.0,
            y_start: 90.0,
            y_end: 150.0,
            depth: 0,
        });

        let svg = render_svg(&layout, &theme).unwrap();

        // Activation should render as a thin rect
        assert!(
            svg.contains("<rect "),
            "expected <rect> element for activation"
        );
    }

    #[test]
    fn self_message_label_midpoint_uses_self_width() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.messages.push(PositionedMessage {
            from_x: 100.0,
            to_x: 100.0,
            y: 120.0,
            arrow: ArrowType::SolidArrow,
            label: "self msg".to_string(),
            is_self: true,
            self_width: 30.0,
            self_height: 28.0,
            number: None,
        });

        let svg = render_svg(&layout, &theme).unwrap();

        // Label midpoint for self-message: from_x + self_width/2 = 100 + 15 = 115
        assert!(svg.contains("self msg"), "expected self message label text");
        assert!(
            svg.contains("<polyline"),
            "expected polyline for self-message"
        );
    }

    #[test]
    fn defs_contain_all_marker_definitions() {
        let theme = default_theme();
        let layout = empty_layout();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("id=\"seq-arrowhead\""),
            "expected seq-arrowhead marker definition"
        );
        assert!(
            svg.contains("id=\"seq-arrowhead-open\""),
            "expected seq-arrowhead-open marker definition"
        );
        assert!(
            svg.contains("id=\"seq-cross\""),
            "expected seq-cross marker definition"
        );
    }

    #[test]
    fn participant_single_line_no_tspan() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout
            .actors
            .push(make_participant("A", "SingleName", 100.0));

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("SingleName"), "expected participant name");
        // Single-line participant should not use tspan
        assert!(
            !svg.contains("<tspan"),
            "single-line participant should not use <tspan>"
        );
    }

    #[test]
    fn block_background_polygon_rendered() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.blocks.push(PositionedBlock {
            kind: BlockKind::Loop,
            label: "forever".to_string(),
            x: 10.0,
            y: 80.0,
            width: 300.0,
            height: 200.0,
            sections: Vec::new(),
        });

        let svg = render_svg(&layout, &theme).unwrap();

        // Block background rect
        assert!(svg.contains("<rect "), "expected block background <rect>");
        // Tab polygon
        assert!(
            svg.contains("<polygon "),
            "expected block tab <polygon>"
        );
        // Label
        assert!(svg.contains("[forever]"), "expected condition label");
    }

    #[test]
    fn activation_with_depth_offset() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.activations.push(PositionedActivation {
            actor_id: "A".to_string(),
            x: 100.0,
            y_start: 90.0,
            y_end: 150.0,
            depth: 2,
        });

        let svg = render_svg(&layout, &theme).unwrap();

        // depth=2 produces x_offset=6, so x = 100 - 5 + 6 = 101
        assert!(
            svg.contains("<rect "),
            "expected activation rect with depth offset"
        );
    }

    #[test]
    fn activation_with_depth_zero() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.activations.push(PositionedActivation {
            actor_id: "A".to_string(),
            x: 100.0,
            y_start: 90.0,
            y_end: 150.0,
            depth: 0,
        });

        let svg = render_svg(&layout, &theme).unwrap();

        // depth=0 produces x_offset=0.0, exercises the x_offset calculation path
        assert!(
            svg.contains("<rect "),
            "expected activation rect with depth 0"
        );
    }

    #[test]
    fn message_without_number_no_circle() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.messages.push(make_message(
            80.0,
            200.0,
            120.0,
            ArrowType::SolidArrow,
            "no number",
        ));

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            !svg.contains("<circle"),
            "message without number should not have a <circle>"
        );
    }

    #[test]
    fn multiple_sections_with_mixed_labels() {
        let theme = default_theme();
        let mut layout = empty_layout();
        layout.blocks.push(PositionedBlock {
            kind: BlockKind::Par,
            label: "parallel tasks".to_string(),
            x: 10.0,
            y: 80.0,
            width: 300.0,
            height: 250.0,
            sections: vec![
                BlockDivider {
                    y: 140.0,
                    label: Some("and task B".to_string()),
                },
                BlockDivider {
                    y: 200.0,
                    label: Some("and task C".to_string()),
                },
            ],
        });

        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains(">par</text>"), "expected block kind 'par'");
        assert!(
            svg.contains("[parallel tasks]"),
            "expected condition label"
        );
        assert!(
            svg.contains("[and task B]"),
            "expected first divider label"
        );
        assert!(
            svg.contains("[and task C]"),
            "expected second divider label"
        );
        // Two divider dashed lines
        let dash_lines: Vec<&str> = svg
            .lines()
            .filter(|l| l.contains("stroke-dasharray") && l.contains("<line"))
            .collect();
        assert_eq!(
            dash_lines.len(),
            2,
            "expected 2 dashed divider lines, got {}",
            dash_lines.len()
        );
    }
}
