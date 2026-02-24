//! Typed label structs and enums for the dagre layout engine.
//!
//! These replace the untyped `serde_json::Value` labels that were used
//! in the original JS port.

use crate::graph::Edge;

// ============================================================================
// Enums
// ============================================================================

/// Direction of rank layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RankDir {
    #[default]
    TB,
    BT,
    LR,
    RL,
}

impl RankDir {
    /// Parse from a case-insensitive string, defaulting to TB.
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bt" => RankDir::BT,
            "lr" => RankDir::LR,
            "rl" => RankDir::RL,
            _ => RankDir::TB,
        }
    }

    pub fn is_lr_or_rl(self) -> bool {
        matches!(self, RankDir::LR | RankDir::RL)
    }

    pub fn is_bt_or_rl(self) -> bool {
        matches!(self, RankDir::BT | RankDir::RL)
    }
}

/// Type of dummy node inserted during layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DummyType {
    Edge,
    EdgeLabel,
    EdgeProxy,
    Border,
    SelfEdge,
    Root,
}

/// Position of an edge label relative to the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LabelPos {
    Left,
    #[default]
    Right,
    Center,
}

impl LabelPos {
    /// Parse from a case-insensitive string, defaulting to Right.
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "l" => LabelPos::Left,
            "c" => LabelPos::Center,
            _ => LabelPos::Right,
        }
    }
}

/// Ranking algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Ranker {
    #[default]
    NetworkSimplex,
    TightTree,
    LongestPath,
}

impl Ranker {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().replace('-', "_").as_str() {
            "tight_tree" => Ranker::TightTree,
            "longest_path" => Ranker::LongestPath,
            _ => Ranker::NetworkSimplex,
        }
    }
}

/// Cycle removal strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Acyclicer {
    Greedy,
}

/// Border type for compound graph border nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorderType {
    Left,
    Right,
}

/// Alignment option for the Brandes-Kopf position algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    UL,
    UR,
    DL,
    DR,
}

impl Align {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ul" => Some(Align::UL),
            "ur" => Some(Align::UR),
            "dl" => Some(Align::DL),
            "dr" => Some(Align::DR),
            _ => None,
        }
    }
}

/// Rank alignment within a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RankAlign {
    #[default]
    Center,
    Top,
    Bottom,
}

impl RankAlign {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "top" => RankAlign::Top,
            "bottom" => RankAlign::Bottom,
            _ => RankAlign::Center,
        }
    }
}

// ============================================================================
// Point
// ============================================================================

/// A 2D point used for edge routing and node positions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

// ============================================================================
// Self-edge record
// ============================================================================

/// A self-edge record stored temporarily on nodes during layout.
#[derive(Debug, Clone)]
pub struct SelfEdgeRecord {
    pub e: Edge,
    pub label: EdgeLabel,
}

// ============================================================================
// Graph label
// ============================================================================

/// Graph-level configuration and computed output.
#[derive(Debug, Clone)]
pub struct GraphLabel {
    // User-configurable options
    pub ranksep: f64,
    pub edgesep: f64,
    pub nodesep: f64,
    pub rankdir: RankDir,
    /// True when the user explicitly set rankdir (vs using the default).
    /// Needed to replicate a JS dagre case-sensitivity quirk in
    /// `makeSpaceForEdgeLabels`.
    pub rankdir_explicit: bool,
    pub align: Option<Align>,
    pub rankalign: RankAlign,
    pub marginx: f64,
    pub marginy: f64,
    pub acyclicer: Option<Acyclicer>,
    pub ranker: Ranker,

    // Computed output
    pub width: f64,
    pub height: f64,

    // Temporary internal state (used during layout, not part of public API)
    pub nesting_root: Option<String>,
    pub node_rank_factor: Option<i64>,
    pub dummy_chains: Vec<String>,
    pub max_rank: Option<i64>,

    // Layer-graph internal (only used in order/build_layer_graph.rs)
    pub root: Option<String>,
}

