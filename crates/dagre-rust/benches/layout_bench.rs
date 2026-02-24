use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use dagre_rust::{layout, EdgeLabel, Graph, GraphLabel, GraphOptions, LabelPos, LayoutGraph, NodeLabel, RankDir};
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

fn bench_layout_example(c: &mut Criterion, name: &str, input_json: &str) {
    let input_ref: MermaidDagreInputRef =
        serde_json::from_str(input_json).expect("input fixture should deserialize");
    let template = build_graph_from_numeric_ref_input(&input_ref);

    c.bench_function(name, |b| {
        b.iter_batched(
            || template.clone(),
            |mut g| layout(black_box(&mut g)),
            BatchSize::SmallInput,
        )
    });
}

fn bench_layout_fixtures(c: &mut Criterion) {
    bench_layout_example(
        c,
        "layout/example2_reduced",
        include_str!("../../../tests/test_loop/example2_mermaidjs_dagre_input_reduced.json"),
    );
    bench_layout_example(
        c,
        "layout/example5_reduced",
        include_str!("../../../tests/test_loop/example5_mermaidjs_dagre_input_reduced.json"),
    );
    bench_layout_example(
        c,
        "layout/example7_reduced",
        include_str!("../../../tests/test_loop/example7_mermaidjs_dagre_input_reduced.json"),
    );
}

criterion_group!(benches, bench_layout_fixtures);
criterion_main!(benches);
