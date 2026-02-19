use pest::Parser;
use pest_derive::Parser;

use crate::ast::sequence::*;
use crate::error::{extract_snippet, MermaidError, Result};

#[derive(Parser)]
#[grammar = "parser/sequence.pest"]
struct SequencePestParser;

/// Parse a Mermaid sequence diagram source string into a SequenceAst.
pub fn parse_sequence(source: &str) -> Result<SequenceAst> {
    let pairs = SequencePestParser::parse(Rule::sequence_diagram, source).map_err(|e| {
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

    let mut ast = SequenceAst::default();

    for pair in pairs {
        if pair.as_rule() == Rule::sequence_diagram {
            for inner in pair.into_inner() {
                process_seq_top_level(&mut ast, inner)?;
            }
        }
    }

    // Resolve implicit participants from messages
    resolve_implicit_participants(&mut ast);

    Ok(ast)
}

fn process_seq_top_level(
    ast: &mut SequenceAst,
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<()> {
    match pair.as_rule() {
        Rule::sequence_header => {}
        Rule::participant_stmt => {
            let p = parse_participant_stmt(pair, ParticipantKind::Participant)?;
            if !ast.participants.iter().any(|x| x.id == p.id) {
                ast.participants.push(p);
            }
        }
        Rule::actor_stmt => {
            let p = parse_participant_stmt(pair, ParticipantKind::Actor)?;
            if !ast.participants.iter().any(|x| x.id == p.id) {
                ast.participants.push(p);
            }
        }
        Rule::message_stmt => {
            let msg = parse_message(pair)?;
            ast.statements.push(SequenceStatement::Message(msg));
        }
        Rule::activate_stmt => {
            let id = extract_participant_id(pair);
            ast.statements.push(SequenceStatement::Activate(id));
        }
        Rule::deactivate_stmt => {
            let id = extract_participant_id(pair);
            ast.statements.push(SequenceStatement::Deactivate(id));
        }
        Rule::autonumber_stmt => {
            ast.autonumber = true;
        }
        Rule::note_stmt => {
            let note = parse_note(pair)?;
            ast.statements.push(SequenceStatement::Note(note));
        }
        Rule::block_alt => {
            let block = parse_block(pair, BlockKind::Alt)?;
            ast.statements.push(SequenceStatement::Block(block));
        }
        Rule::block_loop => {
            let block = parse_block(pair, BlockKind::Loop)?;
            ast.statements.push(SequenceStatement::Block(block));
        }
        Rule::block_opt => {
            let block = parse_block(pair, BlockKind::Opt)?;
            ast.statements.push(SequenceStatement::Block(block));
        }
        Rule::block_par => {
            let block = parse_block(pair, BlockKind::Par)?;
            ast.statements.push(SequenceStatement::Block(block));
        }
        Rule::block_critical => {
            let block = parse_block(pair, BlockKind::Critical)?;
            ast.statements.push(SequenceStatement::Block(block));
        }
        Rule::block_break => {
            let block = parse_block(pair, BlockKind::Break)?;
            ast.statements.push(SequenceStatement::Block(block));
        }
        Rule::block_rect => {
            let block = parse_block(pair, BlockKind::Rect)?;
            ast.statements.push(SequenceStatement::Block(block));
        }
        _ => {}
    }
    Ok(())
}

fn parse_participant_stmt(
    pair: pest::iterators::Pair<'_, Rule>,
    kind: ParticipantKind,
) -> Result<ParticipantDef> {
    let mut id = String::new();
    let mut display_name: Option<String> = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::participant_id => {
                id = inner.as_str().to_string();
            }
            Rule::display_name => {
                display_name = Some(inner.as_str().trim().to_string());
            }
            _ => {}
        }
    }

    Ok(ParticipantDef {
        id,
        display_name,
        kind,
    })
}

fn parse_message(pair: pest::iterators::Pair<'_, Rule>) -> Result<MessageDef> {
    let mut from = String::new();
    let mut to = String::new();
    let mut arrow = ArrowType::SolidArrow;
    let mut label = String::new();
    let mut activate_target = false;
    let mut deactivate_source = false;
    let mut id_count = 0;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::msg_participant_id => {
                if id_count == 0 {
                    from = inner.as_str().to_string();
                } else {
                    to = inner.as_str().to_string();
                }
                id_count += 1;
            }
            Rule::arrow_type => {
                arrow = parse_arrow_type(inner)?;
            }
            Rule::pre_activation => match inner.as_str() {
                "+" => activate_target = true,
                "-" => deactivate_source = true,
                _ => {}
            },
            Rule::post_activation => match inner.as_str() {
                "+" => activate_target = true,
                "-" => deactivate_source = true,
                _ => {}
            },
            Rule::message_text => {
                label = inner.as_str().trim().to_string();
            }
            _ => {}
        }
    }

    Ok(MessageDef {
        from,
        to,
        arrow,
        label,
        activate_target,
        deactivate_source,
    })
}

