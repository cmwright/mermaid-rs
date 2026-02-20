use pest::Parser;
use pest_derive::Parser;

use crate::ast::common::{parse_style_string, StyleProperties};
use crate::ast::flowchart::*;
use crate::error::{extract_snippet, MermaidError, Result};

#[derive(Parser)]
#[grammar = "parser/flowchart.pest"]
struct FlowchartPestParser;

/// Parse a Mermaid flowchart source string into a FlowchartAst.
pub fn parse_flowchart(source: &str) -> Result<FlowchartAst> {
    let pairs = FlowchartPestParser::parse(Rule::flowchart, source).map_err(|e| {
        let (line, col) = match e.line_col {
            pest::error::LineColLocation::Pos((l, c)) => (l, c),
            pest::error::LineColLocation::Span((l, c), _) => (l, c),
        };
        MermaidError::Parse {
            line,
            col,
            message: format!("{}", e),
            source_snippet: Some(extract_snippet(source, line)),
        }
    })?;

    let mut ast = FlowchartAst::default();

    for pair in pairs {
        if pair.as_rule() == Rule::flowchart {
            for inner in pair.into_inner() {
                process_top_level(&mut ast, inner)?;
            }
        }
    }

    Ok(ast)
}

fn process_top_level(ast: &mut FlowchartAst, pair: pest::iterators::Pair<'_, Rule>) -> Result<()> {
    match pair.as_rule() {
        Rule::diagram_header => {
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::direction {
                    ast.direction = parse_direction(inner.as_str())?;
                }
            }
        }
        Rule::link_chain => {
            parse_link_chain(pair, &mut ast.nodes, &mut ast.edges)?;
        }
        Rule::node_stmt => {
            let node = parse_node_stmt(pair)?;
            upsert_node(&mut ast.nodes, node);
        }
        Rule::subgraph_block => {
            let sg = parse_subgraph(pair)?;
            ast.subgraphs.push(sg);
        }
        Rule::class_def_stmt => {
            let cd = parse_class_def(pair)?;
            ast.class_defs.push(cd);
        }
        Rule::class_assign_stmt => {
            let ca = parse_class_assign(pair)?;
            ast.class_assignments.push(ca);
        }
        Rule::style_stmt => {
            let so = parse_style_override(pair)?;
            ast.style_overrides.push(so);
        }
        Rule::directive => {
            // Directives are parsed but currently ignored
        }
        Rule::link_style_stmt => {
            // linkStyle is parsed but currently ignored
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn parse_direction(s: &str) -> Result<Direction> {
    match s.trim() {
        "TB" | "TD" => Ok(Direction::TopToBottom),
        "BT" => Ok(Direction::BottomToTop),
        "LR" => Ok(Direction::LeftToRight),
        "RL" => Ok(Direction::RightToLeft),
        other => Err(MermaidError::Parse {
            line: 0,
            col: 0,
            message: format!("Unknown direction: {}", other),
            source_snippet: None,
        }),
    }
}

fn parse_node_stmt(pair: pest::iterators::Pair<'_, Rule>) -> Result<NodeDef> {
    let inner = pair.into_inner().next().unwrap();
    parse_node_def(inner)
}

fn parse_node_def(pair: pest::iterators::Pair<'_, Rule>) -> Result<NodeDef> {
    let mut id = String::new();
    let mut label: Option<String> = None;
    let mut shape = NodeShape::Rectangle;
    let mut class_shorthand: Option<String> = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::node_id => {
                id = inner.as_str().to_string();
            }
            Rule::class_shorthand => {
                // ":::" ~ identifier
                let ident = inner.into_inner().next().unwrap();
                class_shorthand = Some(ident.as_str().to_string());
            }
            Rule::shape_rect => {
                shape = NodeShape::Rectangle;
                label = Some(extract_label(inner));
            }
            Rule::shape_rounded => {
                shape = NodeShape::RoundedRectangle;
                label = Some(extract_label(inner));
            }
            Rule::shape_stadium => {
                shape = NodeShape::Stadium;
                label = Some(extract_label(inner));
            }
            Rule::shape_subroutine => {
                shape = NodeShape::Subroutine;
                label = Some(extract_label(inner));
            }
            Rule::shape_cylinder => {
                shape = NodeShape::Cylinder;
                label = Some(extract_label(inner));
            }
            Rule::shape_circle => {
                shape = NodeShape::Circle;
                label = Some(extract_label(inner));
            }
            Rule::shape_double_circle => {
                shape = NodeShape::DoubleCircle;
                label = Some(extract_label(inner));
            }
            Rule::shape_diamond => {
                shape = NodeShape::Diamond;
                label = Some(extract_label(inner));
            }
            Rule::shape_hexagon => {
                shape = NodeShape::Hexagon;
                label = Some(extract_label(inner));
            }
            Rule::shape_asymmetric => {
                shape = NodeShape::Asymmetric;
                label = Some(extract_label(inner));
            }
            Rule::shape_trapezoid => {
                shape = NodeShape::Trapezoid;
                label = Some(extract_label(inner));
            }
            Rule::shape_trapezoid_alt => {
                shape = NodeShape::TrapezoidAlt;
                label = Some(extract_label(inner));
            }
            Rule::shape_parallelogram => {
                shape = NodeShape::Parallelogram;
                label = Some(extract_label(inner));
            }
            Rule::shape_parallelogram_alt => {
                shape = NodeShape::ParallelogramAlt;
                label = Some(extract_label(inner));
            }
            _ => {}
        }
    }

    Ok(NodeDef {
        id,
        label,
        shape,
        class_shorthand,
    })
}

