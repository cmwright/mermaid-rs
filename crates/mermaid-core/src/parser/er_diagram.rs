use pest::Parser;
use pest_derive::Parser;

use crate::ast::er_diagram::*;
use crate::error::{extract_snippet, MermaidError, Result};

#[derive(Parser)]
#[grammar = "parser/er_diagram.pest"]
struct ErDiagramPestParser;

/// Parse a Mermaid ER diagram source string into an ErDiagramAst.
pub fn parse_er_diagram(source: &str) -> Result<ErDiagramAst> {
    let pairs = ErDiagramPestParser::parse(Rule::er_diagram, source).map_err(|e| {
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

    let mut ast = ErDiagramAst::default();

    for pair in pairs {
        if pair.as_rule() == Rule::er_diagram {
            for inner in pair.into_inner() {
                process_top_level(&mut ast, inner)?;
            }
        }
    }

    Ok(ast)
}

fn process_top_level(
    ast: &mut ErDiagramAst,
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<()> {
    match pair.as_rule() {
        Rule::diagram_header | Rule::directive => {}
        Rule::relationship => {
            let rel = parse_relationship(pair)?;
            // Ensure entities referenced in relationships exist
            ensure_entity_exists(&mut ast.entities, &rel.entity_a);
            ensure_entity_exists(&mut ast.entities, &rel.entity_b);
            ast.relationships.push(rel);
        }
        Rule::entity_block => {
            let (id, attributes) = parse_entity_block(pair)?;
            upsert_entity(&mut ast.entities, id, attributes);
        }
        Rule::bare_entity => {
            let id = parse_bare_entity(pair)?;
            ensure_entity_exists(&mut ast.entities, &id);
        }
        _ => {}
    }
    Ok(())
}

fn parse_relationship(pair: pest::iterators::Pair<'_, Rule>) -> Result<RelationshipDef> {
    let mut entity_a = String::new();
    let mut entity_b = String::new();
    let mut cardinality_a = Cardinality::OnlyOne;
    let mut cardinality_b = Cardinality::OnlyOne;
    let mut relation_type = RelationType::Identifying;
    let mut label = None;
    let mut entity_count = 0;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::entity_id => {
                if entity_count == 0 {
                    entity_a = inner.as_str().to_string();
                } else {
                    entity_b = inner.as_str().to_string();
                }
                entity_count += 1;
            }
            Rule::left_cardinality => {
                cardinality_a = parse_cardinality(inner.as_str());
            }
            Rule::right_cardinality => {
                cardinality_b = parse_cardinality(inner.as_str());
            }
            Rule::relation_type => {
                relation_type = match inner.as_str() {
                    ".." => RelationType::NonIdentifying,
                    _ => RelationType::Identifying,
                };
            }
            Rule::rel_label => {
                let text = inner.as_str().trim();
                // Strip surrounding quotes if present
                let text = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
                    &text[1..text.len() - 1]
                } else {
                    text
                };
                label = Some(text.to_string());
            }
            _ => {}
        }
    }

    Ok(RelationshipDef {
        entity_a,
        cardinality_a,
        relation_type,
        cardinality_b,
        entity_b,
        label,
    })
}

fn parse_cardinality(s: &str) -> Cardinality {
    match s {
        "||" => Cardinality::OnlyOne,
        "|o" | "o|" => Cardinality::ZeroOrOne,
        "}|" | "|{" => Cardinality::OneOrMore,
        "}o" | "o{" => Cardinality::ZeroOrMore,
        _ => Cardinality::OnlyOne,
    }
}

fn parse_entity_block(
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<(String, Vec<Attribute>)> {
    let mut id = String::new();
    let mut attributes = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::entity_id => {
                id = inner.as_str().to_string();
            }
            Rule::attribute => {
                attributes.push(parse_attribute(inner)?);
            }
            _ => {}
        }
    }

    Ok((id, attributes))
}

