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

/// The line style of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineStyle {
    #[default]
    Solid,
    Dotted,
    Thick,
    Invisible,
}

/// The arrow/terminal type at one end of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArrowEnd {
    #[default]
    None,
    Arrow,
    Circle,
    Cross,
}

/// An edge connecting two nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeDef {
    pub from: String,
    pub to: String,
    pub line_style: LineStyle,
    pub arrow_start: ArrowEnd,
    pub arrow_end: ArrowEnd,
    pub label: Option<String>,
}

/// Legacy edge type enum — kept for backward compatibility with tests.
/// Maps to the decomposed (LineStyle, ArrowEnd) pair.
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

impl EdgeType {
    pub fn to_parts(self) -> (LineStyle, ArrowEnd) {
        match self {
            EdgeType::SolidArrow => (LineStyle::Solid, ArrowEnd::Arrow),
            EdgeType::SolidLine => (LineStyle::Solid, ArrowEnd::None),
            EdgeType::DottedArrow => (LineStyle::Dotted, ArrowEnd::Arrow),
            EdgeType::DottedLine => (LineStyle::Dotted, ArrowEnd::None),
            EdgeType::ThickArrow => (LineStyle::Thick, ArrowEnd::Arrow),
            EdgeType::ThickLine => (LineStyle::Thick, ArrowEnd::None),
        }
    }
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
