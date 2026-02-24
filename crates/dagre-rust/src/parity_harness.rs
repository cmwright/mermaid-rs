use std::collections::HashMap;

use crate::{
    layout, Align, EdgeLabel, Graph, GraphLabel, GraphOptions, LabelPos, LayoutGraph, NodeLabel,
    RankAlign, RankDir, Ranker,
};
use serde::Deserialize;

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

fn rankdir_from_str(s: &str) -> RankDir {
    match s {
        "TB" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        other => panic!("unsupported rankdir in fixture: {other}"),
    }
}

fn build_graph_from_numeric_ref_input(reference: &MermaidDagreInputRef) -> LayoutGraph {
    let mut g = Graph::with_options(&GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });

    let mut gl = GraphLabel::default();
    gl.rankdir = rankdir_from_str(&reference.graph.rankdir);
    gl.nodesep = reference.graph.nodesep;
    gl.ranksep = reference.graph.ranksep;
    gl.marginx = reference.graph.marginx;
    gl.marginy = reference.graph.marginy;
    g.set_graph(gl);

    // Preserve JS insertion order from fixture.
    for n in &reference.nodes {
        let mut nl = NodeLabel::default();
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
        let mut el = EdgeLabel::default();
        el.width = e.width;
        el.height = e.height;
        el.minlen = e.minlen;
        el.weight = e.weight;
        el.labeloffset = e.labeloffset;
        el.labelpos = match e.labelpos.as_str() {
            "l" | "L" => LabelPos::Left,
            "c" | "C" => LabelPos::Center,
            "r" | "R" => LabelPos::Right,
            other => panic!("unsupported labelpos in fixture: {other}"),
        };
        g.set_edge(&e.from, &e.to, Some(el), e.name.as_deref());
    }

    g
}

fn build_graph_from_numeric_ref_input_with_ranker(
    reference: &MermaidDagreInputRef,
    ranker: Ranker,
) -> LayoutGraph {
    let mut g = build_graph_from_numeric_ref_input(reference);
    g.graph_mut().ranker = ranker;
    g
}

fn assert_numeric_parity(input_ref: &MermaidDagreInputRef, output_ref: &MermaidAfterLayoutRef) {
    let mut g = build_graph_from_numeric_ref_input(input_ref);
    layout(&mut g);

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

    let expected_nodes: HashMap<&str, &MermaidAfterNodeRef> =
        output_ref.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    for (id, expected) in expected_nodes {
        let nl = g
            .node(id)
            .unwrap_or_else(|| panic!("missing node in dagre output: {}", id));
        assert!(
            approx_eq(nl.x.unwrap_or_default(), expected.x, 1e-6),
            "node x mismatch for {}",
            id
        );
        assert!(
            approx_eq(nl.y.unwrap_or_default(), expected.y, 1e-6),
            "node y mismatch for {}",
            id
        );
        assert!(
            approx_eq(nl.width, expected.width, 1e-6),
            "node width mismatch for {}",
            id
        );
        assert!(
            approx_eq(nl.height, expected.height, 1e-6),
            "node height mismatch for {}",
            id
        );
        let actual_parent = g.parent(id).map(|s| s.to_string());
        assert_eq!(actual_parent, expected.parent, "node parent mismatch for {}", id);
    }

    for expected in &output_ref.edges {
        let edge_obj = g
            .edges()
            .into_iter()
            .find(|e| e.v == expected.from && e.w == expected.to)
            .unwrap_or_else(|| panic!("missing edge in dagre output: {} -> {}", expected.from, expected.to));
        let el = g
            .edge_by_obj(&edge_obj)
            .unwrap_or_else(|| panic!("missing edge label for {} -> {}", expected.from, expected.to));

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
        for (idx, (actual_p, expected_p)) in el.points.iter().zip(expected.points.iter()).enumerate() {
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
fn parity_example5_identical_numeric_input_produces_identical_output() {
    let input_ref: MermaidDagreInputRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"
    ))
    .expect("input fixture should deserialize");
    let output_ref: MermaidAfterLayoutRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example5_mermaidjs_dagre_after_layout_reduced.json"
    ))
    .expect("after-layout fixture should deserialize");

    assert_numeric_parity(&input_ref, &output_ref);
}

#[test]
fn parity_example2_identical_numeric_input_produces_identical_output() {
    let input_ref: MermaidDagreInputRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example2_mermaidjs_dagre_input_reduced.json"
    ))
    .expect("input fixture should deserialize");
    let output_ref: MermaidAfterLayoutRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example2_mermaidjs_dagre_after_layout_reduced.json"
    ))
    .expect("after-layout fixture should deserialize");

    assert_numeric_parity(&input_ref, &output_ref);
}

#[test]
fn parity_example7_identical_numeric_input_produces_identical_output() {
    let input_ref: MermaidDagreInputRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example7_mermaidjs_dagre_input_reduced.json"
    ))
    .expect("input fixture should deserialize");
    let output_ref: MermaidAfterLayoutRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example7_mermaidjs_dagre_after_layout_reduced.json"
    ))
    .expect("after-layout fixture should deserialize");

    assert_numeric_parity(&input_ref, &output_ref);
}

