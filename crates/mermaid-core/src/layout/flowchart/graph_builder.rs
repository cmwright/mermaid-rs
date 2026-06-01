use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use petgraph::graph::{DiGraph, NodeIndex};

use crate::ast::common::StyleProperties;
use crate::ast::flowchart::{
    ClassAssignment, ClassDef, Direction, EdgeDef, FlowchartAst, NodeDef, NodeShape, StyleOverride,
    SubgraphDef,
};
use crate::error::{MermaidError, Result};
use crate::layout::flowchart::types::*;
use crate::layout::text_measure::{TextMeasurer, TextMetrics};

// ── Class map ───────────────────────────────────────────────

pub fn build_class_map(class_defs: &[ClassDef]) -> HashMap<String, StyleProperties> {
    class_defs
        .iter()
        .map(|cd| (cd.name.clone(), cd.properties.clone()))
        .collect()
}

// ── Node collection ─────────────────────────────────────────

pub fn collect_all_nodes(
    ast: &FlowchartAst,
    class_defs: &HashMap<String, StyleProperties>,
) -> HashMap<String, (NodeDef, StyleProperties)> {
    let mut all_nodes: HashMap<String, (NodeDef, StyleProperties)> = HashMap::new();
    let sg_ids = subgraph_ids_recursive(&ast.subgraphs);

    for node in &ast.nodes {
        // Skip bare node references whose ID matches a subgraph — the parser
        // creates these when an edge references a subgraph ID (e.g. `A --> SG`).
        // They must not be materialised as regular nodes.
        if sg_ids.contains(&node.id) {
            continue;
        }
        let style = resolve_node_style(
            node,
            class_defs,
            &ast.class_assignments,
            &ast.style_overrides,
        );
        all_nodes.insert(node.id.clone(), (node.clone(), style));
    }

    collect_subgraph_nodes(
        &ast.subgraphs,
        class_defs,
        &ast.class_assignments,
        &ast.style_overrides,
        &sg_ids,
        &mut all_nodes,
    );

    // Ensure edge-referenced nodes exist
    for edge in &ast.edges {
        for id in [&edge.from, &edge.to] {
            if sg_ids.contains(id) {
                continue;
            }
            all_nodes.entry(id.clone()).or_insert_with(|| {
                let node = NodeDef {
                    id: id.clone(),
                    label: None,
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                };
                (node, StyleProperties::default())
            });
        }
    }

    all_nodes
}

pub fn collect_node_order_from_ast(ast: &FlowchartAst) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut order = Vec::new();

    let push = |id: &str, order: &mut Vec<String>, seen: &mut HashSet<String>| {
        if seen.insert(id.to_string()) {
            order.push(id.to_string());
        }
    };

    for n in &ast.nodes {
        if n.label.is_some() {
            push(&n.id, &mut order, &mut seen);
        }
    }
    fn walk_subgraphs(
        subgraphs: &[SubgraphDef],
        order: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) {
        for sg in subgraphs {
            for n in &sg.nodes {
                if seen.insert(n.id.clone()) {
                    order.push(n.id.clone());
                }
            }
            for e in &sg.edges {
                if seen.insert(e.from.clone()) {
                    order.push(e.from.clone());
                }
                if seen.insert(e.to.clone()) {
                    order.push(e.to.clone());
                }
            }
            walk_subgraphs(&sg.subgraphs, order, seen);
        }
    }

    for e in &ast.edges {
        push(&e.from, &mut order, &mut seen);
        push(&e.to, &mut order, &mut seen);
    }
    walk_subgraphs(&ast.subgraphs, &mut order, &mut seen);
    order
}

fn collect_leaf_node_order_from_ast(ast: &FlowchartAst) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut order = Vec::new();

    let push = |id: &str, order: &mut Vec<String>, seen: &mut HashSet<String>| {
        if seen.insert(id.to_string()) {
            order.push(id.to_string());
        }
    };

    for n in &ast.nodes {
        push(&n.id, &mut order, &mut seen);
    }

    fn walk_subgraphs_nodes_only(
        subgraphs: &[SubgraphDef],
        order: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) {
        for sg in subgraphs {
            walk_subgraphs_nodes_only(&sg.subgraphs, order, seen);
            for n in &sg.nodes {
                if n.label.is_some() && seen.insert(n.id.clone()) {
                    order.push(n.id.clone());
                }
            }
        }
    }

    walk_subgraphs_nodes_only(&ast.subgraphs, &mut order, &mut seen);
    order
}

