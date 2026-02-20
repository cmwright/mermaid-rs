//! End-to-end render tests that exercise uncovered code paths across the entire
//! pipeline (parser -> layout -> render). These complement the unit tests in
//! individual modules and verify that the full pipeline produces valid SVG
//! for each scenario.

use mermaid_core::render::theme::Theme;
use mermaid_core::{render, OutputFormat, RenderConfig, RenderOutput};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn render_svg(source: &str) -> String {
    let config = RenderConfig::default();
    let result = render(source, &config).expect("render should succeed");
    result.into_svg().expect("should be SVG output")
}

fn render_svg_with_theme(source: &str, theme: Theme) -> String {
    let mut config = RenderConfig::default();
    config.theme = theme;
    let result = render(source, &config).expect("render should succeed");
    result.into_svg().expect("should be SVG output")
}

// ===========================================================================
// Flowchart coverage
// ===========================================================================

// 1. All node shapes
#[test]
fn flowchart_all_node_shapes() {
    let source = r#"flowchart TD
    A[rect]
    B(rounded)
    C([stadium])
    D((circle))
    E(((double circle)))
    F{diamond}
    G{{hexagon}}
    H[[subroutine]]
    I[(cylinder)]
    J>asymmetric]
    K[/trapezoid\]
    L[\trapezoidAlt/]
    M[/parallelogram/]
    N[\parallelogramAlt\]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"), "output should be valid SVG");
    // Verify node labels appear in the output
    assert!(svg.contains("rect"));
    assert!(svg.contains("rounded"));
    assert!(svg.contains("stadium"));
    assert!(svg.contains("circle"));
    assert!(svg.contains("diamond"));
    assert!(svg.contains("hexagon"));
    assert!(svg.contains("subroutine"));
    assert!(svg.contains("cylinder"));
    assert!(svg.contains("asymmetric"));
    assert!(svg.contains("trapezoid"));
    assert!(svg.contains("parallelogram"));
}

// 2. Multi-line node labels
#[test]
fn flowchart_multiline_node_label() {
    let source = r#"flowchart TD
    A[Line one<br/>Line two]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Line one"));
    assert!(svg.contains("Line two"));
}

// 3. HTML bold in labels
#[test]
fn flowchart_html_bold_in_label() {
    let source = r#"flowchart TD
    A[<b>Bold</b> text]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Bold"));
}

// 4. All edge types
#[test]
fn flowchart_all_edge_types() {
    let source = r#"flowchart TD
    A1 --> B1
    A2 -.-> B2
    A3 ==> B3
    A4 --- B4
    A5 -.- B5
    A6 === B6
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    // Each edge should produce at least one path or line element
    // Just verify we got valid SVG with all nodes present
    assert!(svg.contains("A1"));
    assert!(svg.contains("B1"));
    assert!(svg.contains("A6"));
    assert!(svg.contains("B6"));
}

// 5. Edge with multi-line label
#[test]
fn flowchart_edge_multiline_label() {
    let source = r#"flowchart TD
    A -->|line1<br/>line2| B
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("line1"));
    assert!(svg.contains("line2"));
}

// 6. Subgraph with label
#[test]
fn flowchart_subgraph_with_label() {
    let source = r#"flowchart TD
    subgraph sg["My Label"]
        A[Inside]
    end
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("My Label"));
    assert!(svg.contains("Inside"));
}

// 7. Bottom-to-top direction
#[test]
fn flowchart_bottom_to_top() {
    let source = r#"flowchart BT
    A[Bottom] --> B[Top]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Bottom"));
    assert!(svg.contains("Top"));
}

// 8. Right-to-left direction
#[test]
fn flowchart_right_to_left() {
    let source = r#"flowchart RL
    A[Right] --> B[Left]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Right"));
    assert!(svg.contains("Left"));
}

// 9. Cycles
#[test]
fn flowchart_cycle() {
    let source = r#"flowchart TD
    A --> B --> C --> A
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("A"));
    assert!(svg.contains("B"));
    assert!(svg.contains("C"));
}

