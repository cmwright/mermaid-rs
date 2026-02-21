use crate::ast::common::StyleProperties;
use crate::ast::flowchart::{ArrowEnd, Direction, LineStyle};
use crate::ast::statediagram::StateKind;

/// A positioned state node.
#[derive(Debug, Clone)]
pub struct PositionedState {
    pub id: String,
    pub label: String,
    pub kind: StateKind,
    pub style: StyleProperties,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A positioned transition (edge).
#[derive(Debug, Clone)]
pub struct PositionedTransition {
    pub from_id: String,
    pub to_id: String,
    pub line_style: LineStyle,
    pub arrow_end: ArrowEnd,
    pub label: Option<String>,
    pub label_x: Option<f64>,
    pub label_y: Option<f64>,
    pub label_width: Option<f64>,
    pub label_height: Option<f64>,
    pub points: Vec<(f64, f64)>,
    /// Pre-computed SVG path `d` attribute for edges that bypass basis curve
    /// smoothing (e.g. bowed bidirectional edges).
    pub raw_path_d: Option<String>,
}

/// A positioned composite state (container).
#[derive(Debug, Clone)]
pub struct PositionedComposite {
    pub id: String,
    pub label: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub style: StyleProperties,
}

/// A positioned note.
#[derive(Debug, Clone)]
pub struct PositionedNote {
    pub id: String,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The full positioned state diagram.
#[derive(Debug, Clone)]
pub struct PositionedStateDiagram {
    pub states: Vec<PositionedState>,
    pub transitions: Vec<PositionedTransition>,
    pub composites: Vec<PositionedComposite>,
    pub notes: Vec<PositionedNote>,
    pub width: f64,
    pub height: f64,
    pub direction: Direction,
}
