//! ASCII renderer for sequence diagrams.
//!
//! Converts a `SequenceLayout` (the same data consumed by `svg_sequence`) into
//! a Unicode text art representation.

use crate::ast::sequence::{ArrowType, BlockKind, ParticipantKind};
use crate::error::Result;
use crate::layout::sequence::{PositionedMessage, SequenceLayout};
use crate::render::ascii_canvas::TextCanvas;

/// Render a positioned sequence diagram layout as ASCII/Unicode text art.
pub fn render_ascii(layout: &SequenceLayout) -> Result<String> {
    let mut canvas = TextCanvas::from_pixel_size(layout.width, layout.height);

    // 0. Draw participant boxes (visual grouping)
    for pbox in &layout.participant_boxes {
        draw_participant_box(&mut canvas, pbox);
    }

    // 1. Draw blocks (background layer)
    for block in &layout.blocks {
        draw_block(&mut canvas, block);
    }

    // 2. Draw lifelines
    for lifeline in &layout.lifelines {
        draw_lifeline(&mut canvas, lifeline);
    }

    // 3. Draw activations (thicken lifelines)
    for activation in &layout.activations {
        draw_activation(&mut canvas, activation);
    }

    // 4. Draw actor boxes (top and bottom)
    for actor in &layout.actors {
        draw_actor(&mut canvas, actor);
    }

    // 5. Draw notes
    for note in &layout.notes {
        draw_note(&mut canvas, note);
    }

    // 6. Draw messages (foreground)
    for msg in &layout.messages {
        draw_message(&mut canvas, msg, layout.autonumber);
    }

    Ok(canvas.to_string())
}

fn draw_actor(canvas: &mut TextCanvas, actor: &crate::layout::sequence::PositionedActor) {
    let half_w = actor.box_width / 2.0;

    match actor.kind {
        ParticipantKind::Participant => {
            // Top box
            canvas.draw_box_px(
                actor.center_x - half_w,
                actor.top_y,
                actor.box_width,
                actor.box_height,
            );
            canvas.draw_text_centered_px(
                actor.center_x,
                actor.top_y + actor.box_height / 2.0,
                &actor.display_name,
            );

            // Bottom box
            canvas.draw_box_px(
                actor.center_x - half_w,
                actor.bottom_y,
                actor.box_width,
                actor.box_height,
            );
            canvas.draw_text_centered_px(
                actor.center_x,
                actor.bottom_y + actor.box_height / 2.0,
                &actor.display_name,
            );
        }
        ParticipantKind::Actor => {
            // Stick figure: draw a simple representation at top
            let col = canvas.px_to_col(actor.center_x);
            let top_row = canvas.px_to_row(actor.top_y);

            // Head
            canvas.put(col, top_row, 'O');
            // Body
            if top_row + 1 < canvas.px_to_row(actor.top_y + actor.box_height) {
                canvas.put(col, top_row + 1, '│');
                // Arms
                if col > 0 {
                    canvas.put(col - 1, top_row + 1, '─');
                }
                canvas.put(col + 1, top_row + 1, '─');
            }
            // Legs
            if top_row + 2 < canvas.px_to_row(actor.top_y + actor.box_height) {
                if col > 0 {
                    canvas.put(col - 1, top_row + 2, '/');
                }
                canvas.put(col + 1, top_row + 2, '\\');
            }
            // Label below
            let label_row = canvas.px_to_row(actor.top_y + actor.box_height);
            let name_len = actor.display_name.chars().count();
            let start_col = col.saturating_sub(name_len / 2);
            canvas.draw_text(start_col, label_row, &actor.display_name);

            // Bottom: same stick figure
            let bottom_row = canvas.px_to_row(actor.bottom_y);
            canvas.put(col, bottom_row, 'O');
            if bottom_row + 1 < canvas.px_to_row(actor.bottom_y + actor.box_height) {
                canvas.put(col, bottom_row + 1, '│');
                if col > 0 {
                    canvas.put(col - 1, bottom_row + 1, '─');
                }
                canvas.put(col + 1, bottom_row + 1, '─');
            }
            if bottom_row + 2 < canvas.px_to_row(actor.bottom_y + actor.box_height) {
                if col > 0 {
                    canvas.put(col - 1, bottom_row + 2, '/');
                }
                canvas.put(col + 1, bottom_row + 2, '\\');
            }
        }
    }
}

fn draw_lifeline(canvas: &mut TextCanvas, lifeline: &crate::layout::sequence::Lifeline) {
    let col = canvas.px_to_col(lifeline.x);
    let r_start = canvas.px_to_row(lifeline.y_start);
    let r_end = canvas.px_to_row(lifeline.y_end);
    // Draw as dashed vertical line
    canvas.draw_dashed_vline(col, r_start, r_end);
}

