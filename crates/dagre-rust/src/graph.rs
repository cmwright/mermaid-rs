//! Graph data structure mirroring @dagrejs/graphlib.
//!
//! Supports:
//! - Directed/undirected graphs
//! - Multigraph (multiple named edges between same nodes)
//! - Compound graphs (parent/children hierarchy)
//! - Generic node/edge/graph labels with Default + Clone bounds
//! - Insertion-order iteration for nodes and edges

use ahash::AHashMap as HashMap;
use indexmap::IndexMap;

/// Sentinel value for the root of a compound graph (mirrors JS `"\0"`).
const GRAPH_NODE: &str = "\0";

/// Default edge name when none is specified (mirrors JS `"\0"`).
const DEFAULT_EDGE_NAME: &str = "\0";

/// Delimiter used in edge ID strings (mirrors JS `"\u{0001}"`).
const EDGE_KEY_DELIM: &str = "\x01";

/// An edge object, analogous to the frozen `{v, w, name?}` objects in JS graphlib.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    pub v: String,
    pub w: String,
    pub name: Option<String>,
}

impl Edge {
    pub fn new(v: &str, w: &str, name: Option<&str>) -> Self {
        Edge {
            v: v.to_string(),
            w: w.to_string(),
            name: name.map(|s| s.to_string()),
        }
    }
}

/// Options for constructing a Graph, mirroring JS `{ directed?, multigraph?, compound? }`.
#[derive(Debug, Clone)]
pub struct GraphOptions {
    pub directed: bool,
    pub multigraph: bool,
    pub compound: bool,
}

impl Default for GraphOptions {
    fn default() -> Self {
        GraphOptions {
            directed: true,
            multigraph: false,
            compound: false,
        }
    }
}

/// A graph data structure mirroring `@dagrejs/graphlib`'s Graph class.
///
/// Generic over:
/// - `N`: Node label type (default: `serde_json::Value`)
/// - `E`: Edge label type (default: `serde_json::Value`)
/// - `G`: Graph-level label type (default: `serde_json::Value`)
///
/// All label types must implement `Default` and `Clone`.
#[derive(Debug, Clone)]
pub struct Graph<
    N: Default + Clone = serde_json::Value,
    E: Default + Clone = serde_json::Value,
    G: Default + Clone = serde_json::Value,
> {
    is_directed: bool,
    is_multigraph: bool,
    is_compound: bool,

    /// Graph-level label
    label: G,

    // Node storage: insertion-ordered keys + label map
    node_order: Vec<String>,
    nodes: HashMap<String, Option<N>>,

    // Edge storage: insertion-ordered edge IDs + label/obj maps
    edge_order: Vec<String>,
    edge_objs: HashMap<String, Edge>,
    edge_labels: HashMap<String, E>,

    // Adjacency: v -> { edgeId -> Edge }
    in_edges: HashMap<String, Vec<String>>,
    out_edges: HashMap<String, Vec<String>>,

    // Predecessor/successor counts: v -> { u -> count }
    preds: HashMap<String, IndexMap<String, usize>>,
    sucs: HashMap<String, IndexMap<String, usize>>,

    // Compound graph support
    parent: Option<HashMap<String, String>>,
    children: Option<HashMap<String, Vec<String>>>,

    node_count: usize,
    edge_count: usize,
}

impl<N: Default + Clone, E: Default + Clone, G: Default + Clone> Graph<N, E, G> {
    /// Creates a new graph with default options (directed, not multigraph, not compound).
    pub fn new() -> Self {
        Self::with_options(&GraphOptions::default())
    }

    /// Creates a new graph with the given options.
    pub fn with_options(opts: &GraphOptions) -> Self {
        let (parent, children) = if opts.compound {
            let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
            children_map.insert(GRAPH_NODE.to_string(), Vec::new());
            (Some(HashMap::new()), Some(children_map))
        } else {
            (None, None)
        };

        Graph {
            is_directed: opts.directed,
            is_multigraph: opts.multigraph,
            is_compound: opts.compound,
            label: G::default(),
            node_order: Vec::new(),
            nodes: HashMap::new(),
            edge_order: Vec::new(),
            edge_objs: HashMap::new(),
            edge_labels: HashMap::new(),
            in_edges: HashMap::new(),
            out_edges: HashMap::new(),
            preds: HashMap::new(),
            sucs: HashMap::new(),
            parent,
            children,
            node_count: 0,
            edge_count: 0,
        }
    }

