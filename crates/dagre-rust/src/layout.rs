//! Main layout pipeline.
//! Port of dagre's `layout.js`.

use crate::acyclic;
use crate::add_border_segments;
use crate::coordinate_system;
use crate::graph::{Edge, GraphOptions, LayoutGraph};
use crate::nesting_graph;
use crate::normalize;
use crate::order;
use crate::parent_dummy_chains;
use crate::position;
use crate::rank;
use crate::types::*;
use crate::util;
use std::time::Instant;

/// Layout options, mirroring the second argument to JS `dagre.layout(g, opts)`.
#[derive(Debug, Clone, Default)]
pub struct LayoutOpts {
    pub disable_optimal_order_heuristic: bool,
}

#[derive(Debug, Clone)]
pub struct LayoutStageTiming {
    pub stage: &'static str,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Default)]
pub struct LayoutProfile {
    pub stages: Vec<LayoutStageTiming>,
    pub total_ms: f64,
}

/// Performs the full dagre layout on the input graph.
pub fn layout(g: &mut LayoutGraph) {
    layout_with_opts(g, &LayoutOpts::default());
}

/// Performs the full dagre layout with options.
pub fn layout_with_opts(g: &mut LayoutGraph, opts: &LayoutOpts) {
    let mut layout_graph = build_layout_graph(g);
    run_layout(&mut layout_graph, opts);
    update_input_graph(g, &layout_graph);
}

/// Runs full layout and returns per-stage timings.
pub fn layout_profiled(g: &mut LayoutGraph, opts: &LayoutOpts) -> LayoutProfile {
    let mut layout_graph = build_layout_graph(g);
    let profile = run_layout_profiled(&mut layout_graph, opts);
    update_input_graph(g, &layout_graph);
    profile
}

fn run_layout(g: &mut LayoutGraph, opts: &LayoutOpts) {
    make_space_for_edge_labels(g);
    remove_self_edges(g);
    acyclic::run(g);
    nesting_graph::run(g);

    // Rank using a non-compound copy
    {
        let mut ncg = util::as_non_compound_graph(g);
        rank::rank(&mut ncg);
        for v in ncg.node_ids() {
            if let Some(rank) = ncg.node(v).and_then(|n| n.rank)
                && let Some(node) = g.node_mut(v)
            {
                node.rank = Some(rank);
            }
        }
    }

    inject_edge_label_proxies(g);
    util::remove_empty_ranks(g);
    nesting_graph::cleanup(g);
    util::normalize_ranks(g);
    assign_rank_min_max(g);
    remove_edge_label_proxies(g);
    normalize::run(g);
    parent_dummy_chains::parent_dummy_chains(g);
    add_border_segments::add_border_segments(g);
    order::order(g, opts.disable_optimal_order_heuristic);
    insert_self_edges(g);
    coordinate_system::adjust(g);
    position::position(g);
    position_self_edges(g);
    remove_border_nodes(g);
    normalize::undo(g);
    fixup_edge_label_coords(g);
    coordinate_system::undo(g);
    translate_graph(g);
    assign_node_intersects(g);
    reverse_points_for_reversed_edges(g);
    acyclic::undo(g);
}

fn run_layout_profiled(g: &mut LayoutGraph, opts: &LayoutOpts) -> LayoutProfile {
    let mut stages = Vec::new();
    let t0 = Instant::now();

    let mut record = |name: &'static str, start: Instant| {
        stages.push(LayoutStageTiming {
            stage: name,
            duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        });
    };

    let s = Instant::now();
    make_space_for_edge_labels(g);
    record("make_space_for_edge_labels", s);

    let s = Instant::now();
    remove_self_edges(g);
    record("remove_self_edges", s);

    let s = Instant::now();
    acyclic::run(g);
    record("acyclic_run", s);

    let s = Instant::now();
    nesting_graph::run(g);
    record("nesting_graph_run", s);

