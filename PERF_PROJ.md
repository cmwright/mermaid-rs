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

## Phase 2b: Vec-Indexed Graph (Considering)
Could further improve by replacing HashMap-based node/edge storage with Vec-based indexing.
Current assessment: ahash already captured much of the hashing overhead; remaining gains from Vec indexing may be 10-20%.

## Phase 3: Reduce Graph Copies (Planned)
Eliminate full graph copies in as_non_compound_graph, simplify, build_layer_graph.

## Phase 4: NodeLabel Diet (Planned)
Slim down the heavyweight NodeLabel struct.

## Phase 5: mermaid-core Optimizations (Planned)
Font caching, persistent TextMeasurer, etc.
