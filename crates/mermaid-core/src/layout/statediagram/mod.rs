pub mod types;

use std::collections::HashMap;

use crate::ast::flowchart::{
    ArrowEnd, ClassAssignment as FcClassAssignment, ClassDef as FcClassDef, Direction, EdgeDef,
    FlowchartAst, LineStyle, NodeDef, NodeShape, StyleOverride as FcStyleOverride, SubgraphDef,
};
use crate::ast::statediagram::*;
use crate::error::Result;
use crate::layout::flowchart;
use crate::layout::flowchart::types::NodeData;
use crate::layout::text_measure::TextMeasurer;

use self::types::*;

/// Compute layout positions for a state diagram AST.
///
/// Strategy: convert the state diagram AST into a flowchart AST,
/// then replicate the flowchart layout pipeline step-by-step so we
/// can inject special-node size overrides BEFORE Sugiyama runs.
/// This ensures start/end/fork/join/choice nodes get correct sizes
/// during rank assignment and edge routing.
pub fn layout_statediagram(
    ast: &StateDiagramAst,
    measurer: &TextMeasurer<'_>,
) -> Result<PositionedStateDiagram> {
    // Build a map of state IDs to their kind for later enrichment
    let mut kind_map: HashMap<String, StateKind> = HashMap::new();
    collect_kinds(&ast.states, &ast.composites, &mut kind_map);

    // Track which IDs are notes so we can separate them later
    let mut note_ids: Vec<String> = Vec::new();

    // 1. Convert state diagram AST to flowchart AST
    let mut fc_ast = convert_to_flowchart_ast(ast, &mut note_ids);

    // Wrap note text to a max width so notes don't stretch infinitely wide
    wrap_note_labels(&mut fc_ast, &note_ids, measurer);

    let is_horizontal = matches!(
        fc_ast.direction,
        Direction::LeftToRight | Direction::RightToLeft
    );

    // 2. Replicate the flowchart layout pipeline with pre-layout size override
    use crate::layout::flowchart::{compound, edge_routing, graph_builder, normalize};

    let class_defs = graph_builder::build_class_map(&fc_ast.class_defs);
    let all_nodes = graph_builder::collect_all_nodes(&fc_ast, &class_defs);
    let all_edges = graph_builder::collect_all_edges(&fc_ast);

    // Build dagre graph
    let (mut dagre_graph, mut node_data_map) = graph_builder::build_dagre_graph(
        &all_nodes,
        &all_edges,
        measurer,
        fc_ast.direction,
        &fc_ast,
    )?;

    // Override special node sizes in the dagre graph BEFORE layout
    override_dagre_node_sizes(&mut dagre_graph, &mut node_data_map, &kind_map);

    // For unlabeled bidirectional edges, inject phantom label dimensions
    inject_bidi_phantom_widths_dagre(&mut dagre_graph);

    // Run dagre layout
    dagre_rust::layout(&mut dagre_graph);

    let membership = graph_builder::build_subgraph_membership(&fc_ast);

    // Build positioned nodes from dagre results
    let mut positioned_nodes =
        flowchart::build_positioned_nodes_from_dagre(&dagre_graph, &node_data_map);

    // Position subgraphs from dagre compound layout, falling back to
    // bounding-box computation for any subgraphs dagre didn't position.
    let mut positioned_subgraphs = flowchart::build_positioned_subgraphs_from_dagre(
        &dagre_graph,
        &fc_ast.subgraphs,
        &fc_ast.style_overrides,
    );
    if positioned_subgraphs.len() < flowchart::count_subgraphs(&fc_ast.subgraphs) {
        positioned_subgraphs = compound::position_subgraphs(
            &fc_ast.subgraphs,
            &positioned_nodes,
            &fc_ast.style_overrides,
            measurer,
            &membership,
        );
    }

    // Extract bend points and route edges from dagre results
    let extraction = flowchart::extract_edge_data_from_dagre(&dagre_graph);

    let mut positioned_edges = edge_routing::route_edges(
        &positioned_nodes,
        &all_edges,
        is_horizontal,
        &extraction.raw_points,
        &extraction.bend_points,
        &extraction.label_positions,
        &extraction.label_dimensions,
    );

    // Mermaid stateDiagram behavior: transitions that enter a composite from
    // outside land on the composite boundary (not directly on inner [*]).
    adjust_composite_boundary_edge_endpoints(
        &mut positioned_edges,
        &positioned_subgraphs,
        &membership,
        &kind_map,
    );

    // Adjust edge labels for subgraph boundaries
    edge_routing::adjust_labels_for_subgraph_boundaries(
        &mut positioned_edges,
        &positioned_subgraphs,
    );

    // Prevent edge labels from overlapping state nodes
    adjust_labels_for_nodes(&mut positioned_edges, &positioned_nodes);

    // Normalize coordinates
    let (width, height) = normalize::normalize_and_compute_bounds(
        &mut positioned_nodes,
        &mut positioned_edges,
        &mut positioned_subgraphs,
    );

    // 3. Convert results back to state diagram types
    let positioned = flowchart::PositionedGraph {
        nodes: positioned_nodes,
        edges: positioned_edges,
        subgraphs: positioned_subgraphs,
        width,
        height,
        direction: fc_ast.direction,
    };
    let mut result = convert_from_flowchart_result(positioned, &kind_map, &note_ids);

    // Separate bidirectional edges with direct cubic bezier paths
    // (bypasses basis curve smoothing for accurate bow amplitude)
    separate_bidirectional_edges(&mut result.transitions);

    // Expand bounds to cover bowed edge points and repositioned labels
    expand_bounds_for_transitions(&mut result);

    Ok(result)
}

/// Override node sizes in the dagre graph BEFORE layout runs.
fn override_dagre_node_sizes(
    g: &mut dagre_rust::LayoutGraph,
    node_data_map: &mut HashMap<String, NodeData>,
    kind_map: &HashMap<String, StateKind>,
) {
    for (id, kind) in kind_map {
        let (w, h) = match kind {
            StateKind::Start => (14.0, 14.0),
            StateKind::End => (20.0, 20.0),
            StateKind::Fork | StateKind::Join => (70.0, 6.0),
            StateKind::Choice => (28.0, 28.0),
            StateKind::Normal => continue,
        };
        if let Some(nl) = g.node_mut(id) {
            nl.width = w;
            nl.height = h;
        }
        if let Some(nd) = node_data_map.get_mut(id) {
            nd.width = w;
            nd.height = h;
        }
    }
}

/// For bidirectional edge pairs without labels, inject phantom label dimensions
/// so dagre creates spacing between the edge paths.
fn inject_bidi_phantom_widths_dagre(g: &mut dagre_rust::LayoutGraph) {
    let phantom_width = 30.0;
    let phantom_height = 20.0;

    // Collect edges that need phantom widths
    let edges_to_update: Vec<dagre_rust::Edge> = g
        .edges()
        .iter()
        .filter(|e| {
            let has_no_label = g.edge_by_obj(e).map(|el| el.width < 1.0).unwrap_or(true);
            let has_reverse = g
                .out_edges(&e.w, Some(&e.v))
                .is_some_and(|rev_edges| !rev_edges.is_empty());
            has_no_label && has_reverse
        })
        .cloned()
        .collect();

    for e in edges_to_update {
        if let Some(el) = g.edge_mut_by_obj(&e) {
            el.width = phantom_width;
            el.height = phantom_height;
        }
    }
}

