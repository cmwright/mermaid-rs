pub mod flowchart;
pub mod gitgraph;
pub mod mindmap;
pub mod pie;
pub mod sequence;

use crate::error::{MermaidError, Result};

/// Detected diagram type from source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramKind {
    Flowchart,
    GitGraph,
    Mindmap,
    Pie,
    Sequence,
}

/// Detect the diagram kind from the first significant line of source.
pub fn detect_diagram_kind(source: &str) -> Result<DiagramKind> {
    for line in source.lines() {
        let trimmed = line.trim();
        // Skip empty lines, comments, frontmatter, and directives
        if trimmed.is_empty() || trimmed.starts_with("%%") || trimmed == "---" {
            continue;
        }
        // Skip directives (but don't skip lines starting with %%{ that aren't on their own line)
        if trimmed.starts_with("%%{") {
            continue;
        }

        if trimmed.starts_with("gitGraph") {
            return Ok(DiagramKind::GitGraph);
        }

        if trimmed.starts_with("graph") || trimmed.starts_with("flowchart") {
            return Ok(DiagramKind::Flowchart);
        }

        if trimmed.starts_with("mindmap") {
            return Ok(DiagramKind::Mindmap);
        }

        if trimmed.starts_with("pie") {
            return Ok(DiagramKind::Pie);
        }

        if trimmed.starts_with("sequenceDiagram") {
            return Ok(DiagramKind::Sequence);
        }

        return Err(MermaidError::UnsupportedDiagram(
            trimmed
                .split_whitespace()
                .next()
                .unwrap_or("unknown")
                .to_string(),
        ));
    }
    Err(MermaidError::Parse {
        line: 1,
        col: 1,
        message: "Empty or unrecognized diagram source".into(),
        source_snippet: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_flowchart() {
        assert_eq!(
            detect_diagram_kind("flowchart TD\n    A --> B").unwrap(),
            DiagramKind::Flowchart
        );
        assert_eq!(
            detect_diagram_kind("graph LR\n    A --> B").unwrap(),
            DiagramKind::Flowchart
        );
    }

    #[test]
    fn test_detect_pie() {
        assert_eq!(
            detect_diagram_kind("pie title NETFLIX\n    \"A\": 50").unwrap(),
            DiagramKind::Pie
        );
        assert_eq!(
            detect_diagram_kind("pie\n    \"A\": 50").unwrap(),
            DiagramKind::Pie
        );
    }

    #[test]
    fn test_detect_sequence() {
        assert_eq!(
            detect_diagram_kind("sequenceDiagram\n    A->>B: Hello").unwrap(),
            DiagramKind::Sequence
        );
    }

    #[test]
    fn test_detect_with_comments() {
        let source = "%% This is a comment\nflowchart TD\n    A --> B";
        assert_eq!(detect_diagram_kind(source).unwrap(), DiagramKind::Flowchart);
    }

    #[test]
    fn test_detect_unsupported() {
        let result = detect_diagram_kind("unknownDiagram\n    A: 50");
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_empty() {
        let result = detect_diagram_kind("");
        assert!(result.is_err());
    }
}