impl Default for GraphLabel {
    fn default() -> Self {
        GraphLabel {
            ranksep: 50.0,
            edgesep: 20.0,
            nodesep: 50.0,
            rankdir: RankDir::TB,
            rankdir_explicit: false,
            align: None,
            rankalign: RankAlign::Center,
            marginx: 0.0,
            marginy: 0.0,
            acyclicer: None,
            ranker: Ranker::NetworkSimplex,
            width: 0.0,
            height: 0.0,
            nesting_root: None,
            node_rank_factor: None,
            dummy_chains: Vec::new(),
            max_rank: None,
            root: None,
        }
    }
}

// ============================================================================
// Node label
// ============================================================================

/// Label for a node in the layout graph.
#[derive(Debug, Clone)]
pub struct NodeLabel {
    // Core geometry
    pub width: f64,
    pub height: f64,

    // Layout results (set during layout phases)
    pub rank: Option<i64>,
    pub order: Option<i64>,
    pub x: Option<f64>,
    pub y: Option<f64>,

    // Dummy node info
    pub dummy: Option<DummyType>,

    // Compound/subgraph border info
    pub border_top: Option<String>,
    pub border_bottom: Option<String>,
    /// Border left node IDs indexed by rank.
    pub border_left: Vec<Option<String>>,
    /// Border right node IDs indexed by rank.
    pub border_right: Vec<Option<String>>,
    pub border_type: Option<BorderType>,
    pub min_rank: Option<i64>,
    pub max_rank: Option<i64>,

    // Edge-related fields (on dummy nodes created during normalization)
    /// The original edge label, stored on "edge" and "edge-label" dummy nodes.
    pub edge_label: Option<Box<EdgeLabel>>,
    /// The original edge endpoints, stored on dummy chain nodes.
    pub edge_obj: Option<Edge>,
    /// Edge reference for edge-proxy and selfedge dummy nodes.
    pub e: Option<Edge>,
    /// Self-edge records temporarily stored on nodes.
    pub self_edges: Vec<SelfEdgeRecord>,
    /// The original self-edge label, stored on "selfedge" dummy nodes.
    pub self_edge_data: Option<EdgeLabel>,
    /// Label position on edge-label dummy nodes.
    pub label_pos: Option<LabelPos>,

    // Network simplex tree fields (on spanning tree nodes)
    pub low: Option<i64>,
    pub lim: Option<i64>,
    /// Parent in the spanning tree (not compound graph parent).
    pub parent_node: Option<String>,
}

impl Default for NodeLabel {
    fn default() -> Self {
        NodeLabel {
            width: 0.0,
            height: 0.0,
            rank: None,
            order: None,
            x: None,
            y: None,
            dummy: None,
            border_top: None,
            border_bottom: None,
            border_left: Vec::new(),
            border_right: Vec::new(),
            border_type: None,
            min_rank: None,
            max_rank: None,
            edge_label: None,
            edge_obj: None,
            e: None,
            self_edges: Vec::new(),
            self_edge_data: None,
            label_pos: None,
            low: None,
            lim: None,
            parent_node: None,
        }
    }
}

// ============================================================================
// Edge label
// ============================================================================

/// Label for an edge in the layout graph.
#[derive(Debug, Clone)]
pub struct EdgeLabel {
    // Core attributes
    pub weight: f64,
    pub minlen: f64,
    pub width: f64,
    pub height: f64,
    pub labeloffset: f64,
    pub labelpos: LabelPos,
    pub label_rank: Option<i64>,

    // Edge routing output (set during layout)
    pub points: Vec<Point>,
    pub x: Option<f64>,
    pub y: Option<f64>,

    // Acyclic reversal tracking
    pub reversed: bool,
    pub forward_name: Option<String>,

    // Nesting graph
    pub nesting_edge: bool,

    // Network simplex tree
    pub cutvalue: Option<f64>,
}

impl Default for EdgeLabel {
    fn default() -> Self {
        EdgeLabel {
            weight: 1.0,
            minlen: 1.0,
            width: 0.0,
            height: 0.0,
            labeloffset: 10.0,
            labelpos: LabelPos::Right,
            label_rank: None,
            points: Vec::new(),
            x: None,
            y: None,
            reversed: false,
            forward_name: None,
            nesting_edge: false,
            cutvalue: None,
        }
    }
}
