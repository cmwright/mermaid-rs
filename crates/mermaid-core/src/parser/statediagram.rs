use pest::Parser;
use pest_derive::Parser;

use crate::ast::common::parse_style_string;
use crate::ast::statediagram::*;
use crate::error::{extract_snippet, MermaidError, Result};

use super::flowchart::parse_direction;

#[derive(Parser)]
#[grammar = "parser/statediagram.pest"]
struct StateDiagramPestParser;

/// Parse a Mermaid state diagram source string into a StateDiagramAst.
pub fn parse_statediagram(source: &str) -> Result<StateDiagramAst> {
    let pairs = StateDiagramPestParser::parse(Rule::statediagram, source).map_err(|e| {
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

    let mut ast = StateDiagramAst::default();

    for pair in pairs {
        if pair.as_rule() == Rule::statediagram {
            for inner in pair.into_inner() {
                process_top_level(&mut ast, inner)?;
            }
        }
    }

    // Disambiguate [*] into start/end nodes
    let mut counter = 0;
    disambiguate_scope(&mut ast.states, &mut ast.transitions, &mut counter);
    for composite in &mut ast.composites {
        disambiguate_composite(composite, &mut counter);
    }
    // If a composite and a state share the same ID in a scope (e.g. `state X { ... }`
    // plus transition-created implicit state `X`), keep the composite and drop the state.
    prune_state_composite_id_collisions(&mut ast.states, &mut ast.composites);

    Ok(ast)
}

fn process_top_level(
    ast: &mut StateDiagramAst,
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<()> {
    match pair.as_rule() {
        Rule::diagram_header => {
            // Direction is set via direction_stmt, not in the header for state diagrams
        }
        Rule::direction_stmt => {
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::direction_value {
                    ast.direction = parse_direction(inner.as_str())?;
                }
            }
        }
        Rule::transition => {
            let (t, class_shorthands) = parse_transition(pair)?;
            // Ensure states referenced in transitions exist
            ensure_state_exists(&mut ast.states, &t.from, &class_shorthands.0);
            ensure_state_exists(&mut ast.states, &t.to, &class_shorthands.1);
            ast.transitions.push(t);
        }
        Rule::bare_state => {
            let state = parse_bare_state(pair)?;
            upsert_state(&mut ast.states, state);
        }
        Rule::state_declaration => {
            let state = parse_state_declaration(pair)?;
            upsert_state(&mut ast.states, state);
        }
        Rule::composite_state => {
            let composite = parse_composite_state(pair)?;
            ast.composites.push(composite);
        }
        Rule::note_inline | Rule::note_multiline | Rule::note_floating => {
            let note = parse_note(pair)?;
            ast.notes.push(note);
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
            // Directives parsed but currently ignored
        }
        _ => {}
    }
    Ok(())
}

fn process_composite_body(
    composite: &mut CompositeStateDef,
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<()> {
    match pair.as_rule() {
        Rule::direction_stmt => {
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::direction_value {
                    composite.direction = Some(parse_direction(inner.as_str())?);
                }
            }
        }
        Rule::transition => {
            let (t, class_shorthands) = parse_transition(pair)?;
            ensure_state_exists(&mut composite.states, &t.from, &class_shorthands.0);
            ensure_state_exists(&mut composite.states, &t.to, &class_shorthands.1);
            composite.transitions.push(t);
        }
        Rule::bare_state => {
            let state = parse_bare_state(pair)?;
            upsert_state(&mut composite.states, state);
        }
        Rule::state_declaration => {
            let state = parse_state_declaration(pair)?;
            upsert_state(&mut composite.states, state);
        }
        Rule::composite_state => {
            let nested = parse_composite_state(pair)?;
            composite.composites.push(nested);
        }
        Rule::divider => {
            let id = format!("__divider_{}", composite.dividers.len());
            composite.dividers.push(DividerDef { id });
        }
        Rule::note_inline | Rule::note_multiline | Rule::note_floating => {
            let note = parse_note(pair)?;
            composite.notes.push(note);
        }
        _ => {}
    }
    Ok(())
}

/// Parse a transition: State1 --> State2 : Label
/// Returns (TransitionDef, (from_class_shorthand, to_class_shorthand))
fn parse_transition(
    pair: pest::iterators::Pair<'_, Rule>,
) -> Result<(TransitionDef, (Option<String>, Option<String>))> {
    let mut from = String::new();
    let mut to = String::new();
    let mut label = None;
    let mut from_class: Option<String> = None;
    let mut to_class: Option<String> = None;
    let mut seen_arrow = false;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::state_ref => {
                if !seen_arrow {
                    from = inner.as_str().to_string();
                } else {
                    to = inner.as_str().to_string();
                }
            }
            Rule::transition_arrow => {
                seen_arrow = true;
            }
            Rule::transition_label => {
                label = Some(inner.as_str().trim().to_string());
            }
            Rule::class_shorthand => {
                let ident = inner.into_inner().next().unwrap();
                let cls = ident.as_str().to_string();
                if !seen_arrow {
                    from_class = Some(cls);
                } else {
                    to_class = Some(cls);
                }
            }
            _ => {}
        }
    }

    Ok((
        TransitionDef { from, to, label },
        (from_class, to_class),
    ))
}