    // === Graph functions ===

    pub fn is_directed(&self) -> bool {
        self.is_directed
    }

    pub fn is_multigraph(&self) -> bool {
        self.is_multigraph
    }

    pub fn is_compound(&self) -> bool {
        self.is_compound
    }

    pub fn set_graph(&mut self, label: G) -> &mut Self {
        self.label = label;
        self
    }

    pub fn graph(&self) -> &G {
        &self.label
    }

    pub fn graph_mut(&mut self) -> &mut G {
        &mut self.label
    }

    // === Node functions ===

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Returns nodes in insertion order (cloned).
    pub fn nodes(&self) -> Vec<String> {
        self.node_order.clone()
    }

    /// Returns node IDs as references (avoids cloning).
    pub fn node_ids(&self) -> &[String] {
        &self.node_order
    }

    /// Nodes with no in-edges.
    pub fn sources(&self) -> Vec<String> {
        self.node_order
            .iter()
            .filter(|v| self.in_edges.get(*v).is_none_or(|edges| edges.is_empty()))
            .cloned()
            .collect()
    }

    /// Nodes with no out-edges.
    pub fn sinks(&self) -> Vec<String> {
        self.node_order
            .iter()
            .filter(|v| self.out_edges.get(*v).is_none_or(|edges| edges.is_empty()))
            .cloned()
            .collect()
    }

    /// Set node with a label. If node exists, updates label only if value is Some.
    /// Passing `None` creates a node without a label (mirrors JS `setNode(v)` with undefined).
    pub fn set_node(&mut self, v: &str, value: Option<N>) -> &mut Self {
        if let Some(existing) = self.nodes.get_mut(v) {
            if let Some(val) = value {
                *existing = Some(val);
            }
            return self;
        }

        self.nodes.insert(v.to_string(), value);
        self.node_order.push(v.to_string());

        if self.is_compound {
            if let Some(ref mut parent) = self.parent {
                parent.insert(v.to_string(), GRAPH_NODE.to_string());
            }
            if let Some(ref mut children) = self.children {
                children.insert(v.to_string(), Vec::new());
                if let Some(root_children) = children.get_mut(GRAPH_NODE) {
                    root_children.push(v.to_string());
                }
            }
        }

        self.in_edges.insert(v.to_string(), Vec::new());
        self.preds.insert(v.to_string(), IndexMap::new());
        self.out_edges.insert(v.to_string(), Vec::new());
        self.sucs.insert(v.to_string(), IndexMap::new());
        self.node_count += 1;
        self
    }

    /// Convenience: set node without a label (uses Default).
    pub fn set_node_no_label(&mut self, v: &str) -> &mut Self {
        self.set_node(v, None)
    }

    /// Get node label. Returns None if node doesn't exist OR has no label.
    pub fn node(&self, v: &str) -> Option<&N> {
        self.nodes.get(v).and_then(|opt| opt.as_ref())
    }

    /// Get mutable node label.
    pub fn node_mut(&mut self, v: &str) -> Option<&mut N> {
        self.nodes.get_mut(v).and_then(|opt| opt.as_mut())
    }

    pub fn has_node(&self, v: &str) -> bool {
        self.nodes.contains_key(v)
    }

    /// Remove a node and all incident edges.
    pub fn remove_node(&mut self, v: &str) -> &mut Self {
        if !self.nodes.contains_key(v) {
            return self;
        }

        self.nodes.remove(v);
        self.node_order.retain(|n| n != v);

        if self.is_compound {
            self.remove_from_parents_child_list(v);
            if let Some(ref mut parent_map) = self.parent {
                parent_map.remove(v);
            }
            let children_of_v: Vec<String> = self
                .children
                .as_ref()
                .and_then(|c| c.get(v))
                .cloned()
                .unwrap_or_default();
            for child in children_of_v {
                self.set_parent(&child, None);
            }
            if let Some(ref mut children) = self.children {
                children.remove(v);
            }
        }

        let in_edge_ids: Vec<String> = self.in_edges.get(v).cloned().unwrap_or_default();
        for e_id in &in_edge_ids {
            if let Some(edge) = self.edge_objs.get(e_id).cloned() {
                self.remove_edge_by_obj(&edge);
            }
        }
        self.in_edges.remove(v);
        self.preds.remove(v);

        let out_edge_ids: Vec<String> = self.out_edges.get(v).cloned().unwrap_or_default();
        for e_id in &out_edge_ids {
            if let Some(edge) = self.edge_objs.get(e_id).cloned() {
                self.remove_edge_by_obj(&edge);
            }
        }
        self.out_edges.remove(v);
        self.sucs.remove(v);

        self.node_count -= 1;
        self
    }