    let s = Instant::now();
    {
        let mut ncg = util::as_non_compound_graph(g);
        rank::rank(&mut ncg);
        for v in ncg.node_ids() {
            if let Some(rank) = ncg.node(v).and_then(|n| n.rank)
                && let Some(node) = g.node_mut(v)
            {
                node.rank = Some(rank);
            }
        }
    }
    record("rank_assign", s);

    let s = Instant::now();
    inject_edge_label_proxies(g);
    record("inject_edge_label_proxies", s);

    let s = Instant::now();
    util::remove_empty_ranks(g);
    record("remove_empty_ranks", s);

    let s = Instant::now();
    nesting_graph::cleanup(g);
    record("nesting_graph_cleanup", s);

    let s = Instant::now();
    util::normalize_ranks(g);
    record("normalize_ranks", s);

    let s = Instant::now();
    assign_rank_min_max(g);
    record("assign_rank_min_max", s);

    let s = Instant::now();
    remove_edge_label_proxies(g);
    record("remove_edge_label_proxies", s);

    let s = Instant::now();
    normalize::run(g);
    record("normalize_run", s);

    let s = Instant::now();
    parent_dummy_chains::parent_dummy_chains(g);
    record("parent_dummy_chains", s);

    let s = Instant::now();
    add_border_segments::add_border_segments(g);
    record("add_border_segments", s);

    let s = Instant::now();
    order::order(g, opts.disable_optimal_order_heuristic);
    record("order", s);

    let s = Instant::now();
    insert_self_edges(g);
    record("insert_self_edges", s);

    let s = Instant::now();
    coordinate_system::adjust(g);
    record("coordinate_system_adjust", s);

    let s = Instant::now();
    position::position(g);
    record("position", s);

    let s = Instant::now();
    position_self_edges(g);
    record("position_self_edges", s);

    let s = Instant::now();
    remove_border_nodes(g);
    record("remove_border_nodes", s);

    let s = Instant::now();
    normalize::undo(g);
    record("normalize_undo", s);

    let s = Instant::now();
    fixup_edge_label_coords(g);
    record("fixup_edge_label_coords", s);

    let s = Instant::now();
    coordinate_system::undo(g);
    record("coordinate_system_undo", s);

    let s = Instant::now();
    translate_graph(g);
    record("translate_graph", s);

    let s = Instant::now();
    assign_node_intersects(g);
    record("assign_node_intersects", s);

    let s = Instant::now();
    reverse_points_for_reversed_edges(g);
    record("reverse_points_for_reversed_edges", s);

    let s = Instant::now();
    acyclic::undo(g);
    record("acyclic_undo", s);

    LayoutProfile {
        stages,
        total_ms: t0.elapsed().as_secs_f64() * 1000.0,
    }
}

fn update_input_graph(input_graph: &mut LayoutGraph, layout_graph: &LayoutGraph) {
    for v in input_graph.node_ids().to_vec() {
        if let (Some(_), Some(ll)) = (input_graph.node(&v), layout_graph.node(&v)) {
            let x = ll.x;
            let y = ll.y;
            let order = ll.order;
            let rank = ll.rank;

            let has_children = layout_graph
                .children(Some(&v))
                .map(|c| !c.is_empty())
                .unwrap_or(false);
            let width = if has_children { Some(ll.width) } else { None };
            let height = if has_children { Some(ll.height) } else { None };

            if let Some(il) = input_graph.node_mut(&v) {
                if let Some(xv) = x {
                    il.x = Some(xv);
                }
                if let Some(yv) = y {
                    il.y = Some(yv);
                }
                if let Some(o) = order {
                    il.order = Some(o);
                }
                if let Some(r) = rank {
                    il.rank = Some(r);
                }
                if let Some(w) = width {
                    il.width = w;
                }
                if let Some(h) = height {
                    il.height = h;
                }
            }
        }
    }

    let edge_ids: Vec<String> = input_graph.edge_ids().to_vec();
    for eid in &edge_ids {
        let eobj = match input_graph.edge_obj_by_id(eid) {
            Some(e) => e.clone(),
            None => continue,
        };
        let Some(ll) = layout_graph.edge_by_obj(&eobj) else {
            continue;
        };
        let points = ll.points.clone();
        let x = ll.x;
        let y = ll.y;
        if let Some(il) = input_graph.edge_label_mut_by_id(eid) {
            il.points = points;
            if let Some(xv) = x {
                il.x = Some(xv);
                if let Some(yv) = y {
                    il.y = Some(yv);
                }
            }
        }
    }

    let lg = layout_graph.graph();
    input_graph.graph_mut().width = lg.width;
    input_graph.graph_mut().height = lg.height;
}