/// Parse a bare state: StateID : Description or StateID:::class
fn parse_bare_state(pair: pest::iterators::Pair<'_, Rule>) -> Result<StateDef> {
    let mut id = String::new();
    let mut label = None;
    let mut class_shorthand = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::state_id => {
                id = inner.as_str().to_string();
            }
            Rule::state_description => {
                label = Some(inner.as_str().trim().to_string());
            }
            Rule::class_shorthand => {
                let ident = inner.into_inner().next().unwrap();
                class_shorthand = Some(ident.as_str().to_string());
            }
            _ => {}
        }
    }

    Ok(StateDef {
        id,
        label,
        kind: StateKind::Normal,
        class_shorthand,
    })
}

/// Parse a state declaration: state "Name" as id, or state id <<fork>>
fn parse_state_declaration(pair: pest::iterators::Pair<'_, Rule>) -> Result<StateDef> {
    let mut id = String::new();
    let mut label = None;
    let mut kind = StateKind::Normal;
    let mut class_shorthand = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::quoted_text => {
                label = Some(inner.as_str().trim().to_string());
            }
            Rule::state_id => {
                id = inner.as_str().to_string();
            }
            Rule::state_stereotype => {
                for stereo_inner in inner.into_inner() {
                    if stereo_inner.as_rule() == Rule::stereotype_kind {
                        kind = match stereo_inner.as_str() {
                            "fork" => StateKind::Fork,
                            "join" => StateKind::Join,
                            "choice" => StateKind::Choice,
                            _ => StateKind::Normal,
                        };
                    }
                }
            }
            Rule::class_shorthand => {
                let ident = inner.into_inner().next().unwrap();
                class_shorthand = Some(ident.as_str().to_string());
            }
            _ => {}
        }
    }

    Ok(StateDef {
        id,
        label,
        kind,
        class_shorthand,
    })
}

/// Parse a composite state: state CompositeID { ... }
fn parse_composite_state(pair: pest::iterators::Pair<'_, Rule>) -> Result<CompositeStateDef> {
    let mut composite = CompositeStateDef::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::quoted_text => {
                composite.label = Some(inner.as_str().trim().to_string());
            }
            Rule::state_id => {
                composite.id = inner.as_str().to_string();
            }
            Rule::class_shorthand => {
                // class shorthand on composite - ignore for now, could be stored
            }
            _ => {
                // Composite body statements
                process_composite_body(&mut composite, inner)?;
            }
        }
    }

    Ok(composite)
}

/// Parse a note statement.
fn parse_note(pair: pest::iterators::Pair<'_, Rule>) -> Result<NoteDef> {
    let rule = pair.as_rule();

    match rule {
        Rule::note_floating => {
            let mut text = String::new();
            let mut id = None;
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::quoted_text => {
                        text = inner.as_str().trim().to_string();
                    }
                    Rule::state_id => {
                        id = Some(inner.as_str().to_string());
                    }
                    _ => {}
                }
            }
            Ok(NoteDef {
                id,
                target_state: None,
                position: None,
                text,
            })
        }
        Rule::note_inline => {
            let mut position = None;
            let mut target_state = None;
            let mut text = String::new();
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::note_position => {
                        position = Some(match inner.as_str() {
                            "left" => NotePosition::Left,
                            _ => NotePosition::Right,
                        });
                    }
                    Rule::state_ref => {
                        target_state = Some(inner.as_str().to_string());
                    }
                    Rule::note_inline_text => {
                        text = inner.as_str().trim().to_string();
                    }
                    _ => {}
                }
            }
            Ok(NoteDef {
                id: None,
                target_state,
                position,
                text,
            })
        }
        Rule::note_multiline => {
            let mut position = None;
            let mut target_state = None;
            let mut text = String::new();
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::note_position => {
                        position = Some(match inner.as_str() {
                            "left" => NotePosition::Left,
                            _ => NotePosition::Right,
                        });
                    }
                    Rule::state_ref => {
                        target_state = Some(inner.as_str().to_string());
                    }
                    Rule::note_multiline_text => {
                        text = inner.as_str().trim().to_string();
                    }
                    _ => {}
                }
            }
            Ok(NoteDef {
                id: None,
                target_state,
                position,
                text,
            })
        }
        _ => unreachable!(),
    }
}