    // === Compound graph functions ===

    pub fn set_parent(&mut self, v: &str, parent: Option<&str>) -> &mut Self {
        if !self.is_compound {
            panic!("Cannot set parent in a non-compound graph");
        }

        let parent_str = match parent {
            None => GRAPH_NODE.to_string(),
            Some(p) => {
                let mut ancestor: Option<String> = Some(p.to_string());
                while let Some(ref a) = ancestor {
                    if a == v {
                        panic!("Setting {} as parent of {} would create a cycle", p, v);
                    }
                    ancestor = self.parent(a).map(|s| s.to_string());
                }
                self.set_node(p, None);
                p.to_string()
            }
        };

        self.set_node(v, None);
        self.remove_from_parents_child_list(v);

        if let Some(ref mut parent_map) = self.parent {
            parent_map.insert(v.to_string(), parent_str.clone());
        }
        if let Some(ref mut children) = self.children {
            children
                .entry(parent_str)
                .or_insert_with(Vec::new)
                .push(v.to_string());
        }
        self
    }

    fn remove_from_parents_child_list(&mut self, v: &str) {
        let parent_of_v = self.parent.as_ref().and_then(|p| p.get(v)).cloned();
        if let Some(parent_key) = parent_of_v
            && let Some(ref mut children) = self.children
            && let Some(siblings) = children.get_mut(&parent_key)
        {
            siblings.retain(|c| c != v);
        }
    }

    /// Returns the parent of node v, or None if v is a root node.
    pub fn parent(&self, v: &str) -> Option<&str> {
        if self.is_compound
            && let Some(ref parent_map) = self.parent
            && let Some(p) = parent_map.get(v)
            && p != GRAPH_NODE
        {
            return Some(p.as_str());
        }
        None
    }

    /// Returns direct children of node v. If v is None, returns root-level children.
    pub fn children(&self, v: Option<&str>) -> Option<Vec<String>> {
        let v = v.unwrap_or(GRAPH_NODE);
        if self.is_compound {
            self.children.as_ref().and_then(|c| c.get(v)).cloned()
        } else if v == GRAPH_NODE {
            Some(self.nodes())
        } else if self.has_node(v) {
            Some(Vec::new())
        } else {
            None
        }
    }

    /// Returns direct children by reference (compound graphs only).
    /// Avoids cloning child IDs in hot traversal paths.
    pub fn children_ids(&self, v: Option<&str>) -> Option<&[String]> {
        if !self.is_compound {
            return None;
        }
        let v = v.unwrap_or(GRAPH_NODE);
        self.children
            .as_ref()
            .and_then(|c| c.get(v))
            .map(Vec::as_slice)
    }

    // === Predecessor/Successor/Neighbor functions ===

    pub fn predecessors(&self, v: &str) -> Option<Vec<String>> {
        self.preds.get(v).map(|m| m.keys().cloned().collect())
    }

    /// Returns predecessor count map by reference (avoids cloning keys).
    pub fn predecessor_map(&self, v: &str) -> Option<&IndexMap<String, usize>> {
        self.preds.get(v)
    }

    pub fn successors(&self, v: &str) -> Option<Vec<String>> {
        self.sucs.get(v).map(|m| m.keys().cloned().collect())
    }

    /// Returns successor count map by reference (avoids cloning keys).
    pub fn successor_map(&self, v: &str) -> Option<&IndexMap<String, usize>> {
        self.sucs.get(v)
    }

    pub fn neighbors(&self, v: &str) -> Option<Vec<String>> {
        self.predecessors(v).map(|preds| {
            let mut seen = ahash::AHashSet::new();
            let mut result = Vec::new();
            for p in &preds {
                if seen.insert(p.clone()) {
                    result.push(p.clone());
                }
            }
            if let Some(succs) = self.successors(v) {
                for s in succs {
                    if seen.insert(s.clone()) {
                        result.push(s);
                    }
                }
            }
            result
        })
    }