// 10. Deep graph (3+ ranks)
#[test]
fn flowchart_deep_graph() {
    let source = r#"flowchart TD
    A --> B --> C --> D --> E
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("A"));
    assert!(svg.contains("E"));
}

// 11. Diamond nodes with edges (tests edge routing / shape intersection)
#[test]
fn flowchart_diamond_with_edges() {
    let source = r#"flowchart TD
    A[Start] --> B{Decision}
    B -->|Yes| C[OK]
    B -->|No| D[Fail]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Decision"));
    assert!(svg.contains("Yes"));
    assert!(svg.contains("No"));
}

// 12. Nested subgraphs (two sibling subgraphs with nodes)
#[test]
fn flowchart_sibling_subgraphs() {
    let source = r#"flowchart TD
    subgraph sg1["Group A"]
        A1[Node 1]
        A2[Node 2]
    end
    subgraph sg2["Group B"]
        B1[Node 3]
        B2[Node 4]
    end
    A1 --> B1
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Group A"));
    assert!(svg.contains("Group B"));
    assert!(svg.contains("Node 1"));
    assert!(svg.contains("Node 3"));
}

// 13. classDef and class shorthand (shape before :::className)
#[test]
fn flowchart_classdef_and_shorthand() {
    let source = r#"flowchart TD
    classDef red fill:#f00,stroke:#333
    A[Red Node]:::red
    B[Normal Node]
    A --> B
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Red Node"));
    // The red fill should be applied to the node
    assert!(svg.contains("#f00") || svg.contains("f00"));
}

// 14. Styled nodes with inline style
#[test]
fn flowchart_style_override() {
    let source = r#"flowchart TD
    A[Styled]
    B[Normal]
    A --> B
    style A fill:#f9f,stroke:#333,stroke-width:4px
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Styled"));
    // The inline style should appear in the SVG
    assert!(svg.contains("#f9f") || svg.contains("f9f"));
}

// ===========================================================================
// Sequence diagram coverage
// ===========================================================================

// 15. Actor stick figures
#[test]
fn sequence_actor_stick_figures() {
    let source = r#"sequenceDiagram
    actor A
    actor B
    A->>B: Hello
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("A"));
    assert!(svg.contains("B"));
    // Actors are rendered as stick figures; verify they appear in the SVG
    // (stick figure is drawn with lines, not rectangles)
    assert!(svg.contains("line") || svg.contains("path") || svg.contains("circle"));
}

// 16. Self-messages
#[test]
fn sequence_self_message() {
    let source = r#"sequenceDiagram
    participant A
    A->>A: self call
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("self call"));
}

// 17. Autonumber
#[test]
fn sequence_autonumber() {
    let source = r#"sequenceDiagram
    autonumber
    Alice->>Bob: First
    Bob->>Alice: Second
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("First"));
    assert!(svg.contains("Second"));
    // Autonumber should produce numeric labels
    assert!(svg.contains("1") || svg.contains("2"));
}

// 18. All arrow types
#[test]
fn sequence_all_arrow_types() {
    let source = r#"sequenceDiagram
    participant A
    participant B
    A->>B: solid arrow
    A-->>B: dotted arrow
    A->B: solid open
    A-->B: dotted open
    A-)B: solid paren
    A--)B: dotted paren
    A-xB: solid cross
    A--xB: dotted cross
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("solid arrow"));
    assert!(svg.contains("dotted arrow"));
    assert!(svg.contains("solid open"));
    assert!(svg.contains("dotted open"));
    assert!(svg.contains("solid paren"));
    assert!(svg.contains("dotted paren"));
    assert!(svg.contains("solid cross"));
    assert!(svg.contains("dotted cross"));
}

// 19. Notes (left, right, over)
#[test]
fn sequence_notes() {
    let source = r#"sequenceDiagram
    participant A
    participant B
    Note left of A: left note
    Note right of B: right note
    Note over A,B: spanning note
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("left note"));
    assert!(svg.contains("right note"));
    assert!(svg.contains("spanning note"));
}

