use pest::Parser;
use pest_derive::Parser;

use crate::ast::gitgraph::*;
use crate::error::{extract_snippet, MermaidError, Result};

#[derive(Parser)]
#[grammar = "parser/gitgraph.pest"]
struct GitGraphPestParser;

/// Parse a Mermaid gitGraph source string into a GitGraphAst.
pub fn parse_gitgraph(source: &str) -> Result<GitGraphAst> {
    let pairs = GitGraphPestParser::parse(Rule::gitgraph, source).map_err(|e| {
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

    let mut ast = GitGraphAst::default();

    for pair in pairs {
        if pair.as_rule() == Rule::gitgraph {
            for inner in pair.into_inner() {
                process_git_statement(&mut ast, inner)?;
            }
        }
    }

    Ok(ast)
}

fn process_git_statement(
    ast: &mut GitGraphAst,
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<()> {
    match pair.as_rule() {
        Rule::gitgraph_header => {}
        Rule::commit_stmt => {
            let commit = parse_commit(pair)?;
            ast.commands.push(GitCommand::Commit(commit));
        }
        Rule::branch_stmt => {
            let name = extract_branch_name(pair);
            ast.commands.push(GitCommand::Branch(BranchDef { name }));
        }
        Rule::checkout_stmt => {
            let name = extract_branch_name(pair);
            ast.commands
                .push(GitCommand::Checkout(CheckoutDef { name }));
        }
        Rule::merge_stmt => {
            let branch = extract_branch_name(pair);
            ast.commands.push(GitCommand::Merge(MergeDef { branch }));
        }
        _ => {}
    }
    Ok(())
}

fn parse_commit(pair: pest::iterators::Pair<'_, Rule>) -> Result<CommitDef> {
    let mut def = CommitDef {
        id: None,
        message: None,
        tag: None,
        commit_type: CommitType::Normal,
    };

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::commit_id => {
                def.id = Some(strip_quotes(extract_quoted_value(inner)));
            }
            Rule::commit_tag => {
                def.tag = Some(strip_quotes(extract_quoted_value(inner)));
            }
            Rule::commit_type => {
                for type_inner in inner.into_inner() {
                    if type_inner.as_rule() == Rule::commit_type_value {
                        def.commit_type = match type_inner.as_str() {
                            "REVERSE" => CommitType::Reverse,
                            "HIGHLIGHT" => CommitType::Highlight,
                            _ => CommitType::Normal,
                        };
                    }
                }
            }
            Rule::commit_message => {
                def.message = Some(strip_quotes(extract_quoted_value(inner)));
            }
            _ => {}
        }
    }

    Ok(def)
}

fn extract_branch_name(pair: pest::iterators::Pair<'_, Rule>) -> String {
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::branch_name {
            return inner.as_str().to_string();
        }
    }
    String::new()
}

fn extract_quoted_value(pair: pest::iterators::Pair<'_, Rule>) -> String {
    let fallback = pair.as_str().to_string();
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::quoted_value {
            return inner.as_str().to_string();
        }
    }
    fallback
}