// === Build layout graph ===

fn build_layout_graph(input_graph: &LayoutGraph) -> LayoutGraph {
    let mut g = LayoutGraph::with_options(&GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });

    let ig = input_graph.graph();
    let graph_label = GraphLabel {
        nodesep: ig.nodesep,
        edgesep: ig.edgesep,
        ranksep: ig.ranksep,
        marginx: ig.marginx,
        marginy: ig.marginy,
        rankdir: ig.rankdir,
        rankdir_explicit: ig.rankdir_explicit,
        rankalign: ig.rankalign,
        acyclicer: ig.acyclicer,
        ranker: ig.ranker,
        align: ig.align,
        ..Default::default()
    };
    g.set_graph(graph_label);

    // Copy nodes
    for v in input_graph.node_ids() {
        if let Some(n) = input_graph.node(v) {
            let new_node = NodeLabel {
                width: n.width,
                height: n.height,
                rank: n.rank,
                ..Default::default()
            };
            g.set_node(v, Some(new_node));
        }

        if let Some(parent) = input_graph.parent(v) {
            g.set_parent(v, Some(parent));
        }
    }

    // Ensure all nodes (including implicitly-created subgraph parents) have labels
    // in the layout graph. The input graph preserves None labels for JS parity,
    // but the layout pipeline needs every node to have a NodeLabel.
    let missing_labels: Vec<String> = g
        .node_ids()
        .iter()
        .filter(|v| g.node(v).is_none())
        .cloned()
        .collect();
    for v in missing_labels {
        g.set_node(&v, Some(NodeLabel::default()));
    }

    // Copy edges
    for eid in input_graph.edge_ids() {
        let Some(eobj) = input_graph.edge_obj_by_id(eid) else {
            continue;
        };
        if let Some(el) = input_graph.edge_label_by_id(eid) {
            let new_edge = EdgeLabel {
                minlen: el.minlen,
                weight: el.weight,
                width: el.width,
                height: el.height,
                labeloffset: el.labeloffset,
                labelpos: el.labelpos,
                ..Default::default()
            };
            g.set_edge_with_obj(eobj, Some(new_edge));
        }
    }

    g
}

fn make_space_for_edge_labels(g: &mut LayoutGraph) {
    let ranksep = g.graph().ranksep;
    g.graph_mut().ranksep = ranksep / 2.0;

    let rankdir = g.graph().rankdir;
    let rankdir_explicit = g.graph().rankdir_explicit;

    let edge_ids: Vec<String> = g.edge_ids().to_vec();
    for eid in &edge_ids {
        if let Some(edge) = g.edge_label_mut_by_id(eid) {
            edge.minlen *= 2.0;

            if edge.labelpos != LabelPos::Center {
                let labeloffset = edge.labeloffset;
                // NB: JS dagre has a case-sensitivity bug here. The default
                // rankdir is lowercase "tb" but the comparison uses uppercase
                // "TB"/"BT", so for default TB the condition never matches and
                // labeloffset is always added to height. When the user
                // explicitly sets rankdir to "TB" or "BT", the uppercase value
                // is preserved and the condition matches, adding to width. We
                // replicate that behaviour for pixel parity.
                let add_to_width =
                    rankdir_explicit && (rankdir == RankDir::TB || rankdir == RankDir::BT);
                if add_to_width {
                    edge.width += labeloffset;
                } else {
                    edge.height += labeloffset;
                }
            }
        }
    }
}

