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

// ===========================================================================
// State diagram coverage
// ===========================================================================

// Simple state diagram with start/end
#[test]
fn statediagram_simple() {
    let source = r#"stateDiagram-v2
  [*] --> Still
  Still --> [*]
  Still --> Moving
  Moving --> Still
  Moving --> Crash
  Crash --> [*]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Still"));
    assert!(svg.contains("Moving"));
    assert!(svg.contains("Crash"));
    // Start node (filled circle)
    assert!(svg.contains(r#"r="7"#));
    // End node (donut)
    assert!(svg.contains(r#"r="10"#));
    assert!(svg.contains(r#"r="5"#));
}

// Composite (nested) state
#[test]
fn statediagram_composite() {
    let source = r#"stateDiagram-v2
  [*] --> Active
  state Active {
    [*] --> Idle
    Idle --> Processing : start
    Processing --> Idle : done
  }
  Active --> [*]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Active"));
    assert!(svg.contains("Idle"));
    assert!(svg.contains("Processing"));
    assert!(svg.contains("start"));
    assert!(svg.contains("done"));
}

// Fork and join
#[test]
fn statediagram_fork_join() {
    let source = r#"stateDiagram-v2
  state fork_state <<fork>>
  [*] --> fork_state
  fork_state --> State2
  fork_state --> State3

  state join_state <<join>>
  State2 --> join_state
  State3 --> join_state
  join_state --> State4
  State4 --> [*]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("State2"));
    assert!(svg.contains("State3"));
    assert!(svg.contains("State4"));
    // Fork/join bars (rect with height 6)
    assert!(svg.contains(r#"height="6""#));
}

// Choice state
#[test]
fn statediagram_choice() {
    let source = r#"stateDiagram-v2
  state if_state <<choice>>
  [*] --> IsPositive
  IsPositive --> if_state
  if_state --> False : if n < 0
  if_state --> True : if n >= 0
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("IsPositive"));
    assert!(svg.contains("False"));
    assert!(svg.contains("True"));
    // Choice diamond (polygon)
    assert!(svg.contains("polygon"));
}

// Notes
#[test]
fn statediagram_notes() {
    let source = r#"stateDiagram-v2
  State1 : The state with a note
  note right of State1
    Important information!
    You can write notes.
  end note
  [*] --> State1
  State1 --> [*]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("The state with a note"));
    assert!(svg.contains("Important information!"));
}

// LR direction
#[test]
fn statediagram_direction_lr() {
    let source = r#"stateDiagram-v2
  direction LR
  [*] --> A
  A --> B
  B --> [*]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("A"));
    assert!(svg.contains("B"));
}

// State descriptions
#[test]
fn statediagram_descriptions() {
    let source = r#"stateDiagram-v2
  s1 : Idle state
  s2 : Processing state
  [*] --> s1
  s1 --> s2 : Start
  s2 --> [*]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Idle state"));
    assert!(svg.contains("Processing state"));
    assert!(svg.contains("Start"));
}

// Quoted state names with aliases
#[test]
fn statediagram_quoted_names() {
    let source = r#"stateDiagram-v2
  state "This is a long state" as s1
  state "Another state" as s2
  [*] --> s1
  s1 --> s2 : trigger
  s2 --> [*]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("This is a long state"));
    assert!(svg.contains("Another state"));
    assert!(svg.contains("trigger"));
}

// classDef styling
#[test]
fn statediagram_classdef() {
    let source = r#"stateDiagram-v2
  classDef highlight fill:#f3e6ff,stroke:#9370DB
  [*] --> Active:::highlight
  Active --> [*]
"#;
    let svg = render_svg(source);
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Active"));
    assert!(svg.contains("#f3e6ff") || svg.contains("f3e6ff"));
}

// -----------------------------------------------------------------------
// State diagram bug fix regression tests
// -----------------------------------------------------------------------

/// Helper: parse "translate(x, y)" from a line, returning (x, y) if found.
fn parse_translate(line: &str) -> Option<(f64, f64)> {
    let start = line.find("translate(")? + "translate(".len();
    let end = line[start..].find(')')? + start;
    let inner = &line[start..end];
    let mut parts = inner.split(',');
    let x: f64 = parts.next()?.trim().parse().ok()?;
    let y: f64 = parts.next()?.trim().parse().ok()?;
    Some((x, y))
}

/// Helper: find start and end node positions from SVG.
/// Start node: translate(...) followed by circle r="7" fill="#333"
/// End node: translate(...) followed by circle r="10" fill="none"
fn find_start_end_positions(svg: &str) -> (Option<(f64, f64)>, Option<(f64, f64)>) {
    let mut start_pos = None;
    let mut end_pos = None;

    let lines: Vec<&str> = svg.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if let Some((x, y)) = parse_translate(line) {
            if i + 1 < lines.len() {
                let next = lines[i + 1];
                if next.contains("r=\"7\"") && next.contains("fill=\"#333") {
                    start_pos = Some((x, y));
                } else if next.contains("r=\"10\"") && next.contains("fill=\"none\"") {
                    end_pos = Some((x, y));
                }
            }
        }
    }
    (start_pos, end_pos)
}

/// Helper: find state node positions by matching translate + node-text pattern.
fn find_state_positions(svg: &str) -> Vec<(String, f64, f64)> {
    let mut positions = Vec::new();
    let lines: Vec<&str> = svg.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if let Some((x, y)) = parse_translate(line) {
            for j in i + 1..lines.len().min(i + 4) {
                if lines[j].contains("<text") && lines[j].contains("node-text") {
                    // Extract text content: find ">text</text>"
                    if let Some(name) = extract_text_content(lines[j]) {
                        if !name.is_empty() {
                            positions.push((name, x, y));
                        }
                    }
                    break;
                }
                if lines[j].contains("</g>") {
                    break;
                }
            }
        }
    }
    positions
}

/// Helper: extract text content from a line like <text ...>Content</text>
fn extract_text_content(line: &str) -> Option<String> {
    // Find the closing ">" of the opening tag, then extract until "</text"
    let close_tag = "</text";
    let close_pos = line.find(close_tag)?;
    // Search backwards from close_pos to find the last ">" before it
    let before_close = &line[..close_pos];
    let gt_pos = before_close.rfind('>')?;
    let text = &before_close[gt_pos + 1..];
    Some(text.to_string())
}

/// Helper: extract edge label positions from SVG.
/// Returns Vec<(label_text, x, y)>
fn find_edge_label_positions(svg: &str) -> Vec<(String, f64, f64)> {
    let mut labels = Vec::new();
    for line in svg.lines() {
        if line.contains("edge-label") && line.contains("<text") {
            let x = parse_attr(line, "x=\"");
            let y = parse_attr(line, "y=\"");
            if let (Some(x), Some(y)) = (x, y) {
                if let Some(text) = extract_text_content(line) {
                    if !text.is_empty() {
                        labels.push((text, x, y));
                    }
                }
            }
        }
    }
    labels
}

/// Helper: parse a numeric attribute value like x="123.45"
fn parse_attr(line: &str, prefix: &str) -> Option<f64> {
    let start = line.find(prefix)? + prefix.len();
    let end = line[start..].find('"')? + start;
    line[start..end].parse().ok()
}

/// Helper: extract path d="" strings from SVG edge paths.
fn find_edge_paths(svg: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in svg.lines() {
        if line.contains("<path") && line.contains("fill=\"none\"") && line.contains("d=\"") {
            let start = line.find("d=\"").unwrap() + 3;
            let end = line[start..].find('"').unwrap() + start;
            paths.push(line[start..end].to_string());
        }
    }
    paths
}

/// Helper: parse "M x y" from the start of a path d string.
fn parse_path_start(d: &str) -> Option<(f64, f64)> {
    let d = d.trim();
    if !d.starts_with('M') {
        return None;
    }
    let rest = d[1..].trim();
    let mut parts = rest.splitn(3, ' ');
    let x: f64 = parts.next()?.parse().ok()?;
    let y: f64 = parts.next()?.parse().ok()?;
    Some((x, y))
}

/// Bug fix #1: Start [*] node should be on its own rank at the top (TB direction).
/// Before the fix, start node was placed on the same row as other states.
#[test]
fn statediagram_start_node_on_own_rank_tb() {
    let source = r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Processing : Start
    Processing --> Idle : Done
    Processing --> [*] : Error
"#;
    let svg = render_svg(source);
    let (start_pos, end_pos) = find_start_end_positions(&svg);

    let (_start_x, start_y) = start_pos.expect("should find start node");
    let (_end_x, end_y) = end_pos.expect("should find end node");

    let state_positions = find_state_positions(&svg);

    // Start node should be above all other state nodes (lower y in TB)
    for (name, _x, y) in &state_positions {
        assert!(
            start_y < *y,
            "Start node (y={start_y:.1}) should be above {name} (y={y:.1})"
        );
    }

    // End node should be below Processing
    let processing_y = state_positions
        .iter()
        .find(|(name, _, _)| name == "Processing")
        .map(|(_, _, y)| *y)
        .expect("should find Processing");
    assert!(
        end_y > processing_y,
        "End node (y={end_y:.1}) should be below Processing (y={processing_y:.1})"
    );
}

/// Bug fix #3: Arrow endpoints should be close to the small circle nodes.
/// Before the fix, arrows stopped short because intersect_shape used wrong sizes.
#[test]
fn statediagram_arrow_reaches_start_end_nodes() {
    let source = r#"stateDiagram-v2
    [*] --> StateA
    StateA --> [*]
"#;
    let svg = render_svg(source);

    let (start_pos, end_pos) = find_start_end_positions(&svg);
    let (start_x, start_y) = start_pos.expect("should find start node");
    let (_end_x, end_y) = end_pos.expect("should find end node");

    // Extract edge path start points
    let paths = find_edge_paths(&svg);

    // The edge from [*] should start near the start circle center
    // SVG coords are offset by translate(8,8) from the outer <g>
    let offset = 8.0;
    let start_cx = start_x + offset;
    let start_cy = start_y + offset;

    let found_near_start = paths.iter().any(|d| {
        if let Some((px, py)) = parse_path_start(d) {
            let dist = ((px - start_cx).powi(2) + (py - start_cy).powi(2)).sqrt();
            dist < 20.0 // within ~20px of center (r=7 + tolerance)
        } else {
            false
        }
    });
    assert!(
        found_near_start,
        "An edge should start near the start node at ({start_cx:.1}, {start_cy:.1})"
    );

    // The edge to [*] should end near the end circle center
    let end_cx = start_x + offset; // Both on same x in this simple diagram
    let end_cy = end_y + offset;

    // Parse last point from paths (the "L x y" before the closing quote)
    let found_near_end = paths.iter().any(|d| {
        // Find the last "L x y" or last two numbers before the end
        let parts: Vec<&str> = d.split(' ').collect();
        if parts.len() >= 2 {
            let ly: Option<f64> = parts[parts.len() - 1].parse().ok();
            let lx: Option<f64> = parts[parts.len() - 2]
                .trim_start_matches('L')
                .trim_start_matches('C')
                .parse()
                .ok();
            if let (Some(px), Some(py)) = (lx, ly) {
                let dist = ((px - end_cx).powi(2) + (py - end_cy).powi(2)).sqrt();
                return dist < 25.0;
            }
        }
        false
    });
    assert!(
        found_near_end,
        "An edge should end near the end node at ({end_cx:.1}, {end_cy:.1})"
    );
}

/// Bug fix #2: Bidirectional edges should not overlap.
/// When A-->B and B-->A both exist, their paths should be visually separated.
#[test]
fn statediagram_bidirectional_edges_separated() {
    let source = r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Active : Start
    Active --> Idle : Stop
    Active --> [*]
"#;
    let svg = render_svg(source);

    // Extract edge paths
    let paths = find_edge_paths(&svg);

    // There should be at least 4 edges
    assert!(
        paths.len() >= 4,
        "Expected at least 4 edge paths, got {}",
        paths.len()
    );

    // The "Start" and "Stop" edge labels should be at different positions
    let labels = find_edge_label_positions(&svg);

    let start_label = labels.iter().find(|(t, _, _)| t == "Start");
    let stop_label = labels.iter().find(|(t, _, _)| t == "Stop");

    if let (Some((_, sx, sy)), Some((_, tx, ty))) = (start_label, stop_label) {
        let x_diff = (sx - tx).abs();
        let y_diff = (sy - ty).abs();
        let total_diff = x_diff + y_diff;
        assert!(
            total_diff > 5.0,
            "Start label at ({sx:.1},{sy:.1}) and Stop label at ({tx:.1},{ty:.1}) \
             should be separated (diff={total_diff:.1})"
        );
    }
}

/// Bug fix #1 (LR variant): Start node should be leftmost in LR direction.
#[test]
fn statediagram_start_node_on_own_rank_lr() {
    let source = r#"stateDiagram-v2
    direction LR
    [*] --> Idle
    Idle --> Active
    Active --> [*]
"#;
    let svg = render_svg(source);
    let (start_pos, end_pos) = find_start_end_positions(&svg);

    let (start_x, _start_y) = start_pos.expect("should find start node");
    let (end_x, _end_y) = end_pos.expect("should find end node");

    // In LR, start should have the smallest x (leftmost)
    let state_positions = find_state_positions(&svg);
    for (name, x, _y) in &state_positions {
        assert!(
            start_x < *x,
            "Start node (x={start_x:.1}) should be left of {name} (x={x:.1})"
        );
    }

    // End should be rightmost
    assert!(
        end_x > start_x,
        "End node (x={end_x:.1}) should be right of start (x={start_x:.1})"
    );
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