fn extract_label(pair: pest::iterators::Pair<'_, Rule>) -> String {
    // The label is the text content inside the shape delimiters.
    // It's captured as bracket_text, paren_text, brace_text, etc.
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::bracket_text
            | Rule::paren_text
            | Rule::brace_text
            | Rule::slash_text
            | Rule::backslash_text
            | Rule::quoted_text => {
                return inner.as_str().trim().to_string();
            }
            _ => {}
        }
    }
    String::new()
}

fn parse_link_chain(
    pair: pest::iterators::Pair<'_, Rule>,
    nodes: &mut Vec<NodeDef>,
    edges: &mut Vec<EdgeDef>,
) -> Result<()> {
    let mut items: Vec<pest::iterators::Pair<'_, Rule>> = pair.into_inner().collect();

    if items.is_empty() {
        return Ok(());
    }

    let first_node = parse_node_def(items.remove(0))?;
    let first_id = first_node.id.clone();
    upsert_node(nodes, first_node);

    let mut prev_id = first_id;

    while items.len() >= 2 {
        let edge_pair = items.remove(0);
        let next_node_pair = items.remove(0);

        let (line_style, arrow_start, arrow_end, edge_label) = parse_edge(edge_pair)?;
        let next_node = parse_node_def(next_node_pair)?;
        let next_id = next_node.id.clone();
        upsert_node(nodes, next_node);

        edges.push(EdgeDef {
            from: prev_id.clone(),
            to: next_id.clone(),
            line_style,
            arrow_start,
            arrow_end,
            label: edge_label,
        });

        prev_id = next_id;
    }

    Ok(())
}

/// Decode the start marker character (first char of edge string).
fn decode_start_marker(ch: char) -> ArrowEnd {
    match ch {
        '<' => ArrowEnd::Arrow,
        'x' => ArrowEnd::Cross,
        'o' => ArrowEnd::Circle,
        _ => ArrowEnd::None,
    }
}

/// Decode the end marker character (last char of edge string).
fn decode_end_marker(ch: char) -> ArrowEnd {
    match ch {
        '>' => ArrowEnd::Arrow,
        'x' => ArrowEnd::Cross,
        'o' => ArrowEnd::Circle,
        _ => ArrowEnd::None,
    }
}

/// Parse a full (unlabeled) edge string like `-->`, `<-->`, `--o`, `x--x`, `===`, etc.
/// Returns (LineStyle, ArrowEnd start, ArrowEnd end).
fn parse_full_edge_str(s: &str, style: LineStyle) -> (LineStyle, ArrowEnd, ArrowEnd) {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return (style, ArrowEnd::None, ArrowEnd::None);
    }

    let first = chars[0];
    let last = *chars.last().unwrap();

    let start = if first == '<' || first == 'x' || first == 'o' {
        decode_start_marker(first)
    } else {
        ArrowEnd::None
    };

    let end = if last == '>' || last == 'x' || last == 'o' {
        decode_end_marker(last)
    } else {
        ArrowEnd::None
    };

    (style, start, end)
}

#[allow(clippy::type_complexity)]
fn parse_edge(
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<(LineStyle, ArrowEnd, ArrowEnd, Option<String>)> {
    let mut line_style = LineStyle::Solid;
    let mut arrow_start = ArrowEnd::None;
    let mut arrow_end = ArrowEnd::Arrow;
    let mut label = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::invisible_link => {
                line_style = LineStyle::Invisible;
                arrow_start = ArrowEnd::None;
                arrow_end = ArrowEnd::None;
            }
            Rule::solid_edge_full => {
                let (ls, s, e) = parse_full_edge_str(inner.as_str().trim(), LineStyle::Solid);
                line_style = ls;
                arrow_start = s;
                arrow_end = e;
            }
            Rule::dotted_edge_full => {
                let (ls, s, e) = parse_full_edge_str(inner.as_str().trim(), LineStyle::Dotted);
                line_style = ls;
                arrow_start = s;
                arrow_end = e;
            }
            Rule::thick_edge_full => {
                let (ls, s, e) = parse_full_edge_str(inner.as_str().trim(), LineStyle::Thick);
                line_style = ls;
                arrow_start = s;
                arrow_end = e;
            }
            Rule::solid_arrow_labeled => {
                line_style = LineStyle::Solid;
                arrow_end = ArrowEnd::Arrow;
                arrow_start = extract_labeled_start_marker(inner.as_str());
                label = Some(extract_edge_inline_label(inner));
            }
            Rule::solid_line_labeled => {
                line_style = LineStyle::Solid;
                arrow_end = ArrowEnd::None;
                arrow_start = ArrowEnd::None;
                label = Some(extract_edge_inline_label(inner));
            }
            Rule::dotted_arrow_labeled => {
                line_style = LineStyle::Dotted;
                arrow_end = ArrowEnd::Arrow;
                arrow_start = extract_labeled_start_marker(inner.as_str());
                label = Some(extract_edge_inline_label(inner));
            }
            Rule::dotted_line_labeled => {
                line_style = LineStyle::Dotted;
                arrow_end = ArrowEnd::None;
                arrow_start = ArrowEnd::None;
                label = Some(extract_edge_inline_label(inner));
            }
            Rule::thick_arrow_labeled => {
                line_style = LineStyle::Thick;
                arrow_end = ArrowEnd::Arrow;
                arrow_start = extract_labeled_start_marker(inner.as_str());
                label = Some(extract_edge_inline_label(inner));
            }
            Rule::thick_line_labeled => {
                line_style = LineStyle::Thick;
                arrow_end = ArrowEnd::None;
                arrow_start = ArrowEnd::None;
                label = Some(extract_edge_inline_label(inner));
            }
            Rule::pipe_label => {
                for pipe_inner in inner.into_inner() {
                    if pipe_inner.as_rule() == Rule::pipe_text {
                        label = Some(pipe_inner.as_str().trim().to_string());
                    }
                }
            }
            _ => {}
        }
    }

    Ok((line_style, arrow_start, arrow_end, label))
}

