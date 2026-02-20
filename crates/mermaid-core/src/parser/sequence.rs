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
            // Use position-based check to avoid scanning (small N, but cleaner)
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
            ast.statements
                .push(SequenceStatement::Message(parse_message(pair)?));
        }
        Rule::activate_stmt => {
            ast.statements
                .push(SequenceStatement::Activate(extract_participant_id(pair)));
        }
        Rule::deactivate_stmt => {
            ast.statements
                .push(SequenceStatement::Deactivate(extract_participant_id(pair)));
        }
        Rule::autonumber_stmt => {
            ast.autonumber = true;
        }
        Rule::note_stmt => {
            ast.statements
                .push(SequenceStatement::Note(parse_note(pair)?));
        }
        Rule::block_alt => {
            ast.statements
                .push(SequenceStatement::Block(parse_block(pair, BlockKind::Alt)?));
        }
        Rule::block_loop => {
            ast.statements.push(SequenceStatement::Block(parse_block(
                pair,
                BlockKind::Loop,
            )?));
        }
        Rule::block_opt => {
            ast.statements
                .push(SequenceStatement::Block(parse_block(pair, BlockKind::Opt)?));
        }
        Rule::block_par => {
            ast.statements
                .push(SequenceStatement::Block(parse_block(pair, BlockKind::Par)?));
        }
        Rule::block_critical => {
            ast.statements.push(SequenceStatement::Block(parse_block(
                pair,
                BlockKind::Critical,
            )?));
        }
        Rule::block_break => {
            ast.statements.push(SequenceStatement::Block(parse_block(
                pair,
                BlockKind::Break,
            )?));
        }
        Rule::block_rect => {
            ast.statements.push(SequenceStatement::Block(parse_block(
                pair,
                BlockKind::Rect,
            )?));
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

    for inner in pair.into_inner() {
        // Inline the statement processing instead of creating a temporary AST
        match inner.as_rule() {
            Rule::message_stmt => {
                let msg = parse_message(inner)?;
                stmts.push(SequenceStatement::Message(msg));
            }
            Rule::activate_stmt => {
                let id = extract_participant_id(inner);
                stmts.push(SequenceStatement::Activate(id));
            }
            Rule::deactivate_stmt => {
                let id = extract_participant_id(inner);
                stmts.push(SequenceStatement::Deactivate(id));
            }
            Rule::note_stmt => {
                let note = parse_note(inner)?;
                stmts.push(SequenceStatement::Note(note));
            }
            Rule::block_alt => {
                let block = parse_block(inner, BlockKind::Alt)?;
                stmts.push(SequenceStatement::Block(block));
            }
            Rule::block_loop => {
                let block = parse_block(inner, BlockKind::Loop)?;
                stmts.push(SequenceStatement::Block(block));
            }
            Rule::block_opt => {
                let block = parse_block(inner, BlockKind::Opt)?;
                stmts.push(SequenceStatement::Block(block));
            }
            Rule::block_par => {
                let block = parse_block(inner, BlockKind::Par)?;
                stmts.push(SequenceStatement::Block(block));
            }
            Rule::block_critical => {
                let block = parse_block(inner, BlockKind::Critical)?;
                stmts.push(SequenceStatement::Block(block));
            }
            Rule::block_break => {
                let block = parse_block(inner, BlockKind::Break)?;
                stmts.push(SequenceStatement::Block(block));
            }
            Rule::block_rect => {
                let block = parse_block(inner, BlockKind::Rect)?;
                stmts.push(SequenceStatement::Block(block));
            }
            _ => {}
        }
    }

    Ok(stmts)
}

