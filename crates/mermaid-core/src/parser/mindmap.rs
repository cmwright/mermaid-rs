use pest::Parser;
use pest_derive::Parser;

use crate::ast::mindmap::*;
use crate::error::{MermaidError, Result};
use crate::render::html_util::normalize_br;

#[derive(Parser)]
#[grammar = "parser/mindmap.pest"]
struct MindmapPestParser;

/// Parse a Mermaid mindmap source string into a MindmapAst.
pub fn parse_mindmap(source: &str) -> Result<MindmapAst> {
    let preprocessed = preprocess(source);

    let pairs = MindmapPestParser::parse(Rule::mindmap_doc, &preprocessed).map_err(|e| {
        let (line, col) = match e.line_col {
            pest::error::LineColLocation::Pos((l, c)) => (l, c),
            pest::error::LineColLocation::Span((l, c), _) => (l, c),
        };
        MermaidError::Parse {
            line,
            col,
            message: format!("{}", e),
            source_snippet: None,
        }
    })?;

    let mut root: Option<MindmapNode> = None;

    for pair in pairs {
        if pair.as_rule() == Rule::mindmap_doc {
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::block {
                    // The outermost block should contain exactly one root node
                    let mut dummy = MindmapNode {
                        id: String::new(),
                        label: String::new(),
                        shape: MindmapNodeShape::Default,
                        children: Vec::new(),
                        icon: None,
                        css_class: None,
                    };
                    parse_block(inner, &mut dummy)?;
                    if let Some(first) = dummy.children.into_iter().next() {
                        root = Some(first);
                    }
                }
            }
        }
    }

    let root = root.ok_or_else(|| MermaidError::Parse {
        line: 1,
        col: 1,
        message: "Empty mindmap: no root node found".into(),
        source_snippet: None,
    })?;

    Ok(MindmapAst { root })
}

/// Parse a block (between INDENT and DEDENT).
/// Decorator lines in a block modify the parent node.
/// Node lines become children of the parent.
fn parse_block(
    pair: pest::iterators::Pair<'_, Rule>,
    parent: &mut MindmapNode,
) -> Result<()> {
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::statement {
            parse_statement(inner, parent)?;
        }
    }
    Ok(())
}

/// Parse a statement — either a decorator line (modifies parent) or a node line (new child).
fn parse_statement(
    pair: pest::iterators::Pair<'_, Rule>,
    parent: &mut MindmapNode,
) -> Result<()> {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::decorator_line => {
                // Decorators at this block level modify the parent node
                apply_decorator(inner, parent);
            }
            Rule::node_line => {
                let child = parse_node_line(inner)?;
                parent.children.push(child);
            }
            _ => {}
        }
    }
    Ok(())
}

/// Parse a node_line into a MindmapNode, including inline decorators and child block.
fn parse_node_line(pair: pest::iterators::Pair<'_, Rule>) -> Result<MindmapNode> {
    let mut node = MindmapNode {
        id: String::new(),
        label: String::new(),
        shape: MindmapNodeShape::Default,
        children: Vec::new(),
        icon: None,
        css_class: None,
    };

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::node_content => {
                parse_node_content(inner, &mut node);
            }
            Rule::icon_decorator => {
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::icon_value {
                        node.icon = Some(sub.as_str().trim().to_string());
                    }
                }
            }
            Rule::class_decorator => {
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::class_name {
                        node.css_class = Some(sub.as_str().trim().to_string());
                    }
                }
            }
            Rule::block => {
                // Children block — decorators inside modify this node
                parse_block(inner, &mut node)?;
            }
            _ => {}
        }
    }

    if node.id.is_empty() {
        node.id = sanitize_id(&node.label);
    }

    Ok(node)
}

/// Parse node_content to determine shape and label text.
fn parse_node_content(pair: pest::iterators::Pair<'_, Rule>, node: &mut MindmapNode) {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::node_id => {
                node.id = inner.as_str().to_string();
            }
            Rule::circle_node => {
                node.shape = MindmapNodeShape::Circle;
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::circle_text {
                        node.label = normalize_label(sub.as_str());
                    }
                }
            }
            Rule::bang_node => {
                node.shape = MindmapNodeShape::Bang;
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::bang_text {
                        node.label = normalize_label(sub.as_str());
                    }
                }
            }
            Rule::cloud_node => {
                node.shape = MindmapNodeShape::Cloud;
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::cloud_text {
                        node.label = normalize_label(sub.as_str());
                    }
                }
            }
            Rule::hexagon_node => {
                node.shape = MindmapNodeShape::Hexagon;
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::hex_text {
                        node.label = normalize_label(sub.as_str());
                    }
                }
            }
            Rule::rounded_rect_node => {
                node.shape = MindmapNodeShape::RoundedRect;
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::rounded_text {
                        node.label = normalize_label(sub.as_str());
                    }
                }
            }
            Rule::rect_node => {
                node.shape = MindmapNodeShape::Rect;
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::rect_text {
                        node.label = normalize_label(sub.as_str());
                    }
                }
            }
            Rule::bare_node => {
                node.shape = MindmapNodeShape::Default;
                node.label = normalize_label(inner.as_str());
            }
            _ => {}
        }
    }
}