/// For labeled edge variants like `x-- text -->`, check if the first char is a start marker.
fn extract_labeled_start_marker(s: &str) -> ArrowEnd {
    let trimmed = s.trim();
    match trimmed.chars().next() {
        Some('x') => ArrowEnd::Cross,
        Some('o') => ArrowEnd::Circle,
        Some('<') => ArrowEnd::Arrow,
        _ => ArrowEnd::None,
    }
}

fn extract_edge_inline_label(pair: pest::iterators::Pair<'_, Rule>) -> String {
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::edge_inline_text {
            return inner.as_str().trim().to_string();
        }
    }
    String::new()
}

fn parse_subgraph(pair: pest::iterators::Pair<'_, Rule>) -> Result<SubgraphDef> {
    let mut id = String::new();
    let mut label: Option<String> = None;
    let mut direction: Option<Direction> = None;
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut subgraphs = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::subgraph_id => {
                id = inner.as_str().to_string();
            }
            Rule::subgraph_label => {
                label = Some(extract_subgraph_label(inner));
            }
            Rule::subgraph_direction => {
                for dir_inner in inner.into_inner() {
                    if dir_inner.as_rule() == Rule::direction {
                        direction = Some(parse_direction(dir_inner.as_str())?);
                    }
                }
            }
            Rule::link_chain => {
                parse_link_chain(inner, &mut nodes, &mut edges)?;
            }
            Rule::node_stmt => {
                let node = parse_node_stmt(inner)?;
                upsert_node(&mut nodes, node);
            }
            Rule::subgraph_block => {
                let sg = parse_subgraph(inner)?;
                subgraphs.push(sg);
            }
            _ => {}
        }
    }

    Ok(SubgraphDef {
        id,
        label,
        direction,
        nodes,
        edges,
        subgraphs,
    })
}

fn extract_subgraph_label(pair: pest::iterators::Pair<'_, Rule>) -> String {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::quoted_text | Rule::bracket_text => {
                return inner.as_str().trim().to_string();
            }
            _ => {}
        }
    }
    String::new()
}

fn parse_class_def(pair: pest::iterators::Pair<'_, Rule>) -> Result<ClassDef> {
    let mut name = String::new();
    let mut properties = StyleProperties::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::class_name => {
                name = inner.as_str().to_string();
            }
            Rule::style_props_text => {
                properties = parse_style_string(inner.as_str().trim());
            }
            _ => {}
        }
    }

    Ok(ClassDef { name, properties })
}

fn parse_class_assign(pair: pest::iterators::Pair<'_, Rule>) -> Result<ClassAssignment> {
    let mut node_ids = Vec::new();
    let mut class_name = String::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::node_id_list => {
                for id_pair in inner.into_inner() {
                    if id_pair.as_rule() == Rule::node_id {
                        node_ids.push(id_pair.as_str().to_string());
                    }
                }
            }
            Rule::class_name => {
                class_name = inner.as_str().to_string();
            }
            _ => {}
        }
    }

    Ok(ClassAssignment {
        node_ids,
        class_name,
    })
}

fn parse_style_override(pair: pest::iterators::Pair<'_, Rule>) -> Result<StyleOverride> {
    let mut node_id = String::new();
    let mut properties = StyleProperties::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::node_id => {
                node_id = inner.as_str().to_string();
            }
            Rule::style_props_text => {
                properties = parse_style_string(inner.as_str().trim());
            }
            _ => {}
        }
    }

    Ok(StyleOverride {
        node_id,
        properties,
    })
}