fn strip_quotes(s: String) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_gitgraph() {
        let source = r#"gitGraph:
    commit "Ashish"
    branch newbranch
    checkout newbranch
    commit id:"1111"
    commit tag:"test"
    checkout main
    commit type: HIGHLIGHT
    commit
    merge newbranch
    commit
    branch b2
    commit"#;
        let ast = parse_gitgraph(source).unwrap();
        assert_eq!(ast.commands.len(), 12);

        if let GitCommand::Commit(c) = &ast.commands[0] {
            assert_eq!(c.message.as_deref(), Some("Ashish"));
            assert_eq!(c.id, None);
        } else {
            panic!("Expected Commit");
        }

        if let GitCommand::Branch(b) = &ast.commands[1] {
            assert_eq!(b.name, "newbranch");
        } else {
            panic!("Expected Branch");
        }

        if let GitCommand::Commit(c) = &ast.commands[3] {
            assert_eq!(c.id.as_deref(), Some("1111"));
        } else {
            panic!("Expected Commit");
        }

        if let GitCommand::Commit(c) = &ast.commands[4] {
            assert_eq!(c.tag.as_deref(), Some("test"));
        } else {
            panic!("Expected Commit");
        }

        if let GitCommand::Commit(c) = &ast.commands[6] {
            assert_eq!(c.commit_type, CommitType::Highlight);
        } else {
            panic!("Expected Commit");
        }

        if let GitCommand::Merge(m) = &ast.commands[8] {
            assert_eq!(m.branch, "newbranch");
        } else {
            panic!("Expected Merge");
        }
    }

    #[test]
    fn test_parse_gitgraph_without_colon() {
        let source = "gitGraph\n    commit\n    commit";
        let ast = parse_gitgraph(source).unwrap();
        assert_eq!(ast.commands.len(), 2);
    }

    #[test]
    fn test_parse_bare_commit() {
        let source = "gitGraph\n    commit";
        let ast = parse_gitgraph(source).unwrap();
        assert_eq!(ast.commands.len(), 1);
        if let GitCommand::Commit(c) = &ast.commands[0] {
            assert_eq!(c.id, None);
            assert_eq!(c.message, None);
            assert_eq!(c.tag, None);
            assert_eq!(c.commit_type, CommitType::Normal);
        }
    }

    #[test]
    fn test_parse_commit_type_highlight() {
        let source = "gitGraph\n    commit type: HIGHLIGHT";
        let ast = parse_gitgraph(source).unwrap();
        assert_eq!(ast.commands.len(), 1);
        if let GitCommand::Commit(c) = &ast.commands[0] {
            assert_eq!(c.commit_type, CommitType::Highlight);
        } else {
            panic!("Expected Commit");
        }
    }

    #[test]
    fn test_parse_commit_type_reverse() {
        let source = "gitGraph\n    commit type: REVERSE";
        let ast = parse_gitgraph(source).unwrap();
        assert_eq!(ast.commands.len(), 1);
        if let GitCommand::Commit(c) = &ast.commands[0] {
            assert_eq!(c.commit_type, CommitType::Reverse);
        } else {
            panic!("Expected Commit");
        }
    }

    #[test]
    fn test_parse_commit_type_normal() {
        let source = "gitGraph\n    commit type: NORMAL";
        let ast = parse_gitgraph(source).unwrap();
        assert_eq!(ast.commands.len(), 1);
        if let GitCommand::Commit(c) = &ast.commands[0] {
            assert_eq!(c.commit_type, CommitType::Normal);
        } else {
            panic!("Expected Commit");
        }
    }

    #[test]
    fn test_parse_commit_with_message() {
        let source = r#"gitGraph
    commit "some message""#;
        let ast = parse_gitgraph(source).unwrap();
        assert_eq!(ast.commands.len(), 1);
        if let GitCommand::Commit(c) = &ast.commands[0] {
            assert_eq!(c.message.as_deref(), Some("some message"));
        } else {
            panic!("Expected Commit");
        }
    }

    #[test]
    fn test_parse_commit_with_id_and_tag() {
        let source = r#"gitGraph
    commit id:"abc123" tag:"v1.0""#;
        let ast = parse_gitgraph(source).unwrap();
        assert_eq!(ast.commands.len(), 1);
        if let GitCommand::Commit(c) = &ast.commands[0] {
            assert_eq!(c.id.as_deref(), Some("abc123"));
            assert_eq!(c.tag.as_deref(), Some("v1.0"));
        } else {
            panic!("Expected Commit");
        }
    }

    #[test]
    fn test_parse_commit_with_all_options() {
        let source = r#"gitGraph
    commit id:"c1" type: HIGHLIGHT tag:"release""#;
        let ast = parse_gitgraph(source).unwrap();
        if let GitCommand::Commit(c) = &ast.commands[0] {
            assert_eq!(c.id.as_deref(), Some("c1"));
            assert_eq!(c.commit_type, CommitType::Highlight);
            assert_eq!(c.tag.as_deref(), Some("release"));
        } else {
            panic!("Expected Commit");
        }
    }

    #[test]
    fn test_parse_checkout_and_merge() {
        let source = r#"gitGraph
    commit
    branch feature
    checkout feature
    commit
    checkout main
    merge feature"#;
        let ast = parse_gitgraph(source).unwrap();
        assert_eq!(ast.commands.len(), 6);

        if let GitCommand::Branch(b) = &ast.commands[1] {
            assert_eq!(b.name, "feature");
        } else {
            panic!("Expected Branch");
        }

        if let GitCommand::Checkout(c) = &ast.commands[2] {
            assert_eq!(c.name, "feature");
        } else {
            panic!("Expected Checkout");
        }

        if let GitCommand::Checkout(c) = &ast.commands[4] {
            assert_eq!(c.name, "main");
        } else {
            panic!("Expected Checkout");
        }

        if let GitCommand::Merge(m) = &ast.commands[5] {
            assert_eq!(m.branch, "feature");
        } else {
            panic!("Expected Merge");
        }
    }

    #[test]
    fn test_strip_quotes_helper() {
        assert_eq!(strip_quotes("\"hello\"".to_string()), "hello");
        assert_eq!(strip_quotes("noquotes".to_string()), "noquotes");
        assert_eq!(strip_quotes("\"\"".to_string()), "");
        assert_eq!(strip_quotes("x".to_string()), "x");
    }
}