/// Apply a decorator_line (icon or class) to a node.
fn apply_decorator(pair: pest::iterators::Pair<'_, Rule>, node: &mut MindmapNode) {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::icon_decorator => {
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::icon_value {
                        node.icon = Some(sub.as_str().trim().to_string());
                    }
                }
            }
            Rule::class_decorator => {
                for sub in inner.into_inner() {
                    if sub.as_rule() == Rule::class_name {
                        node.css_class = Some(sub.as_str().trim().to_string());
                    }
                }
            }
            _ => {}
        }
    }
}

/// Normalize label text: handle <br/> tags and trim whitespace.
fn normalize_label(s: &str) -> String {
    normalize_br(s).trim().to_string()
}

/// Create a sanitized ID from label text.
fn sanitize_id(label: &str) -> String {
    label
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

// ─── Indentation Preprocessor ─────────────────────────────────────────

/// Convert indentation-based mindmap source into a flat string with
/// \x01 (INDENT) and \x02 (DEDENT) marker characters.
///
/// Key design: child blocks (INDENT) immediately follow the parent node
/// content with no newline in between, so the pest grammar's `block?`
/// in `node_line` can match. Siblings are separated by newlines.
fn preprocess(source: &str) -> String {
    // Phase 1: collect content lines with their indent levels
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut started = false;

    for line in source.lines() {
        let trimmed = line.trim();

        // Skip until we find the "mindmap" header
        if !started {
            if trimmed.starts_with("mindmap") {
                started = true;
            }
            continue;
        }

        // Skip blank lines and comments
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }

        lines.push((count_indent(line), trimmed));
    }

    if lines.is_empty() {
        return String::new();
    }

    // Phase 2: emit preprocessed string
    let mut result = String::new();
    let mut indent_stack: Vec<usize> = Vec::new();

    for i in 0..lines.len() {
        let (indent, content) = lines[i];

        // Handle dedents (going to a shallower level)
        let mut dedented = false;
        while !indent_stack.is_empty() && *indent_stack.last().unwrap() > indent {
            indent_stack.pop();
            result.push('\x02');
            dedented = true;
        }

        // If we dedented to reach a sibling level, add newline separator
        if dedented {
            result.push('\n');
        }

        // Handle indent (new deeper level) — INDENT goes right before content
        if indent_stack.is_empty() || indent > *indent_stack.last().unwrap() {
            indent_stack.push(indent);
            result.push('\x01');
        }

        // Emit content
        result.push_str(content);

        // Check if the next line is a child (deeper indent)
        let next_is_child = i + 1 < lines.len() && lines[i + 1].0 > indent;

        // Only emit newline if next is NOT a child block —
        // child blocks need INDENT immediately after content
        if !next_is_child {
            result.push('\n');
        }
    }

    // Close all remaining open blocks
    while !indent_stack.is_empty() {
        indent_stack.pop();
        result.push('\x02');
    }

    result
}