fn inject_edge_label_proxies(g: &mut LayoutGraph) {
    let edge_ids: Vec<String> = g.edge_ids().to_vec();
    for eid in &edge_ids {
        let Some(eobj) = g.edge_obj_by_id(eid) else {
            continue;
        };
        let (width, height, v_rank, w_rank) = {
            let edge = g.edge_label_by_id(eid).unwrap();
            let w = edge.width;
            let h = edge.height;
            let vr = g.node(&eobj.v).and_then(|n| n.rank).unwrap_or(0) as f64;
            let wr = g.node(&eobj.w).and_then(|n| n.rank).unwrap_or(0) as f64;
            (w, h, vr, wr)
        };
        if width != 0.0 && height != 0.0 {
            let eobj = g.edge_obj_by_id(eid).unwrap();
            let label = NodeLabel {
                rank: Some(((w_rank - v_rank) / 2.0 + v_rank) as i64),
                e: Some(Edge::new(&eobj.v, &eobj.w, eobj.name.as_deref())),
                ..Default::default()
            };
            util::add_dummy_node(g, DummyType::EdgeProxy, label, "_ep");
        }
    }
}

fn assign_rank_min_max(g: &mut LayoutGraph) {
    let mut max_rank: i64 = 0;
    let node_ids: Vec<String> = g.node_ids().to_vec();

    let mut updates: Vec<(String, i64, i64)> = Vec::new();
    for v in &node_ids {
        // Extract only the fields we need rather than cloning the entire NodeLabel
        let (bt, bb) = match g.node(v) {
            Some(node) => (node.border_top.clone(), node.border_bottom.clone()),
            None => continue,
        };
        if let Some(ref bt) = bt {
            let min_r = g.node(bt).and_then(|n| n.rank).unwrap_or(0);
            let bb = bb.as_deref().unwrap_or("");
            let max_r = g.node(bb).and_then(|n| n.rank).unwrap_or(0);
            updates.push((v.clone(), min_r, max_r));
            if max_r > max_rank {
                max_rank = max_r;
            }
        }
    }

    for (v, min_r, max_r) in updates {
        if let Some(node) = g.node_mut(&v) {
            node.min_rank = Some(min_r);
            node.max_rank = Some(max_r);
        }
    }

    g.graph_mut().max_rank = Some(max_rank);
}

fn remove_edge_label_proxies(g: &mut LayoutGraph) {
    let node_ids: Vec<String> = g.node_ids().to_vec();
    let mut to_remove: Vec<(String, Option<i64>, Option<Edge>)> = Vec::new();

    for v in &node_ids {
        let is_proxy = g
            .node(v)
            .map(|n| n.dummy == Some(DummyType::EdgeProxy))
            .unwrap_or(false);
        if is_proxy {
            let node = g.node(v).unwrap();
            to_remove.push((v.clone(), node.rank, node.e.clone()));
        }
    }

    for (v, rank, e_opt) in to_remove {
        let (e_v, e_w, e_name) = match &e_opt {
            Some(e) => (e.v.clone(), e.w.clone(), e.name.clone()),
            None => continue,
        };

        if let Some(edge) = g.edge_mut(&e_v, &e_w, e_name.as_deref()) {
            edge.label_rank = rank;
        }
        g.remove_node(&v);
    }
}