/// Wrap note labels to a maximum pixel width so notes don't stretch too wide.
/// Modifies the FlowchartAst node labels in-place for nodes identified as notes.
fn wrap_note_labels(fc_ast: &mut FlowchartAst, note_ids: &[String], measurer: &TextMeasurer<'_>) {
    let max_note_width = 200.0;

    fn wrap_in_nodes(
        nodes: &mut [NodeDef],
        note_ids: &[String],
        measurer: &TextMeasurer<'_>,
        max_width: f64,
    ) {
        for node in nodes.iter_mut() {
            if note_ids.contains(&node.id) {
                if let Some(ref mut label) = node.label {
                    *label = measurer.wrap_text(label, max_width);
                }
            }
        }
    }

    wrap_in_nodes(&mut fc_ast.nodes, note_ids, measurer, max_note_width);
    for sg in &mut fc_ast.subgraphs {
        wrap_in_subgraph(sg, note_ids, measurer, max_note_width);
    }
}

fn wrap_in_subgraph(
    sg: &mut SubgraphDef,
    note_ids: &[String],
    measurer: &TextMeasurer<'_>,
    max_width: f64,
) {
    for node in &mut sg.nodes {
        if note_ids.contains(&node.id) {
            if let Some(ref mut label) = node.label {
                *label = measurer.wrap_text(label, max_width);
            }
        }
    }
    for nested in &mut sg.subgraphs {
        wrap_in_subgraph(nested, note_ids, measurer, max_width);
    }
}

/// Recursively collect all state kinds into a map.
fn collect_kinds(
    states: &[StateDef],
    composites: &[CompositeStateDef],
    map: &mut HashMap<String, StateKind>,
) {
    for s in states {
        map.insert(s.id.clone(), s.kind);
    }
    for c in composites {
        collect_kinds(&c.states, &c.composites, map);
    }
}

/// Convert a StateDiagramAst into a FlowchartAst.
fn convert_to_flowchart_ast(ast: &StateDiagramAst, note_ids: &mut Vec<String>) -> FlowchartAst {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut subgraphs = Vec::new();

    // Convert states to nodes
    for state in &ast.states {
        nodes.push(convert_state_to_node(state));
    }

    // Convert transitions to edges
    for t in &ast.transitions {
        let from = resolve_composite_transition_endpoint(&t.from, true, &ast.composites);
        let to = resolve_composite_transition_endpoint(&t.to, false, &ast.composites);
        edges.push(EdgeDef {
            from,
            to,
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: t.label.clone(),
            from_side: None,
            to_side: None,
        });
    }

    // Convert composites to subgraphs
    for c in &ast.composites {
        subgraphs.push(convert_composite_to_subgraph(c, note_ids));
    }

    // Convert notes to nodes + invisible edges
    for note in &ast.notes {
        let (note_node, note_edge) = convert_note_to_node_and_edge(note, note_ids);
        nodes.push(note_node);
        if let Some(edge) = note_edge {
            edges.push(edge);
        }
    }

    // Pass through styling
    let class_defs = ast
        .class_defs
        .iter()
        .map(|cd| FcClassDef {
            name: cd.name.clone(),
            properties: cd.properties.clone(),
        })
        .collect();

    let class_assignments = ast
        .class_assignments
        .iter()
        .map(|ca| FcClassAssignment {
            node_ids: ca.node_ids.clone(),
            class_name: ca.class_name.clone(),
        })
        .collect();

    let style_overrides = ast
        .style_overrides
        .iter()
        .map(|so| FcStyleOverride {
            node_id: so.node_id.clone(),
            properties: so.properties.clone(),
        })
        .collect();

    FlowchartAst {
        direction: ast.direction,
        nodes,
        edges,
        subgraphs,
        class_defs,
        class_assignments,
        style_overrides,
    }
}

/// Map a state to a flowchart node.
fn convert_state_to_node(state: &StateDef) -> NodeDef {
    let shape = match state.kind {
        StateKind::Normal => NodeShape::RoundedRectangle,
        StateKind::Start => NodeShape::Circle,
        StateKind::End => NodeShape::DoubleCircle,
        StateKind::Fork | StateKind::Join => NodeShape::Rectangle,
        StateKind::Choice => NodeShape::Diamond,
    };

    // For special nodes (start, end, fork, join, choice), use a small placeholder label
    // so text measurement gives them a minimal size
    let label = match state.kind {
        StateKind::Start | StateKind::End | StateKind::Choice => None,
        StateKind::Fork | StateKind::Join => Some(" ".to_string()),
        StateKind::Normal => state.label.clone(),
    };

    NodeDef {
        id: state.id.clone(),
        label,
        shape,
        class_shorthand: state.class_shorthand.clone(),
    }
}

/// Convert a composite state to a flowchart subgraph (recursive).
fn convert_composite_to_subgraph(
    composite: &CompositeStateDef,
    note_ids: &mut Vec<String>,
) -> SubgraphDef {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut subgraphs = Vec::new();

    for state in &composite.states {
        nodes.push(convert_state_to_node(state));
    }

    for t in &composite.transitions {
        let from = resolve_composite_transition_endpoint(&t.from, true, &composite.composites);
        let to = resolve_composite_transition_endpoint(&t.to, false, &composite.composites);
        edges.push(EdgeDef {
            from,
            to,
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: t.label.clone(),
            from_side: None,
            to_side: None,
        });
    }

    for nested in &composite.composites {
        subgraphs.push(convert_composite_to_subgraph(nested, note_ids));
    }

    // Convert notes inside composites
    for note in &composite.notes {
        let (note_node, note_edge) = convert_note_to_node_and_edge(note, note_ids);
        nodes.push(note_node);
        if let Some(edge) = note_edge {
            edges.push(edge);
        }
    }

    // Handle dividers: each divider creates sub-subgraphs for concurrent regions
    // For now, dividers are treated as visual-only (rendered as dashed lines).
    // The states within the composite are all laid out together.
    // TODO: Full concurrent region support would split into sub-subgraphs.
    for divider in &composite.dividers {
        // Add divider as a node (rendered as horizontal line)
        nodes.push(NodeDef {
            id: divider.id.clone(),
            label: Some(" ".to_string()),
            shape: NodeShape::Rectangle,
            class_shorthand: None,
        });
    }

    SubgraphDef {
        id: composite.id.clone(),
        label: Some(
            composite
                .label
                .clone()
                .unwrap_or_else(|| composite.id.clone()),
        ),
        direction: composite.direction,
        nodes,
        edges,
        subgraphs,
    }
}

