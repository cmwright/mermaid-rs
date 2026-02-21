use pest::Parser;
use pest_derive::Parser;

use crate::ast::architecture::*;
use crate::error::{extract_snippet, MermaidError, Result};

#[derive(Parser)]
#[grammar = "parser/architecture.pest"]
struct ArchitecturePestParser;

/// Parse a Mermaid architecture-beta source string into an ArchitectureAst.
pub fn parse_architecture(source: &str) -> Result<ArchitectureAst> {
    let pairs =
        ArchitecturePestParser::parse(Rule::architecture, source).map_err(|e| {
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

    let mut ast = ArchitectureAst::default();

    for pair in pairs {
        if pair.as_rule() == Rule::architecture {
            for inner in pair.into_inner() {
                process_statement(&mut ast, inner)?;
            }
        }
    }

    Ok(ast)
}

fn process_statement(
    ast: &mut ArchitectureAst,
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<()> {
    match pair.as_rule() {
        Rule::group_stmt => {
            ast.groups.push(parse_group(pair)?);
        }
        Rule::service_stmt => {
            ast.services.push(parse_service(pair)?);
        }
        Rule::junction_stmt => {
            ast.junctions.push(parse_junction(pair)?);
        }
        Rule::edge_stmt => {
            ast.edges.push(parse_edge(pair)?);
        }
        _ => {}
    }
    Ok(())
}

fn parse_group(pair: pest::iterators::Pair<'_, Rule>) -> Result<GroupDef> {
    let mut inner = pair.into_inner();
    let id = inner.next().unwrap().as_str().to_string();
    let icon = Some(inner.next().unwrap().as_str().to_string());
    let label = inner.next().unwrap().as_str().trim().to_string();
    let parent = inner.next().map(|p| p.as_str().to_string());
    Ok(GroupDef {
        id,
        icon,
        label,
        parent,
    })
}

fn parse_service(pair: pest::iterators::Pair<'_, Rule>) -> Result<ServiceDef> {
    let mut inner = pair.into_inner();
    let id = inner.next().unwrap().as_str().to_string();
    let icon = Some(inner.next().unwrap().as_str().to_string());
    let label = inner.next().unwrap().as_str().trim().to_string();
    let parent = inner.next().map(|p| p.as_str().to_string());
    Ok(ServiceDef {
        id,
        icon,
        label,
        parent,
    })
}

fn parse_junction(pair: pest::iterators::Pair<'_, Rule>) -> Result<JunctionDef> {
    let mut inner = pair.into_inner();
    let id = inner.next().unwrap().as_str().to_string();
    let parent = inner.next().map(|p| p.as_str().to_string());
    Ok(JunctionDef { id, parent })
}

fn parse_edge(pair: pest::iterators::Pair<'_, Rule>) -> Result<ArchEdge> {
    let mut inner = pair.into_inner();
    let from = parse_endpoint(inner.next().unwrap())?;
    let arrow_pair = inner.next().unwrap();
    let arrow_str = arrow_pair.as_str();
    let arrow_start = arrow_str.starts_with('<');
    let arrow_end = arrow_str.ends_with('>');
    let to = parse_endpoint(inner.next().unwrap())?;
    Ok(ArchEdge {
        from,
        to,
        arrow_start,
        arrow_end,
    })
}

fn parse_endpoint(pair: pest::iterators::Pair<'_, Rule>) -> Result<EdgeEndpoint> {
    let mut id = String::new();
    let mut group_modifier = false;
    let mut side = PortSide::Right;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::id => id = inner.as_str().to_string(),
            Rule::group_mod => group_modifier = true,
            Rule::port_side => {
                side = parse_port_side(inner.as_str());
            }
            _ => {}
        }
    }

    Ok(EdgeEndpoint {
        id,
        group_modifier,
        side,
    })
}

fn parse_port_side(s: &str) -> PortSide {
    match s {
        "T" => PortSide::Top,
        "B" => PortSide::Bottom,
        "L" => PortSide::Left,
        "R" => PortSide::Right,
        _ => PortSide::Right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal() {
        let source = "architecture-beta\n  service svc(server)[My Service]\n";
        let ast = parse_architecture(source).unwrap();
        assert_eq!(ast.services.len(), 1);
        assert_eq!(ast.services[0].id, "svc");
        assert_eq!(ast.services[0].icon.as_deref(), Some("server"));
        assert_eq!(ast.services[0].label, "My Service");
        assert!(ast.services[0].parent.is_none());
    }

    #[test]
    fn parse_group_with_parent() {
        let source = "architecture-beta\n  group app(server)[Application]\n  group api(server)[API Layer] in app\n";
        let ast = parse_architecture(source).unwrap();
        assert_eq!(ast.groups.len(), 2);
        assert_eq!(ast.groups[0].id, "app");
        assert!(ast.groups[0].parent.is_none());
        assert_eq!(ast.groups[1].id, "api");
        assert_eq!(ast.groups[1].parent.as_deref(), Some("app"));
    }

    #[test]
    fn parse_service_in_group() {
        let source =
            "architecture-beta\n  group g(server)[G]\n  service s(server)[S] in g\n";
        let ast = parse_architecture(source).unwrap();
        assert_eq!(ast.services[0].parent.as_deref(), Some("g"));
    }

    #[test]
    fn parse_junction() {
        let source = "architecture-beta\n  junction j1\n  junction j2 in g\n";
        let ast = parse_architecture(source).unwrap();
        assert_eq!(ast.junctions.len(), 2);
        assert_eq!(ast.junctions[0].id, "j1");
        assert!(ast.junctions[0].parent.is_none());
        assert_eq!(ast.junctions[1].parent.as_deref(), Some("g"));
    }

    #[test]
    fn parse_edge_both_arrows() {
        let source = "architecture-beta\n  service a(server)[A]\n  service b(server)[B]\n  a:R --> L:b\n";
        let ast = parse_architecture(source).unwrap();
        assert_eq!(ast.edges.len(), 1);
        let edge = &ast.edges[0];
        assert_eq!(edge.from.id, "a");
        assert_eq!(edge.from.side, PortSide::Right);
        assert!(!edge.arrow_start);
        assert!(edge.arrow_end);
        assert_eq!(edge.to.id, "b");
        assert_eq!(edge.to.side, PortSide::Left);
    }

    #[test]
    fn parse_edge_no_arrows() {
        let source = "architecture-beta\n  service a(server)[A]\n  service b(server)[B]\n  a:R -- L:b\n";
        let ast = parse_architecture(source).unwrap();
        let edge = &ast.edges[0];
        assert!(!edge.arrow_start);
        assert!(!edge.arrow_end);
    }

    #[test]
    fn parse_edge_bidirectional() {
        let source = "architecture-beta\n  service a(server)[A]\n  service b(server)[B]\n  a:R <--> L:b\n";
        let ast = parse_architecture(source).unwrap();
        let edge = &ast.edges[0];
        assert!(edge.arrow_start);
        assert!(edge.arrow_end);
    }

    #[test]
    fn parse_comments() {
        let source = "architecture-beta\n  %% this is a comment\n  service s(server)[S]\n";
        let ast = parse_architecture(source).unwrap();
        assert_eq!(ast.services.len(), 1);
    }

    #[test]
    fn parse_all_port_sides() {
        let source = "architecture-beta\n  service a(s)[A]\n  service b(s)[B]\n  service c(s)[C]\n  service d(s)[D]\n  a:T -- B:b\n  c:L -- R:d\n";
        let ast = parse_architecture(source).unwrap();
        assert_eq!(ast.edges[0].from.side, PortSide::Top);
        assert_eq!(ast.edges[0].to.side, PortSide::Bottom);
        assert_eq!(ast.edges[1].from.side, PortSide::Left);
        assert_eq!(ast.edges[1].to.side, PortSide::Right);
    }

    #[test]
    fn parse_canonical_example() {
        let source = r#"architecture-beta
  group internet(internet)[Internet]
  group app(server)[Application]
  group data(database)[Data Layer]
  group api(server)[API Layer] in app
  group jobs(server)[Worker Layer] in app
  service user(internet)[User] in internet
  service lb(server)[Load Balancer] in api
  service apiSvc(server)[API Service] in api
  service worker(server)[Background Worker] in jobs
  service db(database)[Postgres] in data
  service cache(disk)[Cache] in data
  user:R --> L:lb
  lb:R --> L:apiSvc
  apiSvc:R --> L:db
  apiSvc:B --> T:cache
  apiSvc:B --> T:worker
  worker:R --> L:db
"#;
        let ast = parse_architecture(source).unwrap();
        assert_eq!(ast.groups.len(), 5);
        assert_eq!(ast.services.len(), 6);
        assert_eq!(ast.edges.len(), 6);
    }
}
