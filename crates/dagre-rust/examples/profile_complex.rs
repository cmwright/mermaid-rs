use dagre_rust::{
    layout_profiled, EdgeLabel, Graph, GraphLabel, GraphOptions, LabelPos, LayoutGraph, LayoutOpts,
    NodeLabel, RankDir,
};
use serde::Deserialize;
use std::collections::HashMap;

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

    let gl = GraphLabel {
        rankdir: rankdir_from_str(&reference.graph.rankdir),
        nodesep: reference.graph.nodesep,
        ranksep: reference.graph.ranksep,
        marginx: reference.graph.marginx,
        marginy: reference.graph.marginy,
        ..Default::default()
    };
    g.set_graph(gl);

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
        let labelpos = match e.labelpos.as_str() {
            "l" | "L" => LabelPos::Left,
            "c" | "C" => LabelPos::Center,
            "r" | "R" => LabelPos::Right,
            other => panic!("unsupported labelpos in fixture: {other}"),
        };
        let el = EdgeLabel {
            width: e.width,
            height: e.height,
            minlen: e.minlen,
            weight: e.weight,
            labeloffset: e.labeloffset,
            labelpos,
            ..Default::default()
        };
        g.set_edge(&e.from, &e.to, Some(el), e.name.as_deref());
    }

    g
}

fn profile_fixture(name: &str, input_json: &str, warmups: usize, runs: usize) {
    let input_ref: MermaidDagreInputRef =
        serde_json::from_str(input_json).expect("input fixture should deserialize");
    let template = build_graph_from_numeric_ref_input(&input_ref);

    for _ in 0..warmups {
        let mut g = template.clone();
        let _ = layout_profiled(&mut g, &LayoutOpts::default());
    }

    let mut totals: HashMap<&'static str, f64> = HashMap::new();
    let mut total_ms = 0.0;
    for _ in 0..runs {
        let mut g = template.clone();
        let profile = layout_profiled(&mut g, &LayoutOpts::default());
        total_ms += profile.total_ms;
        for s in profile.stages {
            *totals.entry(s.stage).or_insert(0.0) += s.duration_ms;
        }
    }

    let avg_total = total_ms / runs as f64;
    let mut rows: Vec<(&'static str, f64)> = totals
        .into_iter()
        .map(|(stage, sum)| (stage, sum / runs as f64))
        .collect();
    rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    println!("\n=== {name} ===");
    println!("avg total: {:.3} ms over {} runs", avg_total, runs);
    for (stage, ms) in rows {
        let pct = if avg_total > 0.0 {
            (ms / avg_total) * 100.0
        } else {
            0.0
        };
        println!("{stage:30} {:8.3} ms  {:6.2}%", ms, pct);
    }
}

fn main() {
    profile_fixture(
        "example5_reduced",
        include_str!("../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"),
        3,
        20,
    );
    profile_fixture(
        "example7_reduced",
        include_str!("../../../tests/test_loop/example7_mermaidjs_dagre_input_reduced.json"),
        3,
        20,
    );
}