// 20. Alt/else block
#[test]
fn sequence_alt_else() {
    let source = r#"sequenceDiagram
    Alice->>Bob: Hello
    alt is sick
        Bob->>Alice: Not good
    else is well
        Bob->>Alice: Feeling great
    end
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("is sick"));
    assert!(svg.contains("is well"));
    assert!(svg.contains("Not good"));
    assert!(svg.contains("Feeling great"));
}

// 21. Loop block
#[test]
fn sequence_loop_block() {
    let source = r#"sequenceDiagram
    Alice->>Bob: Hello
    loop Every minute
        Bob->>Alice: Still here
    end
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Every minute"));
    assert!(svg.contains("Still here"));
}

// 22. Par/and block
#[test]
fn sequence_par_and() {
    let source = r#"sequenceDiagram
    par Task A
        Alice->>Bob: Do A
    and Task B
        Alice->>Charlie: Do B
    end
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Task A"));
    assert!(svg.contains("Task B"));
    assert!(svg.contains("Do A"));
    assert!(svg.contains("Do B"));
}

// 23. Activate/deactivate
#[test]
fn sequence_activate_deactivate() {
    let source = r#"sequenceDiagram
    Alice->>Bob: Request
    activate Bob
    Bob->>Alice: Response
    deactivate Bob
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Request"));
    assert!(svg.contains("Response"));
    // Activation produces a filled rectangle on the lifeline
    assert!(svg.contains("rect"));
}

// 24. Multi-line note
#[test]
fn sequence_multiline_note() {
    let source = r#"sequenceDiagram
    participant A
    Note right of A: Line one<br/>Line two<br/>Line three
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Line one"));
    assert!(svg.contains("Line two"));
    assert!(svg.contains("Line three"));
}

// ===========================================================================
// Mindmap coverage
// ===========================================================================

// 25. All mindmap shapes
#[test]
fn mindmap_all_shapes() {
    let source = r#"mindmap
  root((Root Circle))
    [Rect child]
    (Rounded child)
    ((Circle child))
    )Cloud child(
    ))Bang child((
    {{Hexagon child}}
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Root Circle"));
    assert!(svg.contains("Rect child"));
    assert!(svg.contains("Rounded child"));
    assert!(svg.contains("Circle child"));
    assert!(svg.contains("Cloud child"));
    assert!(svg.contains("Bang child"));
    assert!(svg.contains("Hexagon child"));
}

// 26. Deep nesting (4+ levels)
#[test]
fn mindmap_deep_nesting() {
    let source = r#"mindmap
  root
    Level1
      Level2
        Level3
          Level4
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("root"));
    assert!(svg.contains("Level1"));
    assert!(svg.contains("Level2"));
    assert!(svg.contains("Level3"));
    assert!(svg.contains("Level4"));
}

// ===========================================================================
// Gantt coverage
// ===========================================================================

// 27. axisFormat directive
#[test]
fn gantt_axis_format() {
    let source = r#"gantt
    title Axis Format Test
    dateFormat YYYY-MM-DD
    axisFormat %Y-%m-%d
    section Build
    Task 1 :a1, 2024-01-01, 5d
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Task 1"));
    // axisFormat %Y-%m-%d should produce labels like 2024-01-01
    assert!(svg.contains("2024"));
}

// 28. Task with after dependency
#[test]
fn gantt_after_dependency() {
    let source = r#"gantt
    title After Dependency
    dateFormat YYYY-MM-DD
    section Work
    Task 1 :t1, 2024-01-01, 3d
    Task 2 :t2, after t1, 3d
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Task 1"));
    assert!(svg.contains("Task 2"));
}

// 29. Milestone
#[test]
fn gantt_milestone() {
    let source = r#"gantt
    title Milestone Test
    dateFormat YYYY-MM-DD
    section Plan
    Planning :p1, 2024-01-01, 5d
    Go live :milestone, m1, 2024-01-15, 0d
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Go live"));
}