fn resolve_composite_transition_endpoint(
    endpoint_id: &str,
    is_source: bool,
    composites_in_scope: &[CompositeStateDef],
) -> String {
    let Some(target_composite) = composites_in_scope.iter().find(|c| c.id == endpoint_id) else {
        return endpoint_id.to_string();
    };

    let desired_kind = if is_source {
        StateKind::End
    } else {
        StateKind::Start
    };

    if let Some(id) = target_composite
        .states
        .iter()
        .find(|s| s.kind == desired_kind)
        .map(|s| s.id.clone())
    {
        return id;
    }

    // Mermaid-like fallback: if a composite has no explicit start/end pseudo-state,
    // use a concrete inner state so external transitions remain routable.
    let normal_candidates: Vec<&StateDef> = target_composite
        .states
        .iter()
        .filter(|s| !matches!(s.kind, StateKind::Start | StateKind::End))
        .collect();

    if is_source {
        normal_candidates
            .last()
            .map(|s| s.id.clone())
            .unwrap_or_else(|| endpoint_id.to_string())
    } else {
        normal_candidates
            .first()
            .map(|s| s.id.clone())
            .unwrap_or_else(|| endpoint_id.to_string())
    }
}

/// Convert a note to a node + invisible edge for anchoring.
fn convert_note_to_node_and_edge(
    note: &NoteDef,
    note_ids: &mut Vec<String>,
) -> (NodeDef, Option<EdgeDef>) {
    let note_node_id = note
        .id
        .clone()
        .unwrap_or_else(|| format!("__note_{}", note_ids.len()));

    note_ids.push(note_node_id.clone());

    let node = NodeDef {
        id: note_node_id.clone(),
        label: Some(note.text.clone()),
        shape: NodeShape::Rectangle,
        class_shorthand: None,
    };

    let edge = note.target_state.as_ref().map(|target| EdgeDef {
        from: target.clone(),
        to: note_node_id,
        line_style: LineStyle::Dotted,
        arrow_start: ArrowEnd::None,
        arrow_end: ArrowEnd::None,
        label: None,
        from_side: None,
        to_side: None,
    });

    (node, edge)
}

/// Separate bidirectional edges so A→B and B→A don't overlap.
///
/// Generates direct SVG cubic bezier paths (stored in `raw_path_d`) that
/// bypass the B-spline basis curve smoothing, giving precise control over
/// the bow amplitude. Each edge in a bidirectional pair bows to opposite sides.
fn separate_bidirectional_edges(transitions: &mut [PositionedTransition]) {
    use std::collections::HashSet;

    let edge_keys: HashSet<(String, String)> = transitions
        .iter()
        .map(|e| (e.from_id.clone(), e.to_id.clone()))
        .collect();

    let mut processed: HashSet<(String, String)> = HashSet::new();

    for i in 0..transitions.len() {
        let key = (transitions[i].from_id.clone(), transitions[i].to_id.clone());
        let reverse_key = (transitions[i].to_id.clone(), transitions[i].from_id.clone());

        if processed.contains(&key) || !edge_keys.contains(&reverse_key) {
            continue;
        }

        let Some(j) = transitions
            .iter()
            .position(|e| e.from_id == reverse_key.0 && e.to_id == reverse_key.1)
        else {
            continue;
        };

        processed.insert(key);
        processed.insert(reverse_key);

        let pts_i = &transitions[i].points;
        let pts_j = &transitions[j].points;
        if pts_i.len() < 2 || pts_j.len() < 2 {
            continue;
        }

        // Compute perpendicular direction from edge i's start→end
        let (sx, sy) = pts_i[0];
        let (ex, ey) = *pts_i.last().unwrap();
        let dx = ex - sx;
        let dy = ey - sy;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 {
            continue;
        }

        let (px, py) = (-dy / len, dx / len);
        let bow = (len * 0.45).clamp(25.0, 60.0);

        // Edge i: bow in +perpendicular direction
        transitions[i].raw_path_d = Some(make_cubic_bezier_bow(sx, sy, ex, ey, px, py, bow));

        // Edge j: bow in -perpendicular direction
        let (sx_j, sy_j) = pts_j[0];
        let (ex_j, ey_j) = *pts_j.last().unwrap();
        transitions[j].raw_path_d =
            Some(make_cubic_bezier_bow(sx_j, sy_j, ex_j, ey_j, -px, -py, bow));

        // Reposition labels to the bow apex
        let mid_x = (sx + ex) / 2.0;
        let mid_y = (sy + ey) / 2.0;
        if let Some(ref mut lx) = transitions[i].label_x {
            *lx = mid_x + px * bow;
        }
        if let Some(ref mut ly) = transitions[i].label_y {
            *ly = mid_y + py * bow;
        }
        if let Some(ref mut lx) = transitions[j].label_x {
            *lx = mid_x - px * bow;
        }
        if let Some(ref mut ly) = transitions[j].label_y {
            *ly = mid_y - py * bow;
        }

        // Update the points arrays so normalize_and_compute_bounds (already done)
        // would have captured them — but since we're post-normalize, we need to
        // store bow extremes as points for any future bounds computation.
        let apex_i = (mid_x + px * bow, mid_y + py * bow);
        transitions[i].points = vec![(sx, sy), apex_i, (ex, ey)];
        let apex_j = (mid_x - px * bow, mid_y - py * bow);
        transitions[j].points = vec![(sx_j, sy_j), apex_j, (ex_j, ey_j)];
    }
}

/// Build an SVG cubic bezier path `d` attribute for a bowed edge.
/// Uses a single `C` command with control points offset perpendicular to the
/// edge direction, creating a smooth arc without B-spline smoothing.
fn make_cubic_bezier_bow(sx: f64, sy: f64, ex: f64, ey: f64, px: f64, py: f64, bow: f64) -> String {
    // Control points at 1/3 and 2/3 along the edge, both offset by bow
    let cx1 = sx + (ex - sx) * 0.33 + px * bow;
    let cy1 = sy + (ey - sy) * 0.33 + py * bow;
    let cx2 = sx + (ex - sx) * 0.67 + px * bow;
    let cy2 = sy + (ey - sy) * 0.67 + py * bow;

    format!(
        "M {} {} C {} {} {} {} {} {}",
        sx, sy, cx1, cy1, cx2, cy2, ex, ey
    )
}

