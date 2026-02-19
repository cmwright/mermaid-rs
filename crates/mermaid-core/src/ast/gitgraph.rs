/// Top-level AST for a gitGraph diagram.
#[derive(Debug, Clone, Default)]
pub struct GitGraphAst {
    pub commands: Vec<GitCommand>,
}

/// A command in the gitGraph body.
#[derive(Debug, Clone)]
pub enum GitCommand {
    Commit(CommitDef),
    Branch(BranchDef),
    Checkout(CheckoutDef),
    Merge(MergeDef),
}

/// A commit with optional attributes.
#[derive(Debug, Clone)]
pub struct CommitDef {
    pub id: Option<String>,
    pub message: Option<String>,
    pub tag: Option<String>,
    pub commit_type: CommitType,
}

/// The visual type of a commit node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommitType {
    #[default]
    Normal,
    Reverse,
    Highlight,
}

/// Create a new branch from the current branch.
#[derive(Debug, Clone)]
pub struct BranchDef {
    pub name: String,
}

/// Switch to an existing branch.
#[derive(Debug, Clone)]
pub struct CheckoutDef {
    pub name: String,
}

/// Merge another branch into the current branch.
#[derive(Debug, Clone)]
pub struct MergeDef {
    pub branch: String,
}
