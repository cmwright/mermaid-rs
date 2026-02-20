use crate::ast::sequence::*;
use crate::error::Result;
use crate::layout::text_measure::TextMeasurer;
use crate::render::html_util;
use crate::render::theme::Theme;
use std::collections::HashMap;

// ── Constants ────────────────────────────────────────────────

const ACTOR_MARGIN: f64 = 50.0;
const ACTOR_BOX_PAD_H: f64 = 16.0;
const ACTOR_BOX_PAD_V: f64 = 8.0;
const MESSAGE_SPACING: f64 = 20.0; // space before AND after each message arrow
const SELF_MSG_WIDTH: f64 = 30.0;
const SELF_MSG_HEIGHT: f64 = 28.0;
const BLOCK_PADDING: f64 = 8.0; // space at end of block (reduced to match nested inset spacing)
const BLOCK_HEADER_HEIGHT: f64 = 25.0; // space for label tab area
const BLOCK_SIDE_PADDING: f64 = 20.0;
const BLOCK_NEST_INSET: f64 = 8.0;
const NOTE_PADDING: f64 = 8.0;
const NOTE_MAX_WIDTH: f64 = 200.0;
const DIAGRAM_PADDING: f64 = 30.0;
const STICK_FIGURE_HEIGHT: f64 = 40.0;
const STICK_FIGURE_LABEL_GAP: f64 = 4.0;
const ACTOR_TO_FIRST_MSG_PADDING: f64 = 30.0; // Extra padding between top actors and first message
const LAST_MSG_TO_ACTOR_PADDING: f64 = 10.0; // Reduced padding between last message and bottom actors

// ── Positioned output types ─────────────────────────────────

#[derive(Debug, Clone)]
pub struct SequenceLayout {
    pub width: f64,
    pub height: f64,
    pub actors: Vec<PositionedActor>,
    pub lifelines: Vec<Lifeline>,
    pub messages: Vec<PositionedMessage>,
    pub blocks: Vec<PositionedBlock>,
    pub notes: Vec<PositionedNote>,
    pub activations: Vec<PositionedActivation>,
    pub autonumber: bool,
}

#[derive(Debug, Clone)]
pub struct PositionedActor {
    pub id: String,
    pub display_name: String,
    pub kind: ParticipantKind,
    pub center_x: f64,
    pub top_y: f64,
    pub box_width: f64,
    pub box_height: f64,
    pub bottom_y: f64,
}

#[derive(Debug, Clone)]
pub struct Lifeline {
    pub actor_id: String,
    pub x: f64,
    pub y_start: f64,
    pub y_end: f64,
}