fn collect_subgraph_nodes(
    subgraphs: &[SubgraphDef],
    class_defs: &HashMap<String, StyleProperties>,
    class_assignments: &[ClassAssignment],
    style_overrides: &[StyleOverride],
    sg_ids: &HashSet<String>,
    all_nodes: &mut HashMap<String, (NodeDef, StyleProperties)>,
) {
    for sg in subgraphs {
        for node in &sg.nodes {
            let style = resolve_node_style(node, class_defs, class_assignments, style_overrides);
            if node.label.is_some() {
                // Labeled definition always wins — overwrite any prior bare reference.
                all_nodes.insert(node.id.clone(), (node.clone(), style));
            } else {
                // Bare reference (from cross-subgraph link chain) — don't overwrite
                // a labeled definition that was already inserted.
                all_nodes
                    .entry(node.id.clone())
                    .or_insert((node.clone(), style));
            }
        }
        for edge in &sg.edges {
            for id in [&edge.from, &edge.to] {
                if sg_ids.contains(id) {
                    continue;
                }
                all_nodes.entry(id.clone()).or_insert_with(|| {
                    let node = NodeDef {
                        id: id.clone(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    };
                    (node, StyleProperties::default())
                });
            }
        }
        collect_subgraph_nodes(
            &sg.subgraphs,
            class_defs,
            class_assignments,
            style_overrides,
            sg_ids,
            all_nodes,
        );
    }
}

fn resolve_node_style(
    node: &NodeDef,
    class_defs: &HashMap<String, StyleProperties>,
    class_assignments: &[ClassAssignment],
    style_overrides: &[StyleOverride],
) -> StyleProperties {
    let mut style = StyleProperties::default();

    if let Some(class_name) = &node.class_shorthand {
        if let Some(class_style) = class_defs.get(class_name) {
            style = style.merge(class_style);
        }
    }

    for ca in class_assignments {
        if ca.node_ids.contains(&node.id) {
            if let Some(class_style) = class_defs.get(&ca.class_name) {
                style = style.merge(class_style);
            }
        }
    }

    for so in style_overrides {
        if so.node_id == node.id {
            style = style.merge(&so.properties);
        }
    }

    style
}

// ── Edge collection ─────────────────────────────────────────

pub fn collect_all_edges(ast: &FlowchartAst) -> Vec<EdgeDef> {
    let mut all_edges = Vec::new();
    collect_subgraph_edges(&ast.subgraphs, &mut all_edges);
    all_edges.extend(ast.edges.iter().cloned());
    all_edges
}

fn collect_subgraph_edges(subgraphs: &[SubgraphDef], all_edges: &mut Vec<EdgeDef>) {
    for sg in subgraphs {
        collect_subgraph_edges(&sg.subgraphs, all_edges);
        all_edges.extend(sg.edges.iter().cloned());
    }
}

// ── Subgraph membership ─────────────────────────────────────

/// Maps node ID to its subgraph containment path.
/// E.g., node "X" in subgraph "Inner" inside "Outer" gets path ["Outer", "Inner"].
pub type SubgraphMembership = HashMap<String, Vec<String>>;

pub fn build_subgraph_membership(ast: &FlowchartAst) -> SubgraphMembership {
    let mut membership = SubgraphMembership::new();
    // Let subgraphs claim nodes first (innermost wins for bare nodes,
    // labeled definitions always win over bare references).
    collect_membership(&ast.subgraphs, &[], &mut membership);
    // Then register top-level nodes that weren't claimed by any subgraph.
    for node in &ast.nodes {
        membership.entry(node.id.clone()).or_default();
    }
    membership
}

fn collect_membership(
    subgraphs: &[SubgraphDef],
    parent_path: &[String],
    membership: &mut SubgraphMembership,
) {
    for sg in subgraphs {
        let mut path = parent_path.to_vec();
        path.push(sg.id.clone());
        // Process children first so innermost subgraphs claim bare nodes
        // before their parents can.
        collect_membership(&sg.subgraphs, &path, membership);
        for node in &sg.nodes {
            if node.label.is_some() {
                // Explicit definition (has a label) — always takes priority.
                // This is the subgraph where the node was actually declared.
                membership.insert(node.id.clone(), path.clone());
            } else {
                // Implicit reference (bare node from a link-chain target) —
                // only claim it if no other subgraph has claimed it yet.
                membership
                    .entry(node.id.clone())
                    .or_insert_with(|| path.clone());
            }
        }
        for edge in &sg.edges {
            for id in [&edge.from, &edge.to] {
                membership.entry(id.clone()).or_insert_with(|| path.clone());
            }
        }
    }
}

// ── Petgraph construction ───────────────────────────────────

pub fn build_petgraph(
    all_nodes: &HashMap<String, (NodeDef, StyleProperties)>,
    edges: &[EdgeDef],
    measurer: &TextMeasurer<'_>,
) -> Result<(DiGraph<NodeData, EdgeData>, HashMap<String, NodeIndex>)> {
    let mut graph = DiGraph::new();
    let mut index_map: HashMap<String, NodeIndex> = HashMap::new();

    // Sort nodes by ID to ensure deterministic NodeIndex assignment.
    let mut sorted_nodes: Vec<_> = all_nodes.iter().collect();
    sorted_nodes.sort_by_key(|(id, _)| id.as_str());

    for (id, (node_def, style)) in sorted_nodes {
        let raw_label = node_def.label.clone().unwrap_or_else(|| id.clone());
        let (label, text_metrics) = build_display_label_and_metrics(&raw_label, measurer);
        let (width, height) = compute_node_size(&node_def.shape, &text_metrics);

        let data = NodeData {
            id: id.clone(),
            label,
            shape: node_def.shape,
            style: style.clone(),
            width,
            height,
        };

        let idx = graph.add_node(data);
        index_map.insert(id.clone(), idx);
    }

    for edge in edges {
        let from_idx = index_map
            .get(&edge.from)
            .ok_or_else(|| MermaidError::Layout(format!("Unknown source node: {}", edge.from)))?;
        let to_idx = index_map
            .get(&edge.to)
            .ok_or_else(|| MermaidError::Layout(format!("Unknown target node: {}", edge.to)))?;

        let (label_width, label_height) = edge
            .label
            .as_deref()
            .map_or((0.0, 0.0), |label| edge_label_dimensions(label, measurer));

        graph.add_edge(
            *from_idx,
            *to_idx,
            EdgeData {
                label: edge.label.clone(),
                line_style: edge.line_style,
                arrow_start: edge.arrow_start,
                arrow_end: edge.arrow_end,
                label_width,
                label_height,
            },
        );
    }

    Ok((graph, index_map))
}

// ── Dagre graph construction ────────────────────────────────

/// Convert a mermaid Direction to a dagre RankDir.
pub fn direction_to_rankdir(dir: Direction) -> dagre_rust::RankDir {
    match dir {
        Direction::TopToBottom => dagre_rust::RankDir::TB,
        Direction::BottomToTop => dagre_rust::RankDir::BT,
        Direction::LeftToRight => dagre_rust::RankDir::LR,
        Direction::RightToLeft => dagre_rust::RankDir::RL,
    }
}

/// Build a dagre LayoutGraph from collected nodes and edges.
///
/// Returns the LayoutGraph and a parallel list of NodeData (keyed by node ID)
/// so callers can reconstruct PositionedNodes with shape/style info after layout.
pub fn build_dagre_graph(
    all_nodes: &HashMap<String, (NodeDef, StyleProperties)>,
    edges: &[EdgeDef],
    measurer: &TextMeasurer<'_>,
    direction: Direction,
    ast: &FlowchartAst,
) -> Result<(dagre_rust::LayoutGraph, HashMap<String, NodeData>)> {
    build_dagre_graph_with_fixed_node_sizes(all_nodes, edges, measurer, direction, ast, None)
}

pub fn build_dagre_graph_with_fixed_node_sizes(
    all_nodes: &HashMap<String, (NodeDef, StyleProperties)>,
    edges: &[EdgeDef],
    measurer: &TextMeasurer<'_>,
    direction: Direction,
    ast: &FlowchartAst,
    fixed_node_sizes: Option<&HashMap<String, (f64, f64)>>,
) -> Result<(dagre_rust::LayoutGraph, HashMap<String, NodeData>)> {
    let membership = build_subgraph_membership(ast);
    build_dagre_graph_with_fixed_node_sizes_and_membership(
        all_nodes,
        edges,
        measurer,
        direction,
        ast,
        fixed_node_sizes,
        &membership,
    )
}

pub(crate) fn build_dagre_graph_with_fixed_node_sizes_and_membership(
    all_nodes: &HashMap<String, (NodeDef, StyleProperties)>,
    edges: &[EdgeDef],
    measurer: &TextMeasurer<'_>,
    direction: Direction,
    ast: &FlowchartAst,
    fixed_node_sizes: Option<&HashMap<String, (f64, f64)>>,
    membership: &SubgraphMembership,
) -> Result<(dagre_rust::LayoutGraph, HashMap<String, NodeData>)> {
    let mut g = dagre_rust::Graph::with_options(&dagre_rust::GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });

    // Configure graph label — match mermaid.js settings
    let mut gl = dagre_rust::GraphLabel::default();
    gl.rankdir = direction_to_rankdir(direction);
    gl.nodesep = NODE_SEP;
    gl.edgesep = EDGE_SEP;
    gl.ranksep = RANK_SEP;
    gl.marginx = 8.0;
    gl.marginy = 8.0;
    g.set_graph(gl);

    let mut node_data_map: HashMap<String, NodeData> = HashMap::new();

    let sg_ids = subgraph_ids_recursive(&ast.subgraphs);

    // Mermaid.js/graphlib are sensitive to insertion order.
    // For compound/subgraph charts Mermaid inserts subgraphs first (reverse declaration
    // order, recursively), then leaf nodes in AST encounter order.
    let ordered_ids = if ast.subgraphs.is_empty() {
        let mut ids = collect_node_order_from_ast(ast);
        let seen_ids: HashSet<String> = ids.iter().cloned().collect();
        let mut leftovers: Vec<_> = all_nodes
            .keys()
            .filter(|id| !seen_ids.contains(*id))
            .cloned()
            .collect();
        leftovers.sort();
        ids.extend(leftovers);
        ids
    } else {
        let mut ids = Vec::new();
        collect_subgraph_order_reverse(&ast.subgraphs, &mut ids);

        for id in collect_leaf_node_order_from_ast(ast) {
            if !sg_ids.contains(&id) {
                ids.push(id);
            }
        }

        let seen: HashSet<String> = ids.iter().cloned().collect();
        let mut leftovers: Vec<_> = all_nodes
            .keys()
            .filter(|id| !seen.contains(*id))
            .cloned()
            .collect();
        leftovers.sort();
        ids.extend(leftovers);
        ids
    };

    for id in &ordered_ids {
        let Some((node_def, style)) = all_nodes.get(id) else {
            if sg_ids.contains(id) {
                g.set_node(id, Some(dagre_rust::NodeLabel::default()));
            }
            continue;
        };
        let raw_label = node_def.label.clone().unwrap_or_else(|| (*id).clone());
        let (label, text_metrics) = build_display_label_and_metrics(&raw_label, measurer);
        let (mut width, mut height) = compute_node_size(&node_def.shape, &text_metrics);
        if let Some((fw, fh)) = fixed_node_sizes.and_then(|m| m.get(id)).copied() {
            width = fw;
            height = fh;
        }

        let mut nl = dagre_rust::NodeLabel::default();
        nl.width = width;
        nl.height = height;
        g.set_node(id, Some(nl));

        node_data_map.insert(
            id.clone(),
            NodeData {
                id: id.clone(),
                label,
                shape: node_def.shape,
                style: style.clone(),
                width,
                height,
            },
        );
    }

    let ordered_edges: Vec<&EdgeDef> = edges.iter().collect();

    for (edge_idx, edge) in ordered_edges.iter().enumerate() {
        if !all_nodes.contains_key(&edge.from) && !sg_ids.contains(&edge.from) {
            return Err(MermaidError::Layout(format!(
                "Unknown source node: {}",
                edge.from
            )));
        }
        if !all_nodes.contains_key(&edge.to) && !sg_ids.contains(&edge.to) {
            return Err(MermaidError::Layout(format!(
                "Unknown target node: {}",
                edge.to
            )));
        }

        let (label_width, label_height) = edge
            .label
            .as_deref()
            .map_or((0.0, 0.0), |label| edge_label_dimensions(label, measurer));

        let mut el = dagre_rust::EdgeLabel::default();
        el.width = label_width;
        el.height = label_height;
        el.labelpos = dagre_rust::LabelPos::Center;
        // Give every edge a unique name (its positional index) so that parallel
        // edges between the same pair of nodes are kept distinct by dagre's
        // multigraph rather than collapsing into one. route_edges reconstructs
        // the same index by enumerating the identical edge slice.
        g.set_edge(&edge.from, &edge.to, Some(el), Some(&edge_idx.to_string()));
    }

    // Register subgraph nodes in dagre and set parent relationships
    // so dagre's compound layout can handle subgraph containment.
    register_subgraph_hierarchy(&mut g, &ast.subgraphs, &membership, Some(&ordered_ids));

    Ok((g, node_data_map))
}