/// Insert a node, or update it if a node with the same ID already exists.
/// Later definitions can add shape/label info to an initially bare node.
fn upsert_node(nodes: &mut Vec<NodeDef>, new_node: NodeDef) {
    if let Some(existing) = nodes.iter_mut().find(|n| n.id == new_node.id) {
        // Update label and shape if the new definition provides them
        if new_node.label.is_some() {
            existing.label = new_node.label;
            existing.shape = new_node.shape;
        }
        if new_node.class_shorthand.is_some() {
            existing.class_shorthand = new_node.class_shorthand;
        }
    } else {
        nodes.push(new_node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_flowchart() {
        let source = "flowchart TD\n    A[Start] --> B[End]";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.direction, Direction::TopToBottom);
        assert_eq!(ast.nodes.len(), 2);
        assert_eq!(ast.edges.len(), 1);
        assert_eq!(ast.nodes[0].id, "A");
        assert_eq!(ast.nodes[0].label.as_deref(), Some("Start"));
        assert_eq!(ast.nodes[0].shape, NodeShape::Rectangle);
        assert_eq!(ast.nodes[1].id, "B");
        assert_eq!(ast.edges[0].from, "A");
        assert_eq!(ast.edges[0].to, "B");
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
    }

    #[test]
    fn test_parse_dotted_arrow_labeled() {
        let source = "flowchart TD\n    A -. text .-> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges.len(), 1);
        assert_eq!(ast.edges[0].line_style, LineStyle::Dotted);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
        assert_eq!(ast.edges[0].label.as_deref(), Some("text"));
    }

    #[test]
    fn test_parse_dotted_arrow_labeled_with_quotes() {
        let source = "flowchart TD\n    A -. \"blocked by\" .-> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges.len(), 1);
        assert_eq!(ast.edges[0].line_style, LineStyle::Dotted);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
        assert_eq!(ast.edges[0].label.as_deref(), Some("\"blocked by\""));
    }

    #[test]
    fn test_parse_direction_lr() {
        let source = "graph LR\n    A --> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.direction, Direction::LeftToRight);
    }

    #[test]
    fn test_parse_node_shapes() {
        let source = r#"flowchart TD
    A[Rectangle]
    B(Rounded)
    C{Diamond}
    D((Circle))
    E([Stadium])
    F[[Subroutine]]
    G[(Cylinder)]
    H{{Hexagon}}"#;
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes.len(), 8);
        assert_eq!(ast.nodes[0].shape, NodeShape::Rectangle);
        assert_eq!(ast.nodes[1].shape, NodeShape::RoundedRectangle);
        assert_eq!(ast.nodes[2].shape, NodeShape::Diamond);
        assert_eq!(ast.nodes[3].shape, NodeShape::Circle);
        assert_eq!(ast.nodes[4].shape, NodeShape::Stadium);
        assert_eq!(ast.nodes[5].shape, NodeShape::Subroutine);
        assert_eq!(ast.nodes[6].shape, NodeShape::Cylinder);
        assert_eq!(ast.nodes[7].shape, NodeShape::Hexagon);
    }

    #[test]
    fn test_parse_edge_types() {
        let source = "flowchart TD\n    A --> B\n    B --- C\n    C -.-> D\n    D ==> E";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges.len(), 4);
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
        assert_eq!(ast.edges[1].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[1].arrow_end, ArrowEnd::None);
        assert_eq!(ast.edges[2].line_style, LineStyle::Dotted);
        assert_eq!(ast.edges[2].arrow_end, ArrowEnd::Arrow);
        assert_eq!(ast.edges[3].line_style, LineStyle::Thick);
        assert_eq!(ast.edges[3].arrow_end, ArrowEnd::Arrow);
    }

    #[test]
    fn test_parse_edge_chain() {
        let source = "flowchart TD\n    A --> B --> C";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes.len(), 3);
        assert_eq!(ast.edges.len(), 2);
        assert_eq!(ast.edges[0].from, "A");
        assert_eq!(ast.edges[0].to, "B");
        assert_eq!(ast.edges[1].from, "B");
        assert_eq!(ast.edges[1].to, "C");
    }

    #[test]
    fn test_parse_class_def() {
        let source =
            "flowchart TD\n    A[Node]\n    classDef myClass fill:#f9f,stroke:#333,stroke-width:4px";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.class_defs.len(), 1);
        assert_eq!(ast.class_defs[0].name, "myClass");
        assert!(ast.class_defs[0].properties.fill.is_some());
    }

    #[test]
    fn test_parse_pipe_labels() {
        let source = "flowchart TD\n    A[Start] --> B{Decision}\n    B -->|Yes| C[OK]\n    B -->|No| D[Fail]";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes.len(), 4);
        assert_eq!(ast.edges.len(), 3);
        assert_eq!(ast.edges[1].from, "B");
        assert_eq!(ast.edges[1].to, "C");
        assert_eq!(ast.edges[1].label.as_deref(), Some("Yes"));
        assert_eq!(ast.edges[2].label.as_deref(), Some("No"));
    }

    #[test]
    fn test_parse_bare_nodes() {
        let source = "flowchart TD\n    A\n    B\n    A --> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes.len(), 2);
        assert_eq!(ast.nodes[0].id, "A");
        assert_eq!(ast.nodes[0].label, None); // bare node, no label
    }

    #[test]
    fn test_parse_inline_edge_labels_with_spaces() {
        // Test case #4 from examples comparison - edge labels with spaces
        let source = r#"graph LR
    A[Square Rect] -- Link text --> B((Circle))
    A --> C(Round Rect)
    B --> D{Rhombus}
    C --> D"#;
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes.len(), 4);
        assert_eq!(ast.edges.len(), 4);
        assert_eq!(ast.edges[0].label.as_deref(), Some("Link text"));
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
    }

    #[test]
    fn test_parse_double_circle_shape() {
        let source = "flowchart TD\n    A(((double circle)))";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes[0].shape, NodeShape::DoubleCircle);
        assert_eq!(ast.nodes[0].label.as_deref(), Some("double circle"));
    }

    #[test]
    fn test_parse_trapezoid_shape() {
        let source = "flowchart TD\n    A[/trapezoid\\]";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes[0].shape, NodeShape::Trapezoid);
        assert_eq!(ast.nodes[0].label.as_deref(), Some("trapezoid"));
    }

    #[test]
    fn test_parse_trapezoid_alt_shape() {
        let source = "flowchart TD\n    A[\\trapezoidAlt/]";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes[0].shape, NodeShape::TrapezoidAlt);
        assert_eq!(ast.nodes[0].label.as_deref(), Some("trapezoidAlt"));
    }

    #[test]
    fn test_parse_parallelogram_shape() {
        let source = "flowchart TD\n    A[/parallelogram/]";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes[0].shape, NodeShape::Parallelogram);
        assert_eq!(ast.nodes[0].label.as_deref(), Some("parallelogram"));
    }

    #[test]
    fn test_parse_parallelogram_alt_shape() {
        let source = "flowchart TD\n    A[\\parallelogramAlt\\]";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes[0].shape, NodeShape::ParallelogramAlt);
        assert_eq!(ast.nodes[0].label.as_deref(), Some("parallelogramAlt"));
    }

    #[test]
    fn test_parse_asymmetric_shape() {
        let source = "flowchart TD\n    A>asymmetric]";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes[0].shape, NodeShape::Asymmetric);
        assert_eq!(ast.nodes[0].label.as_deref(), Some("asymmetric"));
    }

    #[test]
    fn test_parse_subroutine_shape() {
        let source = "flowchart TD\n    A[[subroutine]]";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes[0].shape, NodeShape::Subroutine);
        assert_eq!(ast.nodes[0].label.as_deref(), Some("subroutine"));
    }

    #[test]
    fn test_parse_cylinder_shape() {
        let source = "flowchart TD\n    A[(cylinder)]";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes[0].shape, NodeShape::Cylinder);
        assert_eq!(ast.nodes[0].label.as_deref(), Some("cylinder"));
    }

    #[test]
    fn test_parse_subgraph_with_quoted_label() {
        let source = r#"flowchart TD
    subgraph sg1["My Label"]
        A --> B
    end"#;
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.subgraphs.len(), 1);
        assert_eq!(ast.subgraphs[0].id, "sg1");
        assert_eq!(ast.subgraphs[0].label.as_deref(), Some("My Label"));
    }

    #[test]
    fn test_parse_subgraph_with_bracket_label() {
        let source = r#"flowchart TD
    subgraph sg1[Bracket Label]
        A --> B
    end"#;
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.subgraphs[0].label.as_deref(), Some("Bracket Label"));
    }

    #[test]
    fn test_parse_comment_lines_ignored() {
        let source = "flowchart TD\n    %% This is a comment\n    A --> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes.len(), 2);
        assert_eq!(ast.edges.len(), 1);
    }

    #[test]
    fn test_parse_class_def_and_assign() {
        let source = r#"flowchart TD
    A[Node A]
    B[Node B]
    classDef highlight fill:#ff0,stroke:#333
    class A,B highlight"#;
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.class_defs.len(), 1);
        assert_eq!(ast.class_defs[0].name, "highlight");
        assert!(ast.class_defs[0].properties.fill.is_some());
        assert_eq!(ast.class_assignments.len(), 1);
        assert_eq!(ast.class_assignments[0].class_name, "highlight");
        assert_eq!(ast.class_assignments[0].node_ids, vec!["A", "B"]);
    }

    #[test]
    fn test_parse_style_directive() {
        let source = "flowchart TD\n    A[Node]\n    style A fill:#f00,stroke:#000";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.style_overrides.len(), 1);
        assert_eq!(ast.style_overrides[0].node_id, "A");
        assert!(ast.style_overrides[0].properties.fill.is_some());
    }

    #[test]
    fn test_parse_all_node_shapes() {
        let source = r#"flowchart TD
    A[Rectangle]
    B(Rounded)
    C{Diamond}
    D((Circle))
    E(((DoubleCircle)))
    F([Stadium])
    G[[Subroutine]]
    H[(Cylinder)]
    I{{Hexagon}}
    J>Asymmetric]
    K[/Trapezoid\]
    L[\TrapezoidAlt/]
    M[/Parallelogram/]
    N[\ParallelogramAlt\]"#;
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes.len(), 14);
        assert_eq!(ast.nodes[0].shape, NodeShape::Rectangle);
        assert_eq!(ast.nodes[1].shape, NodeShape::RoundedRectangle);
        assert_eq!(ast.nodes[2].shape, NodeShape::Diamond);
        assert_eq!(ast.nodes[3].shape, NodeShape::Circle);
        assert_eq!(ast.nodes[4].shape, NodeShape::DoubleCircle);
        assert_eq!(ast.nodes[5].shape, NodeShape::Stadium);
        assert_eq!(ast.nodes[6].shape, NodeShape::Subroutine);
        assert_eq!(ast.nodes[7].shape, NodeShape::Cylinder);
        assert_eq!(ast.nodes[8].shape, NodeShape::Hexagon);
        assert_eq!(ast.nodes[9].shape, NodeShape::Asymmetric);
        assert_eq!(ast.nodes[10].shape, NodeShape::Trapezoid);
        assert_eq!(ast.nodes[11].shape, NodeShape::TrapezoidAlt);
        assert_eq!(ast.nodes[12].shape, NodeShape::Parallelogram);
        assert_eq!(ast.nodes[13].shape, NodeShape::ParallelogramAlt);
    }

    #[test]
    fn test_parse_direction_bt() {
        let source = "flowchart BT\n    A --> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.direction, Direction::BottomToTop);
    }

    #[test]
    fn test_parse_direction_rl() {
        let source = "flowchart RL\n    A --> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.direction, Direction::RightToLeft);
    }

    #[test]
    fn test_parse_thick_line_edge() {
        let source = "flowchart TD\n    A === B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Thick);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::None);
    }

    #[test]
    fn test_parse_dotted_line_edge() {
        let source = "flowchart TD\n    A -.- B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Dotted);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::None);
    }

    #[test]
    fn test_parse_thick_arrow_labeled() {
        let source = "flowchart TD\n    A == thick label ==> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Thick);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
        assert_eq!(ast.edges[0].label.as_deref(), Some("thick label"));
    }

    #[test]
    fn test_parse_solid_line_labeled() {
        let source = "flowchart TD\n    A -- line label --- B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::None);
        assert_eq!(ast.edges[0].label.as_deref(), Some("line label"));
    }

    #[test]
    fn test_parse_class_shorthand() {
        let source = "flowchart TD\n    A[Node]:::myClass --> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(
            ast.nodes[0].class_shorthand.as_deref(),
            Some("myClass")
        );
    }

    #[test]
    fn test_parse_subgraph_with_direction() {
        let source = r#"flowchart TD
    subgraph sg1
        direction LR
        A --> B
    end"#;
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.subgraphs[0].direction, Some(Direction::LeftToRight));
    }

    #[test]
    fn test_upsert_node_updates_existing() {
        let mut nodes = vec![NodeDef {
            id: "A".to_string(),
            label: None,
            shape: NodeShape::Rectangle,
            class_shorthand: None,
        }];
        let new_node = NodeDef {
            id: "A".to_string(),
            label: Some("Updated".to_string()),
            shape: NodeShape::Circle,
            class_shorthand: Some("cls".to_string()),
        };
        upsert_node(&mut nodes, new_node);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].label.as_deref(), Some("Updated"));
        assert_eq!(nodes[0].shape, NodeShape::Circle);
        assert_eq!(nodes[0].class_shorthand.as_deref(), Some("cls"));
    }

    #[test]
    fn test_parse_direction_invalid() {
        let result = parse_direction("INVALID");
        assert!(result.is_err());
        if let Err(crate::error::MermaidError::Parse { message, .. }) = result {
            assert!(message.contains("Unknown direction"));
        }
    }

    #[test]
    fn test_parse_direction_empty_and_whitespace() {
        let result = parse_direction("  ");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_directive_ignored() {
        let source = r#"flowchart TD
    %%{init: {'theme':'base'}}%%
    A --> B"#;
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes.len(), 2);
        assert_eq!(ast.edges.len(), 1);
    }

    #[test]
    fn test_upsert_node_update_with_no_label_no_class() {
        // Update existing node with a bare node (no label, no class_shorthand) - no changes
        let mut nodes = vec![NodeDef {
            id: "A".to_string(),
            label: Some("Original".to_string()),
            shape: NodeShape::Rectangle,
            class_shorthand: Some("cls".to_string()),
        }];
        let bare_node = NodeDef {
            id: "A".to_string(),
            label: None,
            shape: NodeShape::Rectangle,
            class_shorthand: None,
        };
        upsert_node(&mut nodes, bare_node);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].label.as_deref(), Some("Original"));
        assert_eq!(nodes[0].class_shorthand.as_deref(), Some("cls"));
    }

    #[test]
    fn test_edge_referencing_undeclared_nodes() {
        // A and B only appear in edges, not in node_stmt - they're implicitly declared
        let source = "flowchart TD\n    X --> Y\n    Y --> Z";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.nodes.len(), 3);
        assert_eq!(ast.edges.len(), 2);
        assert!(ast.nodes.iter().any(|n| n.id == "X"));
        assert!(ast.nodes.iter().any(|n| n.id == "Y"));
        assert!(ast.nodes.iter().any(|n| n.id == "Z"));
    }

    #[test]
    fn test_class_assign_with_class_keyword() {
        let source = r#"flowchart TD
    A[Node A]
    B[Node B]
    classDef highlight fill:#ff0
    class A,B highlight"#;
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.class_assignments.len(), 1);
        assert_eq!(ast.class_assignments[0].node_ids, vec!["A", "B"]);
        assert_eq!(ast.class_assignments[0].class_name, "highlight");
    }

    // ── New edge variant tests ──────────────────────────────────

    #[test]
    fn test_parse_circle_edge() {
        let source = "flowchart TD\n    A --o B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::None);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Circle);
    }

    #[test]
    fn test_parse_cross_edge() {
        let source = "flowchart TD\n    A --x B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::None);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Cross);
    }

    #[test]
    fn test_parse_bidirectional_arrow() {
        let source = "flowchart TD\n    A <--> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::Arrow);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
    }

    #[test]
    fn test_parse_double_circle_edge() {
        let source = "flowchart TD\n    A o--o B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::Circle);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Circle);
    }

    #[test]
    fn test_parse_double_cross_edge() {
        let source = "flowchart TD\n    A x--x B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::Cross);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Cross);
    }

    #[test]
    fn test_parse_invisible_link() {
        let source = "flowchart TD\n    A ~~~ B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Invisible);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::None);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::None);
    }

    #[test]
    fn test_parse_dotted_circle_edge() {
        let source = "flowchart TD\n    A -.-> B\n    C -.-o D";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Dotted);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
        assert_eq!(ast.edges[1].line_style, LineStyle::Dotted);
        assert_eq!(ast.edges[1].arrow_end, ArrowEnd::Circle);
    }

    #[test]
    fn test_parse_thick_circle_edge() {
        let source = "flowchart TD\n    A ==> B\n    C ==o D";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Thick);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
        assert_eq!(ast.edges[1].line_style, LineStyle::Thick);
        assert_eq!(ast.edges[1].arrow_end, ArrowEnd::Circle);
    }

    #[test]
    fn test_parse_thick_cross_edge() {
        let source = "flowchart TD\n    A ==x B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Thick);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Cross);
    }

    #[test]
    fn test_parse_dotted_line_labeled() {
        let source = "flowchart TD\n    A -. label .- B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Dotted);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::None);
        assert_eq!(ast.edges[0].label.as_deref(), Some("label"));
    }

    #[test]
    fn test_parse_thick_line_labeled() {
        let source = "flowchart TD\n    A == label === B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Thick);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::None);
        assert_eq!(ast.edges[0].label.as_deref(), Some("label"));
    }

    #[test]
    fn test_parse_bidirectional_dotted() {
        let source = "flowchart TD\n    A <-.-> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Dotted);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::Arrow);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
    }

    #[test]
    fn test_parse_bidirectional_thick() {
        let source = "flowchart TD\n    A <==> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Thick);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::Arrow);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
    }

    #[test]
    fn test_parse_cross_start_arrow_end() {
        let source = "flowchart TD\n    A x--> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::Cross);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
    }

    #[test]
    fn test_parse_circle_start_arrow_end() {
        let source = "flowchart TD\n    A o--> B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_start, ArrowEnd::Circle);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Arrow);
    }

    #[test]
    fn test_parse_longer_invisible_link() {
        let source = "flowchart TD\n    A ~~~~ B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Invisible);
    }

    #[test]
    fn test_parse_link_style_ignored() {
        let source = "flowchart TD\n    A --> B\n    linkStyle 0 stroke:#ff3,stroke-width:4px";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges.len(), 1);
        assert_eq!(ast.nodes.len(), 2);
    }

    #[test]
    fn test_parse_link_style_default_ignored() {
        let source = "flowchart TD\n    A --> B\n    linkStyle default stroke:#ff3";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges.len(), 1);
    }

    #[test]
    fn test_parse_solid_arrow_with_pipe_label() {
        let source = "flowchart TD\n    A --o|label| B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].line_style, LineStyle::Solid);
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Circle);
        assert_eq!(ast.edges[0].label.as_deref(), Some("label"));
    }

    #[test]
    fn test_parse_cross_edge_with_pipe_label() {
        let source = "flowchart TD\n    A --x|label| B";
        let ast = parse_flowchart(source).unwrap();
        assert_eq!(ast.edges[0].arrow_end, ArrowEnd::Cross);
        assert_eq!(ast.edges[0].label.as_deref(), Some("label"));
    }
}

