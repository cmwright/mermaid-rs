# Performance Optimization Project — mermaid-rs

## Baselines (Phase 0)

### dagre-rust standalone layout benchmarks
| Fixture | Time |
|---------|------|
| example2_reduced | 668 µs |
| example5_reduced | 10.1 ms |
| example7_reduced | 19.7 ms |

### dagre-rust per-stage profile (example5, 10ms avg)
| Stage | Time | % |
|-------|------|---|
| order (crossing minimization) | 3.5 ms | 34.6% |
| position (Brandes-Kopf) | 2.7 ms | 26.6% |
| rank_assign (network simplex) | 2.3 ms | 22.8% |
| add_border_segments | 0.32 ms | 3.2% |
| remove_border_nodes | 0.29 ms | 2.8% |
| everything else | ~1.0 ms | ~10% |

### mermaid-core flowchart end-to-end benchmarks
| Complexity | End-to-End | Layout | Parse | Render SVG |
|------------|-----------|--------|-------|------------|
| simple | 307 µs | 278 µs (91%) | 10 µs | 14 µs |
| medium | 1.82 ms | 1.73 ms (95%) | 39 µs | 56 µs |
| complex | 9.83 ms | 9.89 ms (>99%) | 105 µs | 100 µs |

**Key insight**: Layout dominates everything. For complex flowcharts, dagre layout is >99% of total time.

---

## Phase 1: Low-Hanging Fruit -- COMPLETE

Replace unnecessary allocations across dagre-rust:
- Replaced ~18 `g.nodes()` (Vec clone) with `g.node_ids()` / `.to_vec()` across all modules
- Replaced ~22 `g.edges()` (Vec<Edge> clone) with `g.edge_ids()` + by-ID access
- Replaced ~12 `.cloned().unwrap_or_default()` on NodeLabel with targeted field extraction
- Added `edge_label_mut_by_id()` for zero-copy edge mutation
- Changed AtomicUsize ordering from SeqCst to Relaxed

### Phase 1 Results
| dagre-rust | Before | After | Change |
|------------|--------|-------|--------|
| example2 | 668 µs | 301 µs | **-55%** |
| example5 | 10.1 ms | 9.3 ms | **-8%** |
| example7 | 19.7 ms | 12.6 ms | **-36%** |

---

## Phase 2a: ahash for Faster Hashing -- COMPLETE

Replaced `std::collections::HashMap` / `HashSet` with `ahash::AHashMap` / `AHashSet` across all dagre-rust modules. Default SipHash is DoS-resistant but slow; ahash is 2-3x faster for string keys.

### Phase 2a Results (cumulative from baseline)
| dagre-rust | Baseline | Phase 1 | + ahash | Total |
|------------|----------|---------|---------|-------|
| example2 | 668 µs | 301 µs | **231 µs** | **-65%** |
| example5 | 10.1 ms | 9.3 ms | **6.97 ms** | **-31%** |
| example7 | 19.7 ms | 12.6 ms | **9.05 ms** | **-54%** |

| flowchart layout | Baseline | + ahash | Total |
|-----------------|----------|---------|-------|
| simple | 278 µs | **220 µs** | **-21%** |
| medium | 1.73 ms | **1.32 ms** | **-24%** |
| complex | 9.89 ms | **7.47 ms** | **-24%** |

| flowchart e2e | Baseline | + ahash | Total |
|--------------|----------|---------|-------|
| simple | 307 µs | **251 µs** | **-18%** |
| medium | 1.82 ms | **1.45 ms** | **-20%** |
| complex | 9.83 ms | **7.73 ms** | **-21%** |

### Per-stage profile (example5, post all optimizations: 7.17ms avg)
| Stage | Before | After | Change |
|-------|--------|-------|--------|
| order | 3.5 ms | 2.6 ms | **-26%** |
| position | 2.7 ms | 1.8 ms | **-33%** |
| rank_assign | 2.3 ms | 1.7 ms | **-26%** |
| **total** | **10.1 ms** | **7.2 ms** | **-29%** |

All tests green: 20/20 dagre-rust parity, 77/77 e2e, 1/1 examples_comparison.

---

## Phase 3: Reduce Allocations and Graph Copies -- COMPLETE

Multiple targeted optimizations to reduce allocations and avoid expensive graph copies:

### Quick wins
- Added `for_each_neighbor()` to Graph for zero-alloc neighbor iteration
- Rewrote network simplex DFS traversals (pre/postorder, low/lim assignment) to use iterative stacks + `for_each_neighbor()` instead of allocating `Vec<String>` per node
- Removed unnecessary `NodeLabel::clone()` in `sort_subgraph` (just borrow instead)
- Switched `longest_path` DFS from `out_edges()` (clones all Edge objects) to `out_edge_ids()` + `edge_label_by_id()`
- Switched `successor_weights`/`predecessor_weights` from `in_edges()`/`out_edges()` to ID-based APIs
- Switched `init_order` DFS from `successors()` (allocates Vec) to `successor_map()` (borrows in place)

### Graph copy reduction
- `simplify()`: only copies `rank` field of NodeLabel instead of cloning all 25+ fields per node
- `as_non_compound_graph_for_rank()`: new lightweight variant used for ranking that skips full NodeLabel clones
- Ranking phase (`rank_assign`) dropped from 1.88ms → 1.30ms (-31%) on example5

### Phase 3 Results (cumulative from baseline)
| dagre-rust | Baseline | Phase 1+2a | Phase 3 | Total |
|------------|----------|-----------|---------|-------|
| example2 | 668 µs | 231 µs | **230 µs** | **-66%** |
| example5 | 10.1 ms | 6.97 ms | **6.52 ms** | **-35%** |
| example7 | 19.7 ms | 9.05 ms | **8.83 ms** | **-55%** |