#[derive(Debug, Clone)]
pub struct PositionedMessage {
    pub from_x: f64,
    pub to_x: f64,
    pub y: f64,
    pub arrow: ArrowType,
    pub label: String,
    pub is_self: bool,
    pub self_width: f64,
    pub self_height: f64,
    pub number: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct PositionedBlock {
    pub kind: BlockKind,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub sections: Vec<BlockDivider>,
}

#[derive(Debug, Clone)]
pub struct BlockDivider {
    pub y: f64,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PositionedNote {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct PositionedActivation {
    pub actor_id: String,
    pub x: f64,
    pub y_start: f64,
    pub y_end: f64,
    pub depth: usize,
}

// ── Layout algorithm ────────────────────────────────────────

pub fn layout_sequence(
    ast: &SequenceAst,
    measurer: &TextMeasurer,
    theme: &Theme,
) -> Result<SequenceLayout> {
    // Phase A: Resolve participants and measure display names
    let mut actor_infos: Vec<ActorInfo> = Vec::new();
    let actor_idx = build_actor_map(ast, measurer, theme, &mut actor_infos);

    // Phase B: Horizontal positioning
    // First pass: position actors without considering message widths
    position_actors_horizontal(&mut actor_infos);

    // Widen gaps for message labels
    widen_for_messages(ast, measurer, &actor_idx, &mut actor_infos);

    // Phase C: Vertical layout
    let top_box_height = actor_infos
        .iter()
        .map(|a| a.box_height)
        .fold(0.0_f64, f64::max);
    let lifeline_start_y = DIAGRAM_PADDING + top_box_height + ACTOR_TO_FIRST_MSG_PADDING;
    let mut cursor_y = lifeline_start_y;

    let mut messages = Vec::new();
    let mut blocks: Vec<PositionedBlock> = Vec::new();
    let mut notes = Vec::new();
    let mut activations: Vec<PositionedActivation> = Vec::new();
    let mut activation_stack: HashMap<String, Vec<f64>> = HashMap::new();
    let mut msg_number: usize = 0;

    layout_statements(
        &ast.statements,
        ast.autonumber,
        &actor_idx,
        &actor_infos,
        measurer,
        &mut cursor_y,
        &mut messages,
        &mut blocks,
        &mut notes,
        &mut activations,
        &mut activation_stack,
        &mut msg_number,
        0, // block_depth
    );

    // Close any unclosed activations
    for (actor_id, starts) in &activation_stack {
        for (depth, y_start) in starts.iter().enumerate() {
            if let Some(idx) = actor_idx.get(actor_id.as_str()) {
                activations.push(PositionedActivation {
                    actor_id: actor_id.clone(),
                    x: actor_infos[*idx].center_x,
                    y_start: *y_start,
                    y_end: cursor_y,
                    depth,
                });
            }
        }
    }

    // Phase D: Finalize
    cursor_y += LAST_MSG_TO_ACTOR_PADDING;
    let bottom_box_y = cursor_y;
    cursor_y += top_box_height + DIAGRAM_PADDING;

    // Build positioned actors (top and bottom boxes)
    let mut positioned_actors: Vec<PositionedActor> = Vec::new();
    for info in &actor_infos {
        positioned_actors.push(PositionedActor {
            id: info.id.clone(),
            display_name: info.display_name.clone(),
            kind: info.kind,
            center_x: info.center_x,
            top_y: DIAGRAM_PADDING,
            box_width: info.box_width,
            box_height: info.box_height,
            bottom_y: bottom_box_y,
        });
    }

    // Build lifelines - extend from bottom of top actors to top of bottom actors
    let lifeline_y_start = DIAGRAM_PADDING + top_box_height;
    let lifeline_y_end = bottom_box_y;
    let mut lifelines: Vec<Lifeline> = Vec::new();
    for info in &actor_infos {
        lifelines.push(Lifeline {
            actor_id: info.id.clone(),
            x: info.center_x,
            y_start: lifeline_y_start,
            y_end: lifeline_y_end,
        });
    }

    // Compute bounding box (consider actors, notes, and blocks)
    let rightmost_actor = actor_infos
        .iter()
        .map(|a| a.center_x + a.box_width / 2.0)
        .fold(0.0_f64, f64::max);
    let rightmost_note = notes.iter().map(|n| n.x + n.width).fold(0.0_f64, f64::max);
    let rightmost_block = blocks.iter().map(|b| b.x + b.width).fold(0.0_f64, f64::max);
    let width = rightmost_actor.max(rightmost_note).max(rightmost_block) + DIAGRAM_PADDING;
    let height = cursor_y;

    Ok(SequenceLayout {
        width,
        height,
        actors: positioned_actors,
        lifelines,
        messages,
        blocks,
        notes,
        activations,
        autonumber: ast.autonumber,
    })
}

// ── Internal helpers ────────────────────────────────────────

#[derive(Debug, Clone)]
struct ActorInfo {
    id: String,
    display_name: String,
    kind: ParticipantKind,
    center_x: f64,
    box_width: f64,
    box_height: f64,
}

fn build_actor_map<'a>(
    ast: &'a SequenceAst,
    measurer: &TextMeasurer,
    _theme: &Theme,
    actor_infos: &mut Vec<ActorInfo>,
) -> HashMap<&'a str, usize> {
    let mut actor_idx: HashMap<&'a str, usize> = HashMap::new();

    for p in &ast.participants {
        let display = p.display_name.as_deref().unwrap_or(&p.id);
        let clean = html_util::strip_html_tags(&html_util::normalize_br(display));
        let metrics = measurer.measure_multiline(&clean, 2.0);

        let (box_width, box_height) = match p.kind {
            ParticipantKind::Participant => {
                let w = metrics.width + ACTOR_BOX_PAD_H * 2.0;
                let h = metrics.height + ACTOR_BOX_PAD_V * 2.0;
                (w.max(40.0), h.max(30.0))
            }
            ParticipantKind::Actor => {
                // Stick figure: width is max of label width and figure width
                let figure_w = 30.0;
                let w = metrics.width.max(figure_w) + ACTOR_BOX_PAD_H;
                let h = STICK_FIGURE_HEIGHT + STICK_FIGURE_LABEL_GAP + metrics.height;
                (w.max(40.0), h)
            }
        };

        let idx = actor_infos.len();
        actor_idx.insert(&p.id, idx);
        actor_infos.push(ActorInfo {
            id: p.id.clone(),
            display_name: display.to_string(),
            kind: p.kind,
            center_x: 0.0,
            box_width,
            box_height,
        });
    }

