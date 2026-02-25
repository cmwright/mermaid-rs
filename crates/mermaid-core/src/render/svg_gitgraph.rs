use crate::ast::gitgraph::CommitType;
use crate::error::Result;
use crate::layout::gitgraph::*;
use crate::render::svg_util::{build_orthogonal_path, escape_xml};
use crate::render::theme::Theme;

const SVG_PADDING: f64 = 8.0;
const COMMIT_RADIUS: f64 = 10.0;

// Branch color palette
const BRANCH_COLORS: &[&str] = &[
    "#0049B7", // main - blue
    "#B7A400", // branch 1 - yellow-green
    "#00B746", // branch 2 - green
    "#B74600", // branch 3 - orange
    "#5500B7", // branch 4 - purple
    "#B70049", // branch 5 - pink
    "#00B7B7", // branch 6 - cyan
    "#B75500", // branch 7 - dark orange
];

pub fn render_svg(layout: &GitGraphLayout, theme: &Theme) -> Result<String> {
    let view_w = (layout.width + 2.0 * SVG_PADDING).ceil();
    let view_h = (layout.height + 2.0 * SVG_PADDING).ceil();

    let mut svg = String::with_capacity(8192);

    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
        view_w as i64, view_h as i64, view_w as i64, view_h as i64,
    ));
    svg.push('\n');

    // Style block
    svg.push_str(&format!(
        "<style>\
\n  svg {{ background: {}; }}\
\n  .git-label {{ font-family: {}; font-size: {:.0}px; font-weight: bold; }}\
\n  .git-hash {{ font-family: {}; font-size: {:.0}px; }}\
\n  .git-tag {{ font-family: {}; font-size: {:.0}px; }}\
\n</style>",
        theme.background.to_css(),
        theme.font_family,
        theme.font_size,
        theme.font_family,
        theme.font_size * 0.75,
        theme.font_family,
        theme.font_size * 0.85,
    ));
    svg.push('\n');

    // Content group
    svg.push_str(&format!(
        r#"<g transform="translate({}, {})">"#,
        SVG_PADDING, SVG_PADDING
    ));
    svg.push('\n');

    // Layer 1: Branch lines
    for line in &layout.branch_lines {
        svg.push_str(&render_branch_line(line));
    }

    // Layer 2: Connections (merge/branch curves)
    for conn in &layout.connections {
        svg.push_str(&render_connection(conn));
    }

    // Layer 3: Commit nodes
    for commit in &layout.commits {
        svg.push_str(&render_commit(commit));
    }

    // Layer 4: Tags
    for tag in &layout.tags {
        svg.push_str(&render_tag(tag));
    }

    // Layer 5: Commit hash labels
    for commit in &layout.commits {
        svg.push_str(&render_commit_label(commit));
    }

    // Layer 6: Branch labels
    for label in &layout.branch_labels {
        svg.push_str(&render_branch_label(label));
    }

    svg.push_str("</g>\n");
    svg.push_str("</svg>\n");

    Ok(svg)
}

fn get_color(index: usize) -> &'static str {
    BRANCH_COLORS[index % BRANCH_COLORS.len()]
}

fn render_branch_line(line: &PositionedBranchLine) -> String {
    let color = get_color(line.color_index);
    if line.is_dotted {
        format!(
            "  <line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"2\" stroke-dasharray=\"5,5\" opacity=\"0.4\"/>\n",
            line.x_start, line.y, line.x_end, line.y, color,
        )
    } else {
        format!(
            r#"  <line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="3"/>"#,
            line.x_start, line.y, line.x_end, line.y, color,
        ) + "\n"
    }
}

fn render_connection(conn: &PositionedConnection) -> String {
    let color = get_color(conn.color_index);
    let path_d = build_orthogonal_path(&conn.points);
    format!(
        r#"  <path d="{}" fill="none" stroke="{}" stroke-width="3"/>"#,
        path_d, color,
    ) + "\n"
}

fn render_commit(commit: &PositionedCommit) -> String {
    let color = get_color(commit.color_index);

    // Merge commits: hollow circle (white fill, colored stroke)
    if commit.is_merge {
        return format!(
            "  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{}\" fill=\"#ffffff\" stroke=\"{}\" stroke-width=\"3\"/>\n",
            commit.x, commit.y, COMMIT_RADIUS, color,
        );
    }

    match commit.commit_type {
        CommitType::Normal => {
            format!(
                r#"  <circle cx="{:.1}" cy="{:.1}" r="{}" fill="{}" stroke="{}" stroke-width="2"/>"#,
                commit.x, commit.y, COMMIT_RADIUS, color, color,
            ) + "\n"
        }
        CommitType::Highlight => {
            let half = COMMIT_RADIUS;
            let white = "#ffffff";
            format!(
                "  <rect x=\"{:.1}\" y=\"{:.1}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"3\"/>\n",
                commit.x - half,
                commit.y - half,
                half * 2.0,
                half * 2.0,
                white,
                color,
            )
        }
        CommitType::Reverse => {
            let white = "#ffffff";
            let mut s = format!(
                r#"  <circle cx="{:.1}" cy="{:.1}" r="{}" fill="{}" stroke="{}" stroke-width="2"/>"#,
                commit.x, commit.y, COMMIT_RADIUS, color, color,
            );
            s.push('\n');
            s.push_str(&format!(
                "  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                commit.x,
                commit.y,
                COMMIT_RADIUS - 3.0,
                white,
            ));
            s
        }
    }
}