/// Expand diagram bounds to account for bowed edge control points and labels
/// that may extend past the original viewBox.
fn expand_bounds_for_transitions(diagram: &mut PositionedStateDiagram) {
    let padding = 8.0;
    let mut max_x = diagram.width;
    let mut max_y = diagram.height;
    let mut min_x = 0.0f64;
    let mut min_y = 0.0f64;

    for t in &diagram.transitions {
        for &(px, py) in &t.points {
            min_x = min_x.min(px);
            min_y = min_y.min(py);
            max_x = max_x.max(px + padding);
            max_y = max_y.max(py + padding);
        }
        if let (Some(lx), Some(lw)) = (t.label_x, t.label_width) {
            let half = lw / 2.0;
            min_x = min_x.min(lx - half);
            max_x = max_x.max(lx + half + padding);
        }
        if let (Some(ly), Some(lh)) = (t.label_y, t.label_height) {
            let half = lh / 2.0;
            min_y = min_y.min(ly - half);
            max_y = max_y.max(ly + half + padding);
        }
    }

    // If anything extends to negative coords, shift everything
    if min_x < 0.0 || min_y < 0.0 {
        let shift_x = if min_x < 0.0 { -min_x } else { 0.0 };
        let shift_y = if min_y < 0.0 { -min_y } else { 0.0 };

        for s in &mut diagram.states {
            s.x += shift_x;
            s.y += shift_y;
        }
        for t in &mut diagram.transitions {
            for p in &mut t.points {
                p.0 += shift_x;
                p.1 += shift_y;
            }
            if let Some(ref mut lx) = t.label_x {
                *lx += shift_x;
            }
            if let Some(ref mut ly) = t.label_y {
                *ly += shift_y;
            }
            // Regenerate raw_path_d with shifted coordinates
            if t.raw_path_d.is_some() && t.points.len() >= 3 {
                let (sx, sy) = t.points[0];
                let (ax, ay) = t.points[1]; // apex
                let (ex, ey) = t.points[2];
                // Reconstruct cubic bezier control points from apex
                let dx = ex - sx;
                let dy = ey - sy;
                let mid_x = (sx + ex) / 2.0;
                let mid_y = (sy + ey) / 2.0;
                let bow_x = ax - mid_x;
                let bow_y = ay - mid_y;
                let cx1 = sx + dx * 0.33 + bow_x;
                let cy1 = sy + dy * 0.33 + bow_y;
                let cx2 = sx + dx * 0.67 + bow_x;
                let cy2 = sy + dy * 0.67 + bow_y;
                t.raw_path_d = Some(format!(
                    "M {} {} C {} {} {} {} {} {}",
                    sx, sy, cx1, cy1, cx2, cy2, ex, ey
                ));
            }
        }
        for c in &mut diagram.composites {
            c.x += shift_x;
            c.y += shift_y;
        }
        for n in &mut diagram.notes {
            n.x += shift_x;
            n.y += shift_y;
        }
        max_x += shift_x;
        max_y += shift_y;
    }

    diagram.width = max_x;
    diagram.height = max_y;
}

/// Prevent edge labels from overlapping state nodes.
/// For each label, check if it overlaps any node and push it away.
fn adjust_labels_for_nodes(
    edges: &mut [flowchart::PositionedEdge],
    nodes: &[flowchart::PositionedNode],
) {
    let clearance = 4.0;

    for edge in edges.iter_mut() {
        let Some(lx) = edge.label_x else { continue };
        let Some(ly) = edge.label_y else { continue };
        let lw = edge.label_width.unwrap_or(0.0);
        let lh = edge.label_height.unwrap_or(0.0);
        if lw < 1.0 || lh < 1.0 {
            continue;
        }

        let mut cur_x = lx;
        let mut cur_y = ly;
        let lhw = lw / 2.0;
        let lhh = lh / 2.0;

        // Don't check against the edge's own source/target nodes
        let from_id = &edge.from_id;
        let to_id = &edge.to_id;

        for node in nodes {
            if node.id == *from_id || node.id == *to_id {
                continue;
            }

            let nhw = node.width / 2.0;
            let nhh = node.height / 2.0;
            let n_left = node.x - nhw;
            let n_right = node.x + nhw;
            let n_top = node.y - nhh;
            let n_bottom = node.y + nhh;

            let l_left = cur_x - lhw;
            let l_right = cur_x + lhw;
            let l_top = cur_y - lhh;
            let l_bottom = cur_y + lhh;

            // Check AABB overlap
            if l_right > n_left && l_left < n_right && l_bottom > n_top && l_top < n_bottom {
                // Push label away: find the smallest displacement
                let push_left = n_left - l_right - clearance;
                let push_right = n_right - l_left + clearance;
                let push_up = n_top - l_bottom - clearance;
                let push_down = n_bottom - l_top + clearance;

                // Choose the smallest absolute displacement
                let options = [
                    (push_left.abs(), push_left, 0.0),
                    (push_right.abs(), push_right, 0.0),
                    (push_up.abs(), 0.0, push_up),
                    (push_down.abs(), 0.0, push_down),
                ];
                if let Some(&(_, dx, dy)) =
                    options.iter().min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
                {
                    cur_x += dx;
                    cur_y += dy;
                }
            }
        }

        edge.label_x = Some(cur_x);
        edge.label_y = Some(cur_y);
    }
}

fn adjust_composite_boundary_edge_endpoints(
    edges: &mut [flowchart::PositionedEdge],
    subgraphs: &[flowchart::PositionedSubgraph],
    membership: &flowchart::graph_builder::SubgraphMembership,
    kind_map: &HashMap<String, StateKind>,
) {
    let subgraph_by_id: HashMap<&str, &flowchart::PositionedSubgraph> =
        subgraphs.iter().map(|sg| (sg.id.as_str(), sg)).collect();

    for edge in edges.iter_mut() {
        if edge.points.len() < 2 {
            continue;
        }

        // Case 1: external transition entering a composite's inner start pseudo-state.
        let is_target_start = kind_map.get(&edge.to_id).copied() == Some(StateKind::Start);
        if is_target_start {
            let Some(target_path) = membership.get(&edge.to_id) else {
                continue;
            };
            let Some(target_parent) = target_path.last() else {
                continue;
            };

            let source_same_parent = membership
                .get(&edge.from_id)
                .and_then(|p| p.last())
                .is_some_and(|p| p == target_parent);
            if source_same_parent {
                continue;
            }

            let Some(sg) = subgraph_by_id.get(target_parent.as_str()) else {
                continue;
            };
            let source_point = edge.points[0];
            let boundary_point = if source_point.1 <= sg.y {
                (source_point.0.clamp(sg.x, sg.x + sg.width), sg.y)
            } else {
                intersect_subgraph_boundary_toward(sg, source_point)
            };
            // Use a clean two-point segment to avoid top-hook artifacts.
            edge.points = vec![source_point, boundary_point];
            continue;
        }

        // Case 2: external transition leaving a composite toward an end pseudo-state.
        let is_target_end = kind_map.get(&edge.to_id).copied() == Some(StateKind::End);
        if is_target_end {
            let Some(source_parent) = membership.get(&edge.from_id).and_then(|p| p.last()) else {
                continue;
            };

            let target_same_parent = membership
                .get(&edge.to_id)
                .and_then(|p| p.last())
                .is_some_and(|p| p == source_parent);
            if target_same_parent {
                continue;
            }

            let Some(sg) = subgraph_by_id.get(source_parent.as_str()) else {
                continue;
            };
            let target_point = *edge.points.last().unwrap();
            let bottom = sg.y + sg.height;
            let boundary_point = if target_point.1 >= bottom {
                (target_point.0.clamp(sg.x, sg.x + sg.width), bottom)
            } else {
                intersect_subgraph_boundary_toward(sg, target_point)
            };
            edge.points = vec![boundary_point, target_point];
        }
    }
}

fn intersect_subgraph_boundary_toward(
    sg: &flowchart::PositionedSubgraph,
    toward: (f64, f64),
) -> (f64, f64) {
    let cx = sg.x + sg.width / 2.0;
    let cy = sg.y + sg.height / 2.0;
    let hw = sg.width / 2.0;
    let hh = sg.height / 2.0;
    let dx = toward.0 - cx;
    let dy = toward.1 - cy;

    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return (cx, sg.y);
    }

    let abs_dx = dx.abs();
    let abs_dy = dy.abs();
    if abs_dy * hw > abs_dx * hh {
        let y = if dy > 0.0 { cy + hh } else { cy - hh };
        let x = if abs_dy > 1e-9 {
            cx + (y - cy) * dx / dy
        } else {
            cx
        };
        (x, y)
    } else {
        let x = if dx > 0.0 { cx + hw } else { cx - hw };
        let y = if abs_dx > 1e-9 {
            cy + (x - cx) * dy / dx
        } else {
            cy
        };
        (x, y)
    }
}