    pub fn is_leaf(&self, v: &str) -> bool {
        let neighbors = if self.is_directed {
            self.successors(v)
        } else {
            self.neighbors(v)
        };
        neighbors.is_some_and(|n| n.is_empty())
    }

    /// Creates a new graph with nodes filtered by the predicate.
    pub fn filter_nodes<F>(&self, filter: F) -> Graph<N, E, G>
    where
        F: Fn(&str) -> bool,
    {
        let mut copy = Graph::with_options(&GraphOptions {
            directed: self.is_directed,
            multigraph: self.is_multigraph,
            compound: self.is_compound,
        });
        copy.set_graph(self.label.clone());

        for v in &self.node_order {
            if filter(v) {
                copy.set_node(v, self.nodes[v].clone());
            }
        }

        for e_id in &self.edge_order {
            if let Some(edge) = self.edge_objs.get(e_id)
                && copy.has_node(&edge.v) && copy.has_node(&edge.w)
            {
                let label = self.edge_labels.get(e_id).cloned().unwrap_or_default();
                copy.set_edge_with_obj(edge, Some(label));
            }
        }

        if self.is_compound {
            let mut parents_cache: HashMap<String, Option<String>> = HashMap::new();
            for v in copy.nodes() {
                let parent = self.find_parent_for_filter(&v, &copy, &mut parents_cache);
                copy.set_parent(&v, parent.as_deref());
            }
        }

        copy
    }

    fn find_parent_for_filter(
        &self,
        v: &str,
        copy: &Graph<N, E, G>,
        cache: &mut HashMap<String, Option<String>>,
    ) -> Option<String> {
        let parent = self.parent(v);
        match parent {
            None => None,
            Some(p) => {
                if copy.has_node(p) {
                    return Some(p.to_string());
                }
                if let Some(cached) = cache.get(p) {
                    return cached.clone();
                }
                let result = self.find_parent_for_filter(p, copy, cache);
                cache.insert(p.to_string(), result.clone());
                result
            }
        }
    }

    // === Edge functions ===

    pub fn edge_count(&self) -> usize {
        self.edge_count
    }

    /// Returns all edges in insertion order.
    pub fn edges(&self) -> Vec<Edge> {
        self.edge_order
            .iter()
            .filter_map(|e_id| self.edge_objs.get(e_id).cloned())
            .collect()
    }

    /// Returns edge IDs in insertion order (no edge cloning).
    pub fn edge_ids(&self) -> &[String] {
        &self.edge_order
    }

    /// Returns edge object by edge ID (no edge cloning).
    pub fn edge_obj_by_id(&self, edge_id: &str) -> Option<&Edge> {
        self.edge_objs.get(edge_id)
    }

    /// Returns edge label by edge ID (no edge cloning).
    pub fn edge_label_by_id(&self, edge_id: &str) -> Option<&E> {
        self.edge_labels.get(edge_id)
    }

    /// Returns mutable edge label by edge ID (no edge cloning).
    pub fn edge_label_mut_by_id(&mut self, edge_id: &str) -> Option<&mut E> {
        self.edge_labels.get_mut(edge_id)
    }

    /// Set an edge by (v, w) with optional name and label.
    pub fn set_edge(
        &mut self,
        v: &str,
        w: &str,
        value: Option<E>,
        name: Option<&str>,
    ) -> &mut Self {
        let (ev, ew) = if !self.is_directed && v > w {
            (w, v)
        } else {
            (v, w)
        };
        let e = edge_args_to_id(self.is_directed, ev, ew, name);

        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            self.edge_labels.entry(e.clone())
        {
            if let Some(val) = value {
                entry.insert(val);
            }
            return self;
        }

        if name.is_some() && !self.is_multigraph {
            panic!("Cannot set a named edge when isMultigraph = false");
        }

        self.set_node(ev, None);
        self.set_node(ew, None);

        let label = value.unwrap_or_default();
        self.edge_labels.insert(e.clone(), label);

        let edge_obj = Edge {
            v: ev.to_string(),
            w: ew.to_string(),
            name: name.map(|s| s.to_string()),
        };
        let ev = edge_obj.v.clone();
        let ew = edge_obj.w.clone();

        self.edge_objs.insert(e.clone(), edge_obj);
        self.edge_order.push(e.clone());

        increment_or_init_entry(self.preds.get_mut(&ew).unwrap(), &ev);
        increment_or_init_entry(self.sucs.get_mut(&ev).unwrap(), &ew);

        self.in_edges.get_mut(&ew).unwrap().push(e.clone());
        self.out_edges.get_mut(&ev).unwrap().push(e);

        self.edge_count += 1;
        self
    }