fn translate_graph(g: &mut LayoutGraph) {
    let mut min_x = f64::INFINITY;
    let mut max_x = 0.0f64;
    let mut min_y = f64::INFINITY;
    let mut max_y = 0.0f64;

    let margin_x = g.graph().marginx;
    let margin_y = g.graph().marginy;

    fn get_extremes_node(
        n: &NodeLabel,
        min_x: &mut f64,
        max_x: &mut f64,
        min_y: &mut f64,
        max_y: &mut f64,
    ) {
        let x = n.x.unwrap_or(0.0);
        let y = n.y.unwrap_or(0.0);
        let w = n.width;
        let h = n.height;
        *min_x = min_x.min(x - w / 2.0);
        *max_x = max_x.max(x + w / 2.0);
        *min_y = min_y.min(y - h / 2.0);
        *max_y = max_y.max(y + h / 2.0);
    }

    fn get_extremes_edge(
        e: &EdgeLabel,
        min_x: &mut f64,
        max_x: &mut f64,
        min_y: &mut f64,
        max_y: &mut f64,
    ) {
        let x = e.x.unwrap_or(0.0);
        let y = e.y.unwrap_or(0.0);
        let w = e.width;
        let h = e.height;
        *min_x = min_x.min(x - w / 2.0);
        *max_x = max_x.max(x + w / 2.0);
        *min_y = min_y.min(y - h / 2.0);
        *max_y = max_y.max(y + h / 2.0);
    }

    for v in g.node_ids() {
        if let Some(node) = g.node(v) {
            get_extremes_node(node, &mut min_x, &mut max_x, &mut min_y, &mut max_y);
        }
    }
    for eid in g.edge_ids() {
        if let Some(edge) = g.edge_label_by_id(eid)
            && edge.x.is_some()
        {
            get_extremes_edge(edge, &mut min_x, &mut max_x, &mut min_y, &mut max_y);
        }
    }

    min_x -= margin_x;
    min_y -= margin_y;

    let node_ids: Vec<String> = g.node_ids().to_vec();
    for v in &node_ids {
        if let Some(node) = g.node_mut(v) {
            if let Some(x) = node.x {
                node.x = Some(x - min_x);
            }
            if let Some(y) = node.y {
                node.y = Some(y - min_y);
            }
        }
    }

    let edge_ids: Vec<String> = g.edge_ids().to_vec();
    for eid in &edge_ids {
        if let Some(edge) = g.edge_label_mut_by_id(eid) {
            for p in &mut edge.points {
                p.x -= min_x;
                p.y -= min_y;
            }
            if let Some(x) = edge.x {
                edge.x = Some(x - min_x);
            }
            if let Some(y) = edge.y {
                edge.y = Some(y - min_y);
            }
        }
    }

    let gl = g.graph_mut();
    gl.width = max_x - min_x + margin_x;
    gl.height = max_y - min_y + margin_y;
}

fn assign_node_intersects(g: &mut LayoutGraph) {
    let edge_ids: Vec<String> = g.edge_ids().to_vec();
    for eid in &edge_ids {
        let eobj = match g.edge_obj_by_id(eid) {
            Some(e) => e.clone(),
            None => continue,
        };
        let mut points = g
            .edge_label_by_id(eid)
            .map(|edge| edge.points.clone())
            .unwrap_or_default();
        let (vx, vy, vw, vh) = g
            .node(&eobj.v)
            .map(|n| (n.x.unwrap_or(0.0), n.y.unwrap_or(0.0), n.width, n.height))
            .unwrap_or((0.0, 0.0, 0.0, 0.0));
        let (wx, wy, ww, wh) = g
            .node(&eobj.w)
            .map(|n| (n.x.unwrap_or(0.0), n.y.unwrap_or(0.0), n.width, n.height))
            .unwrap_or((0.0, 0.0, 0.0, 0.0));

        let (p1, p2) = if points.is_empty() {
            let pw = Point { x: wx, y: wy };
            let pv = Point { x: vx, y: vy };
            (pw, pv)
        } else {
            (points[0], *points.last().unwrap())
        };

        let node_v = NodeLabel {
            x: Some(vx),
            y: Some(vy),
            width: vw,
            height: vh,
            ..Default::default()
        };
        let node_w = NodeLabel {
            x: Some(wx),
            y: Some(wy),
            width: ww,
            height: wh,
            ..Default::default()
        };

        let start = util::intersect_rect(&node_v, &p1);
        let end = util::intersect_rect(&node_w, &p2);

        points.insert(0, start);
        points.push(end);

        if let Some(edge_label) = g.edge_label_mut_by_id(eid) {
            edge_label.points = points;
        }
    }
}