fn parse_arrow_type(pair: pest::iterators::Pair<'_, Rule>) -> Result<ArrowType> {
    if let Some(inner) = pair.into_inner().next() {
        return Ok(match inner.as_rule() {
            Rule::solid_arrow_seq => ArrowType::SolidArrow,
            Rule::dotted_arrow_seq => ArrowType::DottedArrow,
            Rule::solid_open_arrow => ArrowType::SolidOpen,
            Rule::dotted_open_arrow => ArrowType::DottedOpen,
            Rule::solid_open_paren => ArrowType::SolidParen,
            Rule::dotted_open_paren => ArrowType::DottedParen,
            Rule::solid_cross => ArrowType::SolidCross,
            Rule::dotted_cross => ArrowType::DottedCross,
            _ => ArrowType::SolidArrow,
        });
    }
    Ok(ArrowType::SolidArrow)
}

fn extract_participant_id(pair: pest::iterators::Pair<'_, Rule>) -> String {
    for inner in pair.into_inner() {
        if inner.as_rule() == Rule::participant_id {
            return inner.as_str().to_string();
        }
    }
    String::new()
}

fn parse_note(pair: pest::iterators::Pair<'_, Rule>) -> Result<NoteDef> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::note_left_of => {
            let (participants, text) = extract_note_parts(inner);
            Ok(NoteDef {
                position: NotePosition::LeftOf,
                participants,
                text,
            })
        }
        Rule::note_right_of => {
            let (participants, text) = extract_note_parts(inner);
            Ok(NoteDef {
                position: NotePosition::RightOf,
                participants,
                text,
            })
        }
        Rule::note_over => {
            let (participants, text) = extract_note_over_parts(inner);
            Ok(NoteDef {
                position: NotePosition::Over,
                participants,
                text,
            })
        }
        _ => Ok(NoteDef {
            position: NotePosition::Over,
            participants: Vec::new(),
            text: String::new(),
        }),
    }
}

fn extract_note_parts(pair: pest::iterators::Pair<'_, Rule>) -> (Vec<String>, String) {
    let mut participants = Vec::new();
    let mut text = String::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::participant_id => {
                participants.push(inner.as_str().to_string());
            }
            Rule::note_text => {
                text = inner.as_str().trim().to_string();
            }
            _ => {}
        }
    }

    (participants, text)
}

fn extract_note_over_parts(pair: pest::iterators::Pair<'_, Rule>) -> (Vec<String>, String) {
    let mut participants = Vec::new();
    let mut text = String::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::participant_id_list => {
                for id_pair in inner.into_inner() {
                    if id_pair.as_rule() == Rule::participant_id {
                        participants.push(id_pair.as_str().to_string());
                    }
                }
            }
            Rule::note_text => {
                text = inner.as_str().trim().to_string();
            }
            _ => {}
        }
    }

    (participants, text)
}