    /// Set an edge using an Edge object.
    pub fn set_edge_with_obj(&mut self, edge: &Edge, value: Option<E>) -> &mut Self {
        self.set_edge(&edge.v, &edge.w, value, edge.name.as_deref())
    }

    /// Get edge label by (v, w, name).
    pub fn edge(&self, v: &str, w: &str, name: Option<&str>) -> Option<&E> {
        let e = edge_args_to_id(self.is_directed, v, w, name);
        self.edge_labels.get(&e)
    }

    /// Get edge label by Edge object.
    pub fn edge_by_obj(&self, edge: &Edge) -> Option<&E> {
        self.edge(&edge.v, &edge.w, edge.name.as_deref())
    }

    /// Get mutable edge label by (v, w, name).
    pub fn edge_mut(&mut self, v: &str, w: &str, name: Option<&str>) -> Option<&mut E> {
        let e = edge_args_to_id(self.is_directed, v, w, name);
        self.edge_labels.get_mut(&e)
    }

    /// Get mutable edge label by Edge object.
    pub fn edge_mut_by_obj(&mut self, edge: &Edge) -> Option<&mut E> {
        let e = edge_args_to_id(self.is_directed, &edge.v, &edge.w, edge.name.as_deref());
        self.edge_labels.get_mut(&e)
    }

    pub fn has_edge(&self, v: &str, w: &str, name: Option<&str>) -> bool {
        let e = edge_args_to_id(self.is_directed, v, w, name);
        self.edge_labels.contains_key(&e)
    }

    pub fn has_edge_obj(&self, edge: &Edge) -> bool {
        self.has_edge(&edge.v, &edge.w, edge.name.as_deref())
    }

    /// Remove edge by (v, w, name).
    pub fn remove_edge(&mut self, v: &str, w: &str, name: Option<&str>) -> &mut Self {
        let e = edge_args_to_id(self.is_directed, v, w, name);
        self.remove_edge_by_id(&e)
    }

    /// Remove edge by Edge object.
    pub fn remove_edge_by_obj(&mut self, edge: &Edge) -> &mut Self {
        self.remove_edge(&edge.v, &edge.w, edge.name.as_deref())
    }

    fn remove_edge_by_id(&mut self, e: &str) -> &mut Self {
        if let Some(edge) = self.edge_objs.remove(e) {
            let v = &edge.v;
            let w = &edge.w;

            self.edge_labels.remove(e);
            self.edge_order.retain(|eid| eid != e);

            decrement_or_remove_entry(self.preds.get_mut(w).unwrap(), v);
            decrement_or_remove_entry(self.sucs.get_mut(v).unwrap(), w);

            if let Some(in_e) = self.in_edges.get_mut(w) {
                in_e.retain(|eid| eid != e);
            }
            if let Some(out_e) = self.out_edges.get_mut(v) {
                out_e.retain(|eid| eid != e);
            }

            self.edge_count -= 1;
        }
        self
    }

    /// In-edges for node v, optionally from node u.
    pub fn in_edges(&self, v: &str, u: Option<&str>) -> Option<Vec<Edge>> {
        if self.is_directed {
            self.filter_edges_from_map(&self.in_edges, v, u)
        } else {
            self.node_edges(v, u)
        }
    }

    /// Out-edges for node v, optionally to node w.
    pub fn out_edges(&self, v: &str, w: Option<&str>) -> Option<Vec<Edge>> {
        if self.is_directed {
            self.filter_edges_from_map(&self.out_edges, v, w)
        } else {
            self.node_edges(v, w)
        }
    }

    /// In-edge IDs for node v (directed graphs only), without cloning edge objects.
    pub fn in_edge_ids(&self, v: &str) -> Option<&[String]> {
        if self.is_directed {
            self.in_edges.get(v).map(Vec::as_slice)
        } else {
            None
        }
    }

    /// Out-edge IDs for node v (directed graphs only), without cloning edge objects.
    pub fn out_edge_ids(&self, v: &str) -> Option<&[String]> {
        if self.is_directed {
            self.out_edges.get(v).map(Vec::as_slice)
        } else {
            None
        }
    }

