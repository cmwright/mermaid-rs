# Session State — Layout Pipeline Refactoring

## Goal

Refactoring the layout pipeline in mermaid-rs to remove all post-hoc hacks and instead properly implement the dagre/Sugiyama/Brandes-Kopf algorithms, EXACTLY matching what mermaid.js + dagre do. The reference source for mermaid.js and dagre are in the parent folder `../dagre` and `../mermaid`.

## Rules

- EXACTLY COPY what mermaid.js + dagre do — no inventing custom solutions, no hacks
- When fixing bugs, first understand how dagre handles the situation before writing code
- Do not introduce new hacks — understand the root cause and fix it properly
- Remove dead code that isn't wired into the pipeline
- All tests must pass (currently 813 lib + 78 e2e + 1 examples + 10 png), zero warnings
- **Do NOT ease/loosen any tests** — the tests are based on mermaid.js output and are correct

## Current Test Status (after latest changes)

- **78 e2e tests**: all pass
- **10 png tests**: all pass
- **1 examples test**: passes
- **813 lib tests**: 812 pass, **1 fail**
- **Compiler warnings**: 0

The 1 remaining failure:
- `test_org_flowchart_edge_corridors` — edge RootOU→OO1 waypoint x=1091.0 outside corridor [735.1, 1020.4]

Previously there were 2 failures; `test_org_flowchart_sibling_rank_alignment` now passes after Step 1.

## What Was Completed This Session

### Step 1: Match dagre's network simplex initialization (DONE)

Three changes to `crates/mermaid-core/src/layout/flowchart/sugiyama/rank_assignment.rs`:

1. **Longest-path initialization rewritten** (lines ~381-498): Replaced forward-propagation Kahn's topo sort with backward DFS from sources matching dagre's `longestPath` exactly. Sinks get rank 0, everything propagates backward via `rank = min(dfs(successor) - minlen)`.

2. **Feasible tree construction rewritten** (lines ~500-580): Replaced our greedy one-at-a-time approach with dagre's `feasibleTree` algorithm: grow maximal tight tree via DFS, find minimum slack crossing edge, shift ALL tree node ranks uniformly by delta, repeat.