/// Register subgraph nodes in dagre and set parent relationships for compound layout.
fn register_subgraph_hierarchy(
    g: &mut dagre_rust::LayoutGraph,
    subgraphs: &[SubgraphDef],
    membership: &SubgraphMembership,
    leaf_order: Option<&[String]>,
) {
    let sg_ids = subgraph_ids_recursive(subgraphs);

    // First, register all subgraph nodes and set their parent relationships
    register_subgraphs_recursive(g, subgraphs, None);

    // Then set parent for all leaf nodes based on membership.
    // Use deterministic ordering matching node insertion order when available.
    if let Some(order) = leaf_order {
        for node_id in order {
            let Some(path) = membership.get(node_id) else {
                continue;
            };
            if !path.is_empty() && !sg_ids.contains(node_id) && g.node(node_id).is_some() {
                let parent = &path[path.len() - 1];
                g.set_parent(node_id, Some(parent));
            }
        }
    } else {
        let mut node_ids: Vec<_> = membership.keys().cloned().collect();
        node_ids.sort();
        for node_id in node_ids {
            let Some(path) = membership.get(&node_id) else {
                continue;
            };
            if !path.is_empty() && !sg_ids.contains(&node_id) && g.node(&node_id).is_some() {
                let parent = &path[path.len() - 1];
                g.set_parent(&node_id, Some(parent));
            }
        }
    }
}

fn register_subgraphs_recursive(
    g: &mut dagre_rust::LayoutGraph,
    subgraphs: &[SubgraphDef],
    parent_id: Option<&str>,
) {
    for sg in subgraphs {
        // Ensure the subgraph node exists in dagre (as a compound parent)
        if g.node(&sg.id).is_none() {
            g.set_node(&sg.id, Some(dagre_rust::NodeLabel::default()));
        }
        // Set subgraph's parent (if nested)
        if let Some(pid) = parent_id {
            g.set_parent(&sg.id, Some(pid));
        }
        // Recursively handle nested subgraphs
        register_subgraphs_recursive(g, &sg.subgraphs, Some(&sg.id));
    }
}

pub fn subgraph_ids_recursive(subgraphs: &[SubgraphDef]) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for sg in subgraphs {
        ids.insert(sg.id.clone());
        ids.extend(subgraph_ids_recursive(&sg.subgraphs));
    }
    ids
}

fn collect_subgraph_order_reverse(subgraphs: &[SubgraphDef], out: &mut Vec<String>) {
    for sg in subgraphs.iter().rev() {
        out.push(sg.id.clone());
        collect_subgraph_order_reverse(&sg.subgraphs, out);
    }
}

fn compute_node_size(shape: &NodeShape, text: &TextMetrics) -> (f64, f64) {
    let base_w = (text.width + 2.0 * NODE_PADDING_H).max(MIN_NODE_WIDTH);
    let base_h = (text.height + 2.0 * NODE_PADDING_V).max(MIN_NODE_HEIGHT);
    const RECT_LABEL_EXTRA_WIDTH: f64 = 4.0;
    // Mermaid flowchart default node padding.
    const MERMAID_NODE_PADDING: f64 = 15.0;
    // Safety inset so text never visually touches shape borders.
    const MIN_TEXT_INSET: f64 = 2.0;
    // Extra guard because Rust-side font metrics can under-estimate browser glyph extents,
    // especially with mixed punctuation/non-latin labels.
    const SHAPE_TEXT_WIDTH_GUARD: f64 = 1.12;
    const SHAPE_TEXT_HEIGHT_GUARD: f64 = 1.08;

    match shape {
        NodeShape::Rectangle
        | NodeShape::RoundedRectangle
        | NodeShape::Stadium
        | NodeShape::Subroutine
        | NodeShape::Cylinder => (base_w + RECT_LABEL_EXTRA_WIDTH, base_h),
        // Mermaid question.ts: s = (bbox.width + padding) + (bbox.height + padding)
        NodeShape::Diamond => {
            let mermaid_s =
                (text.width + MERMAID_NODE_PADDING) + (text.height + MERMAID_NODE_PADDING);
            // For a diamond with equal diagonals s, inscribed axis-aligned rect constraint is:
            // rect_w + rect_h <= s
            let guard_w = text.width * SHAPE_TEXT_WIDTH_GUARD + 2.0 * MIN_TEXT_INSET;
            let guard_h = text.height * SHAPE_TEXT_HEIGHT_GUARD + 2.0 * MIN_TEXT_INSET;
            let min_s = guard_w + guard_h;
            let s = mermaid_s.max(min_s);
            (s, s)
        }
        // Mermaid circle.ts: diameter = bbox.width + padding
        NodeShape::Circle => {
            let mermaid_d = text.width + MERMAID_NODE_PADDING;
            // Circle must contain full text bbox (plus safety inset): d >= diagonal(rect)
            let rect_w = text.width * SHAPE_TEXT_WIDTH_GUARD + 2.0 * MIN_TEXT_INSET;
            let rect_h = text.height * SHAPE_TEXT_HEIGHT_GUARD + 2.0 * MIN_TEXT_INSET;
            let min_d = (rect_w * rect_w + rect_h * rect_h).sqrt();
            let diameter = mermaid_d.max(min_d);
            (diameter, diameter)
        }
        // Mermaid doubleCircle.ts: inner diameter = bbox.width + padding, outer adds gap*2
        NodeShape::DoubleCircle => {
            let mermaid_inner_d = text.width + MERMAID_NODE_PADDING;
            let rect_w = text.width * SHAPE_TEXT_WIDTH_GUARD + 2.0 * MIN_TEXT_INSET;
            let rect_h = text.height * SHAPE_TEXT_HEIGHT_GUARD + 2.0 * MIN_TEXT_INSET;
            let min_inner_d = (rect_w * rect_w + rect_h * rect_h).sqrt();
            let inner_d = mermaid_inner_d.max(min_inner_d);
            let outer_d = inner_d + 10.0;
            (outer_d, outer_d)
        }
        NodeShape::Hexagon => (base_w + base_h * 0.5, base_h),
        NodeShape::Asymmetric => {
            // The left V-notch eats into text area: notch = hh * 0.6 = base_h * 0.3
            // Add full notch width (text is shifted right by notch/2, so both sides need room)
            let notch = base_h * 0.3;
            (base_w + 2.0 * notch + RECT_LABEL_EXTRA_WIDTH, base_h)
        }
        _ => (base_w, base_h),
    }
}

