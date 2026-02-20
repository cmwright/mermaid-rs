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

    #[test]
    fn test_parse_pie_multiple_slices() {
        let source = r#"pie
    "Dogs" : 36
    "Cats" : 85
    "Rats" : 15
    "Birds" : 42"#;
        let ast = parse_pie(source).unwrap();
        assert_eq!(ast.title, None);
        assert_eq!(ast.slices.len(), 4);
        assert_eq!(ast.slices[0].label, "Dogs");
        assert_eq!(ast.slices[0].value, 36.0);
        assert_eq!(ast.slices[1].label, "Cats");
        assert_eq!(ast.slices[1].value, 85.0);
        assert_eq!(ast.slices[2].label, "Rats");
        assert_eq!(ast.slices[2].value, 15.0);
        assert_eq!(ast.slices[3].label, "Birds");
        assert_eq!(ast.slices[3].value, 42.0);
    }

    #[test]
    fn test_parse_pie_title_with_spaces() {
        let source = r#"pie title My Pie Chart Title
    "A" : 50
    "B" : 50"#;
        let ast = parse_pie(source).unwrap();
        assert_eq!(ast.title.as_deref(), Some("My Pie Chart Title"));
        assert_eq!(ast.slices.len(), 2);
    }

    #[test]
    fn test_parse_pie_no_title_no_slices() {
        let source = "pie";
        let ast = parse_pie(source).unwrap();
        assert_eq!(ast.title, None);
        assert_eq!(ast.slices.len(), 0);
    }

    #[test]
    fn test_parse_pie_semicolon_separator() {
        let source = "pie\n    \"A\" : 30;    \"B\" : 70";
        let ast = parse_pie(source).unwrap();
        assert_eq!(ast.slices.len(), 2);
        assert_eq!(ast.slices[0].label, "A");
        assert_eq!(ast.slices[0].value, 30.0);
        assert_eq!(ast.slices[1].label, "B");
        assert_eq!(ast.slices[1].value, 70.0);
    }

    #[test]
    fn test_parse_pie_labels_with_special_chars() {
        let source = r#"pie
    "Slice with spaces & stuff!" : 60
    "Another (slice)" : 40"#;
        let ast = parse_pie(source).unwrap();
        assert_eq!(ast.slices.len(), 2);
        assert_eq!(ast.slices[0].label, "Slice with spaces & stuff!");
        assert_eq!(ast.slices[1].label, "Another (slice)");
    }

    #[test]
    fn test_parse_pie_error_invalid_input() {
        // Syntactically invalid input triggers parse error (covers Span variant of LineColLocation)
        let source = "pie\n    \"A\" : not_a_number";
        let result = parse_pie(source);
        assert!(result.is_err());
        if let Err(crate::error::MermaidError::Parse { message, .. }) = result {
            assert!(message.contains("number") || message.contains("Parse"));
        }
    }

    #[test]
    fn test_parse_pie_error_malformed() {
        // Completely invalid input - no valid pie structure
        let source = "not pie chart at all";
        let result = parse_pie(source);
        assert!(result.is_err());
    }
}