| flowchart layout | Baseline | Phase 1+2a | Phase 3 | Total |
|-----------------|----------|-----------|---------|-------|
| simple | 278 µs | 220 µs | **204 µs** | **-27%** |
| medium | 1.73 ms | 1.32 ms | **1.23 ms** | **-29%** |
| complex | 9.89 ms | 7.47 ms | **6.73 ms** | **-32%** |

| flowchart e2e | Baseline | Phase 1+2a | Phase 3 | Total |
|--------------|----------|-----------|---------|-------|
| simple | 307 µs | 251 µs | **235 µs** | **-23%** |
| medium | 1.82 ms | 1.45 ms | **1.38 ms** | **-24%** |
| complex | 9.83 ms | 7.73 ms | **6.70 ms** | **-32%** |

### Per-stage profile (example5, post all optimizations: 6.9ms avg)
| Stage | Baseline | After Phase 2a | After Phase 3 | Change from 2a |
|-------|----------|---------------|---------------|----------------|
| order | 3.5 ms | 2.6 ms | 2.6 ms | ~0% |
| position | 2.7 ms | 1.8 ms | 1.9 ms | ~0% |
| rank_assign | 2.3 ms | 1.7 ms | **1.3 ms** | **-24%** |
| **total** | **10.1 ms** | **7.2 ms** | **6.9 ms** | **-4%** |

All tests green: 20/20 dagre-rust parity, 77/77 e2e, 1/1 examples_comparison.

---

## Phase 4: Sampling Profiler Analysis

### Methodology
Used macOS `sample` command (1ms sampling interval, ~4050 samples) on `profile_complex` binary running 500 iterations each of example5 and example7 fixtures in release mode with debug symbols.

### Self-time by category (where the CPU actually spends time)
| Category | % | Samples | Notes |
|----------|---|---------|-------|
| **malloc/free** | **35.4%** | 1357 | Heap alloc/dealloc from String clones, HashMap ops |
| **memcpy/memmove/memset/memcmp** | **20.7%** | 791 | Copying strings, HashMap rehashing |
| **ahash (string hashing)** | **15.0%** | 576 | Hashing String keys for every HashMap lookup |
| other system (kernel/dyld) | 4.9% | 188 | |
| bk::position_x | 3.0% | 115 | BK algorithm actual computation |
| hashbrown insert/rehash/drop | 1.7% | 65 | HashMap structural ops |
| Graph::set_node | 1.6% | 62 | 6+ string clones per node insert |
| String::clone | 1.5% | 58 | Explicit string cloning |
| network_simplex::init_cut_values | 1.0% | 40 | |
| edge_args_to_id | 0.8% | 29 | String concatenation for edge lookup |
| Graph::set_edge | 0.8% | 29 | |
| order::cross_count | 0.7% | 28 | |
| all other dagre functions | ~12% | ~460 | |

**Key insight**: 71% of CPU time is memory management and string hashing overhead. Only ~10% is actual algorithmic work. The `String` node ID system (ported from JavaScript's string-keyed objects) is the dominant bottleneck.

### Per-stage profile (example5, 6.6ms avg)
| Stage | Time | % |
|-------|------|---|
| order (crossing minimization) | 2.5 ms | 38% |
| position (Brandes-Kopf) | 1.8 ms | 28% |
| rank_assign (network simplex) | 1.2 ms | 19% |
| everything else | ~1.0 ms | 15% |

---

## Phase 4: Integer Node/Edge IDs — IN PROGRESS

Replace `String` node and edge identifiers with `u32` integer indices throughout the Graph data structure and all algorithm modules.

### What changes
- `Graph` internal storage: `HashMap<String, X>` → `Vec<X>` indexed by `u32`
- Node IDs: `String` → `NodeId(u32)` newtype
- Edge IDs: `String` → `EdgeId(u32)` newtype
- Edge lookup: string concatenation (`edge_args_to_id`) → `(NodeId, NodeId)` tuple key
- Algorithm-local data structures: `HashMap<String, String>` → `Vec<NodeId>` etc.

### Expected impact
- Eliminates ~35% malloc/free (integers are Copy, no heap allocation)
- Eliminates ~21% memcpy (no string copying, Vec<T> with Copy types)
- Eliminates ~15% ahash (Vec indexing instead of HashMap lookups)
- Estimated **2-5x overall speedup** on layout

### Files affected
- `graph.rs` — core data structure
- All algorithm modules (order/*, position/*, rank/*, util.rs, normalize.rs, etc.)
- `mermaid-core` dagre integration layer

---

## Future Work (after Phase 4)

**Incremental network simplex updates** (rank phase, est. 1.3-2x for ranking)
- `exchange_edges` re-initializes the entire spanning tree on every pivot (full DFS for low/lim values + full postorder for cut values)
- Only the affected subtree needs updating after a pivot — described in Gansner et al. paper
- Files: `rank/network_simplex.rs:exchange_edges`, `init_low_lim_values`, `init_cut_values`

**Lightweight layer graph type** (order phase, est. medium)
- `build_layer_graph` creates a full compound `LayoutGraph` per rank per sweep direction
- Only needs: node order, edge weight, parent/children — could use a much simpler struct
- Files: `order/build_layer_graph.rs`

**mermaid-core optimizations** (outside dagre, est. small for layout-heavy cases)
- Font caching, persistent TextMeasurer, avoid repeated font loading
- Files: `crates/mermaid-core/`