#[test]
#[ignore = "debug helper"]
fn debug_example5_width_by_ranker() {
    let input_ref: MermaidDagreInputRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"
    ))
    .expect("input fixture should deserialize");
    for ranker in [Ranker::NetworkSimplex, Ranker::TightTree, Ranker::LongestPath] {
        let mut g = build_graph_from_numeric_ref_input_with_ranker(&input_ref, ranker);
        layout(&mut g);
        eprintln!(
            "{ranker:?}: width={} height={}",
            g.graph().width,
            g.graph().height
        );
    }
    panic!("debug output above");
}

#[test]
#[ignore = "debug helper"]
fn debug_example5_width_by_rankalign() {
    let input_ref: MermaidDagreInputRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"
    ))
    .expect("input fixture should deserialize");
    for rankalign in [RankAlign::Center, RankAlign::Top, RankAlign::Bottom] {
        let mut g = build_graph_from_numeric_ref_input(&input_ref);
        g.graph_mut().rankalign = rankalign;
        layout(&mut g);
        eprintln!(
            "{rankalign:?}: width={} height={}",
            g.graph().width,
            g.graph().height
        );
    }
    panic!("debug output above");
}

#[test]
#[ignore = "debug helper"]
fn debug_example5_width_by_edgesep_and_rankdir_explicit() {
    let input_ref: MermaidDagreInputRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"
    ))
    .expect("input fixture should deserialize");
    for edgesep in [0.0, 10.0, 20.0, 30.0] {
        for rankdir_explicit in [false, true] {
            let mut g = build_graph_from_numeric_ref_input(&input_ref);
            g.graph_mut().edgesep = edgesep;
            g.graph_mut().rankdir_explicit = rankdir_explicit;
            layout(&mut g);
            eprintln!(
                "edgesep={edgesep:>4} rankdir_explicit={rankdir_explicit}: width={} height={}",
                g.graph().width,
                g.graph().height
            );
        }
    }
    panic!("debug output above");
}

#[test]
#[ignore = "debug helper"]
fn debug_example5_width_by_align() {
    let input_ref: MermaidDagreInputRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"
    ))
    .expect("input fixture should deserialize");
    for align in [None, Some(Align::UL), Some(Align::UR), Some(Align::DL), Some(Align::DR)] {
        let mut g = build_graph_from_numeric_ref_input(&input_ref);
        g.graph_mut().align = align;
        layout(&mut g);
        eprintln!(
            "align={align:?}: width={} height={}",
            g.graph().width,
            g.graph().height
        );
    }
    panic!("debug output above");
}

#[test]
#[ignore = "debug helper"]
fn debug_example5_io_diff_details() {
    let input_ref: MermaidDagreInputRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"
    ))
    .expect("input fixture should deserialize");
    let output_ref: MermaidAfterLayoutRef = serde_json::from_str(include_str!(
        "../../../tests/test_loop/example5_mermaidjs_dagre_after_layout_reduced.json"
    ))
    .expect("after-layout fixture should deserialize");

    let mut g = build_graph_from_numeric_ref_input(&input_ref);
    layout(&mut g);

    eprintln!(
        "graph: ours=({}, {}) expected=({}, {}), node_rank_factor={:?}",
        g.graph().width,
        g.graph().height,
        output_ref.graph.width,
        output_ref.graph.height,
        g.graph().node_rank_factor
    );

    let mut node_diffs: Vec<(String, f64, f64, f64, f64)> = Vec::new();
    for expected in &output_ref.nodes {
        let nl = g
            .node(&expected.id)
            .unwrap_or_else(|| panic!("missing node in dagre output: {}", expected.id));
        let dx = nl.x.unwrap_or_default() - expected.x;
        let dy = nl.y.unwrap_or_default() - expected.y;
        let dw = nl.width - expected.width;
        let dh = nl.height - expected.height;
        node_diffs.push((expected.id.clone(), dx, dy, dw, dh));
    }
    node_diffs.sort_by(|a, b| {
        let sa = a.1.abs() + a.2.abs() + a.3.abs() + a.4.abs();
        let sb = b.1.abs() + b.2.abs() + b.3.abs() + b.4.abs();
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });
    for (id, dx, dy, dw, dh) in node_diffs.into_iter().take(12) {
        eprintln!("node {id:8} dx={dx:9.3} dy={dy:9.3} dw={dw:9.3} dh={dh:9.3}");
    }

    for expected in &output_ref.edges {
        let edge_obj = g
            .edges()
            .into_iter()
            .find(|e| e.v == expected.from && e.w == expected.to)
            .unwrap_or_else(|| panic!("missing edge in dagre output: {} -> {}", expected.from, expected.to));
        let el = g
            .edge_by_obj(&edge_obj)
            .unwrap_or_else(|| panic!("missing edge label for {} -> {}", expected.from, expected.to));
        eprintln!(
            "edge {}->{} points ours={} expected={}",
            expected.from,
            expected.to,
            el.points.len(),
            expected.points.len()
        );
        for (idx, (a, b)) in el.points.iter().zip(expected.points.iter()).enumerate().take(6) {
            eprintln!(
                "  p[{idx}] dx={:.3} dy={:.3}",
                a.x - b.x,
                a.y - b.y
            );
        }
    }
    panic!("debug output above");
}