3. **Pivot loop rewritten** (lines ~615-700):
   - `leaveEdge`: Changed from finding **most negative** cut value to finding **first** negative cut value (matching dagre's `tree.edges().find(...)`)
   - `enterEdge`: Rewrote to match dagre's directional constraint using `flip`/`tailLabel` logic. Previously we scanned all crossing edges; now we only consider candidates in the correct direction.

4. **Fixed non-determinism**: Sorted `local_edges` after `simplify` (HashMap key iteration was random). Also sorted component node lists for deterministic `node_to_local` mapping.

## What Needs To Be Done (in priority order)

### Step 2: Remove flat ordering pre-pass (NOT STARTED)

**File**: `crates/mermaid-core/src/layout/flowchart/sugiyama/mod.rs` lines 197-198

Currently the pipeline runs TWO ordering passes:
```rust
// Flat barycenter ordering as initial pass.
ordering::minimize_crossings(graph, &mut layers, membership, 24);

// Recursive subgraph-aware ordering matching dagre's order() module.
recursive_ordering::minimize_crossings_recursive(...)
```

Dagre only uses the recursive approach (`sortSubgraph` on compound layer graphs). The flat barycenter ordering (`ordering.rs`) should be removed, leaving only `recursive_ordering::minimize_crossings_recursive`.

**What to do**: Remove the `ordering::minimize_crossings` call from the pipeline. If tests fail, debug the recursive ordering module (`recursive_ordering.rs`) to find what it handles differently from dagre's `sortSubgraph`.

**Key dagre reference files**:
- `../dagre/lib/order/index.js` — main order() loop
- `../dagre/lib/order/sort-subgraph.js` — recursive sort
- `../dagre/lib/order/build-layer-graph.js` — compound layer graph construction
- `../dagre/lib/order/init-order.js` — DFS-based initial ordering

**Potential issues**: The recursive ordering may not handle some edge cases that the flat ordering currently compensates for. Focus areas:
- Ensure `ranks_to_layers` DFS ordering matches dagre's `initOrder`
- Check that `sort_subgraph` correctly handles all child types (leaf nodes, subgraph entries, border nodes)
- Verify the constraint graph accumulation across layers works correctly

### Step 3: Remove rank doubling (NOT STARTED)

**File**: `crates/mermaid-core/src/layout/flowchart/sugiyama/mod.rs` lines 150-152

```rust
for rank in ranks.values_mut() {
    *rank *= 2;
}
```

This doubles all ranks to create interstitial space for subgraph padding. Dagre achieves this differently — through `makeSpaceForEdgeLabels` (minlen doubling + halved ranksep). Once coordinate assignment properly accounts for subgraph padding in rank spacing, this extra doubling should be unnecessary.

**What to do**: Remove the rank doubling. Adjust coordinate assignment (`coordinate_assignment.rs`) to properly handle subgraph padding without it. The `RANK_SEP / 2.0` passed to `assign_coordinates` may need to change.

### Step 4: Remove align_sibling_subgraph_ranks and align_within_subgraph_peers (NOT STARTED)

**File**: `crates/mermaid-core/src/layout/flowchart/sugiyama/mod.rs` lines 127-128

```rust
rank_assignment::align_sibling_subgraph_ranks(graph, &mut ranks, ast, membership);
rank_assignment::align_within_subgraph_peers(graph, &mut ranks, membership, ast);
```

And the compaction block on lines 131-143.

These are NOT in dagre's pipeline. They were workarounds for our network simplex finding different optima. Now that Step 1 improved NS to match dagre more closely (fixed 1 of 2 pre-existing failures), try removing them and see if the remaining test failure gets better or worse.

**What to do**: Remove the `align_sibling_subgraph_ranks`, `align_within_subgraph_peers` calls and the compaction block after them. If tests pass, also delete the functions themselves from `rank_assignment.rs` (lines 780-1157) and their helper functions (`collect_sibling_groups_by_depth`, `collect_groups_recursive`, `align_one_sibling_group`, `compute_dependency_tiers`, `align_subgraph_peers_recursive`, `propagate_ranks_forward`, `topological_sort`).

**Note**: Steps 2, 3, and 4 may interact. Consider trying them in combination if individual removal causes regressions.

## Discoveries (accumulated across all sessions)

1. **dagre has NO `sync_dummy_positions`**: Dummy node positions from Brandes-Kopf are used directly as edge waypoints.
2. **dagre has NO `separate_overlapping_sibling_subgraphs`**: Compound layout is handled inside Sugiyama via nesting graph + border nodes.
3. **`removeEmptyRanks` is critical**: Dagre compacts empty ranks while preserving structural gaps.
4. **Parent dummy chains assign dummies to the deepest subgraph** that contains their rank.
5. **Border segment node removal corrupts NodeIndex values**: Fixed by keeping border nodes in the graph and filtering them out during position extraction.
6. **dagre doubles all edge minlens BEFORE the nesting graph** via `makeSpaceForEdgeLabels`.
7. **dagre's nesting graph creates `root → leaf` edges for ALL leaf nodes** with weight=0 and minlen=nodeSep.
8. **Network simplex has multiple optimal solutions** — dagre and our code find different optima. Matching dagre's initialization (backward DFS) + feasible tree (uniform rank shifting) + pivot selection (first negative, directional entering edge) dramatically reduced divergence.
9. **dagre's `simplify` function** merges parallel edges before rank assignment (sum weights, max minlen).
10. **`removeEmptyRanks` timing matters critically**: Runs AFTER rank assignment but BEFORE nesting cleanup.
11. **dagre starts with an UP sweep (not DOWN)**: `i % 2 ? downLayerGraphs : upLayerGraphs` — when i=0, up is used.
12. **dagre's `sortSubgraph` operates on compound layer graphs**: Each rank gets a compound graph where subgraph nodes are compound parents.
13. **dagre's `consumeUnsortable` increments index by 1** (not by `entry.vs.length`) for each consumed unsortable entry.
14. **Border nodes must be in the membership map** for recursive ordering.
15. **dagre's `asNonCompoundGraph` strips nodes that have children** (compound parent nodes and the root).
16. **Subgraph vertical spacing**: Dagre's minlen doubling + halved ranksep achieves proper interstitial space.
17. **`nestingGraph.cleanup` in dagre is very simple**: Just removes the root node and all nesting edges.
18. **Edge label proxies**: dagre uses `injectEdgeLabelProxies` to pin label midpoint ranks.
19. **Pipeline ordering**: normalize.run → parentDummyChains → addBorderSegments (NOT borders first).
20. **HashMap iteration order causes non-determinism**: Fixed by sorting `local_edges` after simplify and sorting component node lists.
21. **dagre's `leaveEdge` finds FIRST negative cut value**, not most negative. Order matters for determinism.
22. **dagre's `enterEdge` uses directional filtering** with `flip`/`tailLabel` logic, not just any crossing edge with min slack.

## Relevant Files

### Pipeline (modified this session):
- `crates/mermaid-core/src/layout/flowchart/sugiyama/mod.rs` — Main pipeline
- `crates/mermaid-core/src/layout/flowchart/sugiyama/rank_assignment.rs` — **Heavily modified**: NS init, feasible tree, pivot loop

### Pipeline (not modified this session, but relevant for next steps):
- `crates/mermaid-core/src/layout/flowchart/sugiyama/ordering.rs` — Flat barycenter ordering (to be removed in Step 2)
- `crates/mermaid-core/src/layout/flowchart/sugiyama/recursive_ordering.rs` — Recursive ordering (needs to work standalone)
- `crates/mermaid-core/src/layout/flowchart/sugiyama/coordinate_assignment.rs` — Position assignment (may need changes for Step 3)
- `crates/mermaid-core/src/layout/flowchart/sugiyama/nesting_graph.rs` — Nesting graph
- `crates/mermaid-core/src/layout/flowchart/sugiyama/border_segments.rs` — Border segments
- `crates/mermaid-core/src/layout/flowchart/sugiyama/dummy_nodes.rs` — Dummy node insertion
- `crates/mermaid-core/src/layout/flowchart/sugiyama/parent_dummy_chains.rs` — Parent dummy chains

### Tests:
- `crates/mermaid-core/src/layout/flowchart/mod.rs` — All subgraph layout tests
- `tests/coverage_e2e.rs` — 78 e2e tests

### Reference implementations:
- `../dagre/lib/rank/util.js` — `longestPath` (backward DFS)
- `../dagre/lib/rank/feasible-tree.js` — `feasibleTree` (uniform rank shifting)
- `../dagre/lib/rank/network-simplex.js` — NS with `leaveEdge` (first negative), `enterEdge` (directional)
- `../dagre/lib/order/index.js` — Main order() loop
- `../dagre/lib/order/sort-subgraph.js` — Compound sortSubgraph
- `../dagre/lib/order/build-layer-graph.js` — Compound layer graph construction
- `../dagre/lib/order/init-order.js` — DFS-based initial ordering
- `../dagre/lib/order/resolve-conflicts.js` — Constraint resolution
- `../dagre/lib/order/sort.js` — sortable/unsortable partitioning
- `../dagre/lib/order/cross-count.js` — Weighted crossing count
- `../dagre/lib/order/add-subgraph-constraints.js` — Constraint recording