fn plain_text_for_measurement(text: &str) -> Cow<'_, str> {
    if !text.contains('<') {
        return Cow::Borrowed(text);
    }

    let normalized = crate::render::html_util::normalize_br(text);
    if !normalized.contains('<') {
        return Cow::Owned(normalized);
    }

    Cow::Owned(crate::render::html_util::strip_html_tags(&normalized))
}

fn measure_text_block(text: &str, measurer: &TextMeasurer<'_>) -> TextMetrics {
    const LINE_SPACING: f32 = 10.0;
    if text.contains('\n') {
        measurer.measure_multiline(text, LINE_SPACING)
    } else {
        measurer.measure(text)
    }
}

fn edge_label_dimensions(label_text: &str, measurer: &TextMeasurer<'_>) -> (f64, f64) {
    const EDGE_LABEL_PAD: f64 = 10.0;
    let plain = plain_text_for_measurement(label_text);
    let metrics = measure_text_block(&plain, measurer);
    (
        metrics.width + EDGE_LABEL_PAD,
        metrics.height + EDGE_LABEL_PAD,
    )
}

fn build_display_label_and_metrics(
    raw_label: &str,
    measurer: &TextMeasurer<'_>,
) -> (String, TextMetrics) {
    let plain = plain_text_for_measurement(raw_label);
    let wrapped_text = measurer.wrap_text(&plain, MAX_NODE_TEXT_WIDTH);
    let label = if wrapped_text != plain {
        wrapped_text.clone()
    } else {
        raw_label.to_string()
    };

    let metrics = apply_html_style_measurement_adjustments(
        raw_label,
        measure_text_block(&wrapped_text, measurer),
        measurer,
    );
    (label, metrics)
}

