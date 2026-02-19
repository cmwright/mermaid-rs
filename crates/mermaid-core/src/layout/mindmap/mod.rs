use crate::ast::mindmap::*;
use crate::error::Result;
use crate::layout::text_measure::TextMeasurer;
use crate::render::html_util::normalize_br;
use crate::render::theme::Theme;

// ── Constants ──────────────────────────────────────────────────────────

const H_SPACING: f64 = 60.0;
const V_SPACING: f64 = 20.0;
const NODE_PAD_H: f64 = 16.0;
const NODE_PAD_V: f64 = 10.0;
const DIAGRAM_PADDING: f64 = 30.0;

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
}

// ── Internal sizing ────────────────────────────────────────────────────

struct SizedNode {
    id: String,
    label: String,
    shape: MindmapNodeShape,
    width: f64,
    height: f64,
    subtree_height: f64,
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
    // Phase 1: measure all nodes and compute subtree sizes
    let sized = measure_node(&ast.root, measurer);

    // Phase 2: position nodes using left-right balanced tree
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Root at origin (we'll normalize later)
    let root_x = 0.0;
    let root_y = 0.0;

    nodes.push(PositionedMindmapNode {
        id: sized.id.clone(),
        label: sized.label.clone(),
        shape: sized.shape,
        x: root_x,
        y: root_y,
        width: sized.width,
        height: sized.height,
        section: 0,
        depth: 0,
        icon: sized.icon.clone(),
        css_class: sized.css_class.clone(),
    });

    // Split first-level children into left and right groups
    let child_count = sized.children.len();
    let right_count = child_count.div_ceil(2); // more on right if odd
    let (right_children, left_children) = sized.children.split_at(right_count);

    // Position right-side children
    position_children(
        right_children,
        &sized.id,
        root_x,
        root_y,
        sized.width,
        true, // right side
        1,
        &mut nodes,
        &mut edges,
    );

    // Position left-side children
    position_children(
        left_children,
        &sized.id,
        root_x,
        root_y,
        sized.width,
        false, // left side
        right_count,
        &mut nodes,
        &mut edges,
    );

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
    let text_metrics = measurer.measure_multiline(&label, 2.0);

    let (pad_h, pad_v) = shape_padding(node.shape);
    let node_w = text_metrics.width + pad_h * 2.0;
    let node_h = text_metrics.height + pad_v * 2.0;

    let children: Vec<SizedNode> = node
        .children
        .iter()
        .map(|c| measure_node(c, measurer))
        .collect();

    let subtree_h = if children.is_empty() {
        node_h
    } else {
        let children_total: f64 = children.iter().map(|c| c.subtree_height).sum::<f64>()
            + V_SPACING * (children.len() as f64 - 1.0);
        node_h.max(children_total)
    };

    SizedNode {
        id: node.id.clone(),
        label,
        shape: node.shape,
        width: node_w,
        height: node_h,
        subtree_height: subtree_h,
        icon: node.icon.clone(),
        css_class: node.css_class.clone(),
        children,
    }
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

// ── Phase 2: Position ──────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn position_children(
    children: &[SizedNode],
    parent_id: &str,
    parent_x: f64,
    parent_y: f64,
    parent_w: f64,
    right_side: bool,
    section_offset: usize,
    nodes: &mut Vec<PositionedMindmapNode>,
    edges: &mut Vec<MindmapEdge>,
) {
    if children.is_empty() {
        return;
    }

    // Total height of all children subtrees
    let total_h: f64 = children.iter().map(|c| c.subtree_height).sum::<f64>()
        + V_SPACING * (children.len() as f64 - 1.0);

    // Start y so children are centered around parent
    let mut current_y = parent_y - total_h / 2.0;

    for (i, child) in children.iter().enumerate() {
        let section = if right_side {
            i
        } else {
            section_offset + i
        };

        let child_center_y = current_y + child.subtree_height / 2.0;

        let child_x = if right_side {
            parent_x + parent_w / 2.0 + H_SPACING + child.width / 2.0
        } else {
            parent_x - parent_w / 2.0 - H_SPACING - child.width / 2.0
        };

        nodes.push(PositionedMindmapNode {
            id: child.id.clone(),
            label: child.label.clone(),
            shape: child.shape,
            x: child_x,
            y: child_center_y,
            width: child.width,
            height: child.height,
            section,
            depth: 1,
            icon: child.icon.clone(),
            css_class: child.css_class.clone(),
        });

        // Edge from parent to child
        let edge_start_x = if right_side {
            parent_x + parent_w / 2.0
        } else {
            parent_x - parent_w / 2.0
        };
        let edge_end_x = if right_side {
            child_x - child.width / 2.0
        } else {
            child_x + child.width / 2.0
        };
        let mid_x = (edge_start_x + edge_end_x) / 2.0;

        edges.push(MindmapEdge {
            from_id: parent_id.to_string(),
            to_id: child.id.clone(),
            points: vec![
                (edge_start_x, parent_y),
                (mid_x, parent_y),
                (mid_x, child_center_y),
                (edge_end_x, child_center_y),
            ],
            section,
        });

        // Recurse into grandchildren
        position_subtree(
            &child.children,
            &child.id,
            child_x,
            child_center_y,
            child.width,
            right_side,
            section,
            2,
            nodes,
            edges,
        );

        current_y += child.subtree_height + V_SPACING;
    }
}

#[allow(clippy::too_many_arguments)]
fn position_subtree(
    children: &[SizedNode],
    parent_id: &str,
    parent_x: f64,
    parent_y: f64,
    parent_w: f64,
    right_side: bool,
    section: usize,
    depth: usize,
    nodes: &mut Vec<PositionedMindmapNode>,
    edges: &mut Vec<MindmapEdge>,
) {
    if children.is_empty() {
        return;
    }

    let total_h: f64 = children.iter().map(|c| c.subtree_height).sum::<f64>()
        + V_SPACING * (children.len() as f64 - 1.0);

    let mut current_y = parent_y - total_h / 2.0;

    for child in children {
        let child_center_y = current_y + child.subtree_height / 2.0;

        let child_x = if right_side {
            parent_x + parent_w / 2.0 + H_SPACING + child.width / 2.0
        } else {
            parent_x - parent_w / 2.0 - H_SPACING - child.width / 2.0
        };

        nodes.push(PositionedMindmapNode {
            id: child.id.clone(),
            label: child.label.clone(),
            shape: child.shape,
            x: child_x,
            y: child_center_y,
            width: child.width,
            height: child.height,
            section,
            depth,
            icon: child.icon.clone(),
            css_class: child.css_class.clone(),
        });

        // Edge
        let edge_start_x = if right_side {
            parent_x + parent_w / 2.0
        } else {
            parent_x - parent_w / 2.0
        };
        let edge_end_x = if right_side {
            child_x - child.width / 2.0
        } else {
            child_x + child.width / 2.0
        };
        let mid_x = (edge_start_x + edge_end_x) / 2.0;

        edges.push(MindmapEdge {
            from_id: parent_id.to_string(),
            to_id: child.id.clone(),
            points: vec![
                (edge_start_x, parent_y),
                (mid_x, parent_y),
                (mid_x, child_center_y),
                (edge_end_x, child_center_y),
            ],
            section,
        });

        // Recurse
        position_subtree(
            &child.children,
            &child.id,
            child_x,
            child_center_y,
            child.width,
            right_side,
            section,
            depth + 1,
            nodes,
            edges,
        );

        current_y += child.subtree_height + V_SPACING;
    }
}