fn parse_block(pair: pest::iterators::Pair<'_, Rule>, kind: BlockKind) -> Result<BlockDef> {
    let mut label = String::new();
    let mut sections = Vec::new();
    let mut current_stmts: Vec<SequenceStatement> = Vec::new();
    let mut first_body = true;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::block_label => {
                if first_body {
                    label = inner.as_str().trim().to_string();
                }
            }
            Rule::block_body => {
                let stmts = parse_block_body(inner)?;
                if first_body {
                    current_stmts = stmts;
                    first_body = false;
                } else {
                    // This shouldn't happen — sections handle their own body
                    current_stmts.extend(stmts);
                }
            }
            Rule::else_section | Rule::and_section | Rule::option_section => {
                // Push current section
                if !current_stmts.is_empty() || sections.is_empty() {
                    sections.push(BlockSection {
                        label: None,
                        statements: std::mem::take(&mut current_stmts),
                    });
                }
                // Parse the divider section
                let (sec_label, sec_stmts) = parse_section(inner)?;
                sections.push(BlockSection {
                    label: sec_label,
                    statements: sec_stmts,
                });
            }
            _ => {}
        }
    }

    // Push remaining statements as a section
    if sections.is_empty() {
        sections.push(BlockSection {
            label: None,
            statements: current_stmts,
        });
    } else if !current_stmts.is_empty() {
        // Shouldn't normally get here since sections grabbed their body
    }

    Ok(BlockDef {
        kind,
        label,
        sections,
    })
}

fn parse_section(
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<(Option<String>, Vec<SequenceStatement>)> {
    let mut label: Option<String> = None;
    let mut stmts = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::block_label => {
                let l = inner.as_str().trim().to_string();
                if !l.is_empty() {
                    label = Some(l);
                }
            }
            Rule::block_body => {
                stmts = parse_block_body(inner)?;
            }
            _ => {}
        }
    }

    Ok((label, stmts))
}

fn parse_block_body(pair: pest::iterators::Pair<'_, Rule>) -> Result<Vec<SequenceStatement>> {
    let mut stmts = Vec::new();
    // Create a temporary mini-AST to reuse the top-level parser
    let mut temp_ast = SequenceAst::default();

    for inner in pair.into_inner() {
        process_seq_top_level(&mut temp_ast, inner)?;
    }

    stmts.extend(temp_ast.statements);
    Ok(stmts)
}