fn draw_activation(
    canvas: &mut TextCanvas,
    activation: &crate::layout::sequence::PositionedActivation,
) {
    let col = canvas.px_to_col(activation.x);
    let r_start = canvas.px_to_row(activation.y_start);
    let r_end = canvas.px_to_row(activation.y_end);
    // Draw activation as a solid vertical bar (overwriting the dashed lifeline)
    for r in r_start..=r_end {
        canvas.put(col, r, '┃');
    }
}

fn draw_message(canvas: &mut TextCanvas, msg: &PositionedMessage, autonumber: bool) {
    let row = canvas.px_to_row(msg.y);

    // Build label with optional autonumber
    let label = if autonumber {
        if let Some(n) = msg.number {
            format!("{}) {}", n, msg.label)
        } else {
            msg.label.clone()
        }
    } else {
        msg.label.clone()
    };

    if msg.is_self {
        draw_self_message(canvas, msg, row, &label);
        return;
    }

    let c_from = canvas.px_to_col(msg.from_x);
    let c_to = canvas.px_to_col(msg.to_x);

    let (left_col, right_col, going_right) = if c_from < c_to {
        (c_from, c_to, true)
    } else {
        (c_to, c_from, false)
    };

    // Draw label above the arrow
    if !label.is_empty() {
        let mid_col = (left_col + right_col) / 2;
        let label_len = label.chars().count();
        let label_start = mid_col.saturating_sub(label_len / 2);
        if row > 0 {
            canvas.draw_text(label_start, row - 1, &label);
        }
    }

    // Determine line characters based on arrow type
    let (line_ch, arrow_ch) = match msg.arrow {
        ArrowType::SolidArrow | ArrowType::SolidCross | ArrowType::SolidParen => {
            if going_right {
                ('─', '▶')
            } else {
                ('─', '◀')
            }
        }
        ArrowType::DottedArrow | ArrowType::DottedCross | ArrowType::DottedParen => {
            if going_right {
                ('╌', '▶')
            } else {
                ('╌', '◀')
            }
        }
        ArrowType::SolidOpen => {
            if going_right {
                ('─', '>')
            } else {
                ('─', '<')
            }
        }
        ArrowType::DottedOpen => {
            if going_right {
                ('╌', '>')
            } else {
                ('╌', '<')
            }
        }
    };

    // Draw the line
    for c in (left_col + 1)..right_col {
        canvas.put(c, row, line_ch);
    }

    // Draw arrowhead at the target end
    if going_right {
        canvas.put(right_col, row, arrow_ch);
    } else {
        canvas.put(left_col, row, arrow_ch);
    }
}

fn draw_self_message(canvas: &mut TextCanvas, msg: &PositionedMessage, row: usize, label: &str) {
    let col = canvas.px_to_col(msg.from_x);
    let self_w = (msg.self_width / 8.0).ceil() as usize;
    let self_w = self_w.max(3);

    // Draw label above
    if !label.is_empty() && row > 0 {
        canvas.draw_text(col + 1, row - 1, label);
    }

    // Draw the self-loop: right, down, left with arrow
    // Top horizontal: col -> col + self_w
    for c in (col + 1)..=(col + self_w) {
        canvas.put(c, row, '─');
    }
    // Corner
    canvas.put(col + self_w, row, '┐');

    // Vertical down
    let bottom_row = row + (msg.self_height / 14.0).ceil() as usize;
    let bottom_row = bottom_row.max(row + 1);
    for r in (row + 1)..bottom_row {
        canvas.put(col + self_w, r, '│');
    }

    // Corner
    canvas.put(col + self_w, bottom_row, '┘');

    // Bottom horizontal back with arrow
    for c in (col + 1)..col + self_w {
        canvas.put(c, bottom_row, '─');
    }
    canvas.put(col + 1, bottom_row, '▶');
}

fn draw_participant_box(
    canvas: &mut TextCanvas,
    pbox: &crate::layout::sequence::PositionedParticipantBox,
) {
    let left = canvas.px_to_col(pbox.x);
    let top = canvas.px_to_row(pbox.y);
    let right = canvas.px_to_col(pbox.x + pbox.width);
    let bottom = canvas.px_to_row(pbox.y + pbox.height);

    canvas.draw_box(left, top, right, bottom);

    if let Some(ref label) = pbox.label {
        let header = format!(" {} ", label);
        let max_w = right.saturating_sub(left).saturating_sub(1);
        let truncated: String = header.chars().take(max_w).collect();
        let center = left + (right.saturating_sub(left).saturating_sub(truncated.len())) / 2;
        canvas.draw_text(center.max(left + 1), top, &truncated);
    }
}