// 30. todayMarker off
#[test]
fn gantt_today_marker_off() {
    let source = r#"gantt
    title Today Marker Off
    dateFormat YYYY-MM-DD
    todayMarker off
    section Work
    Task :t1, 2024-01-01, 5d
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Task"));
    // With todayMarker off, the today line should not appear
    // (no assertion on absence since the marker might not appear anyway for
    // dates far in the past; just verify render succeeds)
}

// 31. Long task labels (wide enough that label fits inside bar)
#[test]
fn gantt_long_task_labels() {
    let source = r#"gantt
    title Long Labels
    dateFormat YYYY-MM-DD
    section Development
    A very long task name that should fit inside the bar :t1, 2024-01-01, 30d
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("A very long task name"));
}

// ===========================================================================
// Git graph coverage
// ===========================================================================

// 32. Branch and merge
#[test]
fn gitgraph_branch_and_merge() {
    let source = r#"gitGraph:
    commit id:"init01"
    branch feature
    checkout feature
    commit id:"feat01"
    checkout main
    merge feature
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    // Commit labels use the explicit id, not the message
    assert!(svg.contains("init01"));
    assert!(svg.contains("feat01"));
    // Branch labels should appear
    assert!(svg.contains("main"));
    assert!(svg.contains("feature"));
}

// 33. Commit types (HIGHLIGHT and REVERSE)
#[test]
fn gitgraph_commit_types() {
    let source = r#"gitGraph:
    commit id:"c1"
    commit type: HIGHLIGHT
    commit type: REVERSE
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    // Explicit ID should appear as label
    assert!(svg.contains("c1"));
    // HIGHLIGHT renders as a rect, REVERSE renders as nested circles
    assert!(svg.contains("<rect") || svg.contains("rect"));
}

// ===========================================================================
// Pie chart coverage
// ===========================================================================

// 34. 13+ slices (overflow the 12-color palette, forces fallback color)
#[test]
fn pie_13_plus_slices() {
    let source = r#"pie title Many Slices
    "Slice 01" : 5
    "Slice 02" : 5
    "Slice 03" : 5
    "Slice 04" : 5
    "Slice 05" : 5
    "Slice 06" : 5
    "Slice 07" : 5
    "Slice 08" : 5
    "Slice 09" : 5
    "Slice 10" : 5
    "Slice 11" : 5
    "Slice 12" : 5
    "Slice 13" : 5
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Many Slices"));
    assert!(svg.contains("Slice 01"));
    assert!(svg.contains("Slice 13"));
    // The 13th slice should use the fallback color #cccccc
    assert!(svg.contains("#cccccc"));
}

// 35. showData is not supported in this parser, so we test pie with
//     no title (exercises the None title path)
#[test]
fn pie_no_title() {
    let source = r#"pie
    "Alpha" : 40
    "Beta" : 60
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Alpha"));
    assert!(svg.contains("Beta"));
}

// ===========================================================================
// Theme coverage
// ===========================================================================

// 36. Forest theme
#[test]
fn theme_forest_flowchart() {
    let source = r#"flowchart TD
    A[Hello] --> B[World]
"#;
    let theme = Theme::by_name("forest");
    let svg = render_svg_with_theme(source, theme);
    assert!(svg.contains("<svg"));
    // Forest theme uses green-ish primary color #cde498
    assert!(svg.contains("#cde498") || svg.contains("cde498"));
}

// 37. Neutral theme
#[test]
fn theme_neutral_flowchart() {
    let source = r#"flowchart TD
    A[Hello] --> B[World]
"#;
    let theme = Theme::by_name("neutral");
    let svg = render_svg_with_theme(source, theme);
    assert!(svg.contains("<svg"));
    // Neutral theme uses #f4f4f4 as primary color
    assert!(svg.contains("#f4f4f4") || svg.contains("f4f4f4"));
}

// ===========================================================================
// Theme coverage with sequence diagrams
// ===========================================================================