/// Convert the flowchart layout result back to state diagram types.
fn convert_from_flowchart_result(
    positioned: flowchart::PositionedGraph,
    kind_map: &HashMap<String, StateKind>,
    note_ids: &[String],
) -> PositionedStateDiagram {
    let mut states = Vec::new();
    let mut notes = Vec::new();

    for node in &positioned.nodes {
        if note_ids.contains(&node.id) {
            notes.push(PositionedNote {
                id: node.id.clone(),
                text: node.label.clone(),
                x: node.x,
                y: node.y,
                width: node.width,
                height: node.height,
            });
        } else {
            let kind = kind_map.get(&node.id).copied().unwrap_or(StateKind::Normal);
            states.push(PositionedState {
                id: node.id.clone(),
                label: node.label.clone(),
                kind,
                style: node.style.clone(),
                x: node.x,
                y: node.y,
                width: node.width,
                height: node.height,
            });
        }
    }

    let transitions = positioned
        .edges
        .iter()
        .map(|e| PositionedTransition {
            from_id: e.from_id.clone(),
            to_id: e.to_id.clone(),
            line_style: e.line_style,
            arrow_end: e.arrow_end,
            label: e.label.clone(),
            label_x: e.label_x,
            label_y: e.label_y,
            label_width: e.label_width,
            label_height: e.label_height,
            points: e.points.clone(),
            raw_path_d: None,
        })
        .collect();

    let composites = positioned
        .subgraphs
        .iter()
        .map(|sg| PositionedComposite {
            id: sg.id.clone(),
            label: sg.label.clone(),
            x: sg.x,
            y: sg.y,
            width: sg.width,
            height: sg.height,
            style: sg.style.clone(),
        })
        .collect();

    PositionedStateDiagram {
        states,
        transitions,
        composites,
        notes,
        width: positioned.width,
        height: positioned.height,
        direction: positioned.direction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontProvider;
    use crate::layout::text_measure::TextMeasurer;
    use crate::parser::statediagram::parse_statediagram;

    fn make_measurer(provider: &FontProvider) -> TextMeasurer<'_> {
        let font = provider.font_ref().unwrap();
        TextMeasurer::new(font, 14.0)
    }

    #[test]
    fn transitions_to_composite_use_inner_start_end() {
        let source = r#"stateDiagram-v2
    [*] --> Active
    Active --> [*]
    state Active {
        [*] --> Idle
        Idle --> [*]
    }"#;
        let ast = parse_statediagram(source).unwrap();
        let mut note_ids = Vec::new();
        let fc = convert_to_flowchart_ast(&ast, &mut note_ids);
        let active = ast.composites.iter().find(|c| c.id == "Active").unwrap();
        let inner_start = active
            .states
            .iter()
            .find(|s| s.kind == StateKind::Start)
            .unwrap()
            .id
            .clone();
        let inner_end = active
            .states
            .iter()
            .find(|s| s.kind == StateKind::End)
            .unwrap()
            .id
            .clone();

        assert!(
            fc.edges
                .iter()
                .any(|e| e.from.starts_with("__start_") && e.to == inner_start),
            "expected external->composite transition to target inner start"
        );
        assert!(
            fc.edges
                .iter()
                .any(|e| e.from == inner_end && e.to.starts_with("__end_")),
            "expected composite->external transition to source inner end"
        );
        assert!(
            !fc.edges
                .iter()
                .any(|e| e.from == "Active" || e.to == "Active"),
            "composite id should not be used as flowchart edge endpoint when inner start/end exist"
        );
    }

    #[test]
    fn transition_from_composite_without_inner_end_falls_back_to_inner_state() {
        let source = r#"stateDiagram-v2
    Active --> [*]
    state Active {
        [*] --> Idle
        Idle --> Processing
        Processing --> Error
    }"#;
        let ast = parse_statediagram(source).unwrap();
        let mut note_ids = Vec::new();
        let fc = convert_to_flowchart_ast(&ast, &mut note_ids);

        // No explicit inner end exists, so edge should not keep composite id as source.
        let exit_edge = fc
            .edges
            .iter()
            .find(|e| e.to.starts_with("__end_"))
            .expect("expected top-level exit edge");
        assert_ne!(
            exit_edge.from, "Active",
            "composite exit should resolve to a concrete inner state"
        );
    }

    #[test]
    fn collect_kinds_maps_all_state_kinds() {
        let states = vec![
            StateDef {
                id: "s1".into(),
                label: None,
                kind: StateKind::Normal,
                class_shorthand: None,
            },
            StateDef {
                id: "s2".into(),
                label: None,
                kind: StateKind::Start,
                class_shorthand: None,
            },
            StateDef {
                id: "s3".into(),
                label: None,
                kind: StateKind::Fork,
                class_shorthand: None,
            },
        ];
        let mut map = HashMap::new();
        collect_kinds(&states, &[], &mut map);
        assert_eq!(map.len(), 3);
        assert_eq!(map["s1"], StateKind::Normal);
        assert_eq!(map["s2"], StateKind::Start);
        assert_eq!(map["s3"], StateKind::Fork);
    }

    #[test]
    fn collect_kinds_recurses_into_composites() {
        let composites = vec![CompositeStateDef {
            id: "comp".into(),
            label: None,
            direction: None,
            states: vec![StateDef {
                id: "inner".into(),
                label: None,
                kind: StateKind::Choice,
                class_shorthand: None,
            }],
            transitions: vec![],
            composites: vec![],
            notes: vec![],
            dividers: vec![],
        }];
        let mut map = HashMap::new();
        collect_kinds(&[], &composites, &mut map);
        assert_eq!(map["inner"], StateKind::Choice);
    }

    #[test]
    fn convert_state_to_node_maps_shapes_correctly() {
        let cases = vec![
            (StateKind::Normal, NodeShape::RoundedRectangle),
            (StateKind::Start, NodeShape::Circle),
            (StateKind::End, NodeShape::DoubleCircle),
            (StateKind::Fork, NodeShape::Rectangle),
            (StateKind::Join, NodeShape::Rectangle),
            (StateKind::Choice, NodeShape::Diamond),
        ];
        for (kind, expected_shape) in cases {
            let state = StateDef {
                id: "test".into(),
                label: Some("Test".into()),
                kind,
                class_shorthand: None,
            };
            let node = convert_state_to_node(&state);
            assert_eq!(
                node.shape, expected_shape,
                "StateKind::{:?} should map to {:?}",
                kind, expected_shape
            );
        }
    }

    #[test]
    fn convert_state_to_node_special_kinds_have_no_label() {
        for kind in [StateKind::Start, StateKind::End, StateKind::Choice] {
            let state = StateDef {
                id: "x".into(),
                label: Some("should be dropped".into()),
                kind,
                class_shorthand: None,
            };
            let node = convert_state_to_node(&state);
            assert!(
                node.label.is_none(),
                "StateKind::{:?} should have no label",
                kind
            );
        }
    }

    #[test]
    fn convert_state_to_node_fork_join_have_space_label() {
        for kind in [StateKind::Fork, StateKind::Join] {
            let state = StateDef {
                id: "fj".into(),
                label: Some("ignored".into()),
                kind,
                class_shorthand: None,
            };
            let node = convert_state_to_node(&state);
            assert_eq!(node.label, Some(" ".to_string()));
        }
    }

    #[test]
    fn convert_note_to_node_and_edge_without_target() {
        let note = NoteDef {
            id: Some("n1".into()),
            target_state: None,
            position: None,
            text: "A note".into(),
        };
        let mut note_ids = Vec::new();
        let (node, edge) = convert_note_to_node_and_edge(&note, &mut note_ids);
        assert_eq!(node.id, "n1");
        assert_eq!(node.label, Some("A note".into()));
        assert!(edge.is_none(), "no edge when note has no target");
        assert_eq!(note_ids, vec!["n1"]);
    }

    #[test]
    fn convert_note_to_node_and_edge_with_target() {
        let note = NoteDef {
            id: None,
            target_state: Some("stateA".into()),
            position: None,
            text: "Attached".into(),
        };
        let mut note_ids = Vec::new();
        let (node, edge) = convert_note_to_node_and_edge(&note, &mut note_ids);
        assert!(node.id.starts_with("__note_"));
        let edge = edge.expect("should have edge when target is set");
        assert_eq!(edge.from, "stateA");
        assert_eq!(edge.to, node.id);
        assert_eq!(edge.line_style, LineStyle::Dotted);
        assert_eq!(edge.arrow_start, ArrowEnd::None);
        assert_eq!(edge.arrow_end, ArrowEnd::None);
    }

    #[test]
    fn convert_to_flowchart_ast_basic_diagram() {
        let source = r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Running
    Running --> [*]"#;
        let ast = parse_statediagram(source).unwrap();
        let mut note_ids = Vec::new();
        let fc = convert_to_flowchart_ast(&ast, &mut note_ids);

        assert!(!fc.nodes.is_empty(), "should have nodes");
        assert!(!fc.edges.is_empty(), "should have edges");
        assert!(note_ids.is_empty(), "no notes in this diagram");
    }

    #[test]
    fn convert_to_flowchart_ast_preserves_styling() {
        let ast = StateDiagramAst {
            direction: Direction::LeftToRight,
            states: vec![StateDef {
                id: "s1".into(),
                label: Some("S1".into()),
                kind: StateKind::Normal,
                class_shorthand: None,
            }],
            transitions: vec![],
            composites: vec![],
            notes: vec![],
            class_defs: vec![crate::ast::statediagram::ClassDef {
                name: "myClass".into(),
                properties: crate::ast::common::StyleProperties {
                    fill: Some(crate::ast::common::Color::Hex("#f00".into())),
                    ..Default::default()
                },
            }],
            class_assignments: vec![crate::ast::statediagram::ClassAssignment {
                node_ids: vec!["s1".into()],
                class_name: "myClass".into(),
            }],
            style_overrides: vec![crate::ast::statediagram::StyleOverride {
                node_id: "s1".into(),
                properties: crate::ast::common::StyleProperties {
                    stroke: Some(crate::ast::common::Color::Hex("#00f".into())),
                    ..Default::default()
                },
            }],
        };
        let mut note_ids = Vec::new();
        let fc = convert_to_flowchart_ast(&ast, &mut note_ids);
        assert_eq!(fc.class_defs.len(), 1);
        assert_eq!(fc.class_defs[0].name, "myClass");
        assert_eq!(fc.class_assignments.len(), 1);
        assert_eq!(fc.style_overrides.len(), 1);
        assert_eq!(fc.direction, Direction::LeftToRight);
    }

    #[test]
    fn convert_composite_to_subgraph_includes_dividers() {
        let composite = CompositeStateDef {
            id: "comp".into(),
            label: Some("Composite".into()),
            direction: None,
            states: vec![],
            transitions: vec![],
            composites: vec![],
            notes: vec![],
            dividers: vec![
                DividerDef { id: "div1".into() },
                DividerDef { id: "div2".into() },
            ],
        };
        let mut note_ids = Vec::new();
        let sg = convert_composite_to_subgraph(&composite, &mut note_ids);
        assert_eq!(sg.id, "comp");
        assert_eq!(sg.label, Some("Composite".into()));
        // Dividers become nodes inside the subgraph
        assert_eq!(sg.nodes.len(), 2);
        assert!(sg.nodes.iter().any(|n| n.id == "div1"));
        assert!(sg.nodes.iter().any(|n| n.id == "div2"));
    }

    #[test]
    fn convert_composite_to_subgraph_defaults_label_to_id() {
        let composite = CompositeStateDef {
            id: "NoLabel".into(),
            label: None,
            direction: None,
            states: vec![],
            transitions: vec![],
            composites: vec![],
            notes: vec![],
            dividers: vec![],
        };
        let mut note_ids = Vec::new();
        let sg = convert_composite_to_subgraph(&composite, &mut note_ids);
        assert_eq!(sg.label, Some("NoLabel".into()));
    }

    #[test]
    fn resolve_composite_transition_endpoint_no_match_returns_original() {
        let result = resolve_composite_transition_endpoint("nonexistent", true, &[]);
        assert_eq!(result, "nonexistent");
    }

    #[test]
    fn resolve_composite_transition_endpoint_finds_start_for_incoming() {
        let composites = vec![CompositeStateDef {
            id: "Active".into(),
            label: None,
            direction: None,
            states: vec![
                StateDef {
                    id: "inner_start".into(),
                    label: None,
                    kind: StateKind::Start,
                    class_shorthand: None,
                },
                StateDef {
                    id: "inner_state".into(),
                    label: None,
                    kind: StateKind::Normal,
                    class_shorthand: None,
                },
            ],
            transitions: vec![],
            composites: vec![],
            notes: vec![],
            dividers: vec![],
        }];
        // is_source=false means incoming to composite, should find Start
        let result = resolve_composite_transition_endpoint("Active", false, &composites);
        assert_eq!(result, "inner_start");
    }

    #[test]
    fn resolve_composite_transition_endpoint_finds_end_for_outgoing() {
        let composites = vec![CompositeStateDef {
            id: "Active".into(),
            label: None,
            direction: None,
            states: vec![
                StateDef {
                    id: "inner_end".into(),
                    label: None,
                    kind: StateKind::End,
                    class_shorthand: None,
                },
                StateDef {
                    id: "inner_state".into(),
                    label: None,
                    kind: StateKind::Normal,
                    class_shorthand: None,
                },
            ],
            transitions: vec![],
            composites: vec![],
            notes: vec![],
            dividers: vec![],
        }];
        // is_source=true means outgoing from composite, should find End
        let result = resolve_composite_transition_endpoint("Active", true, &composites);
        assert_eq!(result, "inner_end");
    }

    #[test]
    fn resolve_composite_transition_endpoint_fallback_to_normal_state() {
        let composites = vec![CompositeStateDef {
            id: "Active".into(),
            label: None,
            direction: None,
            states: vec![
                StateDef {
                    id: "first".into(),
                    label: None,
                    kind: StateKind::Normal,
                    class_shorthand: None,
                },
                StateDef {
                    id: "last".into(),
                    label: None,
                    kind: StateKind::Normal,
                    class_shorthand: None,
                },
            ],
            transitions: vec![],
            composites: vec![],
            notes: vec![],
            dividers: vec![],
        }];
        // is_source=false (incoming) => first normal state
        assert_eq!(
            resolve_composite_transition_endpoint("Active", false, &composites),
            "first"
        );
        // is_source=true (outgoing) => last normal state
        assert_eq!(
            resolve_composite_transition_endpoint("Active", true, &composites),
            "last"
        );
    }

    #[test]
    fn make_cubic_bezier_bow_produces_valid_path() {
        let path = make_cubic_bezier_bow(0.0, 0.0, 100.0, 0.0, 0.0, 1.0, 30.0);
        assert!(path.starts_with("M "), "path should start with M command");
        assert!(path.contains("C "), "path should contain C command");
        // Parse out the endpoint to verify
        let parts: Vec<&str> = path.split_whitespace().collect();
        // M sx sy C cx1 cy1 cx2 cy2 ex ey
        assert_eq!(parts.len(), 10);
        assert_eq!(parts[0], "M");
        assert_eq!(parts[3], "C");
    }

    #[test]
    fn make_cubic_bezier_bow_control_points_offset_perpendicular() {
        // Horizontal edge from (0,0) to (100,0), perpendicular is (0,1), bow=30
        let path = make_cubic_bezier_bow(0.0, 0.0, 100.0, 0.0, 0.0, 1.0, 30.0);
        let parts: Vec<f64> = path
            .replace("M", "")
            .replace("C", "")
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        // parts: [sx, sy, cx1, cy1, cx2, cy2, ex, ey]
        let cy1 = parts[3];
        let cy2 = parts[5];
        // Control points should be offset by bow (30.0) in the y direction
        assert!(cy1 > 0.0, "cy1 should be positive (bowed downward)");
        assert!(cy2 > 0.0, "cy2 should be positive (bowed downward)");
    }

    #[test]
    fn separate_bidirectional_edges_bows_opposing_directions() {
        let mut transitions = vec![
            PositionedTransition {
                from_id: "A".into(),
                to_id: "B".into(),
                line_style: LineStyle::Solid,
                arrow_end: ArrowEnd::Arrow,
                label: None,
                label_x: None,
                label_y: None,
                label_width: None,
                label_height: None,
                points: vec![(0.0, 0.0), (100.0, 0.0)],
                raw_path_d: None,
            },
            PositionedTransition {
                from_id: "B".into(),
                to_id: "A".into(),
                line_style: LineStyle::Solid,
                arrow_end: ArrowEnd::Arrow,
                label: None,
                label_x: None,
                label_y: None,
                label_width: None,
                label_height: None,
                points: vec![(100.0, 0.0), (0.0, 0.0)],
                raw_path_d: None,
            },
        ];

        separate_bidirectional_edges(&mut transitions);

        assert!(
            transitions[0].raw_path_d.is_some(),
            "A→B should get a raw path"
        );
        assert!(
            transitions[1].raw_path_d.is_some(),
            "B→A should get a raw path"
        );

        // The two edges should bow in opposite directions
        // Check that the apex points are on opposite sides
        let apex_0 = transitions[0].points[1];
        let apex_1 = transitions[1].points[1];
        // For a horizontal edge, they should have opposite y values
        assert!(
            (apex_0.1 > 0.0 && apex_1.1 < 0.0) || (apex_0.1 < 0.0 && apex_1.1 > 0.0),
            "bidirectional edges should bow to opposite sides: apex_0.y={}, apex_1.y={}",
            apex_0.1,
            apex_1.1
        );
    }

    #[test]
    fn separate_bidirectional_edges_no_op_for_unidirectional() {
        let mut transitions = vec![PositionedTransition {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_end: ArrowEnd::Arrow,
            label: None,
            label_x: None,
            label_y: None,
            label_width: None,
            label_height: None,
            points: vec![(0.0, 0.0), (100.0, 0.0)],
            raw_path_d: None,
        }];

        separate_bidirectional_edges(&mut transitions);

        assert!(
            transitions[0].raw_path_d.is_none(),
            "unidirectional edge should not get a raw path"
        );
    }

    #[test]
    fn expand_bounds_for_transitions_grows_for_out_of_bounds_points() {
        let mut diagram = PositionedStateDiagram {
            states: vec![],
            transitions: vec![PositionedTransition {
                from_id: "A".into(),
                to_id: "B".into(),
                line_style: LineStyle::Solid,
                arrow_end: ArrowEnd::Arrow,
                label: None,
                label_x: None,
                label_y: None,
                label_width: None,
                label_height: None,
                points: vec![(0.0, 0.0), (200.0, 150.0)],
                raw_path_d: None,
            }],
            composites: vec![],
            notes: vec![],
            width: 100.0,
            height: 80.0,
            direction: Direction::TopToBottom,
        };

        expand_bounds_for_transitions(&mut diagram);

        assert!(
            diagram.width >= 200.0,
            "width should expand to cover point at x=200"
        );
        assert!(
            diagram.height >= 150.0,
            "height should expand to cover point at y=150"
        );
    }

    #[test]
    fn expand_bounds_shifts_negative_coords() {
        let mut diagram = PositionedStateDiagram {
            states: vec![PositionedState {
                id: "s1".into(),
                label: "S1".into(),
                kind: StateKind::Normal,
                style: Default::default(),
                x: 50.0,
                y: 50.0,
                width: 40.0,
                height: 20.0,
            }],
            transitions: vec![PositionedTransition {
                from_id: "A".into(),
                to_id: "B".into(),
                line_style: LineStyle::Solid,
                arrow_end: ArrowEnd::Arrow,
                label: None,
                label_x: None,
                label_y: None,
                label_width: None,
                label_height: None,
                points: vec![(-20.0, -10.0), (50.0, 50.0)],
                raw_path_d: None,
            }],
            composites: vec![],
            notes: vec![],
            width: 100.0,
            height: 80.0,
            direction: Direction::TopToBottom,
        };

        expand_bounds_for_transitions(&mut diagram);

        // After shifting, the state should have moved by (20, 10)
        assert!(
            diagram.states[0].x > 50.0,
            "state x should be shifted positive"
        );
        assert!(
            diagram.states[0].y > 50.0,
            "state y should be shifted positive"
        );
        // All transition points should be non-negative
        for &(px, py) in &diagram.transitions[0].points {
            assert!(px >= 0.0, "shifted point x should be non-negative");
            assert!(py >= 0.0, "shifted point y should be non-negative");
        }
    }

    #[test]
    fn intersect_subgraph_boundary_toward_from_above() {
        let sg = flowchart::PositionedSubgraph {
            id: "sg".into(),
            label: None,
            x: 50.0,
            y: 50.0,
            width: 100.0,
            height: 80.0,
            style: Default::default(),
        };
        // Point above the subgraph center
        let result = intersect_subgraph_boundary_toward(&sg, (100.0, 0.0));
        // Should intersect at the top boundary (y = 50.0)
        assert!(
            (result.1 - 50.0).abs() < 1.0,
            "should intersect near top boundary, got y={}",
            result.1
        );
    }

    #[test]
    fn intersect_subgraph_boundary_toward_from_right() {
        let sg = flowchart::PositionedSubgraph {
            id: "sg".into(),
            label: None,
            x: 50.0,
            y: 50.0,
            width: 100.0,
            height: 80.0,
            style: Default::default(),
        };
        // Point to the right of the subgraph center
        let result = intersect_subgraph_boundary_toward(&sg, (300.0, 90.0));
        // Should intersect at right boundary (x = 50 + 100/2 = 100 + 50 = 150)
        assert!(
            (result.0 - 150.0).abs() < 1.0,
            "should intersect near right boundary, got x={}",
            result.0
        );
    }

    #[test]
    fn convert_from_flowchart_result_separates_notes() {
        let positioned = flowchart::PositionedGraph {
            nodes: vec![
                flowchart::PositionedNode {
                    id: "s1".into(),
                    label: "State 1".into(),
                    x: 50.0,
                    y: 50.0,
                    width: 80.0,
                    height: 40.0,
                    shape: NodeShape::RoundedRectangle,
                    style: Default::default(),
                },
                flowchart::PositionedNode {
                    id: "__note_0".into(),
                    label: "My note".into(),
                    x: 200.0,
                    y: 50.0,
                    width: 100.0,
                    height: 40.0,
                    shape: NodeShape::Rectangle,
                    style: Default::default(),
                },
            ],
            edges: vec![],
            subgraphs: vec![],
            width: 300.0,
            height: 100.0,
            direction: Direction::TopToBottom,
        };
        let kind_map = HashMap::from([("s1".to_string(), StateKind::Normal)]);
        let note_ids = vec!["__note_0".to_string()];

        let result = convert_from_flowchart_result(positioned, &kind_map, &note_ids);

        assert_eq!(result.states.len(), 1);
        assert_eq!(result.states[0].id, "s1");
        assert_eq!(result.notes.len(), 1);
        assert_eq!(result.notes[0].id, "__note_0");
        assert_eq!(result.notes[0].text, "My note");
    }

    #[test]
    fn adjust_labels_for_nodes_pushes_overlapping_labels() {
        use crate::layout::flowchart::{PositionedEdge, PositionedNode};

        let nodes = vec![PositionedNode {
            id: "blocker".into(),
            label: "Blocker".into(),
            x: 100.0,
            y: 100.0,
            width: 60.0,
            height: 40.0,
            shape: NodeShape::RoundedRectangle,
            style: Default::default(),
        }];

        let mut edges = vec![PositionedEdge {
            from_id: "A".into(),
            to_id: "B".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(100.0), // directly on top of the node
            label_y: Some(100.0),
            label_width: Some(30.0),
            label_height: Some(16.0),
            points: vec![(0.0, 0.0), (200.0, 200.0)],
        }];

        adjust_labels_for_nodes(&mut edges, &nodes);

        let new_x = edges[0].label_x.unwrap();
        let new_y = edges[0].label_y.unwrap();
        // Label should have been pushed away from the node center
        let moved = (new_x - 100.0).abs() > 1.0 || (new_y - 100.0).abs() > 1.0;
        assert!(
            moved,
            "label should be pushed away from overlapping node, got ({}, {})",
            new_x, new_y
        );
    }

    #[test]
    fn adjust_labels_for_nodes_skips_edge_endpoints() {
        use crate::layout::flowchart::{PositionedEdge, PositionedNode};

        let nodes = vec![PositionedNode {
            id: "source".into(),
            label: "Source".into(),
            x: 100.0,
            y: 100.0,
            width: 60.0,
            height: 40.0,
            shape: NodeShape::RoundedRectangle,
            style: Default::default(),
        }];

        let mut edges = vec![PositionedEdge {
            from_id: "source".into(), // edge starts from "source" node
            to_id: "target".into(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: Some("test".into()),
            label_x: Some(100.0),
            label_y: Some(100.0),
            label_width: Some(30.0),
            label_height: Some(16.0),
            points: vec![(100.0, 100.0), (200.0, 200.0)],
        }];

        adjust_labels_for_nodes(&mut edges, &nodes);

        // Label should NOT be pushed away because the overlapping node is the edge's own endpoint
        assert_eq!(edges[0].label_x, Some(100.0));
        assert_eq!(edges[0].label_y, Some(100.0));
    }

    #[test]
    fn layout_statediagram_basic_produces_valid_result() {
        let source = r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Running
    Running --> [*]"#;
        let ast = parse_statediagram(source).unwrap();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_statediagram(&ast, &measurer).unwrap();

        assert!(!result.states.is_empty(), "should have states");
        assert!(!result.transitions.is_empty(), "should have transitions");
        assert!(result.width > 0.0, "width should be positive");
        assert!(result.height > 0.0, "height should be positive");
    }

    #[test]
    fn layout_statediagram_with_composites() {
        let source = r#"stateDiagram-v2
    [*] --> Active
    state Active {
        [*] --> Idle
        Idle --> Processing
        Processing --> [*]
    }
    Active --> [*]"#;
        let ast = parse_statediagram(source).unwrap();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_statediagram(&ast, &measurer).unwrap();

        assert!(!result.composites.is_empty(), "should have composites");
        // Composite should have non-zero dimensions
        let comp = &result.composites[0];
        assert!(comp.width > 0.0);
        assert!(comp.height > 0.0);
    }

    #[test]
    fn layout_statediagram_with_notes() {
        let source = r#"stateDiagram-v2
    [*] --> Active
    note right of Active: This is a note"#;
        let ast = parse_statediagram(source).unwrap();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_statediagram(&ast, &measurer).unwrap();

        assert!(!result.notes.is_empty(), "should have notes");
        assert!(!result.notes[0].text.is_empty());
    }

    #[test]
    fn layout_statediagram_with_fork_join() {
        let source = r#"stateDiagram-v2
    [*] --> fork1
    state fork1 <<fork>>
    fork1 --> A
    fork1 --> B
    A --> join1
    B --> join1
    state join1 <<join>>
    join1 --> [*]"#;
        let ast = parse_statediagram(source).unwrap();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_statediagram(&ast, &measurer).unwrap();

        // Fork and join states should have small dimensions
        let fork_state = result.states.iter().find(|s| s.id == "fork1");
        let join_state = result.states.iter().find(|s| s.id == "join1");
        assert!(fork_state.is_some(), "should find fork state");
        assert!(join_state.is_some(), "should find join state");
    }

    #[test]
    fn layout_statediagram_with_choice() {
        let source = r#"stateDiagram-v2
    [*] --> IsPositive
    state IsPositive <<choice>>
    IsPositive --> Yes: positive
    IsPositive --> No: negative"#;
        let ast = parse_statediagram(source).unwrap();
        let provider = FontProvider::default();
        let measurer = make_measurer(&provider);
        let result = layout_statediagram(&ast, &measurer).unwrap();

        let choice = result
            .states
            .iter()
            .find(|s| s.id == "IsPositive")
            .expect("should find choice state");
        assert_eq!(choice.kind, StateKind::Choice);
    }
}
