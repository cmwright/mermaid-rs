use thiserror::Error;

#[derive(Error, Debug)]
pub enum MermaidError {
    #[error("Parse error at line {line}, column {col}: {message}")]
    Parse {
        line: usize,
        col: usize,
        message: String,
        source_snippet: Option<String>,
    },

    #[error("Unsupported diagram type: {0}")]
    UnsupportedDiagram(String),

    #[error("Layout error: {0}")]
    Layout(String),

    #[error("Rendering error: {0}")]
    Render(String),

    #[error("Font error: {0}")]
    Font(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, MermaidError>;

/// Extract a few lines around a given line number for error context.
pub fn extract_snippet(source: &str, line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = line.saturating_sub(2);
    let end = (line + 1).min(lines.len());
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let lineno = start + i + 1;
            let marker = if lineno == line { ">" } else { " " };
            format!("{} {:>4} | {}", marker, lineno, l)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