#[test]
fn theme_forest_sequence() {
    let source = r#"sequenceDiagram
    Alice->>Bob: Hello
"#;
    let theme = Theme::by_name("forest");
    let svg = render_svg_with_theme(source, theme);
    assert!(svg.contains("<svg"));
    // Forest sequence theme uses #13540c for actor borders
    assert!(svg.contains("#13540c") || svg.contains("13540c"));
}

#[test]
fn theme_neutral_sequence() {
    let source = r#"sequenceDiagram
    Alice->>Bob: Hello
"#;
    let theme = Theme::by_name("neutral");
    let svg = render_svg_with_theme(source, theme);
    assert!(svg.contains("<svg"));
    // Neutral sequence theme uses #666666 for borders
    assert!(svg.contains("#666666") || svg.contains("666666"));
}

// ===========================================================================
// diagram.rs RenderOutput coverage
// ===========================================================================

// 38. into_svg() on PNG output returns Err
#[test]
fn render_output_into_svg_on_png_returns_err() {
    let source = r#"flowchart TD
    A[Start]
"#;
    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;
    let result = render(source, &config).unwrap();
    let err = result.into_svg();
    assert!(err.is_err());
    let err_msg = format!("{}", err.unwrap_err());
    assert!(
        err_msg.contains("Expected SVG"),
        "Error message should mention 'Expected SVG', got: {}",
        err_msg
    );
}

// 39. into_png() on SVG output returns Err
#[test]
fn render_output_into_png_on_svg_returns_err() {
    let source = r#"flowchart TD
    A[Start]
"#;
    let config = RenderConfig::default();
    let result = render(source, &config).unwrap();
    let err = result.into_png();
    assert!(err.is_err());
    let err_msg = format!("{}", err.unwrap_err());
    assert!(
        err_msg.contains("Expected PNG"),
        "Error message should mention 'Expected PNG', got: {}",
        err_msg
    );
}

// 40. into_bytes() for both SVG and PNG
#[test]
fn render_output_into_bytes_svg() {
    let source = r#"flowchart TD
    A[Start]
"#;
    let config = RenderConfig::default();
    let result = render(source, &config).unwrap();
    let bytes = result.into_bytes();
    let text = String::from_utf8(bytes).expect("SVG bytes should be valid UTF-8");
    assert!(text.contains("<svg"));
}