#[test]
fn test_case5_parse_and_layout() {
    let source = r#"graph TB
    sq[Square shape] --> ci((Circle shape))

    subgraph A
        od>Odd shape]-- Two line<br/>edge comment --> ro
        di{Diamond with <br/> line break} -.-> ro(Rounded<br>square<br>shape)
        di==>ro2(Rounded square shape)
    end

    e --> od3>Really long text with linebreak<br>in an Odd shape]

    e((Inner / circle<br>and some odd <br>special characters)) --> f(,.?!+-*؛)

    cyr[Cyrillic]-->cyr2((Circle shape Начало));

     classDef green fill:#9f6,stroke:#333,stroke-width:2px;
     classDef orange fill:#f96,stroke:#333,stroke-width:4px;
     class sq,e green
     class di orange"#;

    let ast = parse_flowchart(source).unwrap();

    println!("\n=== Parsed AST ===");
    println!("Nodes: {}", ast.nodes.len());
    for node in &ast.nodes {
        println!(
            "  Node: {} shape={:?} label={:?}",
            node.id,
            node.shape,
            node.label.as_deref().unwrap_or("")
        );
    }

    println!("\nSubgraphs: {}", ast.subgraphs.len());
    for sg in &ast.subgraphs {
        println!(
            "  Subgraph: {} nodes={} edges={}",
            sg.id,
            sg.nodes.len(),
            sg.edges.len()
        );
        for node in &sg.nodes {
            println!("    Node: {} shape={:?}", node.id, node.shape);
        }
    }

    println!("\nClassDefs: {}", ast.class_defs.len());
    for cd in &ast.class_defs {
        println!("  ClassDef: {} fill={:?}", cd.name, cd.properties.fill);
    }

    println!("\nClassAssignments: {}", ast.class_assignments.len());
    for ca in &ast.class_assignments {
        println!("  Class: {} -> nodes {:?}", ca.class_name, ca.node_ids);
    }

    println!("\nAll edges: {}", ast.edges.len());
    for edge in &ast.edges {
        println!(
            "  Edge: {} -> {} (line_style={:?} arrow_end={:?} label={:?})",
            edge.from, edge.to, edge.line_style, edge.arrow_end, edge.label
        );
    }
}

