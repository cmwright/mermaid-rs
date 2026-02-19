/// Shape of a mindmap node, determined by the delimiter syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MindmapNodeShape {
    #[default]
    Default,     // plain text, underline only
    Rect,        // [text]
    RoundedRect, // (text)
    Circle,      // ((text))
    Cloud,       // )text(
    Bang,        // ))text((
    Hexagon,     // {{text}}
}

/// A single node in the mindmap tree.
#[derive(Debug, Clone)]
pub struct MindmapNode {
    pub id: String,
    pub label: String,
    pub shape: MindmapNodeShape,
    pub children: Vec<MindmapNode>,
    pub icon: Option<String>,
    pub css_class: Option<String>,
}

/// Root AST for a mindmap diagram.
#[derive(Debug, Clone)]
pub struct MindmapAst {
    pub root: MindmapNode,
}
