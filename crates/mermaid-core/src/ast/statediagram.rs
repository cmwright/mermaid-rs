pub use super::flowchart::{ClassAssignment, ClassDef, Direction, StyleOverride};

/// The root AST for a state diagram.
#[derive(Debug, Clone, Default)]
pub struct StateDiagramAst {
    pub direction: Direction,
    pub states: Vec<StateDef>,
    pub transitions: Vec<TransitionDef>,
    pub composites: Vec<CompositeStateDef>,
    pub notes: Vec<NoteDef>,
    pub class_defs: Vec<ClassDef>,
    pub class_assignments: Vec<ClassAssignment>,
    pub style_overrides: Vec<StyleOverride>,
}

/// The kind/type of a state node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateKind {
    #[default]
    Normal,
    Start,
    End,
    Fork,
    Join,
    Choice,
}

/// A state definition.
#[derive(Debug, Clone, PartialEq)]
pub struct StateDef {
    pub id: String,
    pub label: Option<String>,
    pub kind: StateKind,
    pub class_shorthand: Option<String>,
}

/// A transition between states.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionDef {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

/// A composite (nested) state containing sub-states.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompositeStateDef {
    pub id: String,
    pub label: Option<String>,
    pub direction: Option<Direction>,
    pub states: Vec<StateDef>,
    pub transitions: Vec<TransitionDef>,
    pub composites: Vec<CompositeStateDef>,
    pub notes: Vec<NoteDef>,
    pub dividers: Vec<DividerDef>,
}

/// A concurrent region divider inside a composite state.
#[derive(Debug, Clone, PartialEq)]
pub struct DividerDef {
    pub id: String,
}

/// Note position relative to a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePosition {
    Left,
    Right,
}

/// A note attached to a state or floating.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteDef {
    pub id: Option<String>,
    pub target_state: Option<String>,
    pub position: Option<NotePosition>,
    pub text: String,
}