    actor_idx
}

fn position_actors_horizontal(actor_infos: &mut [ActorInfo]) {
    let mut x = DIAGRAM_PADDING;
    for (i, info) in actor_infos.iter_mut().enumerate() {
        if i == 0 {
            x += info.box_width / 2.0;
        } else {
            x += ACTOR_MARGIN + info.box_width / 2.0;
        }
        info.center_x = x;
        x += info.box_width / 2.0;
    }
}

fn widen_for_messages(
    ast: &SequenceAst,
    measurer: &TextMeasurer,
    actor_idx: &HashMap<&str, usize>,
    actor_infos: &mut [ActorInfo],
) {
    // Collect required gap widths between adjacent actor pairs
    let mut required_gaps: HashMap<(usize, usize), f64> = HashMap::new();

    fn collect_message_gaps(
        stmts: &[SequenceStatement],
        measurer: &TextMeasurer,
        actor_idx: &HashMap<&str, usize>,
        required_gaps: &mut HashMap<(usize, usize), f64>,
    ) {
        for stmt in stmts {
            match stmt {
                SequenceStatement::Message(msg) => {
                    if msg.from == msg.to {
                        continue; // self-messages don't affect gaps
                    }
                    let normalized = html_util::normalize_br(&msg.label);
                    let label_w = if normalized.contains('\n') {
                        measurer.measure_multiline(&normalized, 2.0).width + 20.0
                    } else {
                        measurer.measure(&normalized).width + 20.0
                    };
                    if let (Some(&from_idx), Some(&to_idx)) = (
                        actor_idx.get(msg.from.as_str()),
                        actor_idx.get(msg.to.as_str()),
                    ) {
                        let lo = from_idx.min(to_idx);
                        let hi = from_idx.max(to_idx);
                        // This message spans from lo to hi; it needs label_w space
                        let entry = required_gaps.entry((lo, hi)).or_insert(0.0);
                        if label_w > *entry {
                            *entry = label_w;
                        }
                    }
                }
                SequenceStatement::Block(block) => {
                    for section in &block.sections {
                        collect_message_gaps(
                            &section.statements,
                            measurer,
                            actor_idx,
                            required_gaps,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    collect_message_gaps(&ast.statements, measurer, actor_idx, &mut required_gaps);

    // For each pair, widen by distributing extra space to intermediate gaps
    let mut extra_per_gap: Vec<f64> = vec![0.0; actor_infos.len().saturating_sub(1)];
    for ((lo, hi), needed) in &required_gaps {
        if *lo >= actor_infos.len() || *hi >= actor_infos.len() || lo == hi {
            continue;
        }
        let current_span = actor_infos[*hi].center_x - actor_infos[*lo].center_x;
        if *needed > current_span {
            let gap_count = (hi - lo) as f64;
            let extra = (*needed - current_span) / gap_count;
            for item in extra_per_gap.iter_mut().take(*hi).skip(*lo) {
                if extra > *item {
                    *item = extra;
                }
            }
        }
    }

    // Apply extra spacing
    let mut cumulative_shift = 0.0;
    for i in 0..actor_infos.len() {
        actor_infos[i].center_x += cumulative_shift;
        if i < extra_per_gap.len() {
            cumulative_shift += extra_per_gap[i];
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_statements(
    statements: &[SequenceStatement],
    autonumber: bool,
    actor_idx: &HashMap<&str, usize>,
    actor_infos: &[ActorInfo],
    measurer: &TextMeasurer,
    cursor_y: &mut f64,
    messages: &mut Vec<PositionedMessage>,
    blocks: &mut Vec<PositionedBlock>,
    notes: &mut Vec<PositionedNote>,
    activations: &mut Vec<PositionedActivation>,
    activation_stack: &mut HashMap<String, Vec<f64>>,
    msg_number: &mut usize,
    block_depth: usize,
) {
    for stmt in statements {
        match stmt {
            SequenceStatement::Message(msg) => {
                // Equal spacing before the arrow
                *cursor_y += MESSAGE_SPACING;

                let from_x = actor_idx
                    .get(msg.from.as_str())
                    .map(|&i| actor_infos[i].center_x)
                    .unwrap_or(0.0);
                let to_x = actor_idx
                    .get(msg.to.as_str())
                    .map(|&i| actor_infos[i].center_x)
                    .unwrap_or(0.0);
                let is_self = msg.from == msg.to;
                let normalized_label = html_util::normalize_br(&msg.label);

                // Add extra vertical space for multiline labels
                let label_lines: Vec<&str> = normalized_label.split('\n').collect();
                if label_lines.len() > 1 {
                    let extra_lines = (label_lines.len() - 1) as f64;
                    *cursor_y += extra_lines * 14.0; // ~14px per extra line
                }

                *msg_number += 1;
                let number = if autonumber { Some(*msg_number) } else { None };

                messages.push(PositionedMessage {
                    from_x,
                    to_x,
                    y: *cursor_y,
                    arrow: msg.arrow,
                    label: normalized_label,
                    is_self,
                    self_width: SELF_MSG_WIDTH,
                    self_height: SELF_MSG_HEIGHT,
                    number,
                });

                // Handle activation modifiers
                if msg.activate_target {
                    let stack = activation_stack.entry(msg.to.clone()).or_default();
                    stack.push(*cursor_y);
                }
                if msg.deactivate_source {
                    if let Some(stack) = activation_stack.get_mut(&msg.from) {
                        if let Some(y_start) = stack.pop() {
                            let depth = stack.len();
                            activations.push(PositionedActivation {
                                actor_id: msg.from.clone(),
                                x: from_x,
                                y_start,
                                y_end: *cursor_y,
                                depth,
                            });
                        }
                    }
                }

                // Equal spacing after the arrow
                *cursor_y += MESSAGE_SPACING;

                // Self-messages need extra height for the loop
                if is_self {
                    *cursor_y += SELF_MSG_HEIGHT;
                }
            }
            SequenceStatement::Activate(id) => {
                let stack = activation_stack.entry(id.clone()).or_default();
                stack.push(*cursor_y);
            }
            SequenceStatement::Deactivate(id) => {
                if let Some(stack) = activation_stack.get_mut(id.as_str()) {
                    if let Some(y_start) = stack.pop() {
                        let depth = stack.len();
                        let x = actor_idx
                            .get(id.as_str())
                            .map(|&i| actor_infos[i].center_x)
                            .unwrap_or(0.0);
                        activations.push(PositionedActivation {
                            actor_id: id.clone(),
                            x,
                            y_start,
                            y_end: *cursor_y,
                            depth,
                        });
                    }
                }
            }
            SequenceStatement::Note(note) => {
                let (note_x, note_w) =
                    compute_note_position(note, actor_idx, actor_infos, measurer);
                let normalized = html_util::normalize_br(&note.text);
                let text_metrics = measurer.measure_multiline(&normalized, 2.0);
                let note_h = text_metrics.height + NOTE_PADDING * 2.0;

                notes.push(PositionedNote {
                    text: normalized,
                    x: note_x,
                    y: *cursor_y,
                    width: note_w,
                    height: note_h,
                });

                *cursor_y += note_h + 10.0;
            }
            SequenceStatement::Block(block) => {
                let block_start_y = *cursor_y;
                *cursor_y += BLOCK_HEADER_HEIGHT;

                let mut dividers = Vec::new();

                // Layout first section's statements
                if let Some(first_section) = block.sections.first() {
                    layout_statements(
                        &first_section.statements,
                        autonumber,
                        actor_idx,
                        actor_infos,
                        measurer,
                        cursor_y,
                        messages,
                        blocks,
                        notes,
                        activations,
                        activation_stack,
                        msg_number,
                        block_depth + 1,
                    );
                }

                // Layout remaining sections (else/and/option dividers)
                for section in block.sections.iter().skip(1) {
                    dividers.push(BlockDivider {
                        y: *cursor_y,
                        label: section.label.clone(),
                    });
                    *cursor_y += BLOCK_HEADER_HEIGHT;

                    layout_statements(
                        &section.statements,
                        autonumber,
                        actor_idx,
                        actor_infos,
                        measurer,
                        cursor_y,
                        messages,
                        blocks,
                        notes,
                        activations,
                        activation_stack,
                        msg_number,
                        block_depth + 1,
                    );
                }

                // Add internal padding to complete block content area
                *cursor_y += BLOCK_PADDING;
                let block_end_y = *cursor_y;

                // Add spacing AFTER the block for subsequent elements
                *cursor_y += MESSAGE_SPACING;

                // Compute block width from referenced actors, inset by nesting depth
                let (block_left, block_right) =
                    compute_block_bounds(block, actor_idx, actor_infos, measurer);

                let inset = block_depth as f64 * BLOCK_NEST_INSET;
                let block_x = (block_left - BLOCK_SIDE_PADDING + inset).max(2.0);
                let block_w = (block_right + BLOCK_SIDE_PADDING - inset) - block_x;

                blocks.push(PositionedBlock {
                    kind: block.kind,
                    label: block.label.clone(),
                    x: block_x,
                    y: block_start_y,
                    width: block_w,
                    height: block_end_y - block_start_y,
                    sections: dividers,
                });
            }
        }
    }
}

fn compute_note_position(
    note: &NoteDef,
    actor_idx: &HashMap<&str, usize>,
    actor_infos: &[ActorInfo],
    measurer: &TextMeasurer,
) -> (f64, f64) {
    // Normalize <br/> to newlines and measure the widest line
    let normalized = html_util::normalize_br(&note.text);
    let max_line_w = normalized
        .split('\n')
        .map(|line| measurer.measure(line).width)
        .fold(0.0_f64, f64::max);
    let text_w = max_line_w + NOTE_PADDING * 2.0;
    let note_w = text_w.clamp(40.0, NOTE_MAX_WIDTH);

    if note.participants.is_empty() {
        return (DIAGRAM_PADDING, note_w);
    }

    match note.position {
        NotePosition::LeftOf => {
            let actor_x = actor_idx
                .get(note.participants[0].as_str())
                .map(|&i| actor_infos[i].center_x)
                .unwrap_or(0.0);
            (actor_x - note_w - 10.0, note_w)
        }
        NotePosition::RightOf => {
            let actor_x = actor_idx
                .get(note.participants[0].as_str())
                .map(|&i| actor_infos[i].center_x)
                .unwrap_or(0.0);
            (actor_x + 10.0, note_w)
        }
        NotePosition::Over => {
            if note.participants.len() == 1 {
                let actor_x = actor_idx
                    .get(note.participants[0].as_str())
                    .map(|&i| actor_infos[i].center_x)
                    .unwrap_or(0.0);
                (actor_x - note_w / 2.0, note_w)
            } else {
                // Span across multiple participants
                let mut min_x = f64::MAX;
                let mut max_x = f64::MIN;
                for pid in &note.participants {
                    if let Some(&i) = actor_idx.get(pid.as_str()) {
                        min_x = min_x.min(actor_infos[i].center_x);
                        max_x = max_x.max(actor_infos[i].center_x);
                    }
                }
                let span = max_x - min_x + 40.0;
                let w = span.max(note_w);
                ((min_x + max_x) / 2.0 - w / 2.0, w)
            }
        }
    }
}

fn compute_block_bounds(
    block: &BlockDef,
    actor_idx: &HashMap<&str, usize>,
    actor_infos: &[ActorInfo],
    measurer: &TextMeasurer,
) -> (f64, f64) {
    let mut min_idx = usize::MAX;
    let mut max_idx = 0;
    let mut min_left_with_label = f64::MAX;
    let mut max_right_with_label = 0.0_f64;

    #[allow(clippy::too_many_arguments)]
    fn scan_for_actors_and_self_messages(
        stmts: &[SequenceStatement],
        actor_idx: &HashMap<&str, usize>,
        actor_infos: &[ActorInfo],
        measurer: &TextMeasurer,
        min_idx: &mut usize,
        max_idx: &mut usize,
        min_left_with_label: &mut f64,
        max_right_with_label: &mut f64,
    ) {
        for stmt in stmts {
            match stmt {
                SequenceStatement::Message(msg) => {
                    if let Some(&i) = actor_idx.get(msg.from.as_str()) {
                        *min_idx = (*min_idx).min(i);
                        *max_idx = (*max_idx).max(i);

                        // For self-messages, calculate left and right extents including centered label
                        if msg.from == msg.to {
                            if let Some(actor) = actor_infos.get(i) {
                                let normalized = html_util::normalize_br(&msg.label);
                                let label_width = if normalized.contains('\n') {
                                    measurer.measure_multiline(&normalized, 2.0).width
                                } else {
                                    measurer.measure(&normalized).width
                                };
                                // Label is centered above the self-message arrow
                                // Self-message spans from actor.center_x to actor.center_x + SELF_MSG_WIDTH
                                // Label is centered at actor.center_x + SELF_MSG_WIDTH / 2
                                let label_center = actor.center_x + SELF_MSG_WIDTH / 2.0;
                                let left_extent = label_center - label_width / 2.0 - 10.0;
                                let right_extent = label_center + label_width / 2.0 + 10.0;
                                *min_left_with_label = min_left_with_label.min(left_extent);
                                *max_right_with_label = max_right_with_label.max(right_extent);
                            }
                        }
                    }
                    if msg.from != msg.to {
                        if let Some(&i) = actor_idx.get(msg.to.as_str()) {
                            *min_idx = (*min_idx).min(i);
                            *max_idx = (*max_idx).max(i);
                        }
                    }
                }
                SequenceStatement::Block(inner_block) => {
                    for section in &inner_block.sections {
                        scan_for_actors_and_self_messages(
                            &section.statements,
                            actor_idx,
                            actor_infos,
                            measurer,
                            min_idx,
                            max_idx,
                            min_left_with_label,
                            max_right_with_label,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    for section in &block.sections {
        scan_for_actors_and_self_messages(
            &section.statements,
            actor_idx,
            actor_infos,
            measurer,
            &mut min_idx,
            &mut max_idx,
            &mut min_left_with_label,
            &mut max_right_with_label,
        );
    }

    if min_idx == usize::MAX || max_idx == 0 {
        // Fallback: use first and last actors
        let left = actor_infos
            .first()
            .map(|a| a.center_x - a.box_width / 2.0)
            .unwrap_or(0.0);
        let right = actor_infos
            .last()
            .map(|a| a.center_x + a.box_width / 2.0)
            .unwrap_or(100.0);
        return (
            left.min(min_left_with_label),
            right.max(max_right_with_label),
        );
    }

    let left = actor_infos[min_idx].center_x - actor_infos[min_idx].box_width / 2.0;
    let right = actor_infos[max_idx].center_x + actor_infos[max_idx].box_width / 2.0;
    (
        left.min(min_left_with_label),
        right.max(max_right_with_label),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontProvider;
    use crate::parser::sequence::parse_sequence;

    fn make_measurer() -> (FontProvider, crate::render::theme::Theme) {
        (FontProvider::default_font(), Theme::default())
    }

    #[test]
    fn test_layout_simple() {
        let source = "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi";
        let ast = parse_sequence(source).unwrap();
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.actors.len(), 2);
        assert!(layout.actors[0].center_x < layout.actors[1].center_x);
        assert_eq!(layout.messages.len(), 2);
        assert!(layout.messages[0].y < layout.messages[1].y);
        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
    }

    #[test]
    fn test_layout_actor_ordering() {
        let source = "sequenceDiagram\n    participant A\n    participant B\n    participant C\n    A->>C: skip B";
        let ast = parse_sequence(source).unwrap();
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.actors.len(), 3);
        assert!(layout.actors[0].center_x < layout.actors[1].center_x);
        assert!(layout.actors[1].center_x < layout.actors[2].center_x);
    }

    #[test]
    fn test_layout_self_message() {
        let source = "sequenceDiagram\n    A->>A: Self call\n    A->>B: Normal";
        let ast = parse_sequence(source).unwrap();
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.messages.len(), 2);
        assert!(layout.messages[0].is_self);
        assert!(!layout.messages[1].is_self);
        // Self message should advance y more
        let gap_self = layout.messages[1].y - layout.messages[0].y;
        assert!(gap_self > MESSAGE_SPACING);
    }

    #[test]
    fn test_layout_alt_block() {
        let source = "sequenceDiagram\n    A->>B: req\n    alt OK\n        B->>A: 200\n    else Fail\n        B->>A: 500\n    end\n    A->>B: done";
        let ast = parse_sequence(source).unwrap();
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.blocks.len(), 1);
        assert_eq!(layout.blocks[0].kind, BlockKind::Alt);
        assert_eq!(layout.blocks[0].sections.len(), 1); // 1 divider (else)
        assert!(layout.blocks[0].height > 0.0);
        assert!(layout.blocks[0].width > 0.0);
    }

    #[test]
    fn test_note_left_of() {
        let ast = SequenceAst {
            participants: vec![
                ParticipantDef {
                    id: "A".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
                ParticipantDef {
                    id: "B".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
            ],
            statements: vec![
                SequenceStatement::Note(NoteDef {
                    position: NotePosition::LeftOf,
                    participants: vec!["A".to_string()],
                    text: "Left note".to_string(),
                }),
            ],
            autonumber: false,
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.notes.len(), 1);
        let note = &layout.notes[0];
        // Note left of A: note should be positioned to the left of actor A
        let actor_a_x = layout.actors[0].center_x;
        assert!(
            note.x + note.width < actor_a_x,
            "Note left of A should end before actor A center. note.x={}, note.width={}, actor_a_x={}",
            note.x, note.width, actor_a_x
        );
    }

    #[test]
    fn test_note_right_of() {
        let ast = SequenceAst {
            participants: vec![
                ParticipantDef {
                    id: "A".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
                ParticipantDef {
                    id: "B".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
            ],
            statements: vec![
                SequenceStatement::Note(NoteDef {
                    position: NotePosition::RightOf,
                    participants: vec!["B".to_string()],
                    text: "Right note".to_string(),
                }),
            ],
            autonumber: false,
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.notes.len(), 1);
        let note = &layout.notes[0];
        // Note right of B: note x should start after actor B center
        let actor_b_x = layout.actors[1].center_x;
        assert!(
            note.x > actor_b_x,
            "Note right of B should start after actor B center. note.x={}, actor_b_x={}",
            note.x, actor_b_x
        );
    }

    #[test]
    fn test_alt_else_block_sections() {
        // alt with else creates dividers for the non-first sections
        let ast = SequenceAst {
            participants: vec![
                ParticipantDef {
                    id: "A".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
                ParticipantDef {
                    id: "B".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
            ],
            statements: vec![
                SequenceStatement::Block(BlockDef {
                    kind: BlockKind::Alt,
                    label: "condition".to_string(),
                    sections: vec![
                        BlockSection {
                            label: Some("condition".to_string()),
                            statements: vec![SequenceStatement::Message(MessageDef {
                                from: "A".to_string(),
                                to: "B".to_string(),
                                arrow: ArrowType::SolidArrow,
                                label: "yes".to_string(),
                                activate_target: false,
                                deactivate_source: false,
                            })],
                        },
                        BlockSection {
                            label: Some("else".to_string()),
                            statements: vec![SequenceStatement::Message(MessageDef {
                                from: "A".to_string(),
                                to: "B".to_string(),
                                arrow: ArrowType::SolidArrow,
                                label: "no".to_string(),
                                activate_target: false,
                                deactivate_source: false,
                            })],
                        },
                        BlockSection {
                            label: Some("else other".to_string()),
                            statements: vec![SequenceStatement::Message(MessageDef {
                                from: "B".to_string(),
                                to: "A".to_string(),
                                arrow: ArrowType::DottedArrow,
                                label: "maybe".to_string(),
                                activate_target: false,
                                deactivate_source: false,
                            })],
                        },
                    ],
                }),
            ],
            autonumber: false,
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.blocks.len(), 1);
        // 2 dividers for the 2 non-first sections (else, else other)
        assert_eq!(layout.blocks[0].sections.len(), 2);
        assert_eq!(layout.blocks[0].sections[0].label.as_deref(), Some("else"));
        assert_eq!(layout.blocks[0].sections[1].label.as_deref(), Some("else other"));
        // Block should contain all 3 messages
        assert_eq!(layout.messages.len(), 3);
    }

    #[test]
    fn test_self_message_layout() {
        // Self-message (A->>A) should be flagged and take extra vertical space
        let ast = SequenceAst {
            participants: vec![
                ParticipantDef {
                    id: "A".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
                ParticipantDef {
                    id: "B".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
            ],
            statements: vec![
                SequenceStatement::Message(MessageDef {
                    from: "A".to_string(),
                    to: "A".to_string(),
                    arrow: ArrowType::SolidArrow,
                    label: "self call".to_string(),
                    activate_target: false,
                    deactivate_source: false,
                }),
                SequenceStatement::Message(MessageDef {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    arrow: ArrowType::SolidArrow,
                    label: "normal".to_string(),
                    activate_target: false,
                    deactivate_source: false,
                }),
            ],
            autonumber: false,
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.messages.len(), 2);
        assert!(layout.messages[0].is_self);
        assert_eq!(layout.messages[0].from_x, layout.messages[0].to_x);
        assert!(!layout.messages[1].is_self);
        // Self message takes extra vertical space
        let y_gap = layout.messages[1].y - layout.messages[0].y;
        assert!(y_gap > SELF_MSG_HEIGHT, "Self message should add extra vertical space");
    }

    #[test]
    fn test_autonumber_assignment() {
        let ast = SequenceAst {
            participants: vec![
                ParticipantDef {
                    id: "A".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
                ParticipantDef {
                    id: "B".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
            ],
            statements: vec![
                SequenceStatement::Message(MessageDef {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    arrow: ArrowType::SolidArrow,
                    label: "first".to_string(),
                    activate_target: false,
                    deactivate_source: false,
                }),
                SequenceStatement::Message(MessageDef {
                    from: "B".to_string(),
                    to: "A".to_string(),
                    arrow: ArrowType::DottedArrow,
                    label: "second".to_string(),
                    activate_target: false,
                    deactivate_source: false,
                }),
                SequenceStatement::Message(MessageDef {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    arrow: ArrowType::SolidArrow,
                    label: "third".to_string(),
                    activate_target: false,
                    deactivate_source: false,
                }),
            ],
            autonumber: true,
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert!(layout.autonumber);
        assert_eq!(layout.messages.len(), 3);
        assert_eq!(layout.messages[0].number, Some(1));
        assert_eq!(layout.messages[1].number, Some(2));
        assert_eq!(layout.messages[2].number, Some(3));
    }

    #[test]
    fn test_autonumber_disabled() {
        let ast = SequenceAst {
            participants: vec![
                ParticipantDef {
                    id: "A".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
                ParticipantDef {
                    id: "B".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
            ],
            statements: vec![
                SequenceStatement::Message(MessageDef {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    arrow: ArrowType::SolidArrow,
                    label: "msg".to_string(),
                    activate_target: false,
                    deactivate_source: false,
                }),
            ],
            autonumber: false,
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert!(!layout.autonumber);
        assert_eq!(layout.messages[0].number, None);
    }

    #[test]
    fn test_actor_kind_stick_figure() {
        // Actor (stick figure) participants should have different sizing
        let ast = SequenceAst {
            participants: vec![
                ParticipantDef {
                    id: "Alice".to_string(),
                    display_name: Some("Alice".to_string()),
                    kind: ParticipantKind::Actor,
                },
                ParticipantDef {
                    id: "Bob".to_string(),
                    display_name: Some("Bob".to_string()),
                    kind: ParticipantKind::Participant,
                },
            ],
            statements: vec![
                SequenceStatement::Message(MessageDef {
                    from: "Alice".to_string(),
                    to: "Bob".to_string(),
                    arrow: ArrowType::SolidArrow,
                    label: "Hello".to_string(),
                    activate_target: false,
                    deactivate_source: false,
                }),
            ],
            autonumber: false,
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.actors.len(), 2);
        assert_eq!(layout.actors[0].kind, ParticipantKind::Actor);
        assert_eq!(layout.actors[1].kind, ParticipantKind::Participant);
        // Stick figure height includes STICK_FIGURE_HEIGHT + label gap + text height
        // so it should be taller than a standard participant box
        assert!(
            layout.actors[0].box_height >= STICK_FIGURE_HEIGHT,
            "Actor (stick figure) box_height={} should be >= STICK_FIGURE_HEIGHT={}",
            layout.actors[0].box_height, STICK_FIGURE_HEIGHT
        );
    }

    #[test]
    fn test_note_multiline_text() {
        let ast = SequenceAst {
            participants: vec![
                ParticipantDef {
                    id: "A".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
            ],
            statements: vec![
                SequenceStatement::Note(NoteDef {
                    position: NotePosition::RightOf,
                    participants: vec!["A".to_string()],
                    text: "Line one<br/>Line two<br/>Line three".to_string(),
                }),
            ],
            autonumber: false,
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.notes.len(), 1);
        let note = &layout.notes[0];
        // Multi-line note should have height proportional to number of lines
        assert!(note.height > 0.0);
        assert!(note.width > 0.0);
    }

    #[test]
    fn test_note_over_empty_participants() {
        // Note with no participants should use fallback position
        let ast = SequenceAst {
            participants: vec![
                ParticipantDef {
                    id: "A".to_string(),
                    display_name: None,
                    kind: ParticipantKind::Participant,
                },
            ],
            statements: vec![
                SequenceStatement::Note(NoteDef {
                    position: NotePosition::Over,
                    participants: vec![],
                    text: "Floating note".to_string(),
                }),
            ],
            autonumber: false,
        };
        let (fp, theme) = make_measurer();
        let font_ref = fp.font_ref().unwrap();
        let measurer = TextMeasurer::new(font_ref, theme.font_size as f32);
        let layout = layout_sequence(&ast, &measurer, &theme).unwrap();

        assert_eq!(layout.notes.len(), 1);
        // Falls back to DIAGRAM_PADDING position
        assert!((layout.notes[0].x - DIAGRAM_PADDING).abs() < 0.01);
    }
}
