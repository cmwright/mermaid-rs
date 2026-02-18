use crate::ast::common::StyleProperties;
use crate::ast::flowchart::{Direction, EdgeType, NodeShape};

// ── Layout constants ────────────────────────────────────────

pub const NODE_PADDING_H: f64 = 12.0;
pub const NODE_PADDING_V: f64 = 8.0;
pub const MIN_NODE_WIDTH: f64 = 0.0;
pub const MIN_NODE_HEIGHT: f64 = 0.0;
pub const NODE_SEP: f64 = 50.0;
pub const RANK_SEP: f64 = 80.0;
pub const SUBGRAPH_PADDING: f64 = 8.0;
pub const SUBGRAPH_TITLE_HEIGHT: f64 = 18.0;
pub const SUBGRAPH_GROUP_GAP: f64 = 20.0;

// ── Positioned types (public API) ───────────────────────────

#[derive(Debug, Clone)]
pub struct PositionedNode {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
    pub style: StyleProperties,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct PositionedEdge {
    pub from_id: String,
    pub to_id: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
    pub label_x: Option<f64>,
    pub label_y: Option<f64>,
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct PositionedSubgraph {
    pub id: String,
    pub label: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub style: StyleProperties,
}

#[derive(Debug, Clone)]
pub struct PositionedGraph {
    pub nodes: Vec<PositionedNode>,
    pub edges: Vec<PositionedEdge>,
    pub subgraphs: Vec<PositionedSubgraph>,
    pub width: f64,
    pub height: f64,
    pub direction: Direction,
}

// ── Internal graph node/edge data ───────────────────────────

#[derive(Debug, Clone)]
pub struct NodeData {
    pub id: String,
    pub label: String,
    pub shape: NodeShape,
    pub style: StyleProperties,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct EdgeData {
    pub label: Option<String>,
    pub edge_type: EdgeType,
}