    /// All edges incident to node v, optionally filtered to node w.
    pub fn node_edges(&self, v: &str, w: Option<&str>) -> Option<Vec<Edge>> {
        if !self.nodes.contains_key(v) {
            return None;
        }
        let mut merged: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if let Some(in_e) = self.in_edges.get(v) {
            for eid in in_e {
                if seen.insert(eid.clone()) {
                    merged.push(eid.clone());
                }
            }
        }
        if let Some(out_e) = self.out_edges.get(v) {
            for eid in out_e {
                if seen.insert(eid.clone()) {
                    merged.push(eid.clone());
                }
            }
        }

        let edges: Vec<Edge> = merged
            .iter()
            .filter_map(|eid| self.edge_objs.get(eid).cloned())
            .collect();

        match w {
            None => Some(edges),
            Some(w_str) => Some(
                edges
                    .into_iter()
                    .filter(|e| (e.v == v && e.w == w_str) || (e.v == w_str && e.w == v))
                    .collect(),
            ),
        }
    }

    fn filter_edges_from_map(
        &self,
        map: &HashMap<String, Vec<String>>,
        v: &str,
        remote: Option<&str>,
    ) -> Option<Vec<Edge>> {
        let edge_ids = map.get(v)?;
        let edges: Vec<Edge> = edge_ids
            .iter()
            .filter_map(|eid| self.edge_objs.get(eid).cloned())
            .collect();

        match remote {
            None => Some(edges),
            Some(r) => Some(
                edges
                    .into_iter()
                    .filter(|e| (e.v == v && e.w == r) || (e.v == r && e.w == v))
                    .collect(),
            ),
        }
    }

    // === Utility methods used by dagre ===

    /// Set path: connect nodes in sequence.
    pub fn set_path(&mut self, vs: &[&str], value: Option<E>) -> &mut Self {
        for window in vs.windows(2) {
            self.set_edge(window[0], window[1], value.clone(), None);
        }
        self
    }

    /// Batch set nodes.
    pub fn set_nodes(&mut self, vs: &[&str], value: Option<N>) -> &mut Self {
        for v in vs {
            self.set_node(v, value.clone());
        }
        self
    }
}

impl<N: Default + Clone, E: Default + Clone, G: Default + Clone> Default for Graph<N, E, G> {
    fn default() -> Self {
        Self::new()
    }
}

// === Helper functions ===

fn increment_or_init_entry(map: &mut IndexMap<String, usize>, k: &str) {
    if let Some(entry) = map.get_mut(k) {
        *entry += 1;
    } else {
        map.insert(k.to_string(), 1);
    }
}

fn decrement_or_remove_entry(map: &mut IndexMap<String, usize>, k: &str) {
    if let Some(count) = map.get_mut(k) {
        *count -= 1;
        if *count == 0 {
            map.shift_remove(k);
        }
    }
}

fn edge_args_to_id(is_directed: bool, v: &str, w: &str, name: Option<&str>) -> String {
    let (v, w) = if !is_directed && v > w {
        (w, v)
    } else {
        (v, w)
    };
    let name_part = name.unwrap_or(DEFAULT_EDGE_NAME);
    let mut id =
        String::with_capacity(v.len() + w.len() + name_part.len() + EDGE_KEY_DELIM.len() * 2);
    id.push_str(v);
    id.push_str(EDGE_KEY_DELIM);
    id.push_str(w);
    id.push_str(EDGE_KEY_DELIM);
    id.push_str(name_part);
    id
}

/// Compute edge ID from an Edge object.
pub fn edge_obj_to_id(is_directed: bool, edge: &Edge) -> String {
    edge_args_to_id(is_directed, &edge.v, &edge.w, edge.name.as_deref())
}

/// Type alias for the layout graph used by the dagre layout engine.
pub type LayoutGraph =
    Graph<crate::types::NodeLabel, crate::types::EdgeLabel, crate::types::GraphLabel>;

