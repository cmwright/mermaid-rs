pub mod ast;
pub mod diagram;
pub mod error;
pub mod font;
pub mod layout;
pub mod parser;
pub mod render;

// Re-export main public API
pub use diagram::{render, OutputFormat, RenderConfig};
pub use error::{MermaidError, Result};
pub use parser::DiagramKind;
