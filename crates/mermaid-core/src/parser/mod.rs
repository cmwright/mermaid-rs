pub mod flowchart;
pub mod gantt;
pub mod gitgraph;
pub mod mindmap;
pub mod pie;
pub mod sequence;

use crate::error::{MermaidError, Result};

/// Detected diagram type from source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramKind {
    Flowchart,
    Gantt,
    GitGraph,
    Mindmap,
    Pie,
    Sequence,
}

/// Detect the diagram kind from the first significant line of source.
pub fn detect_diagram_kind(source: &str) -> Result<DiagramKind> {
    for line in source.lines() {
        let trimmed = line.trim();
        // Skip empty lines, comments (%%...), directives (%%{...}%%), and frontmatter (---)
        if trimmed.is_empty() || trimmed.starts_with("%%") || trimmed == "---" {
            continue;
        }

        if trimmed.starts_with("gantt") {
            return Ok(DiagramKind::Gantt);
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
    fn test_detect_gantt() {
        assert_eq!(
            detect_diagram_kind("gantt\n    title Test\n    dateFormat YYYY-MM-DD").unwrap(),
            DiagramKind::Gantt
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

    #[test]
    fn test_detect_with_leading_comments() {
        let source = "%% This is a comment\n%% Another comment\nflowchart TD\n    A --> B";
        assert_eq!(detect_diagram_kind(source).unwrap(), DiagramKind::Flowchart);
    }

    #[test]
    fn test_detect_with_frontmatter_markers() {
        // The current implementation skips lines that are exactly "---"
        let source = "---\n---\ngantt\n    title Test";
        assert_eq!(detect_diagram_kind(source).unwrap(), DiagramKind::Gantt);
    }

    #[test]
    fn test_detect_with_directive_line() {
        let source = "%%{init: {'theme': 'dark'}}%%\nsequenceDiagram\n    A->>B: Hello";
        assert_eq!(
            detect_diagram_kind(source).unwrap(),
            DiagramKind::Sequence
        );
    }

    #[test]
    fn test_detect_gitgraph() {
        assert_eq!(
            detect_diagram_kind("gitGraph\n    commit").unwrap(),
            DiagramKind::GitGraph
        );
    }

    #[test]
    fn test_detect_mindmap() {
        assert_eq!(
            detect_diagram_kind("mindmap\n    root").unwrap(),
            DiagramKind::Mindmap
        );
    }

    #[test]
    fn test_detect_with_blank_lines() {
        let source = "\n\n\nflowchart TD\n    A --> B";
        assert_eq!(detect_diagram_kind(source).unwrap(), DiagramKind::Flowchart);
    }

    #[test]
    fn test_detect_with_mixed_comments_and_frontmatter() {
        let source = "---\n%% comment inside frontmatter\n---\n%% standalone comment\npie\n    \"A\": 50";
        assert_eq!(detect_diagram_kind(source).unwrap(), DiagramKind::Pie);
    }

    #[test]
    fn test_detect_whitespace_only_returns_error() {
        let source = "   \t  ";
        let result = detect_diagram_kind(source);
        assert!(result.is_err(), "whitespace-only content should return an error");
    }
}