/// Type alias for constraint graphs used in ordering (no labels needed).
pub type ConstraintGraph = Graph<(), (), ()>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_node_operations() {
        let mut g: Graph = Graph::new();
        g.set_node("a", Some(json!(1)));
        g.set_node("b", Some(json!(2)));

        assert!(g.has_node("a"));
        assert!(!g.has_node("c"));
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.node("a"), Some(&json!(1)));
        assert_eq!(g.nodes(), vec!["a", "b"]);

        g.remove_node("a");
        assert!(!g.has_node("a"));
        assert_eq!(g.node_count(), 1);
    }

    #[test]
    fn test_basic_edge_operations() {
        let mut g: Graph = Graph::new();
        g.set_edge("a", "b", Some(json!({"weight": 5})), None);

        assert!(g.has_edge("a", "b", None));
        assert!(!g.has_edge("b", "a", None));
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.edge("a", "b", None), Some(&json!({"weight": 5})));

        assert!(g.has_node("a"));
        assert!(g.has_node("b"));

        g.remove_edge("a", "b", None);
        assert!(!g.has_edge("a", "b", None));
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn test_directed_in_out_edges() {
        let mut g: Graph = Graph::new();
        g.set_edge("a", "b", None, None);
        g.set_edge("c", "b", None, None);
        g.set_edge("b", "d", None, None);

        let in_e = g.in_edges("b", None).unwrap();
        assert_eq!(in_e.len(), 2);

        let out_e = g.out_edges("b", None).unwrap();
        assert_eq!(out_e.len(), 1);
        assert_eq!(out_e[0].w, "d");

        let in_from_a = g.in_edges("b", Some("a")).unwrap();
        assert_eq!(in_from_a.len(), 1);
    }

    #[test]
    fn test_undirected_graph() {
        let mut g: Graph = Graph::with_options(&GraphOptions {
            directed: false,
            multigraph: false,
            compound: false,
        });
        g.set_edge("b", "a", Some(json!(1)), None);

        assert!(g.has_edge("a", "b", None));
        assert!(g.has_edge("b", "a", None));
        assert_eq!(g.edge("a", "b", None), Some(&json!(1)));
    }

    #[test]
    fn test_multigraph() {
        let mut g: Graph = Graph::with_options(&GraphOptions {
            directed: true,
            multigraph: true,
            compound: false,
        });
        g.set_edge("a", "b", Some(json!(1)), Some("x"));
        g.set_edge("a", "b", Some(json!(2)), Some("y"));

        assert_eq!(g.edge_count(), 2);
        assert_eq!(g.edge("a", "b", Some("x")), Some(&json!(1)));
        assert_eq!(g.edge("a", "b", Some("y")), Some(&json!(2)));
    }

    #[test]
    fn test_compound_graph() {
        let mut g: Graph = Graph::with_options(&GraphOptions {
            directed: true,
            multigraph: false,
            compound: true,
        });
        g.set_node("a", None);
        g.set_node("b", None);
        g.set_parent("b", Some("a"));

        assert_eq!(g.parent("b"), Some("a"));
        assert_eq!(g.parent("a"), None);
        assert_eq!(g.children(Some("a")).unwrap(), vec!["b"]);
    }

    #[test]
    fn test_predecessors_successors() {
        let mut g: Graph = Graph::new();
        g.set_edge("a", "b", None, None);
        g.set_edge("c", "b", None, None);

        let preds = g.predecessors("b").unwrap();
        assert!(preds.contains(&"a".to_string()));
        assert!(preds.contains(&"c".to_string()));

        let succs = g.successors("a").unwrap();
        assert_eq!(succs, vec!["b"]);
    }

    #[test]
    fn test_sources_sinks() {
        let mut g: Graph = Graph::new();
        g.set_edge("a", "b", None, None);
        g.set_edge("b", "c", None, None);

        assert_eq!(g.sources(), vec!["a"]);
        assert_eq!(g.sinks(), vec!["c"]);
    }

    #[test]
    fn test_node_edges() {
        let mut g: Graph = Graph::new();
        g.set_edge("a", "b", None, None);
        g.set_edge("b", "c", None, None);

        let edges = g.node_edges("b", None).unwrap();
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_remove_node_removes_edges() {
        let mut g: Graph = Graph::new();
        g.set_edge("a", "b", None, None);
        g.set_edge("b", "c", None, None);

        g.remove_node("b");
        assert_eq!(g.edge_count(), 0);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn test_set_path() {
        let mut g: Graph = Graph::new();
        g.set_path(&["a", "b", "c", "d"], None);

        assert_eq!(g.node_count(), 4);
        assert_eq!(g.edge_count(), 3);
        assert!(g.has_edge("a", "b", None));
        assert!(g.has_edge("b", "c", None));
        assert!(g.has_edge("c", "d", None));
    }
}