fn render_tag(tag: &PositionedTag) -> String {
    let mut s = String::new();
    let h = 20.0;
    let arrow_w = 8.0;
    let x = tag.x - tag.width / 2.0;
    let y = tag.y;
    let tag_fill = "#ffffcc";
    let tag_stroke = "#aaaa33";

    // Tag shape: rectangle with pointed left side
    s.push_str(&format!(
        "  <polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>\n",
        x - arrow_w, y + h / 2.0,     // left point
        x, y,                          // top-left
        x + tag.width, y,              // top-right
        x + tag.width, y + h,          // bottom-right
        x, y + h,                      // bottom-left
        tag_fill, tag_stroke,
    ));

    // Small circle at the tag point
    s.push_str(&format!(
        "  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2\" fill=\"{}\"/>\n",
        x - arrow_w,
        y + h / 2.0,
        tag_stroke,
    ));

    // Tag text
    s.push_str(&format!(
        "  <text class=\"git-tag\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>\n",
        x + tag.width / 2.0,
        y + h / 2.0,
        escape_xml(&tag.text),
    ));

    s
}

fn render_commit_label(commit: &PositionedCommit) -> String {
    let color = get_color(commit.color_index);
    format!(
        "  <text class=\"git-hash\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"hanging\" fill=\"{}\">{}</text>\n",
        commit.x,
        commit.y + COMMIT_RADIUS + 4.0,
        color,
        escape_xml(&commit.id),
    )
}

