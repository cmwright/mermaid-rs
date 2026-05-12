# mermaid-dagre

A 1:1 Rust port of the [dagre](https://github.com/dagrejs/dagre) JavaScript graph layout library. Published as `mermaid-dagre` on crates.io (the import path remains `dagre_rust`).

dagre-rust implements the Sugiyama-style layered graph layout algorithm, producing hierarchical, top-down (or left-right, bottom-top, right-left) layouts for directed graphs. It supports compound graphs (nested subgraphs), multigraphs, edge labels, and all the layout options of the original JS library.

## Purpose

This project exists to provide exact behavioral parity with JS dagre in a Rust environment. If you have a system that relies on dagre's layout output and you need that same layout in Rust (for server-side rendering, WASM, CLI tools, etc.), this crate produces identical coordinates and edge routing.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
mermaid-dagre = "0.1"
```

```rust
use dagre_rust::Graph;
```

### Basic layout

```rust
use dagre_rust::{
    layout, Edge, Graph, GraphOptions, LayoutGraph,
    GraphLabel, NodeLabel, EdgeLabel, RankDir,
};

// Create a directed compound graph
let mut g = LayoutGraph::with_options(&GraphOptions {
    directed: true,
    multigraph: false,
    compound: false,
});

// Configure graph-level layout options
g.set_graph(GraphLabel {
    rankdir: RankDir::TB,
    nodesep: 50.0,
    ranksep: 50.0,
    ..Default::default()
});

// Add nodes with dimensions
g.set_node("a", Some(NodeLabel {
    width: 100.0,
    height: 40.0,
    ..Default::default()
}));
g.set_node("b", Some(NodeLabel {
    width: 100.0,
    height: 40.0,
    ..Default::default()
}));

// Add an edge
g.set_edge("a", "b", Some(EdgeLabel::default()), None);

// Run the layout algorithm
layout(&mut g);

// Read results from nodes
let a = g.node("a").unwrap();
println!("a is at ({}, {})", a.x.unwrap(), a.y.unwrap());

let b = g.node("b").unwrap();
println!("b is at ({}, {})", b.x.unwrap(), b.y.unwrap());

// Read edge routing points
let edge = g.edge("a", "b", None).unwrap();
for pt in &edge.points {
    println!("  point: ({}, {})", pt.x, pt.y);
}

// Read overall graph dimensions
let gl = g.graph();
println!("graph dimensions: {} x {}", gl.width, gl.height);
```

### Compound graphs (nested subgraphs)

```rust
use dagre_rust::*;

let mut g = LayoutGraph::with_options(&GraphOptions {
    directed: true,
    multigraph: false,
    compound: true,  // enable compound graph support
});

g.set_graph(GraphLabel::default());

// Add a parent group node (no dimensions needed -- computed after layout)
g.set_node("group1", Some(NodeLabel::default()));

// Add child nodes
g.set_node("a", Some(NodeLabel { width: 50.0, height: 50.0, ..Default::default() }));
g.set_node("b", Some(NodeLabel { width: 50.0, height: 50.0, ..Default::default() }));

// Nest children under the group
g.set_parent("a", Some("group1"));
g.set_parent("b", Some("group1"));

g.set_edge("a", "b", Some(EdgeLabel::default()), None);

layout(&mut g);

// After layout, the parent node has a bounding box around its children
let group = g.node("group1").unwrap();
println!("group1 bounds: ({}, {}) {}x{}",
    group.x.unwrap(), group.y.unwrap(), group.width, group.height);
```

### Edge labels

```rust
use dagre_rust::*;

let mut g = LayoutGraph::new();
g.set_graph(GraphLabel::default());

g.set_node("a", Some(NodeLabel { width: 80.0, height: 30.0, ..Default::default() }));
g.set_node("b", Some(NodeLabel { width: 80.0, height: 30.0, ..Default::default() }));

// Add an edge with a label that has its own dimensions
g.set_edge("a", "b", Some(EdgeLabel {
    width: 60.0,       // label width
    height: 20.0,      // label height
    labelpos: LabelPos::Center,  // "l", "c", or "r"
    labeloffset: 10.0,
    ..Default::default()
}), None);

layout(&mut g);

// Edge label position is set after layout
let edge = g.edge("a", "b", None).unwrap();
if let (Some(x), Some(y)) = (edge.x, edge.y) {
    println!("edge label at ({}, {})", x, y);
}
```

### Layout options

```rust
use dagre_rust::*;

let mut g = LayoutGraph::new();
g.set_graph(GraphLabel {
    rankdir: RankDir::LR,                      // left-to-right layout
    ranker: Ranker::NetworkSimplex,            // ranking algorithm
    acyclicer: Some(Acyclicer::Greedy),        // greedy cycle removal
    align: Some(Align::UL),                    // upper-left alignment
    rankalign: RankAlign::Top,                 // align nodes to top of rank
    nodesep: 30.0,
    edgesep: 10.0,
    ranksep: 75.0,
    marginx: 20.0,
    marginy: 20.0,
    ..Default::default()
});

// ... add nodes and edges, then call layout(&mut g)
```

### Using `layout_with_opts`

```rust
use dagre_rust::*;

let mut g = LayoutGraph::new();
// ... set up graph ...

let opts = LayoutOpts {
    disable_optimal_order_heuristic: false, // set true to skip crossing minimization iterations
};
layout_with_opts(&mut g, &opts);
```

## API Reference

### Core types

| Type | Description |
|------|-------------|
| `LayoutGraph` | Type alias for `Graph<NodeLabel, EdgeLabel, GraphLabel>` -- the main graph type for layout |
| `Graph<N, E, G>` | Generic graph data structure supporting directed/undirected, multigraph, and compound modes |
| `GraphOptions` | Construction options: `directed`, `multigraph`, `compound` (all `bool`) |
| `Edge` | Edge identifier with fields `v: String`, `w: String`, `name: Option<String>` |

### Label types

| Type | Description |
|------|-------------|
| `GraphLabel` | Graph-level configuration and layout output. Set before layout to configure; read after for `width`/`height` |
| `NodeLabel` | Node configuration (`width`, `height`) and layout output (`x`, `y`, `rank`, `order`) |
| `EdgeLabel` | Edge configuration (`weight`, `minlen`, label dimensions) and layout output (`points`, `x`, `y`) |
| `Point` | A 2D coordinate with `x: f64` and `y: f64` |

### Configuration enums

| Enum | Values | Default | Description |
|------|--------|---------|-------------|
| `RankDir` | `TB`, `BT`, `LR`, `RL` | `TB` | Layout direction |
| `Ranker` | `NetworkSimplex`, `TightTree`, `LongestPath` | `NetworkSimplex` | Ranking algorithm |
| `Acyclicer` | `Greedy` | `None` (DFS) | Cycle removal strategy. Use `Option<Acyclicer>` -- `None` means DFS |
| `Align` | `UL`, `UR`, `DL`, `DR` | `None` | Alignment within ranks. Use `Option<Align>` -- `None` means median of all four |
| `RankAlign` | `Center`, `Top`, `Bottom` | `Center` | Vertical alignment of nodes within a rank |
| `LabelPos` | `Left`, `Right`, `Center` | `Right` | Edge label position relative to the edge |

### Graph-level options (`GraphLabel` fields)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `rankdir` | `RankDir` | `TB` | Layout direction |
| `nodesep` | `f64` | `50.0` | Horizontal separation between nodes in the same rank |
| `edgesep` | `f64` | `20.0` | Horizontal separation between edges in the same rank |
| `ranksep` | `f64` | `50.0` | Vertical separation between ranks |
| `marginx` | `f64` | `0.0` | Horizontal margin around the graph |
| `marginy` | `f64` | `0.0` | Vertical margin around the graph |
| `acyclicer` | `Option<Acyclicer>` | `None` | Cycle removal: `None` = DFS, `Some(Acyclicer::Greedy)` = greedy |
| `ranker` | `Ranker` | `NetworkSimplex` | Ranking algorithm |
| `align` | `Option<Align>` | `None` | Alignment: `None` = median of all four passes |
| `rankalign` | `RankAlign` | `Center` | Vertical alignment within ranks |

### Edge options (`EdgeLabel` fields)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `weight` | `f64` | `1.0` | Edge weight (higher = shorter, straighter) |
| `minlen` | `f64` | `1.0` | Minimum number of ranks the edge should span |
| `width` | `f64` | `0.0` | Width of the edge label |
| `height` | `f64` | `0.0` | Height of the edge label |
| `labelpos` | `LabelPos` | `Right` | Edge label position: `Left`, `Center`, `Right` |
| `labeloffset` | `f64` | `10.0` | Distance of label from the edge |

### Layout output

After calling `layout()` or `layout_with_opts()`:

**On each node (`NodeLabel`):**
- `x: Option<f64>` -- x coordinate of the node center
- `y: Option<f64>` -- y coordinate of the node center
- `rank: Option<i64>` -- assigned rank (layer index)
- `order: Option<i64>` -- position within the rank
- `width` / `height` -- for compound parent nodes, updated to reflect the bounding box of children

**On each edge (`EdgeLabel`):**
- `points: Vec<Point>` -- routing waypoints from source to target
- `x: Option<f64>` -- x coordinate of the edge label (if the edge has label dimensions)
- `y: Option<f64>` -- y coordinate of the edge label

**On the graph (`GraphLabel`):**
- `width: f64` -- total width of the laid-out graph
- `height: f64` -- total height of the laid-out graph

### `Graph` methods

#### Construction
```rust
Graph::new()                          // directed, no multigraph, no compound
Graph::with_options(&GraphOptions { .. })
```

#### Node operations
```rust
g.set_node("id", Some(label))        // add or update a node
g.node("id") -> Option<&N>           // get node label
g.node_mut("id") -> Option<&mut N>   // get mutable node label
g.has_node("id") -> bool
g.remove_node("id")                  // also removes incident edges
g.nodes() -> Vec<String>             // all node IDs (insertion order)
g.node_ids() -> &[String]            // borrowed slice of node IDs
g.node_count() -> usize
g.sources() -> Vec<String>           // nodes with no in-edges
g.sinks() -> Vec<String>             // nodes with no out-edges
```

#### Edge operations
```rust
g.set_edge("v", "w", Some(label), None)       // add or update edge
g.set_edge("v", "w", Some(label), Some("name"))  // named edge (multigraph)
g.edge("v", "w", None) -> Option<&E>          // get edge label
g.edge_mut("v", "w", None) -> Option<&mut E>
g.has_edge("v", "w", None) -> bool
g.remove_edge("v", "w", None)
g.edges() -> Vec<Edge>                        // all edges (insertion order)
g.edge_count() -> usize
g.in_edges("v", None) -> Option<Vec<Edge>>    // edges into v
g.out_edges("v", None) -> Option<Vec<Edge>>   // edges out of v
g.predecessors("v") -> Option<Vec<String>>
g.successors("v") -> Option<Vec<String>>
g.neighbors("v") -> Option<Vec<String>>
```

#### Compound graph operations
```rust
g.set_parent("child", Some("parent"))  // nest child under parent
g.parent("v") -> Option<&str>          // get parent of v
g.children(Some("v")) -> Option<Vec<String>>  // get children of v
g.children(None) -> Option<Vec<String>>       // get root-level nodes
```

#### Graph-level
```rust
g.set_graph(label)
g.graph() -> &G                       // read graph label
g.graph_mut() -> &mut G               // mutate graph label
g.is_directed() -> bool
g.is_multigraph() -> bool
g.is_compound() -> bool
```

## Architecture

The layout pipeline follows the original dagre implementation step-by-step:

1. **Cycle removal** (`acyclic.rs`) -- reverse edges to make the graph acyclic
2. **Nesting graph** (`nesting_graph.rs`) -- encode compound graph hierarchy as rank constraints
3. **Rank assignment** (`rank/`) -- assign vertical ranks using network simplex
4. **Normalization** (`normalize.rs`) -- split long edges with dummy nodes
5. **Ordering** (`order/`) -- minimize edge crossings via barycenter heuristic
6. **Position assignment** (`position/`) -- assign x/y coordinates using Brandes-Kopf
7. **Denormalization** -- remove dummy nodes, restore edge points, compute final coordinates

### Module overview

```
src/
  lib.rs                  Public API (Graph, layout, layout_with_opts)
  graph.rs                Graph data structure (directed, multigraph, compound)
  types.rs                Typed label structs and enums
  layout.rs               Layout pipeline orchestration
  acyclic.rs              Cycle removal
  nesting_graph.rs        Compound graph encoding
  normalize.rs            Long-edge splitting
  add_border_segments.rs  Subgraph border nodes
  coordinate_system.rs    Rank direction transforms
  parent_dummy_chains.rs  Dummy node parenting
  rank/                   Rank assignment
    network_simplex.rs    Network simplex algorithm
    feasible_tree.rs      Initial spanning tree construction
  order/                  Crossing minimization
    barycenter.rs         Barycenter heuristic
    sort_subgraph.rs      Subgraph-aware sorting
    cross_count.rs        Edge crossing counter
  position/
    bk.rs                 Brandes-Kopf coordinate assignment
```

## Test harness

The project uses a parity testing approach to verify exact behavioral equivalence with JS dagre.

### How it works

1. **Test case definitions** (`tests/parity/harness.js`) -- a JS script that programmatically generates graph definitions as JSON, covering simple chains, diamonds, cycles, self-loops, compound graphs, multigraphs, and complex multi-subgraph layouts.

2. **JS reference results** (`tests/parity/js_results.json`) -- each test case is run through JS dagre, and the full layout output (node positions, edge points, graph dimensions) is captured.

3. **Rust parity test** (`tests/parity.rs`) -- deserializes the same test case definitions, runs them through `dagre_rust::layout`, and compares every numeric value against the JS reference with floating-point tolerance (epsilon = 1e-10).

### Running tests

```bash
# Run the Rust parity tests against pre-computed JS results
cargo test

# Regenerate test cases and JS results (requires Node.js and @dagrejs/dagre)
# from the repo root, with the JS dagre repo at ../dagre:
NODE_PATH="../dagre/node_modules:../dagre" node tests/parity/harness.js --generate
NODE_PATH="../dagre/node_modules:../dagre" node tests/parity/harness.js --all -o tests/parity/js_results.json
cargo test

# Or use the shell script:
./tests/parity/run_parity.sh
```

### Test coverage

The test suite includes 65 parity test cases covering:

- Basic layouts (single nodes, chains, diamonds, wide fan-out)
- All 4 rank directions (TB, BT, LR, RL)
- All 3 ranking algorithms (network-simplex, tight-tree, longest-path)
- All 4 alignment options (UL, UR, DL, DR)
- Cycles, self-loops, multigraph edges
- Edge labels (center, left, right positioning)
- Compound/subgraph graphs: simple, nested (2/3/4 levels), sibling subgraphs
- Cross-subgraph edges, disconnected subgraphs, empty subgraphs
- Complex real-world-style graphs (multi-org platform with 22 nodes, 3-level nesting, cross-boundary edges across multiple subgraph containers)
- Weighted edges, varying node sizes, minlen constraints

## License

MIT