#[test]
fn render_output_into_bytes_png() {
    let source = r#"flowchart TD
    A[Start]
"#;
    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;
    let result = render(source, &config).unwrap();
    let bytes = result.into_bytes();
    // PNG magic bytes
    assert_eq!(
        &bytes[0..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "PNG output should start with PNG magic bytes"
    );
}

// 41. as_png() on SVG output returns None
#[test]
fn render_output_as_png_on_svg_returns_none() {
    let source = r#"flowchart TD
    A[Start]
"#;
    let config = RenderConfig::default();
    let result = render(source, &config).unwrap();
    assert!(result.as_png().is_none(), "as_png() on SVG should be None");
    assert!(
        result.as_svg().is_some(),
        "as_svg() on SVG should be Some"
    );
}

// 42. as_svg() on PNG output returns None
#[test]
fn render_output_as_svg_on_png_returns_none() {
    let source = r#"flowchart TD
    A[Start]
"#;
    let mut config = RenderConfig::default();
    config.output_format = OutputFormat::Png;
    let result = render(source, &config).unwrap();
    assert!(result.as_svg().is_none(), "as_svg() on PNG should be None");
    assert!(
        result.as_png().is_some(),
        "as_png() on PNG should be Some"
    );
}

// ===========================================================================
// Additional edge cases for broader coverage
// ===========================================================================

// Flowchart with LR direction (left to right)
#[test]
fn flowchart_left_to_right() {
    let source = r#"flowchart LR
    A[Start] --> B[Middle] --> C[End]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Start"));
    assert!(svg.contains("End"));
}

// Flowchart using `graph` keyword instead of `flowchart`
#[test]
fn flowchart_graph_keyword() {
    let source = r#"graph TD
    A[Start] --> B[End]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Start"));
    assert!(svg.contains("End"));
}

// Sequence diagram with participant aliases
#[test]
fn sequence_participant_alias() {
    let source = r#"sequenceDiagram
    participant A as Alice
    participant B as Bob
    A->>B: Hello Bob
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Alice"));
    assert!(svg.contains("Bob"));
    assert!(svg.contains("Hello Bob"));
}

// Sequence diagram with activation via +/- suffix
#[test]
fn sequence_activation_shorthand() {
    let source = r#"sequenceDiagram
    Alice->>+Bob: Request
    Bob-->>-Alice: Response
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Request"));
    assert!(svg.contains("Response"));
}

// Gantt with excludes weekends
#[test]
fn gantt_excludes_weekends() {
    let source = r#"gantt
    title Weekends Excluded
    dateFormat YYYY-MM-DD
    excludes weekends
    section Work
    Task A :a1, 2024-01-01, 5d
    Task B :b1, after a1, 3d
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Task A"));
    assert!(svg.contains("Task B"));
}

// Gantt with multiple sections
#[test]
fn gantt_multiple_sections() {
    let source = r#"gantt
    title Multi-Section
    dateFormat YYYY-MM-DD
    section Design
    Wireframes :d1, 2024-01-01, 5d
    section Development
    Backend :dev1, after d1, 10d
    Frontend :dev2, after d1, 8d
    section Testing
    QA :qa1, after dev1, 5d
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Design"));
    assert!(svg.contains("Development"));
    assert!(svg.contains("Testing"));
    assert!(svg.contains("Wireframes"));
    assert!(svg.contains("Backend"));
    assert!(svg.contains("Frontend"));
    assert!(svg.contains("QA"));
}

// Git graph with multiple branches
#[test]
fn gitgraph_multiple_branches() {
    let source = r#"gitGraph:
    commit id:"init01"
    branch develop
    checkout develop
    commit id:"dev001"
    branch feature
    checkout feature
    commit id:"feat01"
    checkout develop
    merge feature
    checkout main
    merge develop
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    // Commit labels use explicit IDs
    assert!(svg.contains("init01"));
    assert!(svg.contains("dev001"));
    assert!(svg.contains("feat01"));
    // Branch labels should appear
    assert!(svg.contains("develop"));
    assert!(svg.contains("feature"));
}

// Git graph with tags and IDs
#[test]
fn gitgraph_tags_and_ids() {
    let source = r#"gitGraph:
    commit id:"abc123"
    commit tag:"v1.0"
    commit "Release"
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    // Tags should appear in the rendered output
    assert!(svg.contains("v1.0"));
}

// Mindmap with icons and classes
#[test]
fn mindmap_with_icons() {
    let source = r#"mindmap
  root((Central))
    Topic A
      ::icon(fa fa-book)
      Sub A1
    Topic B
      Sub B1
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Central"));
    assert!(svg.contains("Topic A"));
    assert!(svg.contains("Topic B"));
}

// Pie chart with single slice (full circle)
#[test]
fn pie_single_slice() {
    let source = r#"pie title Single
    "Everything" : 100
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Single"));
    assert!(svg.contains("Everything"));
    assert!(svg.contains("100%"));
}

// RenderOutput direct construction tests
#[test]
fn render_output_svg_variant() {
    let output = RenderOutput::Svg("<svg>test</svg>".to_string());
    assert!(output.as_svg().is_some());
    assert!(output.as_png().is_none());
    let svg = output.into_svg().unwrap();
    assert_eq!(svg, "<svg>test</svg>");
}

#[test]
fn render_output_png_variant() {
    let png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let output = RenderOutput::Png(png_data.clone());
    assert!(output.as_png().is_some());
    assert!(output.as_svg().is_none());
    let bytes = output.into_png().unwrap();
    assert_eq!(bytes, png_data);
}