#[test]
fn test_subgraph_node_with_shape() {
    let source = r#"graph TB
    subgraph A
        od>Odd shape] --> ro
    end"#;

    let ast = parse_flowchart(source).unwrap();
    println!("\n=== Subgraph Parsing ===");
    println!("Subgraphs: {}", ast.subgraphs.len());
    for sg in &ast.subgraphs {
        println!("  Subgraph: {}", sg.id);
        for node in &sg.nodes {
            println!(
                "    Node: {} shape={:?} label={:?}",
                node.id, node.shape, node.label
            );
        }
        for edge in &sg.edges {
            println!(
                "    Edge: {} -> {} (label={:?})",
                edge.from, edge.to, edge.label
            );
        }
    }

    // Check that od has the label
    let sg = &ast.subgraphs[0];
    let od_node = sg.nodes.iter().find(|n| n.id == "od").unwrap();
    assert_eq!(
        od_node.label.as_deref(),
        Some("Odd shape"),
        "od should have label 'Odd shape'"
    );
    assert_eq!(od_node.shape, NodeShape::Asymmetric);
}

#[test]
fn test_subgraph_with_edge_label() {
    let source = r#"graph TB
    subgraph A
        od>Odd shape]-- Two line<br/>edge comment --> ro
    end"#;

    let ast = parse_flowchart(source).unwrap();
    println!("\n=== Subgraph with Edge Label ===");
    println!("Subgraphs: {}", ast.subgraphs.len());
    for sg in &ast.subgraphs {
        println!("  Subgraph: {}", sg.id);
        for node in &sg.nodes {
            println!(
                "    Node: {} shape={:?} label={:?}",
                node.id, node.shape, node.label
            );
        }
        for edge in &sg.edges {
            println!(
                "    Edge: {} -> {} (label={:?})",
                edge.from, edge.to, edge.label
            );
        }
    }
}