/// Count leading spaces in a line (tab = 4 spaces).
fn count_indent(line: &str) -> usize {
    let mut count = 0;
    for ch in line.chars() {
        match ch {
            ' ' => count += 1,
            '\t' => count += 4,
            _ => break,
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_basic() {
        let source = "mindmap\n  root\n    child1\n    child2\n";
        let result = preprocess(source);
        assert!(result.contains('\x01'));
        assert!(result.contains('\x02'));
        assert!(result.contains("root"));
        assert!(result.contains("child1"));
        assert!(result.contains("child2"));
    }

    #[test]
    fn test_preprocess_structure() {
        let source = "mindmap\n  root\n    child1\n      grandchild\n    child2\n";
        let result = preprocess(source);
        // root has children, so no \n between root and child block's INDENT
        // child1 has a child (grandchild), so no \n between child1 and its block's INDENT
        // grandchild has no children, so \n after grandchild
        // DEDENT after grandchild's block, then \n before child2 (sibling)
        assert!(result.contains("root\x01child1\x01grandchild\n"));
    }

    #[test]
    fn test_parse_simple_mindmap() {
        let source = r#"mindmap
  root((mindmap))
    Origins
      Long history
    Research"#;
        let ast = parse_mindmap(source).unwrap();
        assert_eq!(ast.root.label, "mindmap");
        assert_eq!(ast.root.shape, MindmapNodeShape::Circle);
        assert_eq!(ast.root.children.len(), 2);
        assert_eq!(ast.root.children[0].label, "Origins");
        assert_eq!(ast.root.children[0].children.len(), 1);
        assert_eq!(ast.root.children[0].children[0].label, "Long history");
        assert_eq!(ast.root.children[1].label, "Research");
    }

    #[test]
    fn test_parse_all_shapes() {
        let source = r#"mindmap
  root
    [Rect]
    (Rounded)
    ((Circle))
    )Cloud(
    ))Bang((
    {{Hexagon}}"#;
        let ast = parse_mindmap(source).unwrap();
        assert_eq!(ast.root.children.len(), 6);
        assert_eq!(ast.root.children[0].shape, MindmapNodeShape::Rect);
        assert_eq!(ast.root.children[0].label, "Rect");
        assert_eq!(ast.root.children[1].shape, MindmapNodeShape::RoundedRect);
        assert_eq!(ast.root.children[1].label, "Rounded");
        assert_eq!(ast.root.children[2].shape, MindmapNodeShape::Circle);
        assert_eq!(ast.root.children[2].label, "Circle");
        assert_eq!(ast.root.children[3].shape, MindmapNodeShape::Cloud);
        assert_eq!(ast.root.children[3].label, "Cloud");
        assert_eq!(ast.root.children[4].shape, MindmapNodeShape::Bang);
        assert_eq!(ast.root.children[4].label, "Bang");
        assert_eq!(ast.root.children[5].shape, MindmapNodeShape::Hexagon);
        assert_eq!(ast.root.children[5].label, "Hexagon");
    }

    #[test]
    fn test_parse_with_tabs() {
        let source = "mindmap\n\troot\n\t\tchild1\n\t\tchild2\n";
        let ast = parse_mindmap(source).unwrap();
        assert_eq!(ast.root.label, "root");
        assert_eq!(ast.root.children.len(), 2);
    }

    #[test]
    fn test_parse_decorators() {
        let source = r#"mindmap
  root
    A
    :::urgent"#;
        let ast = parse_mindmap(source).unwrap();
        // :::urgent on same-indent line is consumed by A's decorator*
        assert_eq!(ast.root.children.len(), 1);
        assert_eq!(ast.root.children[0].label, "A");
        assert_eq!(ast.root.children[0].css_class.as_deref(), Some("urgent"));
    }

    #[test]
    fn test_count_indent() {
        assert_eq!(count_indent("hello"), 0);
        assert_eq!(count_indent("  hello"), 2);
        assert_eq!(count_indent("    hello"), 4);
        assert_eq!(count_indent("\thello"), 4);
        assert_eq!(count_indent("\t\thello"), 8);
    }

    #[test]
    fn test_parse_canonical_example() {
        let source = r#"mindmap
  root((mindmap))
    Origins
      Long history
      ::icon(fa fa-book)
      Popularisation
        British popular psychology author Tony Buzan
    Research
      On effectiveness<br/>and features
      On Automatic creation
        Uses
            Creative techniques
            Strategic planning
            Argument mapping
    Tools
      Pen and paper
      Mermaid"#;
        let ast = parse_mindmap(source).unwrap();
        assert_eq!(ast.root.label, "mindmap");
        assert_eq!(ast.root.shape, MindmapNodeShape::Circle);
        assert_eq!(ast.root.children.len(), 3);

        // Origins
        let origins = &ast.root.children[0];
        assert_eq!(origins.label, "Origins");
        // ::icon on same-indent line is consumed by Long history's decorator*
        assert_eq!(origins.children.len(), 2);
        assert_eq!(origins.children[0].label, "Long history");
        assert_eq!(
            origins.children[0].icon.as_deref(),
            Some("fa fa-book")
        );
        assert_eq!(origins.children[1].label, "Popularisation");
        assert_eq!(origins.children[1].children.len(), 1);

        // Research
        let research = &ast.root.children[1];
        assert_eq!(research.label, "Research");
        assert_eq!(research.children.len(), 2);
        assert_eq!(research.children[0].label, "On effectiveness\nand features");

        // Tools
        let tools = &ast.root.children[2];
        assert_eq!(tools.label, "Tools");
        assert_eq!(tools.children.len(), 2);
    }
}