fn parse_attribute(pair: pest::iterators::Pair<'_, Rule>) -> Result<Attribute> {
    let mut type_name = String::new();
    let mut name = String::new();
    let mut key = AttributeKey::None;
    let mut comment = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::attr_type => {
                type_name = inner.as_str().to_string();
            }
            Rule::attr_name => {
                name = inner.as_str().to_string();
            }
            Rule::attr_key => {
                key = match inner.as_str() {
                    "PK" => AttributeKey::PK,
                    "FK" => AttributeKey::FK,
                    "UK" => AttributeKey::UK,
                    _ => AttributeKey::None,
                };
            }
            Rule::attr_comment => {
                // Grammar always wraps comments in quotes; strip them
                let raw = inner.as_str();
                let stripped = if raw.len() >= 2 {
                    &raw[1..raw.len() - 1]
                } else {
                    raw
                };
                comment = Some(stripped.to_string());
            }
            _ => {}
        }
    }

    Ok(Attribute {
        type_name,
        name,
        key,
        comment,
    })
}

fn parse_bare_entity(pair: pest::iterators::Pair<'_, Rule>) -> Result<String> {
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::entity_id {
            return Ok(inner.as_str().to_string());
        }
    }
    Ok(String::new())
}

/// Ensure an entity exists in the list (for implicitly declared entities from relationships).
fn ensure_entity_exists(entities: &mut Vec<EntityDef>, id: &str) {
    if !entities.iter().any(|e| e.id == id) {
        entities.push(EntityDef {
            id: id.to_string(),
            alias: None,
            attributes: Vec::new(),
        });
    }
}