fn draw_block(canvas: &mut TextCanvas, block: &crate::layout::sequence::PositionedBlock) {
    let left = canvas.px_to_col(block.x);
    let top = canvas.px_to_row(block.y);
    let right = canvas.px_to_col(block.x + block.width);
    let bottom = canvas.px_to_row(block.y + block.height);

    canvas.draw_box(left, top, right, bottom);

    // Draw block type label in top-left
    let kind_str = match block.kind {
        BlockKind::Alt => "alt",
        BlockKind::Loop => "loop",
        BlockKind::Opt => "opt",
        BlockKind::Par => "par",
        BlockKind::Critical => "critical",
        BlockKind::Break => "break",
        BlockKind::Rect => "rect",
    };
    let header = format!(" {} {} ", kind_str, block.label);
    let max_w = right.saturating_sub(left).saturating_sub(1);
    let truncated: String = header.chars().take(max_w).collect();
    canvas.draw_text(left + 1, top, &truncated);

    // Draw section dividers (dashed horizontal lines)
    for divider in &block.sections {
        let div_row = canvas.px_to_row(divider.y);
        canvas.draw_dashed_hline(left + 1, right.saturating_sub(1), div_row);

        // Draw divider label
        if let Some(ref label) = divider.label {
            let div_label = format!(" {} ", label);
            canvas.draw_text(left + 1, div_row, &div_label);
        }
    }
}

fn draw_note(canvas: &mut TextCanvas, note: &crate::layout::sequence::PositionedNote) {
    let left = canvas.px_to_col(note.x);
    let top = canvas.px_to_row(note.y);
    let right = canvas.px_to_col(note.x + note.width);
    let bottom = canvas.px_to_row(note.y + note.height);

    // Draw note box with folded corner indicator
    canvas.draw_box(left, top, right, bottom);
    // Fold corner
    canvas.put(right, top, '┐');

    // Draw note text
    let available_w = right.saturating_sub(left).saturating_sub(2);
    if available_w > 0 {
        let mid_row = (top + bottom) / 2;
        let text = &note.text;
        let lines: Vec<&str> = text.split('\n').collect();
        let start_row = mid_row.saturating_sub(lines.len() / 2);
        for (i, line) in lines.iter().enumerate() {
            let row = start_row + i;
            if row > top && row < bottom {
                let truncated: String = line.chars().take(available_w).collect();
                canvas.draw_text(left + 1, row, &truncated);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::sequence::*;
    use crate::layout::sequence::*;

    fn simple_layout() -> SequenceLayout {
        SequenceLayout {
            width: 400.0,
            height: 300.0,
            autonumber: false,
            actors: vec![
                PositionedActor {
                    id: "Alice".to_string(),
                    display_name: "Alice".to_string(),
                    kind: ParticipantKind::Participant,
                    center_x: 100.0,
                    top_y: 30.0,
                    box_width: 80.0,
                    box_height: 30.0,
                    bottom_y: 240.0,
                },
                PositionedActor {
                    id: "Bob".to_string(),
                    display_name: "Bob".to_string(),
                    kind: ParticipantKind::Participant,
                    center_x: 300.0,
                    top_y: 30.0,
                    box_width: 80.0,
                    box_height: 30.0,
                    bottom_y: 240.0,
                },
            ],
            lifelines: vec![
                Lifeline {
                    actor_id: "Alice".to_string(),
                    x: 100.0,
                    y_start: 60.0,
                    y_end: 240.0,
                },
                Lifeline {
                    actor_id: "Bob".to_string(),
                    x: 300.0,
                    y_start: 60.0,
                    y_end: 240.0,
                },
            ],
            messages: vec![PositionedMessage {
                from_x: 100.0,
                to_x: 300.0,
                y: 120.0,
                arrow: ArrowType::SolidArrow,
                label: "Hello".to_string(),
                is_self: false,
                self_width: 0.0,
                self_height: 0.0,
                number: None,
            }],
            blocks: vec![],
            notes: vec![],
            activations: vec![],
            participant_boxes: vec![],
        }
    }

    #[test]
    fn test_render_simple_sequence() {
        let layout = simple_layout();
        let result = render_ascii(&layout).unwrap();
        assert!(!result.is_empty());
        assert!(
            result.contains("Alice"),
            "Output should contain 'Alice':\n{}",
            result
        );
        assert!(
            result.contains("Bob"),
            "Output should contain 'Bob':\n{}",
            result
        );
    }

    #[test]
    fn test_render_contains_arrow() {
        let layout = simple_layout();
        let result = render_ascii(&layout).unwrap();
        assert!(
            result.contains('▶') || result.contains('>'),
            "Output should contain arrow:\n{}",
            result
        );
    }

    #[test]
    fn test_render_contains_label() {
        let layout = simple_layout();
        let result = render_ascii(&layout).unwrap();
        assert!(
            result.contains("Hello"),
            "Output should contain message label 'Hello':\n{}",
            result
        );
    }
}