/// Scan all messages for participant IDs not explicitly declared and add them
/// in order of first appearance.
fn resolve_implicit_participants(ast: &mut SequenceAst) {
    let mut seen: Vec<String> = ast.participants.iter().map(|p| p.id.clone()).collect();

    fn scan_statements(
        stmts: &[SequenceStatement],
        seen: &mut Vec<String>,
        implicit: &mut Vec<ParticipantDef>,
    ) {
        for stmt in stmts {
            match stmt {
                SequenceStatement::Message(msg) => {
                    for id in [&msg.from, &msg.to] {
                        if !seen.contains(id) {
                            seen.push(id.clone());
                            implicit.push(ParticipantDef {
                                id: id.clone(),
                                display_name: None,
                                kind: ParticipantKind::Participant,
                            });
                        }
                    }
                }
                SequenceStatement::Block(block) => {
                    for section in &block.sections {
                        scan_statements(&section.statements, seen, implicit);
                    }
                }
                SequenceStatement::Activate(id) | SequenceStatement::Deactivate(id) => {
                    if !seen.contains(id) {
                        seen.push(id.clone());
                        implicit.push(ParticipantDef {
                            id: id.clone(),
                            display_name: None,
                            kind: ParticipantKind::Participant,
                        });
                    }
                }
                SequenceStatement::Note(note) => {
                    for id in &note.participants {
                        if !seen.contains(id) {
                            seen.push(id.clone());
                            implicit.push(ParticipantDef {
                                id: id.clone(),
                                display_name: None,
                                kind: ParticipantKind::Participant,
                            });
                        }
                    }
                }
            }
        }
    }

    let mut implicit = Vec::new();
    scan_statements(&ast.statements, &mut seen, &mut implicit);
    ast.participants.extend(implicit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_sequence() {
        let source = "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.participants.len(), 2);
        assert_eq!(ast.participants[0].id, "Alice");
        assert_eq!(ast.participants[1].id, "Bob");
        assert_eq!(ast.statements.len(), 2);
    }

    #[test]
    fn test_parse_participant_declarations() {
        let source =
            "sequenceDiagram\n    participant A\n    participant B as Bob\n    A->>B: Hello";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.participants.len(), 2);
        assert_eq!(ast.participants[0].id, "A");
        assert_eq!(ast.participants[0].display_name, None);
        assert_eq!(ast.participants[1].id, "B");
        assert_eq!(ast.participants[1].display_name.as_deref(), Some("Bob"));
    }

    #[test]
    fn test_parse_actor() {
        let source =
            "sequenceDiagram\n    actor User\n    participant Server\n    User->>Server: Request";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.participants.len(), 2);
        assert_eq!(ast.participants[0].kind, ParticipantKind::Actor);
        assert_eq!(ast.participants[1].kind, ParticipantKind::Participant);
    }

    #[test]
    fn test_parse_arrow_types() {
        let source = "sequenceDiagram\n    A->>B: solid arrow\n    A-->>B: dotted arrow\n    A->B: solid open arrow\n    A-->B: dotted open arrow\n    A-)B: solid open paren\n    A--)B: dotted open paren\n    A-xB: solid cross\n    A--xB: dotted cross";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 8);
        if let SequenceStatement::Message(m) = &ast.statements[0] {
            assert_eq!(m.arrow, ArrowType::SolidArrow);
        }
        if let SequenceStatement::Message(m) = &ast.statements[1] {
            assert_eq!(m.arrow, ArrowType::DottedArrow);
        }
        if let SequenceStatement::Message(m) = &ast.statements[2] {
            assert_eq!(m.arrow, ArrowType::SolidOpen);
        }
        if let SequenceStatement::Message(m) = &ast.statements[3] {
            assert_eq!(m.arrow, ArrowType::DottedOpen);
        }
        if let SequenceStatement::Message(m) = &ast.statements[4] {
            assert_eq!(m.arrow, ArrowType::SolidParen);
        }
        if let SequenceStatement::Message(m) = &ast.statements[5] {
            assert_eq!(m.arrow, ArrowType::DottedParen);
        }
        if let SequenceStatement::Message(m) = &ast.statements[6] {
            assert_eq!(m.arrow, ArrowType::SolidCross);
        }
        if let SequenceStatement::Message(m) = &ast.statements[7] {
            assert_eq!(m.arrow, ArrowType::DottedCross);
        }
    }

    #[test]
    fn test_parse_activation_syntax() {
        // Test activation marker before target (e.g., ->>+target)
        let source = "sequenceDiagram\n    A->>+B: Activate B\n    B-->>-A: Deactivate A";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 2);
        if let SequenceStatement::Message(m) = &ast.statements[0] {
            assert_eq!(m.from, "A");
            assert_eq!(m.to, "B");
            assert!(m.activate_target);
            assert!(!m.deactivate_source);
        }
        if let SequenceStatement::Message(m) = &ast.statements[1] {
            assert_eq!(m.from, "B");
            assert_eq!(m.to, "A");
            assert!(!m.activate_target);
            assert!(m.deactivate_source);
        }

        // Test activation marker after target (e.g., ->>target+)
        let source2 = "sequenceDiagram\n    A->>B+: Activate B\n    B-->>A-: Deactivate A";
        let ast2 = parse_sequence(source2).unwrap();
        if let SequenceStatement::Message(m) = &ast2.statements[0] {
            assert!(m.activate_target);
        }
        if let SequenceStatement::Message(m) = &ast2.statements[1] {
            assert!(m.deactivate_source);
        }

        // Test combined activation markers
        let source3 = "sequenceDiagram\n    A->>+B+: Activate B\n    B-->>-A-: Deactivate A";
        let ast3 = parse_sequence(source3).unwrap();
        if let SequenceStatement::Message(m) = &ast3.statements[0] {
            assert!(m.activate_target);
        }
        if let SequenceStatement::Message(m) = &ast3.statements[1] {
            assert!(m.deactivate_source);
        }
    }

    #[test]
    fn test_parse_alt_block() {
        let source = "sequenceDiagram\n    A->>B: request\n    alt Success\n        B->>A: 200 OK\n    else Failure\n        B->>A: 500 Error\n    end";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 2); // message + block
        if let SequenceStatement::Block(block) = &ast.statements[1] {
            assert_eq!(block.kind, BlockKind::Alt);
            assert_eq!(block.label, "Success");
            assert_eq!(block.sections.len(), 2);
            assert_eq!(block.sections[0].statements.len(), 1);
            assert_eq!(block.sections[1].label.as_deref(), Some("Failure"));
            assert_eq!(block.sections[1].statements.len(), 1);
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_parse_self_message() {
        let source = "sequenceDiagram\n    A->>A: Self call";
        let ast = parse_sequence(source).unwrap();
        if let SequenceStatement::Message(m) = &ast.statements[0] {
            assert_eq!(m.from, "A");
            assert_eq!(m.to, "A");
            assert_eq!(m.label, "Self call");
        }
    }

    #[test]
    fn test_parse_complex_test_case() {
        let source = r#"sequenceDiagram
    actor User
    participant UI as Factor UI
    participant Kratos as Ory Kratos
    participant IdP as External IdP<br/>(Okta / Azure AD)
    participant SvcUsers as svc-users-v2
    participant DB as User Service DB
    User->>UI: Enter email (alice@acme.com)
    UI->>Kratos: POST /self-service/login {identifier}
    Kratos->>Kratos: Resolve domain → Ory Org → SSO connection
    alt SSO configured
        Kratos->>IdP: SAML AuthnRequest / OIDC /authorize
        IdP->>User: Authentication prompt
        User->>IdP: Authenticate
        IdP->>Kratos: SAML Response / OIDC callback
        Kratos->>Kratos: Create/update identity, assign to Ory Org, issue session
    else Password only
        Kratos->>UI: Show password prompt
        User->>UI: Enter password
        UI->>Kratos: Submit credentials
        Kratos->>Kratos: Validate, issue session
    end
    Kratos->>SvcUsers: Webhook: after.login {identity, session}
    SvcUsers->>DB: Upsert user record (JIT if new)
    UI->>SvcUsers: POST /v1/auth/token (session cookie)
    SvcUsers->>Kratos: GET /sessions/whoami (validate session)
    Kratos-->>SvcUsers: Session + identity
    SvcUsers->>DB: Load roles, entitlements, flags
    SvcUsers-->>UI: Self-minted JWT (ES256)"#;
        let ast = parse_sequence(source).unwrap();
        // 6 explicit participants (1 actor + 5 participants)
        assert_eq!(ast.participants.len(), 6);
        assert_eq!(ast.participants[0].id, "User");
        assert_eq!(ast.participants[0].kind, ParticipantKind::Actor);
        assert_eq!(ast.participants[1].id, "UI");
        assert_eq!(
            ast.participants[1].display_name.as_deref(),
            Some("Factor UI")
        );
        assert_eq!(ast.participants[5].id, "DB");

        // Count messages and blocks
        let mut msg_count = 0;
        let mut block_count = 0;
        for stmt in &ast.statements {
            match stmt {
                SequenceStatement::Message(_) => msg_count += 1,
                SequenceStatement::Block(_) => block_count += 1,
                _ => {}
            }
        }
        // 3 messages before alt + 7 after alt = 10 top-level messages + 1 alt block
        assert_eq!(msg_count, 10);
        assert_eq!(block_count, 1);

        // Check alt block structure
        if let SequenceStatement::Block(block) = &ast.statements[3] {
            assert_eq!(block.kind, BlockKind::Alt);
            assert_eq!(block.label, "SSO configured");
            assert_eq!(block.sections.len(), 2);
            // SSO section: 5 messages
            assert_eq!(block.sections[0].statements.len(), 5);
            // Password section: 4 messages
            assert_eq!(block.sections[1].statements.len(), 4);
            assert_eq!(block.sections[1].label.as_deref(), Some("Password only"));
        } else {
            panic!("Expected Block at position 3");
        }
    }

    #[test]
    fn test_parse_implicit_participants() {
        let source = "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob->>Charlie: Forward";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.participants.len(), 3);
        assert_eq!(ast.participants[0].id, "Alice");
        assert_eq!(ast.participants[1].id, "Bob");
        assert_eq!(ast.participants[2].id, "Charlie");
    }

    #[test]
    fn test_detect_sequence_diagram() {
        use crate::parser::{detect_diagram_kind, DiagramKind};
        let source = "sequenceDiagram\n    A->>B: Hello";
        assert_eq!(detect_diagram_kind(source).unwrap(), DiagramKind::Sequence);
    }
}
