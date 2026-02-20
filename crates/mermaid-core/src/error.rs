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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_snippet_middle_line() {
        let source = "line1\nline2\nline3\nline4\nline5";
        let snippet = extract_snippet(source, 3);
        let lines: Vec<&str> = snippet.lines().collect();
        // Should show lines 1..=3 (line.saturating_sub(2)=1, end=min(4,5)=4 → indices 1..4 → lines 2,3,4 at 0-index but start=1 so lineno 2,3,4)
        assert_eq!(lines.len(), 3);
        // Line 3 should have the `>` marker
        assert!(lines[1].starts_with('>'));
        assert!(lines[1].contains("line3"));
        // Other lines should have a space marker
        assert!(lines[0].starts_with(' '));
        assert!(lines[2].starts_with(' '));
    }

    #[test]
    fn extract_snippet_first_line() {
        let source = "first\nsecond\nthird";
        let snippet = extract_snippet(source, 1);
        let lines: Vec<&str> = snippet.lines().collect();
        // start = 1.saturating_sub(2) = 0, end = min(2, 3) = 2 → 2 lines
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with('>'));
        assert!(lines[0].contains("first"));
        assert!(lines[1].starts_with(' '));
        assert!(lines[1].contains("second"));
    }

    #[test]
    fn extract_snippet_last_line() {
        let source = "aaa\nbbb\nccc\nddd";
        let snippet = extract_snippet(source, 4);
        let lines: Vec<&str> = snippet.lines().collect();
        // start = 4.saturating_sub(2) = 2, end = min(5, 4) = 4 → indices 2..4 → 2 lines
        assert_eq!(lines.len(), 2);
        // Line 4 (last) should have the `>` marker
        let last = lines.last().unwrap();
        assert!(last.starts_with('>'));
        assert!(last.contains("ddd"));
    }

    #[test]
    fn mermaid_error_parse_display() {
        let err = MermaidError::Parse {
            line: 5,
            col: 10,
            message: "unexpected token".to_string(),
            source_snippet: None,
        };
        let display = format!("{}", err);
        assert_eq!(display, "Parse error at line 5, column 10: unexpected token");
    }

    #[test]
    fn mermaid_error_unsupported_diagram_display() {
        let err = MermaidError::UnsupportedDiagram("pie".to_string());
        let display = format!("{}", err);
        assert_eq!(display, "Unsupported diagram type: pie");
    }

    #[test]
    fn mermaid_error_layout_display() {
        let err = MermaidError::Layout("cycle detected".to_string());
        let display = format!("{}", err);
        assert_eq!(display, "Layout error: cycle detected");
    }

    #[test]
    fn mermaid_error_render_display() {
        let err = MermaidError::Render("svg failed".to_string());
        let display = format!("{}", err);
        assert_eq!(display, "Rendering error: svg failed");
    }

    #[test]
    fn mermaid_error_font_display() {
        let err = MermaidError::Font("missing glyph".to_string());
        let display = format!("{}", err);
        assert_eq!(display, "Font error: missing glyph");
    }
}