/// Scan all messages for participant IDs not explicitly declared and add them
/// in order of first appearance.
fn resolve_implicit_participants(ast: &mut SequenceAst) {
    // Use Vec for `seen` - participant counts are typically small (< 20),
    // where linear scan outperforms HashSet due to lower overhead.
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

    #[test]
    fn test_parse_actor_keyword() {
        let source = "sequenceDiagram\n    actor A";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.participants.len(), 1);
        assert_eq!(ast.participants[0].id, "A");
        assert_eq!(ast.participants[0].kind, ParticipantKind::Actor);
    }

    #[test]
    fn test_parse_actor_with_alias() {
        let source = "sequenceDiagram\n    actor U as User";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.participants[0].id, "U");
        assert_eq!(ast.participants[0].display_name.as_deref(), Some("User"));
        assert_eq!(ast.participants[0].kind, ParticipantKind::Actor);
    }

    #[test]
    fn test_parse_autonumber() {
        let source = "sequenceDiagram\n    autonumber\n    A->>B: Hello";
        let ast = parse_sequence(source).unwrap();
        assert!(ast.autonumber);
    }

    #[test]
    fn test_parse_autonumber_default_false() {
        let source = "sequenceDiagram\n    A->>B: Hello";
        let ast = parse_sequence(source).unwrap();
        assert!(!ast.autonumber);
    }

    #[test]
    fn test_parse_solid_paren_arrow() {
        let source = "sequenceDiagram\n    A-)B: async msg";
        let ast = parse_sequence(source).unwrap();
        if let SequenceStatement::Message(m) = &ast.statements[0] {
            assert_eq!(m.arrow, ArrowType::SolidParen);
            assert_eq!(m.label, "async msg");
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn test_parse_dotted_paren_arrow() {
        let source = "sequenceDiagram\n    A--)B: async reply";
        let ast = parse_sequence(source).unwrap();
        if let SequenceStatement::Message(m) = &ast.statements[0] {
            assert_eq!(m.arrow, ArrowType::DottedParen);
            assert_eq!(m.label, "async reply");
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn test_parse_solid_cross_arrow() {
        let source = "sequenceDiagram\n    A-xB: lost msg";
        let ast = parse_sequence(source).unwrap();
        if let SequenceStatement::Message(m) = &ast.statements[0] {
            assert_eq!(m.arrow, ArrowType::SolidCross);
            assert_eq!(m.label, "lost msg");
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn test_parse_dotted_cross_arrow() {
        let source = "sequenceDiagram\n    A--xB: lost reply";
        let ast = parse_sequence(source).unwrap();
        if let SequenceStatement::Message(m) = &ast.statements[0] {
            assert_eq!(m.arrow, ArrowType::DottedCross);
            assert_eq!(m.label, "lost reply");
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn test_parse_loop_block() {
        let source = "sequenceDiagram\n    loop Every minute\n        A->>B: ping\n    end";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Block(block) = &ast.statements[0] {
            assert_eq!(block.kind, BlockKind::Loop);
            assert_eq!(block.label, "Every minute");
            assert_eq!(block.sections.len(), 1);
            assert_eq!(block.sections[0].statements.len(), 1);
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_parse_opt_block() {
        let source = "sequenceDiagram\n    opt Extra details\n        A->>B: info\n    end";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Block(block) = &ast.statements[0] {
            assert_eq!(block.kind, BlockKind::Opt);
            assert_eq!(block.label, "Extra details");
            assert_eq!(block.sections.len(), 1);
            assert_eq!(block.sections[0].statements.len(), 1);
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_parse_par_and_block() {
        let source = "sequenceDiagram\n    par Alice to Bob\n        A->>B: Hello\n    and Alice to John\n        A->>J: Hello\n    end";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Block(block) = &ast.statements[0] {
            assert_eq!(block.kind, BlockKind::Par);
            assert_eq!(block.label, "Alice to Bob");
            assert_eq!(block.sections.len(), 2);
            assert_eq!(block.sections[0].statements.len(), 1);
            assert_eq!(block.sections[1].label.as_deref(), Some("Alice to John"));
            assert_eq!(block.sections[1].statements.len(), 1);
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_parse_critical_block() {
        let source = "sequenceDiagram\n    critical Establish connection\n        A->>B: connect\n    end";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Block(block) = &ast.statements[0] {
            assert_eq!(block.kind, BlockKind::Critical);
            assert_eq!(block.label, "Establish connection");
            assert_eq!(block.sections.len(), 1);
            assert_eq!(block.sections[0].statements.len(), 1);
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_parse_break_block() {
        let source = "sequenceDiagram\n    break When error\n        A->>B: error\n    end";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Block(block) = &ast.statements[0] {
            assert_eq!(block.kind, BlockKind::Break);
            assert_eq!(block.label, "When error");
            assert_eq!(block.sections.len(), 1);
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_parse_rect_block() {
        let source = "sequenceDiagram\n    rect rgb(200, 220, 255)\n        A->>B: inside rect\n    end";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Block(block) = &ast.statements[0] {
            assert_eq!(block.kind, BlockKind::Rect);
            assert_eq!(block.sections.len(), 1);
            assert_eq!(block.sections[0].statements.len(), 1);
        } else {
            panic!("Expected Block");
        }
    }

    #[test]
    fn test_parse_note_left_of() {
        let source = "sequenceDiagram\n    participant A\n    Note left of A: This is a note";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Note(note) = &ast.statements[0] {
            assert_eq!(note.position, NotePosition::LeftOf);
            assert_eq!(note.participants, vec!["A"]);
            assert_eq!(note.text, "This is a note");
        } else {
            panic!("Expected Note");
        }
    }

    #[test]
    fn test_parse_note_right_of() {
        let source = "sequenceDiagram\n    participant A\n    Note right of A: Right note";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Note(note) = &ast.statements[0] {
            assert_eq!(note.position, NotePosition::RightOf);
            assert_eq!(note.participants, vec!["A"]);
            assert_eq!(note.text, "Right note");
        } else {
            panic!("Expected Note");
        }
    }

    #[test]
    fn test_parse_note_over_single() {
        let source = "sequenceDiagram\n    participant A\n    Note over A: Over note";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Note(note) = &ast.statements[0] {
            assert_eq!(note.position, NotePosition::Over);
            assert_eq!(note.participants, vec!["A"]);
            assert_eq!(note.text, "Over note");
        } else {
            panic!("Expected Note");
        }
    }

    #[test]
    fn test_parse_note_over_multiple() {
        let source = "sequenceDiagram\n    participant A\n    participant B\n    Note over A,B: Spanning note";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Note(note) = &ast.statements[0] {
            assert_eq!(note.position, NotePosition::Over);
            assert_eq!(note.participants, vec!["A", "B"]);
            assert_eq!(note.text, "Spanning note");
        } else {
            panic!("Expected Note");
        }
    }

    #[test]
    fn test_parse_activate_deactivate() {
        let source = "sequenceDiagram\n    participant A\n    activate A\n    deactivate A";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 2);
        if let SequenceStatement::Activate(id) = &ast.statements[0] {
            assert_eq!(id, "A");
        } else {
            panic!("Expected Activate");
        }
        if let SequenceStatement::Deactivate(id) = &ast.statements[1] {
            assert_eq!(id, "A");
        } else {
            panic!("Expected Deactivate");
        }
    }

    #[test]
    fn test_parse_self_message_detailed() {
        let source = "sequenceDiagram\n    A->>A: self call";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Message(m) = &ast.statements[0] {
            assert_eq!(m.from, "A");
            assert_eq!(m.to, "A");
            assert_eq!(m.arrow, ArrowType::SolidArrow);
            assert_eq!(m.label, "self call");
        } else {
            panic!("Expected Message");
        }
        // A appears only once as participant
        assert_eq!(ast.participants.len(), 1);
        assert_eq!(ast.participants[0].id, "A");
    }

    #[test]
    fn test_parse_activation_plus_minus_syntax() {
        let source = "sequenceDiagram\n    A->>+B: activate\n    A->>-B: deactivate";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 2);
        if let SequenceStatement::Message(m) = &ast.statements[0] {
            assert_eq!(m.from, "A");
            assert_eq!(m.to, "B");
            assert!(m.activate_target);
            assert!(!m.deactivate_source);
        } else {
            panic!("Expected Message");
        }
        if let SequenceStatement::Message(m) = &ast.statements[1] {
            assert_eq!(m.from, "A");
            assert_eq!(m.to, "B");
            assert!(!m.activate_target);
            assert!(m.deactivate_source);
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn test_parse_implicit_participants_from_blocks() {
        let source = "sequenceDiagram\n    alt Case\n        X->>Y: hello\n    end";
        let ast = parse_sequence(source).unwrap();
        // X and Y should be resolved as implicit participants
        assert!(ast.participants.iter().any(|p| p.id == "X"));
        assert!(ast.participants.iter().any(|p| p.id == "Y"));
    }

    #[test]
    fn test_parse_implicit_participants_from_activate() {
        let source = "sequenceDiagram\n    activate Z";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.participants.len(), 1);
        assert_eq!(ast.participants[0].id, "Z");
        assert_eq!(ast.participants[0].kind, ParticipantKind::Participant);
    }

    #[test]
    fn test_parse_implicit_participants_from_notes() {
        let source = "sequenceDiagram\n    Note left of Q: note text";
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.participants.len(), 1);
        assert_eq!(ast.participants[0].id, "Q");
    }

    #[test]
    fn test_parse_all_arrow_types_explicit() {
        // Test each arrow individually to ensure exhaustive coverage
        let cases = vec![
            ("A-)B: msg", ArrowType::SolidParen),
            ("A--)B: msg", ArrowType::DottedParen),
            ("A-xB: msg", ArrowType::SolidCross),
            ("A--xB: msg", ArrowType::DottedCross),
            ("A->>B: msg", ArrowType::SolidArrow),
            ("A-->>B: msg", ArrowType::DottedArrow),
            ("A->B: msg", ArrowType::SolidOpen),
            ("A-->B: msg", ArrowType::DottedOpen),
        ];
        for (arrow_str, expected) in cases {
            let source = format!("sequenceDiagram\n    {}", arrow_str);
            let ast = parse_sequence(&source).unwrap();
            if let SequenceStatement::Message(m) = &ast.statements[0] {
                assert_eq!(m.arrow, expected, "Failed for: {}", arrow_str);
            } else {
                panic!("Expected Message for: {}", arrow_str);
            }
        }
    }

    #[test]
    fn test_parse_nested_block_in_body() {
        let source = "sequenceDiagram\n    loop Outer\n        alt Inner\n            A->>B: msg\n        else Other\n            B->>A: reply\n        end\n    end";
        let ast = parse_sequence(source).unwrap();
        if let SequenceStatement::Block(outer) = &ast.statements[0] {
            assert_eq!(outer.kind, BlockKind::Loop);
            assert_eq!(outer.sections[0].statements.len(), 1);
            if let SequenceStatement::Block(inner) = &outer.sections[0].statements[0] {
                assert_eq!(inner.kind, BlockKind::Alt);
                assert_eq!(inner.sections.len(), 2);
            } else {
                panic!("Expected inner Block");
            }
        } else {
            panic!("Expected outer Block");
        }
    }

    #[test]
    fn test_parse_sequence_error_invalid_input() {
        let source = "not a sequence diagram";
        let result = parse_sequence(source);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_note_over_single_vs_multiple() {
        let single = "sequenceDiagram\n    participant A\n    Note over A: Single";
        let ast_single = parse_sequence(single).unwrap();
        if let SequenceStatement::Note(n) = &ast_single.statements[0] {
            assert_eq!(n.position, NotePosition::Over);
            assert_eq!(n.participants, vec!["A"]);
        }

        let multi = "sequenceDiagram\n    participant A\n    participant B\n    Note over A,B: Multiple";
        let ast_multi = parse_sequence(multi).unwrap();
        if let SequenceStatement::Note(n) = &ast_multi.statements[0] {
            assert_eq!(n.participants, vec!["A", "B"]);
        }
    }

    #[test]
    fn test_parse_block_sections_non_empty_current_stmts() {
        // alt with else - sections non-empty, first body has stmts
        let source = "sequenceDiagram\n    alt First\n        A->>B: msg1\n    else Second\n        B->>A: msg2\n    end";
        let ast = parse_sequence(source).unwrap();
        if let SequenceStatement::Block(block) = &ast.statements[0] {
            assert_eq!(block.sections.len(), 2);
            assert_eq!(block.sections[0].statements.len(), 1);
            assert_eq!(block.sections[1].statements.len(), 1);
        }
    }

    #[test]
    fn test_parse_opt_critical_break_blocks() {
        let opt = "sequenceDiagram\n    opt Optional\n        A->>B: msg\n    end";
        let ast_opt = parse_sequence(opt).unwrap();
        if let SequenceStatement::Block(b) = &ast_opt.statements[0] {
            assert_eq!(b.kind, BlockKind::Opt);
        }

        let critical = "sequenceDiagram\n    critical Critical\n        A->>B: msg\n    end";
        let ast_crit = parse_sequence(critical).unwrap();
        if let SequenceStatement::Block(b) = &ast_crit.statements[0] {
            assert_eq!(b.kind, BlockKind::Critical);
        }

        let break_block = "sequenceDiagram\n    break Break\n        A->>B: msg\n    end";
        let ast_break = parse_sequence(break_block).unwrap();
        if let SequenceStatement::Block(b) = &ast_break.statements[0] {
            assert_eq!(b.kind, BlockKind::Break);
        }
    }

    #[test]
    fn test_parse_note_edge_cases() {
        // Note with empty text
        let source = "sequenceDiagram\n    participant A\n    Note over A:";
        let ast = parse_sequence(source).unwrap();
        if let SequenceStatement::Note(n) = &ast.statements[0] {
            assert_eq!(n.text, "");
        }
    }

    #[test]
    fn test_parse_deeply_nested_blocks_in_rect() {
        // Comprehensive test: rect block containing activate, deactivate, note,
        // and ALL nested block types (alt, loop, opt, par, critical, break, rect).
        // Exercises parse_block_body branches for nested statements.
        let source = r#"sequenceDiagram
    participant A
    participant B
    rect rgb(200, 220, 255)
        activate A
        deactivate A
        Note right of A: inside rect
        A->>B: msg inside rect
        alt Nested alt
            A->>B: alt msg
        end
        loop Nested loop
            A->>B: loop msg
        end
        opt Nested opt
            A->>B: opt msg
        end
        par Nested par
            A->>B: par msg
        end
        critical Nested critical
            A->>B: critical msg
        end
        break Nested break
            A->>B: break msg
        end
        rect rgb(100, 100, 100)
            A->>B: nested rect msg
        end
    end"#;
        let ast = parse_sequence(source).unwrap();
        assert_eq!(ast.statements.len(), 1);
        if let SequenceStatement::Block(outer) = &ast.statements[0] {
            assert_eq!(outer.kind, BlockKind::Rect);
            let stmts = &outer.sections[0].statements;
            // activate, deactivate, note, message, alt, loop, opt, par, critical, break, rect
            assert!(stmts.len() >= 11);

            // Verify activate and deactivate inside block
            let has_activate = stmts.iter().any(|s| matches!(s, SequenceStatement::Activate(_)));
            let has_deactivate = stmts.iter().any(|s| matches!(s, SequenceStatement::Deactivate(_)));
            let has_note = stmts.iter().any(|s| matches!(s, SequenceStatement::Note(_)));
            assert!(has_activate);
            assert!(has_deactivate);
            assert!(has_note);

            // Verify nested blocks
            let blocks: Vec<_> = stmts
                .iter()
                .filter_map(|s| {
                    if let SequenceStatement::Block(b) = s {
                        Some(b.kind)
                    } else {
                        None
                    }
                })
                .collect();
            assert!(blocks.contains(&BlockKind::Alt));
            assert!(blocks.contains(&BlockKind::Loop));
            assert!(blocks.contains(&BlockKind::Opt));
            assert!(blocks.contains(&BlockKind::Par));
            assert!(blocks.contains(&BlockKind::Critical));
            assert!(blocks.contains(&BlockKind::Break));
            assert!(blocks.contains(&BlockKind::Rect));
        } else {
            panic!("Expected Block");
        }
    }
}
