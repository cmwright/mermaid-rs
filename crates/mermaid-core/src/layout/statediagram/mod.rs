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
        edges.push(EdgeDef {
            from: t.from.clone(),
            to: t.to.clone(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: t.label.clone(),
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
        edges.push(EdgeDef {
            from: t.from.clone(),
            to: t.to.clone(),
            line_style: LineStyle::Solid,
            arrow_start: ArrowEnd::None,
            arrow_end: ArrowEnd::Arrow,
            label: t.label.clone(),
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
        label: composite.label.clone(),
        direction: composite.direction,
        nodes,
        edges,
        subgraphs,
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
