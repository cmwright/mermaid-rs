use pest::Parser;
use pest_derive::Parser;

use crate::ast::pie::*;
use crate::error::{extract_snippet, MermaidError, Result};

#[derive(Parser)]
#[grammar = "parser/pie.pest"]
struct PiePestParser;

/// Parse a Mermaid pie chart source string into a PieAst.
pub fn parse_pie(source: &str) -> Result<PieAst> {
    let pairs = PiePestParser::parse(Rule::pie_chart, source).map_err(|e| {
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

    let mut ast = PieAst::default();

    for pair in pairs {
        if pair.as_rule() == Rule::pie_chart {
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::pie_header => {
                        parse_header(&mut ast, inner)?;
                    }
                    Rule::pie_slice => {
                        let slice = parse_slice(inner)?;
                        ast.slices.push(slice);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(ast)
}

fn parse_header(ast: &mut PieAst, pair: pest::iterators::Pair<'_, Rule>) -> Result<()> {
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::pie_title {
            ast.title = Some(inner.as_str().trim().to_string());
        }
    }
    Ok(())
}

fn parse_slice(pair: pest::iterators::Pair<'_, Rule>) -> Result<PieSlice> {
    let mut label = String::new();
    let mut value = 0.0;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::pie_label => {
                // Remove quotes from the label
                let s = inner.as_str();
                label = s[1..s.len() - 1].to_string();
            }
            Rule::pie_value => {
                value = inner
                    .as_str()
                    .parse::<f64>()
                    .map_err(|_| MermaidError::Parse {
                        line: 0,
                        col: 0,
                        message: format!("Invalid number: {}", inner.as_str()),
                        source_snippet: None,
                    })?;
            }
            _ => {}
        }
    }

    Ok(PieSlice { label, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pie() {
        let source = r#"pie
    "Time spent looking for movie" : 90
    "Time spent watching it" : 10"#;
        let ast = parse_pie(source).unwrap();
        assert_eq!(ast.title, None);
        assert_eq!(ast.slices.len(), 2);
        assert_eq!(ast.slices[0].label, "Time spent looking for movie");
        assert_eq!(ast.slices[0].value, 90.0);
        assert_eq!(ast.slices[1].label, "Time spent watching it");
        assert_eq!(ast.slices[1].value, 10.0);
    }

    #[test]
    fn test_parse_pie_with_title() {
        let source = r#"pie title NETFLIX
    "Time spent looking for movie" : 90
    "Time spent watching it" : 10"#;
        let ast = parse_pie(source).unwrap();
        assert_eq!(ast.title.as_deref(), Some("NETFLIX"));
        assert_eq!(ast.slices.len(), 2);
    }

    #[test]
    fn test_parse_pie_single_slice() {
        let source = r#"pie title Single
    "Only slice" : 100"#;
        let ast = parse_pie(source).unwrap();
        assert_eq!(ast.title.as_deref(), Some("Single"));
        assert_eq!(ast.slices.len(), 1);
        assert_eq!(ast.slices[0].label, "Only slice");
        assert_eq!(ast.slices[0].value, 100.0);
    }

    #[test]
    fn test_parse_pie_decimal_values() {
        let source = r#"pie
    "Slice A" : 33.3
    "Slice B" : 66.7"#;
        let ast = parse_pie(source).unwrap();
        assert_eq!(ast.slices[0].value, 33.3);
        assert_eq!(ast.slices[1].value, 66.7);
    }
}