fn fixup_edge_label_coords(g: &mut LayoutGraph) {
    let edge_ids: Vec<String> = g.edge_ids().to_vec();
    for eid in &edge_ids {
        if let Some(edge) = g.edge_label_mut_by_id(eid)
            && edge.x.is_some()
        {
            let labelpos = edge.labelpos;
            let labeloffset = edge.labeloffset;

            if labelpos == LabelPos::Left || labelpos == LabelPos::Right {
                edge.width -= labeloffset;
            }

            let width = edge.width;
            let x = edge.x.unwrap_or(0.0);

            let new_x = match labelpos {
                LabelPos::Left => x - width / 2.0 - labeloffset,
                LabelPos::Right => x + width / 2.0 + labeloffset,
                LabelPos::Center => x,
            };
            edge.x = Some(new_x);
        }
    }
}

fn reverse_points_for_reversed_edges(g: &mut LayoutGraph) {
    let edge_ids: Vec<String> = g.edge_ids().to_vec();
    for eid in &edge_ids {
        if let Some(edge) = g.edge_label_mut_by_id(eid)
            && edge.reversed
        {
            edge.points.reverse();
        }
    }
}

fn remove_border_nodes(g: &mut LayoutGraph) {
    let node_ids: Vec<String> = g.node_ids().to_vec();
    let mut updates: Vec<(String, f64, f64, f64, f64)> = Vec::new();

    for v in &node_ids {
        let children = g.children(Some(v)).unwrap_or_default();
        if !children.is_empty() {
            // Extract only the fields we need rather than cloning the entire NodeLabel
            let (bt, bb, l_node_name, r_node_name) = match g.node(v) {
                Some(node) => (
                    node.border_top.clone().unwrap_or_default(),
                    node.border_bottom.clone().unwrap_or_default(),
                    node.border_left
                        .last()
                        .and_then(|o| o.clone())
                        .unwrap_or_default(),
                    node.border_right
                        .last()
                        .and_then(|o| o.clone())
                        .unwrap_or_default(),
                ),
                None => continue,
            };

            let lx = g.node(&l_node_name).and_then(|n| n.x).unwrap_or(0.0);
            let rx = g.node(&r_node_name).and_then(|n| n.x).unwrap_or(0.0);
            let ty = g.node(&bt).and_then(|n| n.y).unwrap_or(0.0);
            let by = g.node(&bb).and_then(|n| n.y).unwrap_or(0.0);

            let width = (rx - lx).abs();
            let height = (by - ty).abs();
            let x = lx + width / 2.0;
            let y = ty + height / 2.0;

            updates.push((v.clone(), width, height, x, y));
        }
    }

    for (v, width, height, x, y) in updates {
        if let Some(node) = g.node_mut(&v) {
            node.width = width;
            node.height = height;
            node.x = Some(x);
            node.y = Some(y);
        }
    }

    let border_nodes: Vec<String> = g
        .node_ids()
        .iter()
        .filter(|v| {
            g.node(v)
                .map(|n| n.dummy == Some(DummyType::Border))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    for v in border_nodes {
        g.remove_node(&v);
    }
}

fn remove_self_edges(g: &mut LayoutGraph) {
    let self_edge_ids: Vec<String> = g
        .edge_ids()
        .iter()
        .filter(|eid| g.edge_obj_by_id(eid).map(|e| e.v == e.w).unwrap_or(false))
        .cloned()
        .collect();

    for eid in &self_edge_ids {
        let eobj = match g.edge_obj_by_id(eid) {
            Some(e) => e.clone(),
            None => continue,
        };
        let label = g.edge_label_by_id(eid).cloned();
        if let Some(node) = g.node_mut(&eobj.v)
            && let Some(label) = label
        {
            node.self_edges.push(SelfEdgeRecord {
                e: Edge::new(&eobj.v, &eobj.w, eobj.name.as_deref()),
                label,
            });
        }
        g.remove_edge_by_obj(&eobj);
    }
}

fn insert_self_edges(g: &mut LayoutGraph) {
    let layers = util::build_layer_matrix(g);
    for layer in &layers {
        let mut order_shift: i64 = 0;
        for (i, v) in layer.iter().enumerate() {
            if v.is_empty() {
                continue;
            }

            let (self_edges, rank) = match g.node(v) {
                Some(n) => (n.self_edges.clone(), n.rank.unwrap_or(0)),
                None => continue,
            };
            if let Some(n) = g.node_mut(v) {
                n.order = Some(i as i64 + order_shift);
            }

            for se in &self_edges {
                order_shift += 1;
                let se_width = se.label.width;
                let se_height = se.label.height;

                let dummy_label = NodeLabel {
                    width: se_width,
                    height: se_height,
                    rank: Some(rank),
                    order: Some(i as i64 + order_shift),
                    e: Some(se.e.clone()),
                    self_edge_data: Some(se.label.clone()),
                    ..Default::default()
                };
                util::add_dummy_node(g, DummyType::SelfEdge, dummy_label, "_se");
            }

            if let Some(n) = g.node_mut(v) {
                n.self_edges.clear();
            }
        }
    }
}

fn position_self_edges(g: &mut LayoutGraph) {
    let node_ids: Vec<String> = g.node_ids().to_vec();
    // Collect only the data we need from self-edge dummy nodes
    let mut to_process: Vec<(
        String,
        Option<f64>,
        Option<f64>,
        Option<Edge>,
        Option<EdgeLabel>,
    )> = Vec::new();

    for v in &node_ids {
        let is_self_edge = g
            .node(v)
            .map(|n| n.dummy == Some(DummyType::SelfEdge))
            .unwrap_or(false);
        if is_self_edge {
            let node = g.node(v).unwrap();
            to_process.push((
                v.clone(),
                node.x,
                node.y,
                node.e.clone(),
                node.self_edge_data.clone(),
            ));
        }
    }

    for (v, node_x, node_y, e_opt, self_edge_data) in to_process {
        let e_ref = match e_opt {
            Some(e) => e,
            None => continue,
        };
        let (sx, sw, sy, sh) = g
            .node(&e_ref.v)
            .map(|n| (n.x.unwrap_or(0.0), n.width, n.y.unwrap_or(0.0), n.height))
            .unwrap_or((0.0, 0.0, 0.0, 0.0));

        let x = sx + sw / 2.0;
        let y = sy;
        let nx = node_x.unwrap_or(0.0);
        let dx = nx - x;
        let dy = sh / 2.0;

        let mut label = self_edge_data.unwrap_or_default();
        label.points = vec![
            Point {
                x: x + 2.0 * dx / 3.0,
                y: y - dy,
            },
            Point {
                x: x + 5.0 * dx / 6.0,
                y: y - dy,
            },
            Point { x: x + dx, y },
            Point {
                x: x + 5.0 * dx / 6.0,
                y: y + dy,
            },
            Point {
                x: x + 2.0 * dx / 3.0,
                y: y + dy,
            },
        ];
        label.x = Some(nx);
        label.y = node_y;

        g.set_edge(&e_ref.v, &e_ref.w, Some(label), e_ref.name.as_deref());
        g.remove_node(&v);
    }
}