#[test]
fn test_classdef_style_resolution() {
    use crate::layout::flowchart::graph_builder;

    let source = r#"graph TB
    sq[Square shape] --> ci((Circle shape))
    classDef green fill:#9f6,stroke:#333,stroke-width:2px;
    class sq green"#;

    let ast = parse_flowchart(source).unwrap();
    let class_defs = graph_builder::build_class_map(&ast.class_defs);
    let all_nodes = graph_builder::collect_all_nodes(&ast, &class_defs);

    println!("\n=== Style Resolution ===");
    for (id, (_node, style)) in &all_nodes {
        println!("Node: {}", id);
        println!("  fill: {:?}", style.fill);
        println!("  stroke: {:?}", style.stroke);
        println!("  stroke_width: {:?}", style.stroke_width);
    }
}

#[test]
fn test_classdef_colors_in_svg() {
    use crate::{render, RenderConfig};

    let source = r#"graph TB
    sq[Square shape] --> ci((Circle shape))
    classDef green fill:#9f6,stroke:#333,stroke-width:2px;
    class sq green"#;

    let config = RenderConfig::default();
    let svg = render(source, &config).unwrap().into_svg().unwrap();

    println!("\n=== Generated SVG ===");
    // Check if the green color is in the SVG
    if svg.contains("#9f6") {
        println!("✓ Found green color (#9f6) in SVG");
    } else {
        println!("✗ Green color (#9f6) NOT found in SVG");
        println!("SVG snippet (first 1000 chars):");
        println!("{}", &svg[..svg.len().min(1000)]);
    }
}