/// Insert or update an entity (merge attributes when entity block is encountered).
fn upsert_entity(entities: &mut Vec<EntityDef>, id: String, attributes: Vec<Attribute>) {
    if let Some(existing) = entities.iter_mut().find(|e| e.id == id) {
        if !attributes.is_empty() {
            existing.attributes = attributes;
        }
    } else {
        entities.push(EntityDef {
            id,
            alias: None,
            attributes,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_relationship() {
        let source = "erDiagram\n    CUSTOMER ||--o{ ORDER : places";
        let ast = parse_er_diagram(source).unwrap();
        assert_eq!(ast.relationships.len(), 1);
        assert_eq!(ast.relationships[0].entity_a, "CUSTOMER");
        assert_eq!(ast.relationships[0].entity_b, "ORDER");
        assert_eq!(ast.relationships[0].cardinality_a, Cardinality::OnlyOne);
        assert_eq!(ast.relationships[0].cardinality_b, Cardinality::ZeroOrMore);
        assert_eq!(ast.relationships[0].relation_type, RelationType::Identifying);
        assert_eq!(ast.relationships[0].label.as_deref(), Some("places"));
    }

    #[test]
    fn test_parse_non_identifying_relationship() {
        let source = "erDiagram\n    CUSTOMER ||..o{ ORDER : places";
        let ast = parse_er_diagram(source).unwrap();
        assert_eq!(
            ast.relationships[0].relation_type,
            RelationType::NonIdentifying
        );
    }

    #[test]
    fn test_parse_entity_with_attributes() {
        let source = r#"erDiagram
    CUSTOMER {
        string name PK
        int age
        string email UK "the email"
    }"#;
        let ast = parse_er_diagram(source).unwrap();
        let entity = ast.entities.iter().find(|e| e.id == "CUSTOMER").unwrap();
        assert_eq!(entity.attributes.len(), 3);
        assert_eq!(entity.attributes[0].type_name, "string");
        assert_eq!(entity.attributes[0].name, "name");
        assert_eq!(entity.attributes[0].key, AttributeKey::PK);
        assert_eq!(entity.attributes[1].type_name, "int");
        assert_eq!(entity.attributes[1].name, "age");
        assert_eq!(entity.attributes[1].key, AttributeKey::None);
        assert_eq!(entity.attributes[2].key, AttributeKey::UK);
        assert_eq!(
            entity.attributes[2].comment.as_deref(),
            Some("the email")
        );
    }

    #[test]
    fn test_parse_relationship_and_entity_block() {
        let source = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    CUSTOMER {
        string name PK
    }"#;
        let ast = parse_er_diagram(source).unwrap();
        assert_eq!(ast.relationships.len(), 1);
        let customer = ast.entities.iter().find(|e| e.id == "CUSTOMER").unwrap();
        assert_eq!(customer.attributes.len(), 1);
        assert_eq!(customer.attributes[0].name, "name");
    }

    #[test]
    fn test_implicit_entities_from_relationships() {
        let source = "erDiagram\n    A ||--|{ B : has";
        let ast = parse_er_diagram(source).unwrap();
        assert!(ast.entities.iter().any(|e| e.id == "A"));
        assert!(ast.entities.iter().any(|e| e.id == "B"));
    }

    #[test]
    fn test_parse_all_cardinalities() {
        assert_eq!(parse_cardinality("||"), Cardinality::OnlyOne);
        assert_eq!(parse_cardinality("|o"), Cardinality::ZeroOrOne);
        assert_eq!(parse_cardinality("o|"), Cardinality::ZeroOrOne);
        assert_eq!(parse_cardinality("}|"), Cardinality::OneOrMore);
        assert_eq!(parse_cardinality("|{"), Cardinality::OneOrMore);
        assert_eq!(parse_cardinality("}o"), Cardinality::ZeroOrMore);
        assert_eq!(parse_cardinality("o{"), Cardinality::ZeroOrMore);
    }

    #[test]
    fn test_parse_multiple_relationships() {
        let source = r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains"#;
        let ast = parse_er_diagram(source).unwrap();
        assert_eq!(ast.relationships.len(), 2);
        assert_eq!(ast.relationships[1].entity_a, "ORDER");
        assert_eq!(ast.relationships[1].entity_b, "LINE-ITEM");
    }

    #[test]
    fn test_parse_comments_ignored() {
        let source = "erDiagram\n    %% This is a comment\n    A ||--|{ B : has";
        let ast = parse_er_diagram(source).unwrap();
        assert_eq!(ast.relationships.len(), 1);
    }

    #[test]
    fn test_parse_quoted_label() {
        let source = r#"erDiagram
    CUSTOMER ||--o{ ORDER : "places orders""#;
        let ast = parse_er_diagram(source).unwrap();
        assert_eq!(
            ast.relationships[0].label.as_deref(),
            Some("places orders")
        );
    }

    #[test]
    fn test_parse_error_invalid_input() {
        let result = parse_er_diagram("erDiagram\n    |||BROKEN|||");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_zero_or_one_cardinality_in_relationship() {
        let source = "erDiagram\n    PERSON ||--o| ADDRESS : \"lives at\"";
        let ast = parse_er_diagram(source).unwrap();
        assert_eq!(ast.relationships[0].cardinality_a, Cardinality::OnlyOne);
        assert_eq!(ast.relationships[0].cardinality_b, Cardinality::ZeroOrOne);
    }

    #[test]
    fn test_parse_fk_attribute() {
        let source = r#"erDiagram
    ORDER {
        int id PK
        int customerId FK
    }"#;
        let ast = parse_er_diagram(source).unwrap();
        let order = ast.entities.iter().find(|e| e.id == "ORDER").unwrap();
        assert_eq!(order.attributes[1].key, AttributeKey::FK);
    }

    #[test]
    fn test_parse_cardinality_unknown_fallback() {
        assert_eq!(parse_cardinality("??"), Cardinality::OnlyOne);
    }

    #[test]
    fn test_parse_entity_block_without_prior_relationship() {
        let source = r#"erDiagram
    STANDALONE {
        string name
    }"#;
        let ast = parse_er_diagram(source).unwrap();
        assert_eq!(ast.entities.len(), 1);
        assert_eq!(ast.entities[0].id, "STANDALONE");
        assert_eq!(ast.entities[0].attributes.len(), 1);
    }

    #[test]
    fn test_parse_directive_ignored() {
        let source = "erDiagram\n    %%{init: {}}%%\n    A ||--|{ B : has";
        let ast = parse_er_diagram(source).unwrap();
        assert_eq!(ast.relationships.len(), 1);
    }

    #[test]
    fn test_parse_bare_entity() {
        let source = "erDiagram\n    STANDALONE\n    A ||--|{ B : has";
        let ast = parse_er_diagram(source).unwrap();
        assert!(ast.entities.iter().any(|e| e.id == "STANDALONE"));
        assert!(
            ast.entities
                .iter()
                .find(|e| e.id == "STANDALONE")
                .unwrap()
                .attributes
                .is_empty()
        );
    }

    #[test]
    fn test_parse_bare_entity_no_duplicates() {
        // Bare entity that also appears in a relationship shouldn't duplicate
        let source = "erDiagram\n    A\n    A ||--|{ B : has";
        let ast = parse_er_diagram(source).unwrap();
        let count = ast.entities.iter().filter(|e| e.id == "A").count();
        assert_eq!(count, 1);
    }
}