fn parse_class_def(pair: pest::iterators::Pair<'_, Rule>) -> Result<ClassDef> {
    let mut name = String::new();
    let mut properties = crate::ast::common::StyleProperties::default();

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
            Rule::state_id_list => {
                for id_pair in inner.into_inner() {
                    if id_pair.as_rule() == Rule::state_id {
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
    let mut properties = crate::ast::common::StyleProperties::default();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::state_id => {
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

// ── Start/End [*] disambiguation ────────────────────────────

/// Disambiguate [*] tokens in a scope.
/// [*] as transition source = Start node, [*] as target = End node.
fn disambiguate_scope(
    states: &mut Vec<StateDef>,
    transitions: &mut Vec<TransitionDef>,
    counter: &mut usize,
) {
    let mut start_id: Option<String> = None;
    let mut end_id: Option<String> = None;

    for t in transitions.iter_mut() {
        if t.from == "[*]" {
            if start_id.is_none() {
                let id = format!("__start_{}", *counter);
                *counter += 1;
                states.push(StateDef {
                    id: id.clone(),
                    label: None,
                    kind: StateKind::Start,
                    class_shorthand: None,
                });
                start_id = Some(id);
            }
            t.from = start_id.clone().unwrap();
        }
        if t.to == "[*]" {
            if end_id.is_none() {
                let id = format!("__end_{}", *counter);
                *counter += 1;
                states.push(StateDef {
                    id: id.clone(),
                    label: None,
                    kind: StateKind::End,
                    class_shorthand: None,
                });
                end_id = Some(id);
            }
            t.to = end_id.clone().unwrap();
        }
    }
}

fn disambiguate_composite(composite: &mut CompositeStateDef, counter: &mut usize) {
    disambiguate_scope(&mut composite.states, &mut composite.transitions, counter);
    for nested in &mut composite.composites {
        disambiguate_composite(nested, counter);
    }
}

fn prune_state_composite_id_collisions(
    states: &mut Vec<StateDef>,
    composites: &mut [CompositeStateDef],
) {
    use std::collections::HashSet;

    let composite_ids: HashSet<String> = composites.iter().map(|c| c.id.clone()).collect();
    states.retain(|s| !composite_ids.contains(&s.id));

    for composite in composites.iter_mut() {
        prune_state_composite_id_collisions(&mut composite.states, &mut composite.composites);
    }
}

/// Ensure a state exists in the states list (for implicitly declared states from transitions).
fn ensure_state_exists(
    states: &mut Vec<StateDef>,
    id: &str,
    class_shorthand: &Option<String>,
) {
    // Don't create entries for [*] — these are handled by disambiguation
    if id == "[*]" {
        return;
    }
    if !states.iter().any(|s| s.id == id) {
        states.push(StateDef {
            id: id.to_string(),
            label: None,
            kind: StateKind::Normal,
            class_shorthand: class_shorthand.clone(),
        });
    } else if let Some(cls) = class_shorthand {
        if let Some(existing) = states.iter_mut().find(|s| s.id == id) {
            if existing.class_shorthand.is_none() {
                existing.class_shorthand = Some(cls.clone());
            }
        }
    }
}

/// Insert or update a state.
fn upsert_state(states: &mut Vec<StateDef>, new_state: StateDef) {
    if let Some(existing) = states.iter_mut().find(|s| s.id == new_state.id) {
        if new_state.label.is_some() {
            existing.label = new_state.label;
        }
        if new_state.kind != StateKind::Normal {
            existing.kind = new_state.kind;
        }
        if new_state.class_shorthand.is_some() {
            existing.class_shorthand = new_state.class_shorthand;
        }
    } else {
        states.push(new_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_transition() {
        let source = "stateDiagram-v2\n    State1 --> State2";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.transitions.len(), 1);
        assert_eq!(ast.transitions[0].from, "State1");
        assert_eq!(ast.transitions[0].to, "State2");
        assert_eq!(ast.transitions[0].label, None);
    }

    #[test]
    fn test_parse_transition_with_label() {
        let source = "stateDiagram-v2\n    State1 --> State2 : Do something";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.transitions.len(), 1);
        assert_eq!(
            ast.transitions[0].label.as_deref(),
            Some("Do something")
        );
    }

    #[test]
    fn test_parse_start_end_nodes() {
        let source = "stateDiagram-v2\n    [*] --> State1\n    State1 --> [*]";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.transitions.len(), 2);
        // [*] as source should become a start node
        assert!(ast.transitions[0].from.starts_with("__start_"));
        // [*] as target should become an end node
        assert!(ast.transitions[1].to.starts_with("__end_"));
        // Should have created start and end state defs
        let start = ast.states.iter().find(|s| s.kind == StateKind::Start);
        let end = ast.states.iter().find(|s| s.kind == StateKind::End);
        assert!(start.is_some());
        assert!(end.is_some());
    }

    #[test]
    fn test_parse_state_with_description() {
        let source = "stateDiagram-v2\n    State1 : This is state 1";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.states.len(), 1);
        assert_eq!(ast.states[0].id, "State1");
        assert_eq!(
            ast.states[0].label.as_deref(),
            Some("This is state 1")
        );
    }

    #[test]
    fn test_parse_quoted_state() {
        let source = "stateDiagram-v2\n    state \"Long State Name\" as s1";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.states.len(), 1);
        assert_eq!(ast.states[0].id, "s1");
        assert_eq!(
            ast.states[0].label.as_deref(),
            Some("Long State Name")
        );
    }

    #[test]
    fn test_parse_fork_join_choice() {
        let source = "stateDiagram-v2\n    state fork1 <<fork>>\n    state join1 <<join>>\n    state choice1 <<choice>>";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.states.len(), 3);
        assert_eq!(ast.states[0].kind, StateKind::Fork);
        assert_eq!(ast.states[1].kind, StateKind::Join);
        assert_eq!(ast.states[2].kind, StateKind::Choice);
    }

    #[test]
    fn test_parse_composite_state() {
        let source = "stateDiagram-v2\n    state Composite {\n        Inner1 --> Inner2\n    }";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.composites.len(), 1);
        assert_eq!(ast.composites[0].id, "Composite");
        assert_eq!(ast.composites[0].transitions.len(), 1);
        assert_eq!(ast.composites[0].transitions[0].from, "Inner1");
        assert_eq!(ast.composites[0].transitions[0].to, "Inner2");
    }

    #[test]
    fn test_parse_composite_with_label() {
        let source =
            "stateDiagram-v2\n    state \"My Group\" as grp {\n        A --> B\n    }";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.composites[0].id, "grp");
        assert_eq!(
            ast.composites[0].label.as_deref(),
            Some("My Group")
        );
    }

    #[test]
    fn test_parse_direction() {
        let source = "stateDiagram-v2\n    direction LR\n    A --> B";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.direction, Direction::LeftToRight);
    }

    #[test]
    fn test_parse_divider() {
        let source =
            "stateDiagram-v2\n    state Comp {\n        A --> B\n        --\n        C --> D\n    }";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.composites[0].dividers.len(), 1);
    }

    #[test]
    fn test_parse_note_inline() {
        let source = "stateDiagram-v2\n    State1\n    note right of State1 : Important note";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.notes.len(), 1);
        assert_eq!(
            ast.notes[0].target_state.as_deref(),
            Some("State1")
        );
        assert_eq!(ast.notes[0].position, Some(NotePosition::Right));
        assert_eq!(ast.notes[0].text, "Important note");
    }

    #[test]
    fn test_parse_note_floating() {
        let source = "stateDiagram-v2\n    note \"This is a note\" as N1";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.notes.len(), 1);
        assert_eq!(ast.notes[0].id.as_deref(), Some("N1"));
        assert_eq!(ast.notes[0].text, "This is a note");
    }

    #[test]
    fn test_parse_class_def_and_assign() {
        let source = "stateDiagram-v2\n    State1\n    classDef highlight fill:#ff0,stroke:#333\n    class State1 highlight";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.class_defs.len(), 1);
        assert_eq!(ast.class_defs[0].name, "highlight");
        assert_eq!(ast.class_assignments.len(), 1);
        assert_eq!(ast.class_assignments[0].class_name, "highlight");
    }

    #[test]
    fn test_parse_style_override() {
        let source = "stateDiagram-v2\n    State1\n    style State1 fill:#f00";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.style_overrides.len(), 1);
        assert_eq!(ast.style_overrides[0].node_id, "State1");
        assert!(ast.style_overrides[0].properties.fill.is_some());
    }

    #[test]
    fn test_parse_class_shorthand() {
        let source = "stateDiagram-v2\n    State1:::myClass --> State2";
        let ast = parse_statediagram(source).unwrap();
        let s1 = ast.states.iter().find(|s| s.id == "State1").unwrap();
        assert_eq!(s1.class_shorthand.as_deref(), Some("myClass"));
    }

    #[test]
    fn test_parse_v1_header() {
        let source = "stateDiagram\n    A --> B";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.transitions.len(), 1);
    }

    #[test]
    fn test_parse_comments_ignored() {
        let source =
            "stateDiagram-v2\n    %% This is a comment\n    A --> B";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.transitions.len(), 1);
    }

    #[test]
    fn test_parse_composite_with_start_end() {
        let source = "stateDiagram-v2\n    state Comp {\n        [*] --> Inner\n        Inner --> [*]\n    }";
        let ast = parse_statediagram(source).unwrap();
        let comp = &ast.composites[0];
        // Should have start and end nodes disambiguated
        let has_start = comp.states.iter().any(|s| s.kind == StateKind::Start);
        let has_end = comp.states.iter().any(|s| s.kind == StateKind::End);
        assert!(has_start);
        assert!(has_end);
    }

    #[test]
    fn test_unique_start_end_ids_across_top_level_and_composite() {
        let source = "stateDiagram-v2\n    [*] --> Active\n    Active --> [*]\n    state Active {\n        [*] --> Idle\n        Idle --> [*]\n    }";
        let ast = parse_statediagram(source).unwrap();

        let mut all_ids: Vec<String> = ast
            .states
            .iter()
            .map(|s| s.id.clone())
            .collect();
        let comp = ast.composites.iter().find(|c| c.id == "Active").unwrap();
        all_ids.extend(comp.states.iter().map(|s| s.id.clone()));

        let starts: Vec<&str> = all_ids
            .iter()
            .filter(|id| id.starts_with("__start_"))
            .map(|id| id.as_str())
            .collect();
        let ends: Vec<&str> = all_ids
            .iter()
            .filter(|id| id.starts_with("__end_"))
            .map(|id| id.as_str())
            .collect();

        assert!(
            starts.len() >= 2,
            "expected separate start nodes for top/composite scopes, got {starts:?}"
        );
        assert!(
            ends.len() >= 2,
            "expected separate end nodes for top/composite scopes, got {ends:?}"
        );

        let mut unique_ids = std::collections::HashSet::new();
        for id in all_ids {
            assert!(
                unique_ids.insert(id.clone()),
                "duplicate state id across scopes: {id}"
            );
        }
    }

    #[test]
    fn test_composite_id_does_not_remain_as_normal_state() {
        let source = "stateDiagram-v2\n    [*] --> Active\n    Active --> [*]\n    state Active {\n        [*] --> Idle\n    }";
        let ast = parse_statediagram(source).unwrap();

        assert!(
            ast.composites.iter().any(|c| c.id == "Active"),
            "expected composite Active"
        );
        assert!(
            !ast.states.iter().any(|s| s.id == "Active"),
            "composite id should not also exist as a normal state"
        );
    }

    #[test]
    fn test_parse_nested_composites() {
        let source = "stateDiagram-v2\n    state Outer {\n        state Inner {\n            A --> B\n        }\n    }";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.composites.len(), 1);
        assert_eq!(ast.composites[0].composites.len(), 1);
        assert_eq!(ast.composites[0].composites[0].id, "Inner");
    }

    #[test]
    fn test_parse_multiple_transitions() {
        let source = "stateDiagram-v2\n    [*] --> State1\n    State1 --> State2\n    State2 --> State3\n    State3 --> [*]";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(ast.transitions.len(), 4);
        // States: State1, State2, State3, __start_*, __end_*
        let start_count = ast.states.iter().filter(|s| s.kind == StateKind::Start).count();
        let end_count = ast.states.iter().filter(|s| s.kind == StateKind::End).count();
        assert_eq!(start_count, 1);
        assert_eq!(end_count, 1);
    }

    #[test]
    fn test_parse_direction_inside_composite() {
        let source = "stateDiagram-v2\n    state Comp {\n        direction LR\n        A --> B\n    }";
        let ast = parse_statediagram(source).unwrap();
        assert_eq!(
            ast.composites[0].direction,
            Some(Direction::LeftToRight)
        );
    }

    #[test]
    fn test_implicit_states_from_transitions() {
        let source = "stateDiagram-v2\n    A --> B";
        let ast = parse_statediagram(source).unwrap();
        // A and B should be created implicitly
        assert!(ast.states.iter().any(|s| s.id == "A"));
        assert!(ast.states.iter().any(|s| s.id == "B"));
    }
}
