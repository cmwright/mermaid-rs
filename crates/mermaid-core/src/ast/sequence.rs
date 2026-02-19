/// Top-level AST for a sequence diagram.
#[derive(Debug, Clone, Default)]
pub struct SequenceAst {
    pub participants: Vec<ParticipantDef>,
    pub statements: Vec<SequenceStatement>,
    pub autonumber: bool,
}

/// Explicit participant/actor declaration.
#[derive(Debug, Clone)]
pub struct ParticipantDef {
    pub id: String,
    pub display_name: Option<String>,
    pub kind: ParticipantKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantKind {
    Participant,
    Actor,
}

/// A statement within the sequence body.
#[derive(Debug, Clone)]
pub enum SequenceStatement {
    Message(MessageDef),
    Activate(String),
    Deactivate(String),
    Block(BlockDef),
    Note(NoteDef),
}

/// A message arrow between two participants.
#[derive(Debug, Clone)]
pub struct MessageDef {
    pub from: String,
    pub to: String,
    pub arrow: ArrowType,
    pub label: String,
    pub activate_target: bool,
    pub deactivate_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowType {
    SolidArrow,  // ->>
    DottedArrow, // -->>
    SolidOpen,   // ->
    DottedOpen,  // -->
    SolidParen,  // -)
    DottedParen, // --)
    SolidCross,  // -x
    DottedCross, // --x
}

/// A block (alt/loop/opt/par/critical/break/rect).
#[derive(Debug, Clone)]
pub struct BlockDef {
    pub kind: BlockKind,
    pub label: String,
    pub sections: Vec<BlockSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Alt,
    Loop,
    Opt,
    Par,
    Critical,
    Break,
    Rect,
}

/// One section within a block (main body, or else/and/option).
#[derive(Debug, Clone)]
pub struct BlockSection {
    pub label: Option<String>,
    pub statements: Vec<SequenceStatement>,
}

/// A note attached to one or more participants.
#[derive(Debug, Clone)]
pub struct NoteDef {
    pub position: NotePosition,
    pub participants: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePosition {
    LeftOf,
    RightOf,
    Over,
}
