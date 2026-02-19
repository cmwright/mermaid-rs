use crate::ast::mindmap::*;
use crate::error::Result;
use crate::layout::text_measure::TextMeasurer;
use crate::render::html_util::normalize_br;
use crate::render::theme::Theme;

use std::f64::consts::PI;

// ── Constants ──────────────────────────────────────────────────────────

const LEVEL1_RADIUS: f64 = 130.0; // Distance from root center to first-level children
const LEVEL_GAP: f64 = 30.0; // Minimum gap between parent and child edges
const SIBLING_GAP: f64 = 20.0; // Gap between siblings perpendicular to outward direction
const NODE_PAD_H: f64 = 21.0;
const NODE_PAD_V: f64 = 10.0;
const DIAGRAM_PADDING: f64 = 30.0;
const ROOT_MIN_DIM: f64 = 70.0; // Minimum dimension for root circle
const MAX_TEXT_WIDTH: f64 = 190.0; // Max text width before word-wrapping

// ── Output types ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MindmapLayout {
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<PositionedMindmapNode>,
    pub edges: Vec<MindmapEdge>,
}

#[derive(Debug, Clone)]
pub struct PositionedMindmapNode {
    pub id: String,
    pub label: String,
    pub shape: MindmapNodeShape,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub section: usize, // color section index (based on first-level child)
    pub depth: usize,
    pub icon: Option<String>,
    pub css_class: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MindmapEdge {
    pub from_id: String,
    pub to_id: String,
    pub points: Vec<(f64, f64)>,
    pub section: usize,
    pub depth: usize, // depth of the parent node (0 = from root)
}

// ── Internal sizing ────────────────────────────────────────────────────

struct SizedNode {
    id: String,
    label: String,
    shape: MindmapNodeShape,
    width: f64,
    height: f64,
    subtree_span: f64, // perpendicular span needed for this subtree
    icon: Option<String>,
    css_class: Option<String>,
    children: Vec<SizedNode>,
}

// ── Layout entry point ─────────────────────────────────────────────────

pub fn layout_mindmap(
    ast: &MindmapAst,
    measurer: &TextMeasurer,
    _theme: &Theme,
) -> Result<MindmapLayout> {
    // Phase 1: measure all nodes and compute subtree spans
    let mut sized = measure_node(&ast.root, measurer);

    // Make root node larger and circular (prominent center circle)
    let root_dim = sized.width.max(sized.height).max(ROOT_MIN_DIM);
    sized.width = root_dim;
    sized.height = root_dim;

    // Phase 2: radial positioning
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Root at origin
    nodes.push(PositionedMindmapNode {
        id: sized.id.clone(),
        label: sized.label.clone(),
        shape: sized.shape,
        x: 0.0,
        y: 0.0,
        width: sized.width,
        height: sized.height,
        section: 0,
        depth: 0,
        icon: sized.icon.clone(),
        css_class: sized.css_class.clone(),
    });

    if !sized.children.is_empty() {
        // Distribute ALL children around the full 360° circle,
        // each getting angular space proportional to its subtree weight.
        position_first_level(
            &sized.children,
            &sized.id,
            &mut nodes,
            &mut edges,
        );
    }

    // Phase 3: normalize coordinates (shift so min x/y = DIAGRAM_PADDING)
    let min_x = nodes
        .iter()
        .map(|n| n.x - n.width / 2.0)
        .fold(f64::INFINITY, f64::min);
    let min_y = nodes
        .iter()
        .map(|n| n.y - n.height / 2.0)
        .fold(f64::INFINITY, f64::min);
    let offset_x = DIAGRAM_PADDING - min_x;
    let offset_y = DIAGRAM_PADDING - min_y;

    for node in &mut nodes {
        node.x += offset_x;
        node.y += offset_y;
    }
    for edge in &mut edges {
        for pt in &mut edge.points {
            pt.0 += offset_x;
            pt.1 += offset_y;
        }
    }

    let max_x = nodes
        .iter()
        .map(|n| n.x + n.width / 2.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = nodes
        .iter()
        .map(|n| n.y + n.height / 2.0)
        .fold(f64::NEG_INFINITY, f64::max);

    Ok(MindmapLayout {
        width: max_x + DIAGRAM_PADDING,
        height: max_y + DIAGRAM_PADDING,
        nodes,
        edges,
    })
}

// ── Phase 1: Measure ───────────────────────────────────────────────────

fn measure_node(node: &MindmapNode, measurer: &TextMeasurer) -> SizedNode {
    let label = normalize_br(&node.label);

    // Word-wrap long labels to keep nodes compact
    let label = word_wrap(&label, measurer, MAX_TEXT_WIDTH);

    let text_metrics = measurer.measure_multiline(&label, 2.0);

    let (pad_h, pad_v) = shape_padding(node.shape);
    let node_w = text_metrics.width + pad_h * 2.0;
    let node_h = text_metrics.height + pad_v * 2.0;

    let children: Vec<SizedNode> = node
        .children
        .iter()
        .map(|c| measure_node(c, measurer))
        .collect();

    // Use the diagonal (max possible perpendicular extent) as the span estimate.
    // Since branches can go in any direction around the circle, we can't assume
    // perpendicular is purely vertical. The diagonal covers the worst case.
    let node_diag = (node_w * node_w + node_h * node_h).sqrt();
    let subtree_span = if children.is_empty() {
        node_diag
    } else {
        let children_total: f64 = children.iter().map(|c| c.subtree_span).sum::<f64>()
            + SIBLING_GAP * (children.len() as f64 - 1.0);
        node_diag.max(children_total)
    };

    SizedNode {
        id: node.id.clone(),
        label,
        shape: node.shape,
        width: node_w,
        height: node_h,
        subtree_span,
        icon: node.icon.clone(),
        css_class: node.css_class.clone(),
        children,
    }
}

/// Word-wrap text so no single line exceeds `max_width` pixels.
/// Preserves existing line breaks and only splits at word boundaries.
/// Uses incremental width tracking — each word is measured once.
fn word_wrap(text: &str, measurer: &TextMeasurer, max_width: f64) -> String {
    let space_width = measurer.measure(" ").width;
    let mut result_lines: Vec<String> = Vec::new();

    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.is_empty() {
            result_lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        let mut current_width = 0.0_f64;

        for word in &words {
            let word_width = measurer.measure(word).width;
            if current_line.is_empty() {
                current_line.push_str(word);
                current_width = word_width;
            } else {
                let new_width = current_width + space_width + word_width;
                if new_width <= max_width {
                    current_line.push(' ');
                    current_line.push_str(word);
                    current_width = new_width;
                } else {
                    result_lines.push(current_line);
                    current_line = word.to_string();
                    current_width = word_width;
                }
            }
        }
        if !current_line.is_empty() {
            result_lines.push(current_line);
        }
    }

    result_lines.join("\n")
}

fn shape_padding(shape: MindmapNodeShape) -> (f64, f64) {
    match shape {
        MindmapNodeShape::Circle => (NODE_PAD_H * 1.5, NODE_PAD_V * 1.5),
        MindmapNodeShape::Cloud => (NODE_PAD_H * 1.8, NODE_PAD_V * 1.5),
        MindmapNodeShape::Bang => (NODE_PAD_H * 1.8, NODE_PAD_V * 1.5),
        MindmapNodeShape::Hexagon => (NODE_PAD_H * 1.5, NODE_PAD_V),
        _ => (NODE_PAD_H, NODE_PAD_V),
    }
}

// ── Phase 2: Radial Position ──────────────────────────────────────────

fn count_descendants(node: &SizedNode) -> usize {
    let mut count = node.children.len();
    for child in &node.children {
        count += count_descendants(child);
    }
    count
}

fn position_first_level(
    children: &[SizedNode],
    root_id: &str,
    nodes: &mut Vec<PositionedMindmapNode>,
    edges: &mut Vec<MindmapEdge>,
) {
    if children.is_empty() {
        return;
    }

    // Distribute children around the full 2*PI circle.
    // Weight by descendant count so branches with more nodes get more angular room.
    let weights: Vec<f64> = children
        .iter()
        .map(|c| count_descendants(c) as f64 + 1.0)
        .collect();
    let total_weight: f64 = weights.iter().sum();

    // Start from upper-right and go clockwise
    let mut current_angle = -PI / 2.0;

    for (i, child) in children.iter().enumerate() {
        let sector = 2.0 * PI * weights[i] / total_weight;
        let angle = current_angle + sector / 2.0;

        let child_x = LEVEL1_RADIUS * angle.cos();
        let child_y = LEVEL1_RADIUS * angle.sin();
        let section = i;

        nodes.push(PositionedMindmapNode {
            id: child.id.clone(),
            label: child.label.clone(),
            shape: child.shape,
            x: child_x,
            y: child_y,
            width: child.width,
            height: child.height,
            section,
            depth: 1,
            icon: child.icon.clone(),
            css_class: child.css_class.clone(),
        });

        // Straight edge from root to child
        edges.push(MindmapEdge {
            from_id: root_id.to_string(),
            to_id: child.id.clone(),
            points: vec![(0.0, 0.0), (child_x, child_y)],
            section,
            depth: 0,
        });

        // Recurse into subtree
        position_subtree_radial(
            &child.children,
            &child.id,
            child_x,
            child_y,
            child.width,
            child.height,
            angle,
            section,
            2,
            nodes,
            edges,
        );

        current_angle += sector;
    }
}

#[allow(clippy::too_many_arguments)]
fn position_subtree_radial(
    children: &[SizedNode],
    parent_id: &str,
    parent_x: f64,
    parent_y: f64,
    parent_w: f64,
    parent_h: f64,
    outward_angle: f64,
    section: usize,
    depth: usize,
    nodes: &mut Vec<PositionedMindmapNode>,
    edges: &mut Vec<MindmapEdge>,
) {
    if children.is_empty() {
        return;
    }

    // Outward and perpendicular unit vectors
    let out_dx = outward_angle.cos();
    let out_dy = outward_angle.sin();
    let perp_dx = -outward_angle.sin();
    let perp_dy = outward_angle.cos();

    // Angle-aware perpendicular extent: the perpendicular direction depends on outward_angle.
    // For each child, compute the actual perpendicular extent of its bounding box.
    let cos_a = outward_angle.cos().abs();
    let sin_a = outward_angle.sin().abs();

    // Compute effective perpendicular spans for each child
    let effective_spans: Vec<f64> = children
        .iter()
        .map(|c| {
            // Perpendicular extent of this node's bounding box
            let node_perp = c.width * sin_a + c.height * cos_a;
            // Use the larger of the pre-computed subtree span or the angle-aware extent
            c.subtree_span.max(node_perp)
        })
        .collect();

    let total_span: f64 =
        effective_spans.iter().sum::<f64>() + SIBLING_GAP * (children.len() as f64 - 1.0);

    let mut current_perp = -total_span / 2.0;

    for (idx, child) in children.iter().enumerate() {
        let span = effective_spans[idx];
        let child_perp_mid = current_perp + span / 2.0;

        // Angle-aware clearance: compute half-extent of each rectangle in the outward direction
        let parent_r = parent_w / 2.0 * cos_a + parent_h / 2.0 * sin_a;
        let child_r = child.width / 2.0 * cos_a + child.height / 2.0 * sin_a;
        let outward_dist = parent_r + LEVEL_GAP + child_r;

        let child_x = parent_x + out_dx * outward_dist + perp_dx * child_perp_mid;
        let child_y = parent_y + out_dy * outward_dist + perp_dy * child_perp_mid;

        nodes.push(PositionedMindmapNode {
            id: child.id.clone(),
            label: child.label.clone(),
            shape: child.shape,
            x: child_x,
            y: child_y,
            width: child.width,
            height: child.height,
            section,
            depth,
            icon: child.icon.clone(),
            css_class: child.css_class.clone(),
        });

        // Straight edge from parent to child
        edges.push(MindmapEdge {
            from_id: parent_id.to_string(),
            to_id: child.id.clone(),
            points: vec![(parent_x, parent_y), (child_x, child_y)],
            section,
            depth,
        });

        // Recurse with same outward angle
        position_subtree_radial(
            &child.children,
            &child.id,
            child_x,
            child_y,
            child.width,
            child.height,
            outward_angle,
            section,
            depth + 1,
            nodes,
            edges,
        );

        current_perp += span + SIBLING_GAP;
    }
}