#[test]
fn test_complex_graph_all_nodes_rendered() {
    use crate::{render, RenderConfig};

    let source = r#"graph TB
    sq[Square shape] --> ci((Circle shape))

    subgraph A
        od>Odd shape]-- Two line<br/>edge comment --> ro
        di{Diamond with <br/> line break} -.-> ro(Rounded<br>square<br>shape)
        di==>ro2(Rounded square shape)
    end

    e --> od3>Really long text with linebreak<br>in an Odd shape]

    e((Inner / circle<br>and some odd <br>special characters)) --> f(,.?!+-*؛)

    cyr[Cyrillic]-->cyr2((Circle shape Начало));

     classDef green fill:#9f6,stroke:#333,stroke-width:2px;
     classDef orange fill:#f96,stroke:#333,stroke-width:4px;
     class sq,e green
     class di orange"#;

    let config = RenderConfig::default();
    let svg = render(source, &config).unwrap().into_svg().unwrap();

    // Check for colors
    println!("Green (#9f6) count: {}", svg.matches("#9f6").count());
    println!("Orange (#f96) count: {}", svg.matches("#f96").count());

    // Check all nodes are present
    for node in &[
        "sq", "ci", "od", "ro", "di", "ro2", "od3", "e", "f", "cyr", "cyr2",
    ] {
        if svg.contains(&format!("{}", node)) || svg.contains(&format!("id=\"{}\"", node)) {
            println!("✓ Node '{}' found in SVG", node);
        } else {
            println!("✗ Node '{}' NOT found in SVG", node);
        }
    }
}

#[test]
fn test_complex_graph_svg_output() {
    use crate::{render, RenderConfig};

    let source = r#"graph TB
    sq[Square shape] --> ci((Circle shape))

    subgraph A
        od>Odd shape]-- Two line<br/>edge comment --> ro
        di{Diamond with <br/> line break} -.-> ro(Rounded<br>square<br>shape)
        di==>ro2(Rounded square shape)
    end

    e --> od3>Really long text with linebreak<br>in an Odd shape]

    e((Inner / circle<br>and some odd <br>special characters)) --> f(,.?!+-*؛)

    cyr[Cyrillic]-->cyr2((Circle shape Начало));

     classDef green fill:#9f6,stroke:#333,stroke-width:2px;
     classDef orange fill:#f96,stroke:#333,stroke-width:4px;
     class sq,e green
     class di orange"#;

    let config = RenderConfig::default();
    let svg = render(source, &config).unwrap().into_svg().unwrap();

    // Save SVG to file for inspection
    std::fs::write("/tmp/case5_debug.svg", &svg).unwrap();
    println!("SVG saved to /tmp/case5_debug.svg");
    println!("SVG length: {} bytes", svg.len());

    // Print node count
    let node_count = svg.matches("<g transform").count();
    println!("Node count (g elements): {}", node_count);
}

#[test]
fn test_subgraph_membership_paths() {
    use crate::layout::flowchart::graph_builder;

    let source = r#"graph TB
    sq[Square shape] --> ci((Circle shape))

    subgraph A
        od>Odd shape]-- Two line<br/>edge comment --> ro
        di{Diamond with <br/> line break} -.-> ro(Rounded<br>square<br>shape)
        di==>ro2(Rounded square shape)
    end

    e --> od3>Really long text with linebreak<br>in an Odd shape]

    e((Inner / circle<br>and some odd <br>special characters)) --> f(,.?!+-*؛)

    cyr[Cyrillic]-->cyr2((Circle shape Начало));

     classDef green fill:#9f6,stroke:#333,stroke-width:2px;
     classDef orange fill:#f96,stroke:#333,stroke-width:4px;
     class sq,e green
     class di orange"#;

    let ast = parse_flowchart(source).unwrap();
    let membership = graph_builder::build_subgraph_membership(&ast);

    println!("\n=== Subgraph Membership ===");
    for (node_id, path) in &membership {
        if !path.is_empty() {
            println!("  {} -> {:?}", node_id, path);
        }
    }
}
