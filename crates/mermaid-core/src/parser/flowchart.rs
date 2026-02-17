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
        match pair.as_rule() {
            Rule::flowchart => {
                for inner in pair.into_inner() {
                    process_top_level(&mut ast, inner)?;
                }
            }
            _ => {}
        }
    }

    Ok(ast)
}

fn process_top_level(
    ast: &mut FlowchartAst,
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<()> {
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
        _ => {}
    }
    Ok(())
}

fn parse_direction(s: &str) -> Result<Direction> {
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

    // link_chain = { node_def ~ (edge ~ node_def)+ }
    // Items alternate: node_def, edge, node_def, edge, node_def, ...
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

        let (edge_type, edge_label) = parse_edge(edge_pair)?;
        let next_node = parse_node_def(next_node_pair)?;
        let next_id = next_node.id.clone();
        upsert_node(nodes, next_node);

        edges.push(EdgeDef {
            from: prev_id.clone(),
            to: next_id.clone(),
            edge_type,
            label: edge_label,
        });

        prev_id = next_id;
    }

    Ok(())
}

fn parse_edge(
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<(EdgeType, Option<String>)> {
    let mut edge_type = EdgeType::SolidArrow;
    let mut label = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::solid_arrow => edge_type = EdgeType::SolidArrow,
            Rule::solid_line => edge_type = EdgeType::SolidLine,
            Rule::dotted_arrow => edge_type = EdgeType::DottedArrow,
            Rule::dotted_line => edge_type = EdgeType::DottedLine,
            Rule::thick_arrow => edge_type = EdgeType::ThickArrow,
            Rule::thick_line => edge_type = EdgeType::ThickLine,
            Rule::solid_arrow_labeled => {
                edge_type = EdgeType::SolidArrow;
                label = Some(extract_edge_inline_label(inner));
            }
            Rule::solid_line_labeled => {
                edge_type = EdgeType::SolidLine;
                label = Some(extract_edge_inline_label(inner));
            }
            Rule::dotted_arrow_labeled => {
                edge_type = EdgeType::DottedArrow;
                label = Some(extract_edge_inline_label(inner));
            }
            Rule::thick_arrow_labeled => {
                edge_type = EdgeType::ThickArrow;
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

    Ok((edge_type, label))
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
        assert_eq!(ast.edges[0].edge_type, EdgeType::SolidArrow);
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
        assert_eq!(ast.edges[0].edge_type, EdgeType::SolidArrow);
        assert_eq!(ast.edges[1].edge_type, EdgeType::SolidLine);
        assert_eq!(ast.edges[2].edge_type, EdgeType::DottedArrow);
        assert_eq!(ast.edges[3].edge_type, EdgeType::ThickArrow);
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
}
