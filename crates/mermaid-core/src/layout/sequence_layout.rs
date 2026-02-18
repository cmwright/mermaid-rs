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
const MESSAGE_SPACING: f64 = 40.0;
const SELF_MSG_WIDTH: f64 = 30.0;
const SELF_MSG_HEIGHT: f64 = 28.0;
const BLOCK_PADDING: f64 = 10.0;
const BLOCK_HEADER_HEIGHT: f64 = 20.0;
const NOTE_PADDING: f64 = 8.0;
const NOTE_MAX_WIDTH: f64 = 200.0;
const DIAGRAM_PADDING: f64 = 10.0;
const STICK_FIGURE_HEIGHT: f64 = 40.0;
const STICK_FIGURE_LABEL_GAP: f64 = 4.0;

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
    let top_box_height = actor_infos.iter().map(|a| a.box_height).fold(0.0_f64, f64::max);
    let mut cursor_y = DIAGRAM_PADDING + top_box_height + 15.0;
    let lifeline_start_y = cursor_y;

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
    cursor_y += 15.0;
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

    // Build lifelines
    let mut lifelines: Vec<Lifeline> = Vec::new();
    for info in &actor_infos {
        lifelines.push(Lifeline {
            actor_id: info.id.clone(),
            x: info.center_x,
            y_start: lifeline_start_y,
            y_end: bottom_box_y,
        });
    }

    // Compute bounding box
    let rightmost = actor_infos
        .iter()
        .map(|a| a.center_x + a.box_width / 2.0)
        .fold(0.0_f64, f64::max);
    let width = rightmost + DIAGRAM_PADDING;
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
        let display = p
            .display_name
            .as_deref()
            .unwrap_or(&p.id);
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
    actor_infos: &mut Vec<ActorInfo>,
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
                    let label_w = measurer.measure(&msg.label).width + 20.0;
                    if let (Some(&from_idx), Some(&to_idx)) =
                        (actor_idx.get(msg.from.as_str()), actor_idx.get(msg.to.as_str()))
                    {
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
                        collect_message_gaps(&section.statements, measurer, actor_idx, required_gaps);
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
            for g in *lo..*hi {
                if extra > extra_per_gap[g] {
                    extra_per_gap[g] = extra;
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
) {
    for stmt in statements {
        match stmt {
            SequenceStatement::Message(msg) => {
                let from_x = actor_idx
                    .get(msg.from.as_str())
                    .map(|&i| actor_infos[i].center_x)
                    .unwrap_or(0.0);
                let to_x = actor_idx
                    .get(msg.to.as_str())
                    .map(|&i| actor_infos[i].center_x)
                    .unwrap_or(0.0);
                let is_self = msg.from == msg.to;

                *msg_number += 1;
                let number = if autonumber { Some(*msg_number) } else { None };

                messages.push(PositionedMessage {
                    from_x,
                    to_x,
                    y: *cursor_y,
                    arrow: msg.arrow,
                    label: msg.label.clone(),
                    is_self,
                    self_width: SELF_MSG_WIDTH,
                    self_height: SELF_MSG_HEIGHT,
                    number,
                });

                // Handle activation modifiers
                if msg.activate_target {
                    let stack = activation_stack
                        .entry(msg.to.clone())
                        .or_insert_with(Vec::new);
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

                if is_self {
                    *cursor_y += MESSAGE_SPACING + SELF_MSG_HEIGHT;
                } else {
                    *cursor_y += MESSAGE_SPACING;
                }
            }
            SequenceStatement::Activate(id) => {
                let stack = activation_stack
                    .entry(id.clone())
                    .or_insert_with(Vec::new);
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
                let (note_x, note_w) = compute_note_position(note, actor_idx, actor_infos, measurer);
                let text_metrics = measurer.measure_multiline(&note.text, 2.0);
                let note_h = text_metrics.height + NOTE_PADDING * 2.0;

                notes.push(PositionedNote {
                    text: note.text.clone(),
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
                    );
                }

                *cursor_y += BLOCK_PADDING;

                // Compute block width from referenced actors
                let (block_left, block_right) =
                    compute_block_bounds(block, actor_idx, actor_infos);

                blocks.push(PositionedBlock {
                    kind: block.kind,
                    label: block.label.clone(),
                    x: block_left - BLOCK_PADDING,
                    y: block_start_y,
                    width: (block_right - block_left) + BLOCK_PADDING * 2.0,
                    height: *cursor_y - block_start_y,
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
    let text_w = measurer.measure(&note.text).width + NOTE_PADDING * 2.0;
    let note_w = text_w.min(NOTE_MAX_WIDTH).max(40.0);

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
) -> (f64, f64) {
    let mut min_idx = usize::MAX;
    let mut max_idx = 0;

    fn scan_for_actors(
        stmts: &[SequenceStatement],
        actor_idx: &HashMap<&str, usize>,
        min_idx: &mut usize,
        max_idx: &mut usize,
    ) {
        for stmt in stmts {
            match stmt {
                SequenceStatement::Message(msg) => {
                    if let Some(&i) = actor_idx.get(msg.from.as_str()) {
                        *min_idx = (*min_idx).min(i);
                        *max_idx = (*max_idx).max(i);
                    }
                    if let Some(&i) = actor_idx.get(msg.to.as_str()) {
                        *min_idx = (*min_idx).min(i);
                        *max_idx = (*max_idx).max(i);
                    }
                }
                SequenceStatement::Block(inner_block) => {
                    for section in &inner_block.sections {
                        scan_for_actors(&section.statements, actor_idx, min_idx, max_idx);
                    }
                }
                _ => {}
            }
        }
    }

    for section in &block.sections {
        scan_for_actors(&section.statements, actor_idx, &mut min_idx, &mut max_idx);
    }

    if min_idx == usize::MAX || max_idx == 0 {
        // Fallback: use first and last actors
        let left = actor_infos.first().map(|a| a.center_x - a.box_width / 2.0).unwrap_or(0.0);
        let right = actor_infos.last().map(|a| a.center_x + a.box_width / 2.0).unwrap_or(100.0);
        return (left, right);
    }

    let left = actor_infos[min_idx].center_x - actor_infos[min_idx].box_width / 2.0;
    let right = actor_infos[max_idx].center_x + actor_infos[max_idx].box_width / 2.0;
    (left, right)
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
}
