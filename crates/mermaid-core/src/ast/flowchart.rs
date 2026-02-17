use super::common::StyleProperties;

/// The root AST for a flowchart diagram.
#[derive(Debug, Clone, Default)]
pub struct FlowchartAst {
    pub direction: Direction,
    pub nodes: Vec<NodeDef>,
    pub edges: Vec<EdgeDef>,
    pub subgraphs: Vec<SubgraphDef>,
    pub class_defs: Vec<ClassDef>,
    pub class_assignments: Vec<ClassAssignment>,
    pub style_overrides: Vec<StyleOverride>,
}

/// Graph direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    TopToBottom,
    BottomToTop,
    LeftToRight,
    RightToLeft,
}

/// A node declaration with its id, shape, and label.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeDef {
    pub id: String,
    pub label: Option<String>,
    pub shape: NodeShape,
    pub class_shorthand: Option<String>,
}

/// The shape of a node, determined by delimiter syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeShape {
    #[default]
    Rectangle,
    RoundedRectangle,
    Stadium,
    Subroutine,
    Cylinder,
    Circle,
    DoubleCircle,
    Diamond,
    Hexagon,
    Asymmetric,
    Parallelogram,
    ParallelogramAlt,
    Trapezoid,
    TrapezoidAlt,
}

/// An edge connecting two nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeDef {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

/// The visual type of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeType {
    #[default]
    SolidArrow,
    SolidLine,
    DottedArrow,
    DottedLine,
    ThickArrow,
    ThickLine,
}

/// A subgraph (group of nodes).
#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphDef {
    pub id: String,
    pub label: Option<String>,
    pub direction: Option<Direction>,
    pub nodes: Vec<NodeDef>,
    pub edges: Vec<EdgeDef>,
    pub subgraphs: Vec<SubgraphDef>,
}

/// A classDef declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub name: String,
    pub properties: StyleProperties,
}

/// A class assignment: `class nodeA,nodeB myClass`
#[derive(Debug, Clone, PartialEq)]
pub struct ClassAssignment {
    pub node_ids: Vec<String>,
    pub class_name: String,
}

/// An inline style override: `style nodeA fill:#f00`
#[derive(Debug, Clone, PartialEq)]
pub struct StyleOverride {
    pub node_id: String,
    pub properties: StyleProperties,
}