fn apply_html_style_measurement_adjustments(
    raw_label: &str,
    mut metrics: TextMetrics,
    measurer: &TextMeasurer<'_>,
) -> TextMetrics {
    if !crate::render::html_util::has_html(raw_label) {
        return metrics;
    }

    let normalized = crate::render::html_util::normalize_br(raw_label);
    if !normalized.contains("<b>")
        && !normalized.contains("<B>")
        && !normalized.contains("<strong>")
        && !normalized.contains("<STRONG>")
    {
        return metrics;
    }

    // Approximate bold text expansion for HTML labels by widening bold segments.
    // This mirrors Mermaid's browser measurement behavior more closely than treating
    // all segments as regular-weight text.
    const BOLD_WIDTH_MULTIPLIER: f64 = 1.14;
    let mut max_line_width = 0.0_f64;
    for line in normalized.lines() {
        let segments = crate::render::html_util::parse_segments(line);
        let mut line_width = 0.0_f64;
        for seg in segments {
            let mut seg_width = measurer.measure(&seg.text).width;
            if seg.bold {
                seg_width *= BOLD_WIDTH_MULTIPLIER;
            }
            line_width += seg_width;
        }
        if line_width > max_line_width {
            max_line_width = line_width;
        }
    }

    if max_line_width > 0.0 {
        metrics.width = metrics.width.max(max_line_width);
    }

    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::common::{parse_style_string, StyleProperties};
    use crate::font::FontProvider;
    use crate::parser::flowchart::parse_flowchart;
    use serde::Deserialize;
    use std::collections::{HashMap, HashSet};

    fn make_measurer(provider: &FontProvider) -> TextMeasurer<'_> {
        let font = provider.font_ref().unwrap();
        TextMeasurer::new(font, 14.0)
    }

    #[test]
    fn test_collect_subgraph_nodes_nested() {
        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "Outer".to_string(),
                label: None,
                direction: None,
                nodes: vec![NodeDef {
                    id: "A".into(),
                    label: None,
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                }],
                edges: vec![],
                subgraphs: vec![SubgraphDef {
                    id: "Inner".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![NodeDef {
                        id: "B".into(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    }],
                    edges: vec![],
                    subgraphs: vec![],
                }],
            }],
            ..Default::default()
        };
        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        assert!(all_nodes.contains_key("A"));
        assert!(all_nodes.contains_key("B"));
    }

    #[test]
    fn test_resolve_node_style_class_defs_and_overrides() {
        let ast = FlowchartAst {
            nodes: vec![NodeDef {
                id: "A".into(),
                label: None,
                shape: NodeShape::Rectangle,
                class_shorthand: Some("green".into()),
            }],
            class_defs: vec![
                ClassDef {
                    name: "green".to_string(),
                    properties: parse_style_string("fill:#9f6"),
                },
                ClassDef {
                    name: "orange".to_string(),
                    properties: parse_style_string("fill:#f96"),
                },
            ],
            class_assignments: vec![ClassAssignment {
                node_ids: vec!["B".into()],
                class_name: "orange".to_string(),
            }],
            style_overrides: vec![StyleOverride {
                node_id: "A".into(),
                properties: parse_style_string("stroke:#333"),
            }],
            edges: vec![],
            subgraphs: vec![SubgraphDef {
                id: "SG".to_string(),
                label: None,
                direction: None,
                nodes: vec![NodeDef {
                    id: "B".into(),
                    label: None,
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                }],
                edges: vec![],
                subgraphs: vec![],
            }],
            ..Default::default()
        };
        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        let (_, style_a) = all_nodes.get("A").unwrap();
        assert!(style_a.fill.is_some() || style_a.stroke.is_some());
    }

    #[test]
    fn test_collect_subgraph_edges_recursive() {
        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "Outer".to_string(),
                label: None,
                direction: None,
                nodes: vec![],
                edges: vec![EdgeDef {
                    from: "A".into(),
                    to: "B".into(),
                    line_style: crate::ast::flowchart::LineStyle::Solid,
                    arrow_start: crate::ast::flowchart::ArrowEnd::None,
                    arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                    label: None,
                    from_side: None,
                    to_side: None,
                }],
                subgraphs: vec![SubgraphDef {
                    id: "Inner".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![],
                    edges: vec![EdgeDef {
                        from: "C".into(),
                        to: "D".into(),
                        line_style: crate::ast::flowchart::LineStyle::Solid,
                        arrow_start: crate::ast::flowchart::ArrowEnd::None,
                        arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                        label: None,
                        from_side: None,
                        to_side: None,
                    }],
                    subgraphs: vec![],
                }],
            }],
            ..Default::default()
        };
        let all_edges = collect_all_edges(&ast);
        assert!(all_edges.iter().any(|e| e.from == "A" && e.to == "B"));
        assert!(all_edges.iter().any(|e| e.from == "C" && e.to == "D"));
    }

    #[test]
    fn test_collect_membership_nested() {
        let ast = FlowchartAst {
            subgraphs: vec![SubgraphDef {
                id: "Outer".to_string(),
                label: None,
                direction: None,
                nodes: vec![NodeDef {
                    id: "A".into(),
                    label: None,
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                }],
                edges: vec![],
                subgraphs: vec![SubgraphDef {
                    id: "Inner".to_string(),
                    label: None,
                    direction: None,
                    nodes: vec![NodeDef {
                        id: "B".into(),
                        label: None,
                        shape: NodeShape::Rectangle,
                        class_shorthand: None,
                    }],
                    edges: vec![],
                    subgraphs: vec![],
                }],
            }],
            ..Default::default()
        };
        let membership = build_subgraph_membership(&ast);
        assert_eq!(membership.get("A").unwrap(), &vec!["Outer".to_string()]);
        assert_eq!(
            membership.get("B").unwrap(),
            &vec!["Outer".to_string(), "Inner".to_string()]
        );
    }

    #[test]
    fn test_collect_all_nodes_edge_referenced() {
        // Edge references nodes not in ast.nodes -> or_insert_with (lines 53-60)
        let ast = FlowchartAst {
            nodes: vec![],
            edges: vec![EdgeDef {
                from: "A".into(),
                to: "B".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
                from_side: None,
                to_side: None,
            }],
            ..Default::default()
        };
        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        assert!(all_nodes.contains_key("A"));
        assert!(all_nodes.contains_key("B"));
        let (node_a, _) = all_nodes.get("A").unwrap();
        assert_eq!(node_a.shape, NodeShape::Rectangle);
    }

    #[test]
    fn test_collect_subgraph_nodes_edge_referenced() {
        // Subgraph edge references nodes not in sg.nodes -> or_insert_with (lines 82-89)
        let ast = FlowchartAst {
            nodes: vec![],
            edges: vec![],
            subgraphs: vec![SubgraphDef {
                id: "SG".to_string(),
                label: None,
                direction: None,
                nodes: vec![],
                edges: vec![EdgeDef {
                    from: "X".into(),
                    to: "Y".into(),
                    line_style: crate::ast::flowchart::LineStyle::Solid,
                    arrow_start: crate::ast::flowchart::ArrowEnd::None,
                    arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                    label: None,
                    from_side: None,
                    to_side: None,
                }],
                subgraphs: vec![],
            }],
            ..Default::default()
        };
        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        assert!(all_nodes.contains_key("X"));
        assert!(all_nodes.contains_key("Y"));
    }

    #[test]
    fn test_collect_all_nodes_does_not_materialize_subgraph_id_from_edges() {
        let ast = FlowchartAst {
            nodes: vec![],
            edges: vec![EdgeDef {
                from: "__start".into(),
                to: "Comp".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
                from_side: None,
                to_side: None,
            }],
            subgraphs: vec![SubgraphDef {
                id: "Comp".to_string(),
                label: Some("Comp".to_string()),
                direction: None,
                nodes: vec![NodeDef {
                    id: "Inner".into(),
                    label: Some("Inner".into()),
                    shape: NodeShape::Rectangle,
                    class_shorthand: None,
                }],
                edges: vec![],
                subgraphs: vec![],
            }],
            ..Default::default()
        };

        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);

        assert!(all_nodes.contains_key("__start"));
        assert!(
            !all_nodes.contains_key("Comp"),
            "subgraph id should remain a compound container, not a leaf node"
        );
    }

    #[test]
    fn test_compute_node_size_shapes() {
        let mut all_nodes = HashMap::new();
        for (id, shape) in [
            ("D", NodeShape::Diamond),
            ("C", NodeShape::Circle),
            ("DC", NodeShape::DoubleCircle),
            ("H", NodeShape::Hexagon),
            ("A", NodeShape::Asymmetric),
        ] {
            all_nodes.insert(
                id.to_string(),
                (
                    NodeDef {
                        id: id.to_string(),
                        label: Some("X".into()),
                        shape,
                        class_shorthand: None,
                    },
                    StyleProperties::default(),
                ),
            );
        }
        let edges = vec![
            EdgeDef {
                from: "D".into(),
                to: "C".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
                from_side: None,
                to_side: None,
            },
            EdgeDef {
                from: "C".into(),
                to: "DC".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
                from_side: None,
                to_side: None,
            },
            EdgeDef {
                from: "DC".into(),
                to: "H".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
                from_side: None,
                to_side: None,
            },
            EdgeDef {
                from: "H".into(),
                to: "A".into(),
                line_style: crate::ast::flowchart::LineStyle::Solid,
                arrow_start: crate::ast::flowchart::ArrowEnd::None,
                arrow_end: crate::ast::flowchart::ArrowEnd::Arrow,
                label: None,
                from_side: None,
                to_side: None,
            },
        ];
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let (graph, _) = build_petgraph(&all_nodes, &edges, &measurer).unwrap();
        for (id, expected_shape) in [
            ("D", NodeShape::Diamond),
            ("C", NodeShape::Circle),
            ("DC", NodeShape::DoubleCircle),
            ("H", NodeShape::Hexagon),
            ("A", NodeShape::Asymmetric),
        ] {
            let node = graph.node_indices().find(|i| graph[*i].id == id).unwrap();
            assert_eq!(graph[node].shape, expected_shape);
            assert!(graph[node].width > 0.0);
            assert!(graph[node].height > 0.0);
        }
    }

    #[test]
    fn test_circle_and_diamond_never_overflow_text_bbox() {
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let labels = [
            "Circle",
            "Link text",
            "Inner / circle\nand some odd\nspecial characters",
            "WIDE WIDE WIDE LABEL",
        ];

        for label in labels {
            let metrics = if label.contains('\n') {
                measurer.measure_multiline(label, 10.0)
            } else {
                measurer.measure(label)
            };

            let (cw, ch) = compute_node_size(&NodeShape::Circle, &metrics);
            let rect_diag =
                (metrics.width * metrics.width + metrics.height * metrics.height).sqrt();
            assert!(
                cw >= rect_diag && ch >= rect_diag,
                "circle too small for label '{label}': node=({cw},{ch}) text=({}, {}) diag={rect_diag}",
                metrics.width,
                metrics.height
            );

            let (dw, dh) = compute_node_size(&NodeShape::Diamond, &metrics);
            assert!(
                dw >= metrics.width + metrics.height && dh >= metrics.width + metrics.height,
                "diamond too small for label '{label}': node=({dw},{dh}) text=({}, {})",
                metrics.width,
                metrics.height
            );
        }
    }

    #[derive(Debug, Deserialize)]
    struct MermaidDagreInputRef {
        graph: MermaidGraphRef,
        nodes: Vec<MermaidNodeRef>,
        edges: Vec<MermaidEdgeRef>,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidGraphRef {
        rankdir: String,
        nodesep: f64,
        ranksep: f64,
        marginx: f64,
        marginy: f64,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidNodeRef {
        id: String,
        width: Option<f64>,
        height: Option<f64>,
        parent: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidEdgeRef {
        from: String,
        to: String,
        name: Option<String>,
        width: f64,
        height: f64,
        minlen: f64,
        weight: f64,
        labeloffset: f64,
        labelpos: String,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidOrderSnapshot {
        graph: MermaidGraphRef,
        node_order: Vec<String>,
        edge_order: Vec<MermaidOrderEdgeRef>,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidOrderEdgeRef {
        from: String,
        to: String,
        minlen: f64,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidAfterLayoutRef {
        graph: MermaidAfterGraphRef,
        nodes: Vec<MermaidAfterNodeRef>,
        edges: Vec<MermaidAfterEdgeRef>,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidAfterGraphRef {
        width: f64,
        height: f64,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidAfterNodeRef {
        id: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        parent: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidAfterEdgeRef {
        from: String,
        to: String,
        minlen: f64,
        points: Vec<MermaidPointRef>,
    }

    #[derive(Debug, Deserialize)]
    struct MermaidPointRef {
        x: f64,
        y: f64,
    }

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    fn milli(v: f64) -> i64 {
        (v * 1000.0).round() as i64
    }

    fn build_graph_from_numeric_ref_input(
        reference: &MermaidDagreInputRef,
    ) -> dagre_rust::LayoutGraph {
        let mut g = dagre_rust::Graph::with_options(&dagre_rust::GraphOptions {
            directed: true,
            multigraph: true,
            compound: true,
        });

        let mut gl = dagre_rust::GraphLabel::default();
        gl.rankdir = match reference.graph.rankdir.as_str() {
            "TB" => dagre_rust::RankDir::TB,
            "BT" => dagre_rust::RankDir::BT,
            "LR" => dagre_rust::RankDir::LR,
            "RL" => dagre_rust::RankDir::RL,
            other => panic!("unsupported rankdir in fixture: {other}"),
        };
        gl.nodesep = reference.graph.nodesep;
        gl.ranksep = reference.graph.ranksep;
        gl.marginx = reference.graph.marginx;
        gl.marginy = reference.graph.marginy;
        g.set_graph(gl);

        for n in &reference.nodes {
            let mut nl = dagre_rust::NodeLabel::default();
            if let Some(w) = n.width {
                nl.width = w;
            }
            if let Some(h) = n.height {
                nl.height = h;
            }
            g.set_node(&n.id, Some(nl));
        }

        for n in &reference.nodes {
            if let Some(parent) = &n.parent {
                g.set_parent(&n.id, Some(parent));
            }
        }

        for e in &reference.edges {
            let mut el = dagre_rust::EdgeLabel::default();
            el.width = e.width;
            el.height = e.height;
            el.minlen = e.minlen;
            el.weight = e.weight;
            el.labeloffset = e.labeloffset;
            el.labelpos = match e.labelpos.as_str() {
                "l" | "L" => dagre_rust::LabelPos::Left,
                "c" | "C" => dagre_rust::LabelPos::Center,
                "r" | "R" => dagre_rust::LabelPos::Right,
                other => panic!("unsupported labelpos in fixture: {other}"),
            };
            g.set_edge(&e.from, &e.to, Some(el), e.name.as_deref());
        }

        g
    }

    fn build_example5_dagre_and_ref() -> (dagre_rust::LayoutGraph, MermaidDagreInputRef) {
        let source = include_str!("../../../../../tests/test_loop/input_mermaid.mmd");
        let ast = parse_flowchart(source).expect("example #5 should parse");

        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        let all_edges = collect_all_edges(&ast);
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);

        let (g, _) = build_dagre_graph(&all_nodes, &all_edges, &measurer, ast.direction, &ast)
            .expect("dagre graph build should succeed");

        let reference: MermaidDagreInputRef = serde_json::from_str(include_str!(
            "../../../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"
        ))
        .expect("reference fixture should deserialize");

        (g, reference)
    }

    fn build_example2_dagre_and_ref() -> (dagre_rust::LayoutGraph, MermaidDagreInputRef) {
        let source = include_str!("../../../../../tests/test_loop/example2_input.mmd");
        let ast = parse_flowchart(source).expect("example #2 should parse");

        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        let all_edges = collect_all_edges(&ast);
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);

        let (g, _) = build_dagre_graph(&all_nodes, &all_edges, &measurer, ast.direction, &ast)
            .expect("dagre graph build should succeed");

        let reference: MermaidDagreInputRef = serde_json::from_str(include_str!(
            "../../../../../tests/test_loop/example2_mermaidjs_dagre_input_reduced.json"
        ))
        .expect("reference fixture should deserialize");

        (g, reference)
    }

    fn build_example7_dagre_and_ref() -> (dagre_rust::LayoutGraph, MermaidDagreInputRef) {
        let source = include_str!("../../../../../tests/test_loop/complex_subgraphs.mmd");
        let ast = parse_flowchart(source).expect("example #7 should parse");

        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        let all_edges = collect_all_edges(&ast);
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);

        let (g, _) = build_dagre_graph(&all_nodes, &all_edges, &measurer, ast.direction, &ast)
            .expect("dagre graph build should succeed");

        let reference: MermaidDagreInputRef = serde_json::from_str(include_str!(
            "../../../../../tests/test_loop/example7_mermaidjs_dagre_input_reduced.json"
        ))
        .expect("reference fixture should deserialize");

        (g, reference)
    }

    #[test]
    fn test_example5_dagre_input_structure_matches_mermaidjs_debug_snapshot() {
        let (g, reference) = build_example5_dagre_and_ref();

        let gl = g.graph();
        assert_eq!(reference.graph.rankdir, format!("{:?}", gl.rankdir));
        assert!(
            approx_eq(reference.graph.nodesep, gl.nodesep, 1e-6),
            "nodesep mismatch: expected {}, got {}",
            reference.graph.nodesep,
            gl.nodesep
        );
        assert!(
            approx_eq(reference.graph.ranksep, gl.ranksep, 1e-6),
            "ranksep mismatch: expected {}, got {}",
            reference.graph.ranksep,
            gl.ranksep
        );
        assert!(approx_eq(reference.graph.marginx, gl.marginx, 1e-6));
        assert!(approx_eq(reference.graph.marginy, gl.marginy, 1e-6));

        let expected_nodes: HashMap<_, _> =
            reference.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        assert_eq!(expected_nodes.len(), g.nodes().len());

        for node_id in g.nodes() {
            let expected = expected_nodes
                .get(node_id.as_str())
                .unwrap_or_else(|| panic!("missing expected node: {node_id}"));
            let actual_parent = g.parent(&node_id).map(|s| s.to_string());
            assert_eq!(
                expected.parent, actual_parent,
                "parent mismatch for {}",
                node_id
            );
        }

        let expected_edges: HashSet<_> = reference
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), milli(e.minlen)))
            .collect();
        let actual_edges: HashSet<_> = g
            .edges()
            .into_iter()
            .map(|edge| {
                let el = g
                    .edge_by_obj(&edge)
                    .unwrap_or_else(|| panic!("missing edge label for {} -> {}", edge.v, edge.w));
                (edge.v.clone(), edge.w.clone(), milli(el.minlen))
            })
            .collect();
        let actual_edges: HashSet<_> = actual_edges
            .iter()
            .map(|(v, w, m)| (v.as_str(), w.as_str(), *m))
            .collect();
        assert_eq!(expected_edges, actual_edges);
    }

    #[test]
    fn test_example2_dagre_input_structure_matches_mermaidjs_debug_snapshot() {
        let (g, reference) = build_example2_dagre_and_ref();

        let gl = g.graph();
        assert_eq!(reference.graph.rankdir, format!("{:?}", gl.rankdir));
        assert!(
            approx_eq(reference.graph.nodesep, gl.nodesep, 1e-6),
            "nodesep mismatch: expected {}, got {}",
            reference.graph.nodesep,
            gl.nodesep
        );
        assert!(
            approx_eq(reference.graph.ranksep, gl.ranksep, 1e-6),
            "ranksep mismatch: expected {}, got {}",
            reference.graph.ranksep,
            gl.ranksep
        );
        assert!(approx_eq(reference.graph.marginx, gl.marginx, 1e-6));
        assert!(approx_eq(reference.graph.marginy, gl.marginy, 1e-6));

        let expected_nodes: HashMap<_, _> =
            reference.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        assert!(
            g.nodes().len() >= expected_nodes.len(),
            "expected at least {} nodes, got {}",
            expected_nodes.len(),
            g.nodes().len()
        );

        for node_id in expected_nodes.keys() {
            let expected = expected_nodes
                .get(node_id)
                .unwrap_or_else(|| panic!("missing expected node ref: {node_id}"));
            let actual_parent = g.parent(&node_id).map(|s| s.to_string());
            assert_eq!(
                expected.parent, actual_parent,
                "parent mismatch for {}",
                node_id
            );
        }

        let expected_edges: HashSet<_> = reference
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), milli(e.minlen)))
            .collect();
        let actual_edges: HashSet<_> = g
            .edges()
            .into_iter()
            .map(|edge| {
                let el = g
                    .edge_by_obj(&edge)
                    .unwrap_or_else(|| panic!("missing edge label for {} -> {}", edge.v, edge.w));
                (edge.v.clone(), edge.w.clone(), milli(el.minlen))
            })
            .collect();
        let actual_edges: HashSet<_> = actual_edges
            .iter()
            .map(|(v, w, m)| (v.as_str(), w.as_str(), *m))
            .collect();
        for expected in &expected_edges {
            assert!(
                actual_edges.contains(expected),
                "missing expected edge {:?}; actual edge count={}",
                expected,
                actual_edges.len()
            );
        }
    }

    #[test]
    fn test_example7_dagre_input_structure_matches_mermaidjs_debug_snapshot() {
        let (g, reference) = build_example7_dagre_and_ref();

        let gl = g.graph();
        assert_eq!(reference.graph.rankdir, format!("{:?}", gl.rankdir));
        assert!(
            approx_eq(reference.graph.nodesep, gl.nodesep, 1e-6),
            "nodesep mismatch: expected {}, got {}",
            reference.graph.nodesep,
            gl.nodesep
        );
        assert!(
            approx_eq(reference.graph.ranksep, gl.ranksep, 1e-6),
            "ranksep mismatch: expected {}, got {}",
            reference.graph.ranksep,
            gl.ranksep
        );
        assert!(approx_eq(reference.graph.marginx, gl.marginx, 1e-6));
        assert!(approx_eq(reference.graph.marginy, gl.marginy, 1e-6));

        let expected_nodes: HashMap<_, _> =
            reference.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        assert_eq!(expected_nodes.len(), g.nodes().len());

        for node_id in g.nodes() {
            let expected = expected_nodes
                .get(node_id.as_str())
                .unwrap_or_else(|| panic!("missing expected node: {node_id}"));
            let actual_parent = g.parent(&node_id).map(|s| s.to_string());
            assert_eq!(
                expected.parent, actual_parent,
                "parent mismatch for {}",
                node_id
            );
        }

        let expected_edges: HashSet<_> = reference
            .edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), milli(e.minlen)))
            .collect();
        let actual_edges: HashSet<_> = g
            .edges()
            .into_iter()
            .map(|edge| {
                let el = g
                    .edge_by_obj(&edge)
                    .unwrap_or_else(|| panic!("missing edge label for {} -> {}", edge.v, edge.w));
                (edge.v.clone(), edge.w.clone(), milli(el.minlen))
            })
            .collect();
        let actual_edges: HashSet<_> = actual_edges
            .iter()
            .map(|(v, w, m)| (v.as_str(), w.as_str(), *m))
            .collect();
        assert_eq!(expected_edges, actual_edges);
    }

    #[test]
    fn test_example5_identical_numeric_dagre_input_produces_identical_output() {
        let input_ref: MermaidDagreInputRef = serde_json::from_str(include_str!(
            "../../../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"
        ))
        .expect("input fixture should deserialize");
        let output_ref: MermaidAfterLayoutRef = serde_json::from_str(include_str!(
            "../../../../../tests/test_loop/example5_mermaidjs_dagre_after_layout_reduced.json"
        ))
        .expect("after-layout fixture should deserialize");

        let mut g = build_graph_from_numeric_ref_input(&input_ref);
        dagre_rust::layout(&mut g);

        let gl = g.graph();
        assert!(
            approx_eq(gl.width, output_ref.graph.width, 1e-6),
            "graph width mismatch (expected {}, got {})",
            output_ref.graph.width,
            gl.width
        );
        assert!(
            approx_eq(gl.height, output_ref.graph.height, 1e-6),
            "graph height mismatch (expected {}, got {})",
            output_ref.graph.height,
            gl.height
        );

        for expected in &output_ref.nodes {
            let nl = g
                .node(&expected.id)
                .unwrap_or_else(|| panic!("missing node in dagre output: {}", expected.id));
            assert!(
                approx_eq(nl.x.unwrap_or_default(), expected.x, 1e-6),
                "node x mismatch for {}",
                expected.id
            );
            assert!(
                approx_eq(nl.y.unwrap_or_default(), expected.y, 1e-6),
                "node y mismatch for {}",
                expected.id
            );
            assert!(
                approx_eq(nl.width, expected.width, 1e-6),
                "node width mismatch for {}",
                expected.id
            );
            assert!(
                approx_eq(nl.height, expected.height, 1e-6),
                "node height mismatch for {}",
                expected.id
            );
            let actual_parent = g.parent(&expected.id).map(|s| s.to_string());
            assert_eq!(
                actual_parent, expected.parent,
                "node parent mismatch for {}",
                expected.id
            );
        }

        for expected in &output_ref.edges {
            let edge_obj = g
                .edges()
                .into_iter()
                .find(|e| e.v == expected.from && e.w == expected.to)
                .unwrap_or_else(|| {
                    panic!(
                        "missing edge in dagre output: {} -> {}",
                        expected.from, expected.to
                    )
                });
            let el = g.edge_by_obj(&edge_obj).unwrap_or_else(|| {
                panic!(
                    "missing edge label for {} -> {}",
                    expected.from, expected.to
                )
            });

            assert!(
                approx_eq(el.minlen, expected.minlen, 1e-6),
                "edge minlen mismatch for {} -> {}",
                expected.from,
                expected.to
            );
            assert_eq!(
                el.points.len(),
                expected.points.len(),
                "edge point count mismatch for {} -> {}",
                expected.from,
                expected.to
            );
            for (idx, (actual_p, expected_p)) in
                el.points.iter().zip(expected.points.iter()).enumerate()
            {
                assert!(
                    approx_eq(actual_p.x, expected_p.x, 1e-6),
                    "edge point[{idx}] x mismatch for {} -> {}",
                    expected.from,
                    expected.to
                );
                assert!(
                    approx_eq(actual_p.y, expected_p.y, 1e-6),
                    "edge point[{idx}] y mismatch for {} -> {}",
                    expected.from,
                    expected.to
                );
            }
        }
    }

    #[test]
    fn test_example4_dagre_order_matches_mermaidjs_debug_snapshot() {
        let source = include_str!("../../../../../tests/test_loop/example4_input.mmd");
        let ast = parse_flowchart(source).expect("example #4 should parse");

        let class_defs = build_class_map(&ast.class_defs);
        let all_nodes = collect_all_nodes(&ast, &class_defs);
        let all_edges = collect_all_edges(&ast);
        let provider = FontProvider::default_font();
        let measurer = make_measurer(&provider);
        let (g, _) = build_dagre_graph(&all_nodes, &all_edges, &measurer, ast.direction, &ast)
            .expect("dagre graph build should succeed");

        let snap: MermaidOrderSnapshot = serde_json::from_str(include_str!(
            "../../../../../tests/test_loop/example4_mermaidjs_dagre_order_snapshot.json"
        ))
        .expect("order snapshot fixture should deserialize");

        let gl = g.graph();
        assert_eq!(snap.graph.rankdir, format!("{:?}", gl.rankdir));
        assert!(approx_eq(snap.graph.nodesep, gl.nodesep, 1e-6));
        assert!(approx_eq(snap.graph.ranksep, gl.ranksep, 1e-6));
        assert!(approx_eq(snap.graph.marginx, gl.marginx, 1e-6));
        assert!(approx_eq(snap.graph.marginy, gl.marginy, 1e-6));

        assert_eq!(snap.node_order, g.nodes(), "node insertion order mismatch");

        let actual_edges: Vec<_> = g
            .edges()
            .into_iter()
            .map(|e| {
                let el = g
                    .edge_by_obj(&e)
                    .unwrap_or_else(|| panic!("missing edge label for {} -> {}", e.v, e.w));
                MermaidOrderEdgeRef {
                    from: e.v,
                    to: e.w,
                    minlen: el.minlen,
                }
            })
            .collect();

        let expected_edges: Vec<_> = snap.edge_order;
        assert_eq!(expected_edges.len(), actual_edges.len());
        for (idx, (exp, act)) in expected_edges.iter().zip(actual_edges.iter()).enumerate() {
            assert_eq!(exp.from, act.from, "edge[{idx}] from mismatch");
            assert_eq!(exp.to, act.to, "edge[{idx}] to mismatch");
            assert!(
                approx_eq(exp.minlen, act.minlen, 1e-6),
                "edge[{idx}] minlen mismatch"
            );
        }
    }

    #[test]
    fn test_example5_dagre_input_matches_mermaidjs_debug_snapshot() {
        let (g, reference) = build_example5_dagre_and_ref();
        const NODE_DIM_TOLERANCE: f64 = 20.0;
        const EDGE_DIM_TOLERANCE: f64 = 20.0;

        let expected_nodes: HashMap<_, _> =
            reference.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        assert_eq!(expected_nodes.len(), g.nodes().len());
        for node_id in g.nodes() {
            let expected = expected_nodes
                .get(node_id.as_str())
                .unwrap_or_else(|| panic!("missing expected node: {node_id}"));
            let nl = g.node(&node_id).expect("node should exist");
            if let Some(w) = expected.width {
                assert!(
                    approx_eq(w, nl.width, NODE_DIM_TOLERANCE),
                    "node width mismatch for {} (expected {}, got {})",
                    node_id,
                    w,
                    nl.width
                );
            }
            if let Some(h) = expected.height {
                assert!(
                    approx_eq(h, nl.height, NODE_DIM_TOLERANCE),
                    "node height mismatch for {} (expected {}, got {})",
                    node_id,
                    h,
                    nl.height
                );
            }
        }

        let expected_edges: HashMap<_, _> = reference
            .edges
            .iter()
            .map(|e| ((e.from.as_str(), e.to.as_str()), e))
            .collect();
        assert_eq!(expected_edges.len(), g.edges().len());

        for edge in g.edges() {
            let el = g
                .edge_by_obj(&edge)
                .unwrap_or_else(|| panic!("missing edge label for {} -> {}", edge.v, edge.w));
            let key = (edge.v.as_str(), edge.w.as_str());
            let expected = expected_edges
                .get(&key)
                .unwrap_or_else(|| panic!("missing expected edge for {} -> {}", edge.v, edge.w));

            assert!(
                approx_eq(expected.width, el.width, EDGE_DIM_TOLERANCE),
                "edge width mismatch for {} -> {}",
                edge.v,
                edge.w
            );
            assert!(
                approx_eq(expected.height, el.height, EDGE_DIM_TOLERANCE),
                "edge height mismatch for {} -> {}",
                edge.v,
                edge.w
            );
            assert!(approx_eq(expected.minlen, el.minlen, 1e-6));
        }
    }

    #[test]
    #[ignore = "debug helper for size parity tuning"]
    fn debug_example5_node_size_diffs_against_mermaidjs() {
        let (g, reference) = build_example5_dagre_and_ref();
        let expected_nodes: HashMap<_, _> =
            reference.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        let mut deltas = Vec::new();
        for node_id in g.nodes() {
            let Some(expected) = expected_nodes.get(node_id.as_str()) else {
                continue;
            };
            let nl = g.node(&node_id).expect("node should exist");
            if let (Some(w), Some(h)) = (expected.width, expected.height) {
                deltas.push((
                    node_id.clone(),
                    (nl.width - w).abs() + (nl.height - h).abs(),
                    nl.width - w,
                    nl.height - h,
                    nl.width,
                    nl.height,
                    w,
                    h,
                ));
            }
        }

        deltas.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        eprintln!("--- Top size deltas (ours - js) ---");
        for (id, score, dw, dh, ow, oh, ew, eh) in deltas.iter().take(15) {
            eprintln!(
                "{id:10} score={score:8.3} dw={dw:8.3} dh={dh:8.3} ours=({ow:8.3},{oh:8.3}) js=({ew:8.3},{eh:8.3})"
            );
        }
        panic!("debug output above");
    }

    #[test]
    #[ignore = "debug helper for input insertion ordering"]
    fn debug_example5_dagre_input_order_vs_mermaidjs() {
        let (g, reference) = build_example5_dagre_and_ref();
        let ours_nodes = g.nodes();
        let expected_nodes: Vec<String> = reference.nodes.iter().map(|n| n.id.clone()).collect();
        eprintln!("ours nodes:    {:?}", ours_nodes);
        eprintln!("expected nodes:{:?}", expected_nodes);

        let ours_edges: Vec<(String, String)> = g.edges().into_iter().map(|e| (e.v, e.w)).collect();
        let expected_edges: Vec<(String, String)> = reference
            .edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect();
        eprintln!("ours edges:    {:?}", ours_edges);
        eprintln!("expected edges:{:?}", expected_edges);
        panic!("debug output above");
    }
}