fn render_branch_label(label: &PositionedBranchLabel) -> String {
    let color = get_color(label.color_index);
    let mut s = String::new();

    let rect_h = 24.0;
    let rx = rect_h / 2.0;

    // Rounded rectangle background
    s.push_str(&format!(
        r#"  <rect x="{:.1}" y="{:.1}" width="{:.1}" height="{}" rx="{}" fill="{}" stroke="none"/>"#,
        label.x,
        label.y - rect_h / 2.0,
        label.width,
        rect_h,
        rx,
        color,
    ));
    s.push('\n');

    // Branch name text (white on colored background)
    let white = "#ffffff";
    s.push_str(&format!(
        "  <text class=\"git-label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" dominant-baseline=\"central\" fill=\"{}\">{}</text>\n",
        label.x + label.width / 2.0,
        label.y,
        white,
        escape_xml(&label.name),
    ));

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::theme::Theme;

    fn minimal_layout(branch_lines: Vec<PositionedBranchLine>) -> GitGraphLayout {
        GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![],
            branch_lines,
            commits: vec![],
            connections: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn test_dotted_branch_line_contains_stroke_dasharray() {
        let layout = minimal_layout(vec![PositionedBranchLine {
            y: 50.0,
            x_start: 10.0,
            x_end: 200.0,
            color_index: 0,
            is_dotted: true,
        }]);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("stroke-dasharray"),
            "Dotted branch line SVG should contain stroke-dasharray"
        );
    }

    #[test]
    fn test_solid_branch_line_no_stroke_dasharray() {
        let layout = minimal_layout(vec![PositionedBranchLine {
            y: 50.0,
            x_start: 10.0,
            x_end: 200.0,
            color_index: 0,
            is_dotted: false,
        }]);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            !svg.contains("stroke-dasharray"),
            "Solid branch line SVG should not contain stroke-dasharray"
        );
    }

    #[test]
    fn test_render_svg_produces_valid_svg() {
        let layout = minimal_layout(vec![]);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.starts_with("<svg "));
        assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>\n"));
    }

    #[test]
    fn test_render_svg_includes_style_block() {
        let layout = minimal_layout(vec![]);
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("<style>"));
        assert!(svg.contains(".git-label"));
        assert!(svg.contains(".git-hash"));
        assert!(svg.contains(".git-tag"));
    }

    #[test]
    fn test_get_color_wraps_around() {
        assert_eq!(get_color(0), BRANCH_COLORS[0]);
        assert_eq!(get_color(7), BRANCH_COLORS[7]);
        assert_eq!(get_color(8), BRANCH_COLORS[0]); // wraps around
        assert_eq!(get_color(9), BRANCH_COLORS[1]);
    }

    #[test]
    fn test_render_normal_commit() {
        let layout = GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![],
            branch_lines: vec![],
            commits: vec![PositionedCommit {
                id: "abc1234".into(),
                x: 100.0,
                y: 50.0,
                commit_type: CommitType::Normal,
                color_index: 0,
                is_merge: false,
            }],
            connections: vec![],
            tags: vec![],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("<circle"), "normal commit should be a circle");
        assert!(svg.contains("abc1234"), "commit hash should be displayed");
    }

    #[test]
    fn test_render_merge_commit_hollow() {
        let layout = GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![],
            branch_lines: vec![],
            commits: vec![PositionedCommit {
                id: "merge1".into(),
                x: 100.0,
                y: 50.0,
                commit_type: CommitType::Normal,
                color_index: 2,
                is_merge: true,
            }],
            connections: vec![],
            tags: vec![],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("fill=\"#ffffff\""),
            "merge commit should have white fill (hollow)"
        );
    }

    #[test]
    fn test_render_highlight_commit_as_square() {
        let layout = GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![],
            branch_lines: vec![],
            commits: vec![PositionedCommit {
                id: "hl1".into(),
                x: 100.0,
                y: 50.0,
                commit_type: CommitType::Highlight,
                color_index: 1,
                is_merge: false,
            }],
            connections: vec![],
            tags: vec![],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("<rect"),
            "highlight commit should render as a rectangle"
        );
    }

    #[test]
    fn test_render_reverse_commit_has_inner_circle() {
        let layout = GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![],
            branch_lines: vec![],
            commits: vec![PositionedCommit {
                id: "rev1".into(),
                x: 100.0,
                y: 50.0,
                commit_type: CommitType::Reverse,
                color_index: 0,
                is_merge: false,
            }],
            connections: vec![],
            tags: vec![],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        let circle_count = svg.matches("<circle").count();
        assert!(
            circle_count >= 2,
            "reverse commit should have outer + inner circle, got {}",
            circle_count
        );
    }

    #[test]
    fn test_render_tag() {
        let layout = GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![],
            branch_lines: vec![],
            commits: vec![],
            connections: vec![],
            tags: vec![PositionedTag {
                text: "v1.0.0".into(),
                x: 100.0,
                y: 30.0,
                width: 60.0,
            }],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("v1.0.0"), "tag text should be displayed");
        assert!(svg.contains("<polygon"), "tag should have a polygon shape");
        assert!(svg.contains("#ffffcc"), "tag should have yellow fill");
    }

    #[test]
    fn test_render_branch_label() {
        let layout = GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![PositionedBranchLabel {
                name: "main".into(),
                x: 50.0,
                y: 30.0,
                width: 60.0,
                color_index: 0,
            }],
            branch_lines: vec![],
            commits: vec![],
            connections: vec![],
            tags: vec![],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("main"), "branch name should be displayed");
        assert!(
            svg.contains("fill=\"#ffffff\""),
            "branch label text should be white"
        );
        assert!(
            svg.contains(BRANCH_COLORS[0]),
            "branch label should use its color"
        );
    }

    #[test]
    fn test_render_connection() {
        let layout = GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![],
            branch_lines: vec![],
            commits: vec![],
            connections: vec![PositionedConnection {
                points: vec![(50.0, 50.0), (100.0, 50.0), (100.0, 100.0)],
                color_index: 1,
            }],
            tags: vec![],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(svg.contains("<path"), "connection should render as a path");
        assert!(
            svg.contains(BRANCH_COLORS[1]),
            "connection should use its branch color"
        );
    }

    #[test]
    fn test_tag_escapes_xml() {
        let layout = GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![],
            branch_lines: vec![],
            commits: vec![],
            connections: vec![],
            tags: vec![PositionedTag {
                text: "v<1>&2".into(),
                x: 100.0,
                y: 30.0,
                width: 60.0,
            }],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        assert!(
            svg.contains("v&lt;1&gt;&amp;2"),
            "tag text should be XML-escaped"
        );
    }

    #[test]
    fn test_render_all_layers_ordering() {
        let layout = GitGraphLayout {
            width: 400.0,
            height: 200.0,
            branch_labels: vec![PositionedBranchLabel {
                name: "dev".into(),
                x: 50.0,
                y: 30.0,
                width: 40.0,
                color_index: 1,
            }],
            branch_lines: vec![PositionedBranchLine {
                y: 50.0,
                x_start: 10.0,
                x_end: 300.0,
                color_index: 0,
                is_dotted: false,
            }],
            commits: vec![PositionedCommit {
                id: "c1".into(),
                x: 100.0,
                y: 50.0,
                commit_type: CommitType::Normal,
                color_index: 0,
                is_merge: false,
            }],
            connections: vec![PositionedConnection {
                points: vec![(100.0, 50.0), (150.0, 80.0)],
                color_index: 1,
            }],
            tags: vec![PositionedTag {
                text: "v1".into(),
                x: 100.0,
                y: 20.0,
                width: 30.0,
            }],
        };
        let theme = Theme::default();
        let svg = render_svg(&layout, &theme).unwrap();

        // All layers should be present
        assert!(svg.contains("<line")); // branch lines
        assert!(svg.contains("<path")); // connections
        assert!(svg.contains("<circle")); // commits
        assert!(svg.contains("v1")); // tags
        assert!(svg.contains("dev")); // branch labels
        assert!(svg.contains("c1")); // commit hashes
    }
}
